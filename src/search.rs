use anyhow::{Context, Result};
use serde::Serialize;
use std::path::Path;

use crate::manifest::Manifest;
use crate::transcribe;

/// 検索ヒット結果
#[derive(Debug, Serialize)]
pub struct SearchResult {
    pub segment_index: usize,
    /// セグメント先頭起点の相対時刻（ms）
    pub segment_timestamp_ms: u64,
    /// 動画全体での絶対時刻（ms）
    pub absolute_timestamp_ms: u64,
    pub text: String,
}

/// manifest.json を読み込み、全 transcript を横断検索して結果を返す
pub fn search_transcript(manifest_path: &Path, query: &str) -> Result<Vec<SearchResult>> {
    let content = std::fs::read_to_string(manifest_path)
        .with_context(|| format!("manifest.json の読み込みに失敗しました: {}", manifest_path.display()))?;

    let manifest: Manifest = serde_json::from_str(&content)
        .with_context(|| "manifest.json のパースに失敗しました")?;

    // manifest.json が置かれているディレクトリを基準にパスを解決する
    let base_dir = manifest_path
        .parent()
        .unwrap_or_else(|| Path::new("."));

    let query_lower = query.to_lowercase();
    let mut results: Vec<SearchResult> = Vec::new();

    for segment in &manifest.segments {
        let Some(ref transcript_rel) = segment.transcript else {
            continue;
        };

        let transcript_path = base_dir.join(transcript_rel);
        let transcript_content = match std::fs::read_to_string(&transcript_path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!(
                    "  Warning: transcript ファイルを読み込めませんでした ({}): {}",
                    transcript_path.display(),
                    e
                );
                continue;
            }
        };

        let segment_offset_ms = (segment.start * 1000.0) as u64;

        // 拡張子に応じてパース方法を切り替える
        let ext = transcript_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        if ext == "srt" || ext == "vtt" {
            // SRT/VTT: parse_srt でエントリ単位に検索
            let entries = transcribe::parse_srt(&transcript_content);
            for entry in &entries {
                if entry.text.to_lowercase().contains(&query_lower) {
                    results.push(SearchResult {
                        segment_index: segment.index,
                        segment_timestamp_ms: entry.start_ms,
                        absolute_timestamp_ms: segment_offset_ms + entry.start_ms,
                        text: entry.text.clone(),
                    });
                }
            }
        } else {
            // TXT: 行単位で検索（タイムスタンプ情報がないので segment 先頭を使う）
            for line in transcript_content.lines() {
                if line.to_lowercase().contains(&query_lower) {
                    results.push(SearchResult {
                        segment_index: segment.index,
                        segment_timestamp_ms: 0,
                        absolute_timestamp_ms: segment_offset_ms,
                        text: line.trim().to_string(),
                    });
                }
            }
        }
    }

    results.sort_by_key(|r| r.absolute_timestamp_ms);
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write_file(dir: &Path, name: &str, content: &str) {
        std::fs::write(dir.join(name), content).unwrap();
    }

    #[test]
    fn test_search_transcript_srt() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path();

        // SRT ファイル
        let srt = "1\n00:00:01,000 --> 00:00:03,000\nファイルを保存します\n\n\
                   2\n00:00:05,000 --> 00:00:07,000\n別の作業を行います\n\n";
        write_file(dir, "seg_001.srt", srt);

        // manifest.json
        let manifest_json = serde_json::json!({
            "source": "test.mp4",
            "total_duration": 120.0,
            "language": "ja",
            "segments": [
                {
                    "index": 1,
                    "start": 600.0,
                    "end": 700.0,
                    "video": "seg_001.mp4",
                    "transcript": "seg_001.srt",
                    "key_frames": []
                }
            ]
        });
        let manifest_path = dir.join("manifest.json");
        std::fs::write(&manifest_path, manifest_json.to_string()).unwrap();

        let results = search_transcript(&manifest_path, "保存").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].segment_index, 1);
        assert_eq!(results[0].segment_timestamp_ms, 1_000);
        // 600s * 1000 + 1000ms = 601_000
        assert_eq!(results[0].absolute_timestamp_ms, 601_000);
        assert!(results[0].text.contains("保存"));
    }

    #[test]
    fn test_search_transcript_vtt() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path();

        // VTT ファイル（タイムスタンプに `.` を使う）
        let vtt = "WEBVTT\n\n\
                   1\n00:00:02.000 --> 00:00:04.000\n画面を録画します\n\n\
                   2\n00:00:06.500 --> 00:00:08.000\n別のシーン\n\n";
        write_file(dir, "seg_001.vtt", vtt);

        let manifest_json = serde_json::json!({
            "source": "test.mp4",
            "total_duration": 120.0,
            "language": "ja",
            "segments": [
                {
                    "index": 1,
                    "start": 300.0,
                    "end": 400.0,
                    "video": "seg_001.mp4",
                    "transcript": "seg_001.vtt",
                    "key_frames": []
                }
            ]
        });
        let manifest_path = dir.join("manifest.json");
        std::fs::write(&manifest_path, manifest_json.to_string()).unwrap();

        let results = search_transcript(&manifest_path, "録画").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].segment_index, 1);
        assert_eq!(results[0].segment_timestamp_ms, 2_000);
        // 300s * 1000 + 2000ms = 302_000
        assert_eq!(results[0].absolute_timestamp_ms, 302_000);
        assert!(results[0].text.contains("録画"));
    }

    #[test]
    fn test_search_transcript_no_match() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path();

        let srt = "1\n00:00:01,000 --> 00:00:03,000\nこんにちは\n\n";
        write_file(dir, "seg_001.srt", srt);

        let manifest_json = serde_json::json!({
            "source": "test.mp4",
            "total_duration": 60.0,
            "language": "ja",
            "segments": [
                {
                    "index": 1,
                    "start": 0.0,
                    "end": 60.0,
                    "video": "seg_001.mp4",
                    "transcript": "seg_001.srt",
                    "key_frames": []
                }
            ]
        });
        let manifest_path = dir.join("manifest.json");
        std::fs::write(&manifest_path, manifest_json.to_string()).unwrap();

        let results = search_transcript(&manifest_path, "存在しないキーワード").unwrap();
        assert!(results.is_empty());
    }
}
