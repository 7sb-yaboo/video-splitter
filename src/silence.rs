use anyhow::{Context, Result};
use regex::Regex;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct SilenceInterval {
    pub start: f64,
    pub end: f64,
}

impl SilenceInterval {
    pub fn midpoint(&self) -> f64 {
        (self.start + self.end) / 2.0
    }
}

/// FFmpeg の silencedetect フィルターで無音区間を検出する
pub fn detect_silence(
    ffmpeg: &str,
    input: &str,
    noise_threshold: f64,
    silence_duration: f64,
    verbose: bool,
) -> Result<Vec<SilenceInterval>> {
    let noise_arg = format!("silencedetect=noise={}dB:d={}", noise_threshold, silence_duration);

    if verbose {
        eprintln!("  Running: {} -i {} -af {} -vn -f null -", ffmpeg, input, noise_arg);
    }

    let output = Command::new(ffmpeg)
        .args(["-i", input, "-af", &noise_arg, "-vn", "-f", "null", "-"])
        .output()
        .with_context(|| format!("Failed to execute ffmpeg: {}", ffmpeg))?;

    // silencedetect の出力は stderr に出る
    let stderr = String::from_utf8_lossy(&output.stderr);

    if verbose {
        eprintln!("  silencedetect stderr (excerpt):");
        for line in stderr.lines().filter(|l| l.contains("silence")) {
            eprintln!("    {}", line);
        }
    }

    parse_silence_intervals(&stderr)
}

/// silencedetect の stderr 出力をパースして SilenceInterval のリストを返す
fn parse_silence_intervals(stderr: &str) -> Result<Vec<SilenceInterval>> {
    let re_start = Regex::new(r"silence_start:\s*([\d.]+)")?;
    let re_end = Regex::new(r"silence_end:\s*([\d.]+)")?;

    let mut starts: Vec<f64> = Vec::new();
    let mut ends: Vec<f64> = Vec::new();

    for line in stderr.lines() {
        if let Some(cap) = re_start.captures(line) {
            if let Ok(t) = cap[1].parse::<f64>() {
                starts.push(t);
            }
        }
        if let Some(cap) = re_end.captures(line) {
            if let Ok(t) = cap[1].parse::<f64>() {
                ends.push(t);
            }
        }
    }

    // start と end をペアにする（end が start より少ない場合は末尾まで無音とみなす）
    let mut intervals = Vec::new();
    for (i, &start) in starts.iter().enumerate() {
        if let Some(&end) = ends.get(i) {
            intervals.push(SilenceInterval { start, end });
        }
        // end がない場合（動画末尾まで無音）は無視する
    }

    Ok(intervals)
}

/// ターゲット時刻に最も近い無音区間の中間点を返す
/// 探索範囲内に候補がなければ None を返す
#[allow(dead_code)]
pub fn find_nearest_split_point(
    intervals: &[SilenceInterval],
    target: f64,
    search_window: f64,
) -> Option<f64> {
    let candidates: Vec<f64> = intervals.iter().map(|iv| iv.midpoint()).collect();
    find_nearest_candidate(&candidates, target, search_window)
}

/// 候補タイムスタンプのリストから、ターゲット時刻に最も近い候補を返す
/// `[target - search_window, target + search_window]` の範囲外は無視する
pub fn find_nearest_candidate(
    candidates: &[f64],
    target: f64,
    search_window: f64,
) -> Option<f64> {
    let lower = target - search_window;
    let upper = target + search_window;

    candidates
        .iter()
        .filter(|&&t| t >= lower && t <= upper)
        .min_by(|&&a, &&b| {
            let da = (a - target).abs();
            let db = (b - target).abs();
            da.partial_cmp(&db).unwrap()
        })
        .copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_silence_intervals() {
        let stderr = r#"
[silencedetect @ 0x...] silence_start: 10.5
[silencedetect @ 0x...] silence_end: 11.2 | silence_duration: 0.7
[silencedetect @ 0x...] silence_start: 605.3
[silencedetect @ 0x...] silence_end: 606.1 | silence_duration: 0.8
"#;
        let intervals = parse_silence_intervals(stderr).unwrap();
        assert_eq!(intervals.len(), 2);
        assert!((intervals[0].start - 10.5).abs() < 1e-6);
        assert!((intervals[0].end - 11.2).abs() < 1e-6);
        assert!((intervals[1].start - 605.3).abs() < 1e-6);
    }

    #[test]
    fn test_find_nearest_split_point() {
        let intervals = vec![
            SilenceInterval { start: 590.0, end: 592.0 }, // midpoint: 591.0
            SilenceInterval { start: 598.0, end: 600.0 }, // midpoint: 599.0
            SilenceInterval { start: 650.0, end: 652.0 }, // midpoint: 651.0
        ];

        // target=600, window=60 -> 591.0 と 599.0 が候補、599.0 が近い
        let point = find_nearest_split_point(&intervals, 600.0, 60.0);
        assert!(point.is_some());
        assert!((point.unwrap() - 599.0).abs() < 1e-6);
    }

    #[test]
    fn test_find_nearest_split_point_no_candidate() {
        let intervals = vec![
            SilenceInterval { start: 100.0, end: 101.0 },
        ];
        // target=600, window=60 -> 候補なし
        let point = find_nearest_split_point(&intervals, 600.0, 60.0);
        assert!(point.is_none());
    }

    #[test]
    fn test_find_nearest_candidate() {
        let candidates = vec![50.0, 100.0, 150.0, 200.0];
        // target=105, window=60 -> [45, 165] 内: 50, 100, 150 -> 100 が最近傍
        let result = find_nearest_candidate(&candidates, 105.0, 60.0);
        assert!(result.is_some());
        assert!((result.unwrap() - 100.0).abs() < 1e-6);
    }

    #[test]
    fn test_find_nearest_candidate_empty() {
        let result = find_nearest_candidate(&[], 100.0, 30.0);
        assert!(result.is_none());
    }
}
