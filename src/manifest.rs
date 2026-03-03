use anyhow::Result;
use serde::Serialize;
use std::path::Path;

/// 処理結果の全体メタデータ
#[derive(Debug, Serialize)]
pub struct Manifest {
    pub source: String,
    pub total_duration: f64,
    pub language: String,
    pub segments: Vec<SegmentMeta>,
}

/// セグメントごとのメタデータ（パスはすべて manifest.json からの相対パス）
#[derive(Debug, Serialize)]
pub struct SegmentMeta {
    pub index: usize,
    pub start: f64,
    pub end: f64,
    /// セグメント動画ファイルの相対パス
    pub video: String,
    /// 文字起こしファイルの相対パス（--transcribe 時のみ）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transcript: Option<String>,
    /// キーフレーム画像の相対パスリスト（--extract-frames 時のみ）
    pub key_frames: Vec<String>,
}

/// マニフェストを JSON ファイルに書き出す
pub fn write_manifest(manifest: &Manifest, path: &Path) -> Result<()> {
    let json = serde_json::to_string_pretty(manifest)
        .map_err(|e| anyhow::anyhow!("JSON シリアライズに失敗しました: {}", e))?;
    std::fs::write(path, json)
        .map_err(|e| anyhow::anyhow!("マニフェストの書き込みに失敗しました: {}", e))?;
    Ok(())
}

/// パスを base ディレクトリからの相対パスに変換する
/// JSON 内では OS に依存しないよう / に統一する
pub fn to_relative(path: &Path, base: &Path) -> String {
    path.strip_prefix(base)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}
