use anyhow::{bail, Context, Result};
use std::path::Path;
use std::process::Command;

/// SRT エントリ（1つの字幕ブロック）
#[derive(Debug, Clone)]
pub struct SrtEntry {
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
}

/// whisper.cpp が実行可能か確認する
pub fn validate_whisper(whisper: &str) -> Result<()> {
    Command::new(whisper)
        .arg("--help")
        .output()
        .with_context(|| {
            format!(
                "whisper.cpp が見つかりません: '{}'\n\
                 whisper.cpp をインストールするか、--whisper オプションでパスを指定してください。",
                whisper
            )
        })?;
    Ok(())
}

/// 動画全体を1回だけ whisper で文字起こしし、SRT エントリのリストを返す
///
/// 処理フロー:
/// 1. ffmpeg で動画全体を 16kHz モノラル PCM WAV に変換（一時ファイル）
/// 2. whisper.cpp で全体文字起こし（SRT 形式で一時ファイルに出力）
/// 3. SRT をパースして Vec<SrtEntry> を返す
/// 4. 一時ファイルを削除
pub fn transcribe_full(
    ffmpeg: &str,
    whisper: &str,
    model: &Path,
    input: &str,
    language: &str,
    verbose: bool,
) -> Result<Vec<SrtEntry>> {
    let tmp_dir = std::env::temp_dir();
    let tmp_wav = tmp_dir.join("vsp-full.wav");
    let tmp_srt_base = tmp_dir.join("vsp-full");
    let tmp_srt = tmp_dir.join("vsp-full.srt");

    let tmp_wav_str = tmp_wav.to_str().context("Invalid temp WAV path")?;
    let tmp_srt_base_str = tmp_srt_base.to_str().context("Invalid temp SRT base path")?;

    // ffmpeg で全体を 16kHz モノラル PCM WAV に変換
    let mut ffmpeg_cmd = Command::new(ffmpeg);
    ffmpeg_cmd.args([
        "-y", "-i", input, "-ar", "16000", "-ac", "1", "-c:a", "pcm_s16le", tmp_wav_str,
    ]);

    if verbose {
        eprintln!(
            "  Running: {} -y -i {} -ar 16000 -ac 1 -c:a pcm_s16le {}",
            ffmpeg, input, tmp_wav_str
        );
    }

    let conv_output = ffmpeg_cmd
        .output()
        .with_context(|| "FFmpeg の実行に失敗しました（全体 WAV 変換）")?;

    if !conv_output.status.success() {
        let _ = std::fs::remove_file(&tmp_wav);
        let stderr = String::from_utf8_lossy(&conv_output.stderr);
        bail!(
            "FFmpeg WAV 変換に失敗しました（exit code: {:?}）\nstderr:\n{}",
            conv_output.status.code(),
            stderr
        );
    }

    // whisper.cpp で全体文字起こし（SRT 形式で出力）
    let model_str = model.to_str().context("Invalid model path")?;

    let mut whisper_cmd = Command::new(whisper);
    whisper_cmd.args([
        "-m",
        model_str,
        "-f",
        tmp_wav_str,
        "-l",
        language,
        "-osrt",
        "-of",
        tmp_srt_base_str,
    ]);

    if verbose {
        eprintln!(
            "  Running: {} -m {} -f {} -l {} -osrt -of {}",
            whisper, model_str, tmp_wav_str, language, tmp_srt_base_str
        );
    }

    let whisper_result = whisper_cmd
        .output()
        .with_context(|| "whisper.cpp の実行に失敗しました");

    // 成功・失敗にかかわらず一時 WAV を削除
    let _ = std::fs::remove_file(&tmp_wav);

    let whisper_output = whisper_result?;

    if !whisper_output.status.success() {
        let _ = std::fs::remove_file(&tmp_srt);
        let stderr = String::from_utf8_lossy(&whisper_output.stderr);
        bail!(
            "whisper.cpp の実行に失敗しました（exit code: {:?}）\nstderr:\n{}",
            whisper_output.status.code(),
            stderr
        );
    }

    // SRT ファイルを読み込み・パース
    let srt_content = std::fs::read_to_string(&tmp_srt)
        .with_context(|| format!("SRT ファイルの読み込みに失敗しました: {}", tmp_srt.display()))?;

    let _ = std::fs::remove_file(&tmp_srt);

    Ok(parse_srt(&srt_content))
}

/// セグメントの時間範囲 [start_s, end_s) に対応する SRT エントリを抽出し、
/// タイムスタンプを start_s 基準に補正して返す
pub fn slice_srt(entries: &[SrtEntry], start_s: f64, end_s: f64) -> Vec<SrtEntry> {
    let start_ms = (start_s * 1000.0) as u64;
    let end_ms = (end_s * 1000.0) as u64;

    entries
        .iter()
        .filter(|e| e.start_ms >= start_ms && e.start_ms < end_ms)
        .map(|e| SrtEntry {
            start_ms: e.start_ms - start_ms,
            end_ms: (e.end_ms - start_ms).min(end_ms - start_ms),
            text: e.text.clone(),
        })
        .collect()
}

/// SRT エントリをファイルに書き出す（format: "srt" / "vtt" / "txt"）
pub fn write_transcript(entries: &[SrtEntry], path: &Path, format: &str) -> Result<()> {
    let content = match format {
        "srt" => format_as_srt(entries),
        "vtt" => format_as_vtt(entries),
        _ => format_as_txt(entries),
    };

    std::fs::write(path, content)
        .with_context(|| format!("文字起こしファイルの書き込みに失敗しました: {}", path.display()))
}

// --- 内部ユーティリティ ---

/// SRT テキストをパースして SrtEntry のリストを返す
pub fn parse_srt(content: &str) -> Vec<SrtEntry> {
    // Windows 改行を正規化
    let content = content.replace("\r\n", "\n");
    let mut entries = Vec::new();

    for block in content.split("\n\n") {
        let block = block.trim();
        if block.is_empty() {
            continue;
        }

        let lines: Vec<&str> = block.lines().collect();
        if lines.len() < 3 {
            continue;
        }

        // 1行目: インデックス番号（スキップ）
        // 2行目: "HH:MM:SS,mmm --> HH:MM:SS,mmm"
        let Some((start_str, end_str)) = lines[1].split_once(" --> ") else {
            continue;
        };

        let (Some(start_ms), Some(end_ms)) =
            (parse_timestamp(start_str), parse_timestamp(end_str))
        else {
            continue;
        };

        // 3行目以降: テキスト
        let text = lines[2..].join("\n").trim().to_string();
        if text.is_empty() {
            continue;
        }

        entries.push(SrtEntry { start_ms, end_ms, text });
    }

    entries
}

/// "HH:MM:SS,mmm" 形式のタイムスタンプをミリ秒に変換する
fn parse_timestamp(s: &str) -> Option<u64> {
    let (hms, ms_str) = s.trim().split_once(',')?;
    let ms: u64 = ms_str.trim().parse().ok()?;

    let parts: Vec<&str> = hms.split(':').collect();
    if parts.len() != 3 {
        return None;
    }
    let h: u64 = parts[0].parse().ok()?;
    let m: u64 = parts[1].parse().ok()?;
    let sec: u64 = parts[2].parse().ok()?;

    Some(h * 3_600_000 + m * 60_000 + sec * 1_000 + ms)
}

/// ミリ秒を "HH:MM:SS,mmm" 形式に変換する（SRT 用）
fn format_timestamp_srt(ms: u64) -> String {
    let h = ms / 3_600_000;
    let m = (ms % 3_600_000) / 60_000;
    let s = (ms % 60_000) / 1_000;
    let millis = ms % 1_000;
    format!("{:02}:{:02}:{:02},{:03}", h, m, s, millis)
}

/// ミリ秒を "HH:MM:SS.mmm" 形式に変換する（VTT 用）
fn format_timestamp_vtt(ms: u64) -> String {
    let h = ms / 3_600_000;
    let m = (ms % 3_600_000) / 60_000;
    let s = (ms % 60_000) / 1_000;
    let millis = ms % 1_000;
    format!("{:02}:{:02}:{:02}.{:03}", h, m, s, millis)
}

fn format_as_srt(entries: &[SrtEntry]) -> String {
    let mut out = String::new();
    for (i, entry) in entries.iter().enumerate() {
        out.push_str(&format!(
            "{}\n{} --> {}\n{}\n\n",
            i + 1,
            format_timestamp_srt(entry.start_ms),
            format_timestamp_srt(entry.end_ms),
            entry.text,
        ));
    }
    out
}

fn format_as_vtt(entries: &[SrtEntry]) -> String {
    let mut out = String::from("WEBVTT\n\n");
    for (i, entry) in entries.iter().enumerate() {
        out.push_str(&format!(
            "{}\n{} --> {}\n{}\n\n",
            i + 1,
            format_timestamp_vtt(entry.start_ms),
            format_timestamp_vtt(entry.end_ms),
            entry.text,
        ));
    }
    out
}

fn format_as_txt(entries: &[SrtEntry]) -> String {
    entries.iter().map(|e| e.text.as_str()).collect::<Vec<_>>().join("\n")
}

// --- テスト ---

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_timestamp() {
        assert_eq!(parse_timestamp("00:00:01,000"), Some(1_000));
        assert_eq!(parse_timestamp("01:02:03,456"), Some(3_723_456));
        assert_eq!(parse_timestamp("00:00:00,000"), Some(0));
    }

    #[test]
    fn test_parse_srt() {
        let srt = "1\n00:00:01,000 --> 00:00:03,500\n Hello world\n\n\
                   2\n00:00:04,000 --> 00:00:06,000\n Second line\n\n";
        let entries = parse_srt(srt);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].start_ms, 1_000);
        assert_eq!(entries[0].end_ms, 3_500);
        assert_eq!(entries[0].text, "Hello world");
        assert_eq!(entries[1].start_ms, 4_000);
    }

    #[test]
    fn test_slice_srt() {
        let entries = vec![
            SrtEntry { start_ms: 1_000, end_ms: 3_000, text: "A".into() },
            SrtEntry { start_ms: 5_000, end_ms: 7_000, text: "B".into() },
            SrtEntry { start_ms: 12_000, end_ms: 14_000, text: "C".into() },
        ];

        // 0s - 10s のスライス
        let sliced = slice_srt(&entries, 0.0, 10.0);
        assert_eq!(sliced.len(), 2);
        assert_eq!(sliced[0].start_ms, 1_000);
        assert_eq!(sliced[1].start_ms, 5_000);

        // 5s - 15s のスライス（タイムスタンプが 5s 基準に補正される）
        let sliced = slice_srt(&entries, 5.0, 15.0);
        assert_eq!(sliced.len(), 2);
        assert_eq!(sliced[0].start_ms, 0);   // 5000 - 5000
        assert_eq!(sliced[1].start_ms, 7_000); // 12000 - 5000
    }

    #[test]
    fn test_format_as_srt() {
        let entries = vec![SrtEntry {
            start_ms: 1_500,
            end_ms: 3_000,
            text: "Test".into(),
        }];
        let out = format_as_srt(&entries);
        assert!(out.contains("00:00:01,500 --> 00:00:03,000"));
        assert!(out.contains("Test"));
    }

    #[test]
    fn test_format_as_vtt() {
        let entries = vec![SrtEntry {
            start_ms: 1_500,
            end_ms: 3_000,
            text: "Test".into(),
        }];
        let out = format_as_vtt(&entries);
        assert!(out.starts_with("WEBVTT"));
        assert!(out.contains("00:00:01.500 --> 00:00:03.000"));
    }
}
