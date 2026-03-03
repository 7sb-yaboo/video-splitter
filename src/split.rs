use anyhow::{bail, Context, Result};
use regex::Regex;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug)]
pub struct Segment {
    pub index: usize,
    pub start: f64,
    pub end: f64,
    pub output_path: PathBuf,
}

/// FFmpeg が実行可能か確認する
pub fn validate_ffmpeg(ffmpeg: &str) -> Result<()> {
    let output = Command::new(ffmpeg)
        .arg("-version")
        .output()
        .with_context(|| {
            format!(
                "FFmpeg が見つかりません: '{}'\n\
                 FFmpeg をインストールするか、--ffmpeg オプションでパスを指定してください。\n\
                 インストール方法: https://ffmpeg.org/download.html",
                ffmpeg
            )
        })?;

    if !output.status.success() {
        bail!(
            "FFmpeg の実行に失敗しました: '{}'\n\
             --ffmpeg オプションで正しいパスを指定してください。",
            ffmpeg
        );
    }

    Ok(())
}

/// ffmpeg -i の stderr から動画の総時間を秒で取得する
pub fn get_video_duration(ffmpeg: &str, input: &str) -> Result<f64> {
    let output = Command::new(ffmpeg)
        .args(["-i", input])
        .output()
        .with_context(|| format!("Failed to execute ffmpeg: {}", ffmpeg))?;

    // ffmpeg -i は入力ファイル情報を stderr に出力し、exit code 1 で終了するのが正常
    let stderr = String::from_utf8_lossy(&output.stderr);

    parse_duration(&stderr)
        .with_context(|| format!("動画の長さを取得できませんでした: {}", input))
}

/// "Duration: HH:MM:SS.ss" 形式の文字列を秒数に変換する
fn parse_duration(stderr: &str) -> Option<f64> {
    let re = Regex::new(r"Duration:\s*(\d+):(\d+):(\d+(?:\.\d+)?)").ok()?;
    let cap = re.captures(stderr)?;

    let hours: f64 = cap[1].parse().ok()?;
    let minutes: f64 = cap[2].parse().ok()?;
    let seconds: f64 = cap[3].parse().ok()?;

    Some(hours * 3600.0 + minutes * 60.0 + seconds)
}

/// 分割ポイントのリストからセグメントリストを構築する
pub fn build_segments(
    split_points: &[f64],
    total_duration: f64,
    input: &Path,
    output_dir: &Path,
) -> Vec<Segment> {
    let stem = input.file_stem().and_then(|s| s.to_str()).unwrap_or("output");
    let ext = input.extension().and_then(|s| s.to_str()).unwrap_or("mp4");

    let mut segments = Vec::new();
    let mut boundaries = vec![0.0_f64];
    boundaries.extend_from_slice(split_points);
    boundaries.push(total_duration);

    let pad_width = boundaries.len().saturating_sub(1).to_string().len().max(3);

    for (i, window) in boundaries.windows(2).enumerate() {
        let start = window[0];
        let end = window[1];
        let index = i + 1;
        let filename = format!("{stem}_{index:0>pad_width$}.{ext}", pad_width = pad_width);
        let output_path = output_dir.join(filename);

        segments.push(Segment {
            index,
            start,
            end,
            output_path,
        });
    }

    segments
}

/// FFmpeg で1セグメントを切り出す
pub fn cut_segment(ffmpeg: &str, input: &str, segment: &Segment, verbose: bool) -> Result<()> {
    let start_str = format!("{:.3}", segment.start);
    let end_str = format!("{:.3}", segment.end);
    let output_str = segment
        .output_path
        .to_str()
        .context("Invalid output path")?;

    // -ss を -i の後に置くことで精度を優先
    // -avoid_negative_ts make_zero で負タイムスタンプを防止
    let mut cmd = Command::new(ffmpeg);
    cmd.args([
        "-y",
        "-i",
        input,
        "-ss",
        &start_str,
        "-to",
        &end_str,
        "-c",
        "copy",
        "-avoid_negative_ts",
        "make_zero",
        output_str,
    ]);

    if verbose {
        eprintln!(
            "  Running: {} -y -i {} -ss {} -to {} -c copy -avoid_negative_ts make_zero {}",
            ffmpeg, input, start_str, end_str, output_str
        );
    }

    let output = cmd
        .output()
        .with_context(|| format!("Failed to execute ffmpeg for segment {}", segment.index))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "FFmpeg failed for segment {} (exit code: {:?})\nstderr:\n{}",
            segment.index,
            output.status.code(),
            stderr
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_duration() {
        let stderr = "  Duration: 01:00:12.50, start: 0.000000, bitrate: 1234 kb/s";
        let duration = parse_duration(stderr);
        assert!(duration.is_some());
        let d = duration.unwrap();
        assert!((d - 3612.5).abs() < 0.01);
    }

    #[test]
    fn test_build_segments() {
        use std::path::PathBuf;
        let input = PathBuf::from("/tmp/lecture.mp4");
        let output_dir = PathBuf::from("/tmp/out");
        let split_points = vec![598.45, 1201.23];
        let segments = build_segments(&split_points, 1800.0, &input, &output_dir);

        assert_eq!(segments.len(), 3);
        assert_eq!(segments[0].index, 1);
        assert!((segments[0].start - 0.0).abs() < 1e-6);
        assert!((segments[0].end - 598.45).abs() < 1e-6);
        assert_eq!(
            segments[0].output_path.file_name().unwrap().to_str().unwrap(),
            "lecture_001.mp4"
        );
        assert!((segments[2].end - 1800.0).abs() < 1e-6);
    }

    #[test]
    fn test_build_segments_padding() {
        use std::path::PathBuf;
        // 9 分割 = 10 セグメント -> インデックスは 01〜10（2桁でよいが min 3桁）
        let input = PathBuf::from("/tmp/video.mp4");
        let output_dir = PathBuf::from("/tmp");
        let split_points: Vec<f64> = (1..=9).map(|i| i as f64 * 100.0).collect();
        let segments = build_segments(&split_points, 1000.0, &input, &output_dir);
        assert_eq!(segments.len(), 10);
        // 3桁パディング
        assert_eq!(
            segments[9].output_path.file_name().unwrap().to_str().unwrap(),
            "video_010.mp4"
        );
    }
}
