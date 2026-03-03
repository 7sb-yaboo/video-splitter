use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

/// セグメント動画からキーフレームを JPEG として抽出する
///
/// 処理フロー:
/// 1. シーン変化検出（scene_threshold > 0.0 の場合）
/// 2. 検出フレームが 0 枚かつ fallback_interval > 0.0 の場合、定間隔サンプリングにフォールバック
/// 3. 出力先: {segment_stem}_frames/frame_0001.jpg, ...
pub fn extract_key_frames(
    ffmpeg: &str,
    segment_path: &Path,
    scene_threshold: f64,
    fallback_interval: f64,
    verbose: bool,
) -> Result<Vec<PathBuf>> {
    let stem = segment_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("segment");

    let frames_dir = segment_path
        .parent()
        .unwrap_or(Path::new("."))
        .join(format!("{}_frames", stem));

    std::fs::create_dir_all(&frames_dir).with_context(|| {
        format!("フレームディレクトリの作成に失敗しました: {}", frames_dir.display())
    })?;

    let input_str = segment_path.to_str().context("Invalid segment path")?;
    let frame_pattern = frames_dir.join("frame_%04d.jpg");
    let frame_pattern_str = frame_pattern.to_str().context("Invalid frame pattern path")?;

    // シーン変化検出
    if scene_threshold > 0.0 {
        let filter = format!("select=gt(scene,{})", scene_threshold);
        run_ffmpeg_extract(ffmpeg, input_str, &filter, frame_pattern_str, verbose);

        let frames = collect_frames(&frames_dir)?;
        if !frames.is_empty() {
            return Ok(frames);
        }

        if verbose {
            eprintln!("  シーン変化フレームが検出されませんでした（threshold={:.2}）", scene_threshold);
        }
    }

    // フォールバック: 定間隔サンプリング
    if fallback_interval > 0.0 {
        let filter = format!("fps=1/{}", fallback_interval);
        run_ffmpeg_extract(ffmpeg, input_str, &filter, frame_pattern_str, verbose);
        return collect_frames(&frames_dir);
    }

    Ok(vec![])
}

/// FFmpeg でフレームを抽出する（失敗しても警告に留める）
fn run_ffmpeg_extract(ffmpeg: &str, input: &str, vf_filter: &str, output_pattern: &str, verbose: bool) {
    let mut cmd = Command::new(ffmpeg);
    cmd.args(["-y", "-i", input, "-vf", vf_filter, "-vsync", "vfr", "-q:v", "3", output_pattern]);

    if verbose {
        eprintln!(
            "  Running: {} -y -i {} -vf \"{}\" -vsync vfr -q:v 3 {}",
            ffmpeg, input, vf_filter, output_pattern
        );
    }

    match cmd.output() {
        Ok(out) if verbose && !out.status.success() => {
            eprintln!(
                "  Warning: ffmpeg exited {:?}: {}",
                out.status.code(),
                String::from_utf8_lossy(&out.stderr).lines().last().unwrap_or("")
            );
        }
        Err(e) if verbose => eprintln!("  Warning: ffmpeg 実行エラー: {}", e),
        _ => {}
    }
}

/// ディレクトリ内の JPEG ファイルをファイル名順で収集する
fn collect_frames(frames_dir: &Path) -> Result<Vec<PathBuf>> {
    let mut frames: Vec<PathBuf> = std::fs::read_dir(frames_dir)
        .with_context(|| format!("フレームディレクトリの読み込みに失敗しました: {}", frames_dir.display()))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .map(|e| e.eq_ignore_ascii_case("jpg"))
                .unwrap_or(false)
        })
        .collect();

    frames.sort();
    Ok(frames)
}
