mod frames;
mod manifest;
mod scene;
mod search;
mod silence;
mod split;
mod transcribe;

use anyhow::{bail, Result};
use clap::Parser;
use std::path::PathBuf;

/// 動画ファイルを指定時間ごとに分割するツール（無音区間で自然に分割）
#[derive(Parser, Debug)]
#[command(name = "video-splitter", version, about, long_about = None)]
struct Cli {
    /// 入力動画ファイル（または --search 使用時は manifest.json パス）
    input: PathBuf,

    /// 分割間隔（秒）
    #[arg(short, long, default_value_t = 600.0)]
    duration: f64,

    /// 出力先ディレクトリ（デフォルト: 入力ファイルと同じディレクトリ）
    #[arg(short, long)]
    output_dir: Option<PathBuf>,

    /// 無音判定閾値 (dB)
    #[arg(long, default_value_t = -30.0)]
    noise_threshold: f64,

    /// 無音の最短持続時間（秒）
    #[arg(long, default_value_t = 0.5)]
    silence_duration: f64,

    /// ターゲット時刻の前後探索範囲（秒）
    #[arg(long, default_value_t = 60.0)]
    search_window: f64,

    /// FFmpeg 実行ファイルパス
    #[arg(long, default_value = "ffmpeg", env = "FFMPEG_PATH")]
    ffmpeg: String,

    /// 詳細ログを表示する
    #[arg(short, long)]
    verbose: bool,

    // ── 文字起こし ──────────────────────────────────────────────────────────

    /// 文字起こしを有効にする（--whisper-model が必須）
    #[arg(long)]
    transcribe: bool,

    /// whisper.cpp 実行ファイルパス
    #[arg(long, default_value = "whisper-cpp", env = "WHISPER_PATH")]
    whisper: String,

    /// モデルファイルパス（--transcribe 時は必須）
    #[arg(long)]
    whisper_model: Option<PathBuf>,

    /// 音声言語コード
    #[arg(long, default_value = "ja")]
    language: String,

    /// 文字起こし出力形式（txt / srt / vtt）
    #[arg(long, default_value = "txt")]
    transcribe_format: String,

    // ── キーフレーム抽出 ────────────────────────────────────────────────────

    /// キーフレーム抽出を有効にする
    #[arg(long)]
    extract_frames: bool,

    /// シーン変化検出の感度（0.0〜1.0、値が大きいほど変化が大きい場面のみ検出）
    #[arg(long, default_value_t = 0.3)]
    frames_scene_threshold: f64,

    /// シーン変化が検出されなかった場合のフォールバック間隔（秒、0.0 = 無効）
    #[arg(long, default_value_t = 30.0)]
    frames_interval: f64,

    // ── シーン変化分割 ──────────────────────────────────────────────────────

    /// シーン変化点も分割候補に加える
    #[arg(long)]
    split_on_scene: bool,

    /// シーン変化検出の閾値（0.0〜1.0）
    #[arg(long, default_value_t = 0.4)]
    scene_threshold: f64,

    // ── マニフェスト ────────────────────────────────────────────────────────

    /// 処理結果をまとめた manifest.json を出力先に生成する
    #[arg(long)]
    manifest: bool,

    // ── 検索 ────────────────────────────────────────────────────────────────

    /// manifest.json からトランスクリプトを横断検索する（INPUT を manifest.json パスとして扱う）
    #[arg(long)]
    search: Option<String>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    run(cli)
}

fn run(cli: Cli) -> Result<()> {
    // ── 検索モード ──────────────────────────────────────────────────────────
    if let Some(ref query) = cli.search {
        if !cli.input.exists() {
            bail!("manifest.json が見つかりません: {}", cli.input.display());
        }
        let results = search::search_transcript(cli.input.as_path(), query)?;
        println!("{}", serde_json::to_string_pretty(&results)?);
        return Ok(());
    }

    // ── 動画処理モード ──────────────────────────────────────────────────────

    // 入力ファイルの存在確認
    if !cli.input.exists() {
        bail!("入力ファイルが見つかりません: {}", cli.input.display());
    }

    let input_str = cli
        .input
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("Invalid input path"))?;

    // 出力先ディレクトリの決定
    let output_dir = match cli.output_dir {
        Some(dir) => dir,
        None => cli
            .input
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."))
            .to_path_buf(),
    };

    if !output_dir.exists() {
        std::fs::create_dir_all(&output_dir)
            .map_err(|e| anyhow::anyhow!("出力ディレクトリの作成に失敗しました: {}", e))?;
    }

    // FFmpeg の存在確認
    split::validate_ffmpeg(&cli.ffmpeg)?;

    // 文字起こしのバリデーション
    if cli.transcribe {
        if cli.whisper_model.is_none() {
            bail!("--transcribe を使用する場合は --whisper-model でモデルファイルパスを指定してください");
        }
        transcribe::validate_whisper(&cli.whisper)?;
    }

    // 動画の総時間取得
    println!("Analyzing video: {}", cli.input.display());
    let total_duration = split::get_video_duration(&cli.ffmpeg, input_str)?;
    let total_minutes = total_duration / 60.0;
    println!("  Total duration: {:.1}s ({:.1} min)", total_duration, total_minutes);

    // 分割不要チェック
    if total_duration <= cli.duration {
        println!(
            "動画の長さ（{:.1}s）が分割間隔（{:.1}s）以下のため、分割は不要です。",
            total_duration, cli.duration
        );
        return Ok(());
    }

    // 無音区間の検出
    println!("Detecting silence in: {}", cli.input.display());
    let silence_intervals = silence::detect_silence(
        &cli.ffmpeg,
        input_str,
        cli.noise_threshold,
        cli.silence_duration,
        cli.verbose,
    )?;
    println!("  Found {} silence interval(s)", silence_intervals.len());

    // 無音区間の中間点を候補プールに追加
    let mut candidates: Vec<f64> = silence_intervals.iter().map(|iv| iv.midpoint()).collect();

    // シーン変化点を候補プールに追加
    if cli.split_on_scene {
        println!("Detecting scene changes...");
        match scene::detect_scene_changes(
            &cli.ffmpeg,
            input_str,
            cli.scene_threshold,
            cli.verbose,
        ) {
            Ok(scene_times) => {
                println!("  Found {} scene change(s)", scene_times.len());
                candidates.extend(scene_times);
            }
            Err(e) => {
                eprintln!("  Warning: シーン変化検出をスキップしました: {}", e);
            }
        }
    }

    // 候補をソートして近傍重複を除去（1秒未満の間隔は同一点とみなす）
    candidates.sort_by(|a, b| a.partial_cmp(b).unwrap());
    candidates = dedup_nearby(candidates, 1.0);

    // 分割ポイントの決定
    let mut split_points: Vec<f64> = Vec::new();
    let mut target = cli.duration;

    while target < total_duration {
        let point =
            silence::find_nearest_candidate(&candidates, target, cli.search_window)
                .unwrap_or(target);
        split_points.push(point);
        target = point + cli.duration;
    }

    println!("Split points ({} total):", split_points.len());
    for (i, &point) in split_points.iter().enumerate() {
        println!("  {:>3}.  {:.3}s ({:.1} min)", i + 1, point, point / 60.0);
    }

    // セグメントの構築
    let segments = split::build_segments(&split_points, total_duration, &cli.input, &output_dir);
    let total_segments = segments.len();
    println!("Splitting into {} segment(s)...", total_segments);

    // 文字起こし: 動画全体を1回だけ whisper で処理し SRT エントリを取得する
    let srt_entries = if cli.transcribe {
        let model = cli.whisper_model.as_deref().unwrap();
        println!("Transcribing full video (1 pass)...");
        let entries = transcribe::transcribe_full(
            &cli.ffmpeg,
            &cli.whisper,
            model,
            input_str,
            &cli.language,
            cli.verbose,
        )?;
        println!("  -> OK: {} entries", entries.len());
        Some(entries)
    } else {
        None
    };

    // 各セグメントをカット → 文字起こしスライス → キーフレーム抽出
    let mut segment_metas: Vec<manifest::SegmentMeta> = Vec::new();

    for segment in &segments {
        println!(
            "[{}/{}] Cutting: {} ({:.1}s - {:.1}s)",
            segment.index,
            total_segments,
            segment.output_path.file_name().unwrap().to_string_lossy(),
            segment.start,
            segment.end,
        );

        split::cut_segment(&cli.ffmpeg, input_str, segment, cli.verbose)?;
        println!("  -> OK: {}", segment.output_path.display());

        // 文字起こしスライス
        let transcript_path: Option<std::path::PathBuf> = if let Some(ref entries) = srt_entries {
            let sliced = transcribe::slice_srt(entries, segment.start, segment.end);
            let path = segment.output_path.with_extension(&cli.transcribe_format);
            transcribe::write_transcript(&sliced, &path, &cli.transcribe_format)?;
            println!("  -> OK: {} ({} entries)", path.display(), sliced.len());
            Some(path)
        } else {
            None
        };

        // キーフレーム抽出
        let frame_paths: Vec<std::path::PathBuf> = if cli.extract_frames {
            match frames::extract_key_frames(
                &cli.ffmpeg,
                &segment.output_path,
                cli.frames_scene_threshold,
                cli.frames_interval,
                cli.verbose,
            ) {
                Ok(paths) => {
                    let frames_dir = segment
                        .output_path
                        .parent()
                        .unwrap()
                        .join(format!(
                            "{}_frames",
                            segment.output_path.file_stem().unwrap().to_string_lossy()
                        ));
                    println!("  -> OK: {}/ ({} frames)", frames_dir.display(), paths.len());
                    paths
                }
                Err(e) => {
                    eprintln!("  Warning: フレーム抽出をスキップしました: {}", e);
                    vec![]
                }
            }
        } else {
            vec![]
        };

        // マニフェスト用データ収集
        if cli.manifest {
            segment_metas.push(manifest::SegmentMeta {
                index: segment.index,
                start: segment.start,
                end: segment.end,
                video: manifest::to_relative(&segment.output_path, &output_dir),
                transcript: transcript_path
                    .as_deref()
                    .map(|p| manifest::to_relative(p, &output_dir)),
                key_frames: frame_paths
                    .iter()
                    .map(|p| manifest::to_relative(p, &output_dir))
                    .collect(),
            });
        }
    }

    // マニフェスト書き出し
    if cli.manifest {
        let manifest_data = manifest::Manifest {
            source: input_str.to_string(),
            total_duration,
            language: cli.language.clone(),
            segments: segment_metas,
        };
        let manifest_path = output_dir.join("manifest.json");
        manifest::write_manifest(&manifest_data, &manifest_path)?;
        println!("Manifest: {}", manifest_path.display());
    }

    println!("Done! {} file(s) created in: {}", total_segments, output_dir.display());
    Ok(())
}

/// ソート済みリストから min_gap 未満の間隔の要素を除外する
fn dedup_nearby(sorted: Vec<f64>, min_gap: f64) -> Vec<f64> {
    let mut result: Vec<f64> = Vec::new();
    for val in sorted {
        if result.last().map_or(true, |&last| val - last >= min_gap) {
            result.push(val);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dedup_nearby() {
        let v = vec![1.0, 1.3, 1.8, 3.0, 3.5, 10.0];
        let result = dedup_nearby(v, 1.0);
        // 1.0 は残る、1.3 は除外（差0.3 < 1.0）、1.8 は残る（差0.5 < 1.0... wait 1.8-1.0=0.8 < 1.0 → 除外）
        // let me recalculate:
        // 1.0 → result=[1.0], last=1.0
        // 1.3: 1.3-1.0=0.3 < 1.0 → skip
        // 1.8: 1.8-1.0=0.8 < 1.0 → skip
        // 3.0: 3.0-1.0=2.0 >= 1.0 → result=[1.0, 3.0], last=3.0
        // 3.5: 3.5-3.0=0.5 < 1.0 → skip
        // 10.0: 10.0-3.0=7.0 >= 1.0 → result=[1.0, 3.0, 10.0]
        assert_eq!(result, vec![1.0, 3.0, 10.0]);
    }

    #[test]
    fn test_dedup_nearby_empty() {
        let result = dedup_nearby(vec![], 1.0);
        assert!(result.is_empty());
    }

    #[test]
    fn test_dedup_nearby_no_duplicates() {
        let v = vec![1.0, 5.0, 10.0];
        let result = dedup_nearby(v.clone(), 1.0);
        assert_eq!(result, v);
    }
}
