use anyhow::{bail, Context, Result};
use std::path::Path;
use std::process::Command;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

/// SRT エントリ（1つの字幕ブロック）
#[derive(Debug, Clone)]
pub struct SrtEntry {
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
}

/// 動画全体を1回だけ whisper で文字起こしし、SRT エントリのリストを返す
///
/// 処理フロー:
/// 1. ffmpeg で動画全体を 16kHz モノラル f32le raw バイナリに変換（一時ファイル）
/// 2. ファイルを読み込み Vec<f32> に変換（4バイト → f32）
/// 3. whisper-rs でモデルロード・推論
/// 4. セグメントを Vec<SrtEntry> に変換
/// 5. 一時 f32 ファイルを削除
pub fn transcribe_full(
    ffmpeg: &str,
    model: &Path,
    input: &str,
    language: &str,
    verbose: bool,
) -> Result<Vec<SrtEntry>> {
    let tmp_dir = std::env::temp_dir();
    let tmp_f32 = tmp_dir.join("vsp-full.f32");
    let tmp_f32_str = tmp_f32.to_str().context("Invalid temp f32 path")?;

    // Step 1: ffmpeg で動画全体を 16kHz モノラル f32le に変換
    let mut ffmpeg_cmd = Command::new(ffmpeg);
    ffmpeg_cmd.args([
        "-y", "-i", input, "-ar", "16000", "-ac", "1", "-f", "f32le", tmp_f32_str,
    ]);

    if verbose {
        eprintln!(
            "  Running: {} -y -i {} -ar 16000 -ac 1 -f f32le {}",
            ffmpeg, input, tmp_f32_str
        );
    }

    let conv_output = ffmpeg_cmd
        .output()
        .with_context(|| "FFmpeg の実行に失敗しました（全体 f32 変換）")?;

    if !conv_output.status.success() {
        let _ = std::fs::remove_file(&tmp_f32);
        let stderr = String::from_utf8_lossy(&conv_output.stderr);
        bail!(
            "FFmpeg f32 変換に失敗しました（exit code: {:?}）\nstderr:\n{}",
            conv_output.status.code(),
            stderr
        );
    }

    // Step 2: ファイルを読み込み Vec<f32> に変換（読み込み失敗時もファイルを削除する）
    let bytes = std::fs::read(&tmp_f32);
    let _ = std::fs::remove_file(&tmp_f32);
    let bytes = bytes
        .with_context(|| format!("f32 ファイルの読み込みに失敗しました: {}", tmp_f32.display()))?;

    let samples: Vec<f32> = bytes
        .chunks_exact(4)
        .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
        .collect();

    // Step 3: whisper-rs でモデルロード・推論
    let model_str = model.to_str().context("Invalid model path")?;
    let ctx = WhisperContext::new_with_params(model_str, WhisperContextParameters::default())
        .map_err(|e| anyhow::anyhow!("whisper モデルの読み込みに失敗しました: {:?}", e))?;
    let mut state = ctx
        .create_state()
        .map_err(|e| anyhow::anyhow!("whisper state の作成に失敗しました: {:?}", e))?;

    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    let lang_opt = if language == "auto" { None } else { Some(language) };
    params.set_language(lang_opt);
    params.set_print_special(false);
    params.set_print_progress(false);

    if verbose {
        eprintln!("  Running whisper inference (language: {:?})...", lang_opt);
    }

    state
        .full(params, &samples)
        .map_err(|e| anyhow::anyhow!("whisper 推論に失敗しました: {:?}", e))?;

    // Step 4: セグメントを Vec<SrtEntry> に変換
    let n = state.full_n_segments();
    let mut entries = Vec::new();
    for i in 0..n {
        let Some(seg) = state.get_segment(i) else {
            continue;
        };
        // start_timestamp / end_timestamp はセンチ秒（1/100 秒）単位 → × 10 でミリ秒
        let start_ms = (seg.start_timestamp() * 10) as u64;
        let end_ms = (seg.end_timestamp() * 10) as u64;
        let text = seg
            .to_str()
            .map_err(|e| anyhow::anyhow!("segment text の取得に失敗しました: {:?}", e))?
            .trim()
            .to_string();
        if text.is_empty() {
            continue;
        }
        entries.push(SrtEntry { start_ms, end_ms, text });
    }

    Ok(entries)
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

/// "HH:MM:SS,mmm"（SRT）または "HH:MM:SS.mmm"（VTT）形式のタイムスタンプをミリ秒に変換する
fn parse_timestamp(s: &str) -> Option<u64> {
    let s = s.trim();
    let (hms, ms_str) = s.split_once(',').or_else(|| s.split_once('.'))?;
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
