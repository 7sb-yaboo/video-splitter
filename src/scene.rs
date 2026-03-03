use anyhow::{Context, Result};
use regex::Regex;
use std::process::Command;

/// FFmpeg の select フィルターで映像のシーン変化タイムスタンプを検出する
pub fn detect_scene_changes(
    ffmpeg: &str,
    input: &str,
    threshold: f64,
    verbose: bool,
) -> Result<Vec<f64>> {
    let vf_arg = format!("select=gt(scene\\,{}),showinfo", threshold);

    if verbose {
        eprintln!(
            "  Running: {} -i {} -vf \"{}\" -vsync vfr -f null -",
            ffmpeg, input, vf_arg
        );
    }

    let output = Command::new(ffmpeg)
        .args(["-i", input, "-vf", &vf_arg, "-vsync", "vfr", "-f", "null", "-"])
        .output()
        .with_context(|| format!("Failed to execute ffmpeg: {}", ffmpeg))?;

    // showinfo の出力は stderr に出る
    let stderr = String::from_utf8_lossy(&output.stderr);

    if verbose {
        eprintln!("  scene detect stderr (excerpt):");
        for line in stderr.lines().filter(|l| l.contains("pts_time")) {
            eprintln!("    {}", line);
        }
    }

    parse_scene_timestamps(&stderr)
}

/// showinfo フィルターの stderr 出力をパースしてタイムスタンプリストを返す
fn parse_scene_timestamps(stderr: &str) -> Result<Vec<f64>> {
    let re = Regex::new(r"pts_time:\s*([\d.]+)")?;
    let mut timestamps: Vec<f64> = Vec::new();

    for cap in re.captures_iter(stderr) {
        if let Ok(t) = cap[1].parse::<f64>() {
            timestamps.push(t);
        }
    }

    timestamps.sort_by(|a, b| a.partial_cmp(b).unwrap());
    Ok(timestamps)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_scene_timestamps() {
        let stderr = r#"
[Parsed_showinfo_1 @ 0x...] n:   0 pts:      0 pts_time:0       pos:     2048 fmt:yuv420p sar:1/1 s:1920x1080 i:P iskey:1 type:I checksum:ABCDEF duration:0.040000
[Parsed_showinfo_1 @ 0x...] n:   5 pts:  12800 pts_time:12.8    pos:   100000 fmt:yuv420p sar:1/1 s:1920x1080 i:P iskey:0 type:P checksum:123456 duration:0.040000
[Parsed_showinfo_1 @ 0x...] n:  18 pts:  46080 pts_time:46.08   pos:   400000 fmt:yuv420p sar:1/1 s:1920x1080 i:P iskey:0 type:P checksum:789ABC duration:0.040000
"#;
        let ts = parse_scene_timestamps(stderr).unwrap();
        assert_eq!(ts.len(), 3);
        assert!((ts[0] - 0.0).abs() < 1e-6);
        assert!((ts[1] - 12.8).abs() < 1e-6);
        assert!((ts[2] - 46.08).abs() < 1e-6);
    }

    #[test]
    fn test_parse_scene_timestamps_empty() {
        let ts = parse_scene_timestamps("no scene info here").unwrap();
        assert!(ts.is_empty());
    }
}
