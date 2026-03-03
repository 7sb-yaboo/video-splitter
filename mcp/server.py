"""
video-splitter MCP サーバー

動画の分割・文字起こし・キーフレーム抽出を行う video-splitter CLI を
MCP ツールとして公開する。

環境変数:
    VIDEO_SPLITTER_BIN  : video-splitter バイナリのパス（デフォルト: "video-splitter"）
"""

from __future__ import annotations

import json
import os
import re
import subprocess
from pathlib import Path
from typing import Any

from mcp.server.fastmcp import FastMCP

# ── 設定 ──────────────────────────────────────────────────────────────────────

BINARY = os.environ.get("VIDEO_SPLITTER_BIN", "video-splitter")
mcp = FastMCP("video-splitter")


# ── ユーティリティ ─────────────────────────────────────────────────────────────


def _run(args: list[str]) -> tuple[int, str, str]:
    """サブプロセスを実行して (returncode, stdout, stderr) を返す"""
    result = subprocess.run(args, capture_output=True, text=True, encoding="utf-8")
    return result.returncode, result.stdout, result.stderr


def _manifest_path(input_path: str, output_dir: str | None) -> Path:
    """manifest.json の出力先パスを返す"""
    if output_dir:
        return Path(output_dir) / "manifest.json"
    return Path(input_path).parent / "manifest.json"


def _load_manifest(manifest_path: str) -> dict[str, Any]:
    path = Path(manifest_path)
    if not path.exists():
        raise FileNotFoundError(f"manifest.json が見つかりません: {manifest_path}")
    with path.open(encoding="utf-8") as f:
        return json.load(f)


def _resolve(manifest_path: str, relative: str) -> Path:
    """manifest.json からの相対パスを絶対パスに解決する"""
    return Path(manifest_path).parent / relative


def _parse_srt(content: str) -> list[dict[str, Any]]:
    """SRT テキストをパースしてエントリのリストを返す"""
    entries = []
    for block in content.replace("\r\n", "\n").split("\n\n"):
        lines = block.strip().splitlines()
        if len(lines) < 3:
            continue
        try:
            start_str, end_str = lines[1].split(" --> ")
            entries.append({
                "start": start_str.strip(),
                "end": end_str.strip(),
                "text": "\n".join(lines[2:]).strip(),
            })
        except (ValueError, IndexError):
            continue
    return entries


def _srt_to_ms(timestamp: str) -> int:
    """'HH:MM:SS,mmm' を絶対ミリ秒に変換する"""
    m = re.match(r"(\d+):(\d+):(\d+)[,.](\d+)", timestamp)
    if not m:
        return 0
    h, mi, s, ms = (int(x) for x in m.groups())
    return h * 3_600_000 + mi * 60_000 + s * 1_000 + ms


def _ms_to_timestamp(ms: int) -> str:
    """絶対ミリ秒を 'HH:MM:SS,mmm' 形式に変換する"""
    h = ms // 3_600_000
    m = (ms % 3_600_000) // 60_000
    s = (ms % 60_000) // 1_000
    millis = ms % 1_000
    return f"{h:02}:{m:02}:{s:02},{millis:03}"


# ── MCP ツール ──────────────────────────────────────────────────────────────────


@mcp.tool()
def process_video(
    input_path: str,
    duration: float = 600.0,
    transcribe: bool = False,
    whisper_model: str = "",
    language: str = "ja",
    transcribe_format: str = "srt",
    extract_frames: bool = False,
    frames_scene_threshold: float = 0.3,
    frames_interval: float = 30.0,
    output_dir: str = "",
) -> dict[str, Any]:
    """
    動画を分割し、文字起こし・キーフレーム抽出を実行する。

    Args:
        input_path            : 入力動画ファイルパス（必須）
        duration              : 分割間隔（秒）
        transcribe            : True で文字起こしを実行する（whisper_model が必要）
        whisper_model         : whisper.cpp モデルファイルパス
        language              : 音声言語コード（ja / en / auto）
        transcribe_format     : 文字起こし出力形式（srt / txt / vtt）
        extract_frames        : True でキーフレーム抽出を実行する
        frames_scene_threshold: シーン変化検出の感度（0.0〜1.0）
        frames_interval       : フォールバック間隔（秒）
        output_dir            : 出力先ディレクトリ（省略時は入力ファイルと同じディレクトリ）

    Returns:
        manifest_path, segments 数などの処理結果
    """
    args = [BINARY, input_path, "--manifest", "--duration", str(duration)]

    if output_dir:
        args += ["--output-dir", output_dir]

    if transcribe:
        if not whisper_model:
            return {"error": "--transcribe には whisper_model の指定が必要です"}
        args += [
            "--transcribe",
            "--whisper-model", whisper_model,
            "--language", language,
            "--transcribe-format", transcribe_format,
        ]

    if extract_frames:
        args += [
            "--extract-frames",
            "--frames-scene-threshold", str(frames_scene_threshold),
            "--frames-interval", str(frames_interval),
        ]

    returncode, stdout, stderr = _run(args)

    if returncode != 0:
        return {
            "error": f"video-splitter が失敗しました（exit code: {returncode}）",
            "detail": stderr.strip(),
        }

    manifest = _manifest_path(input_path, output_dir or None)
    if not manifest.exists():
        return {
            "error": "manifest.json が生成されませんでした",
            "stdout": stdout.strip(),
        }

    data = json.loads(manifest.read_text(encoding="utf-8"))
    return {
        "manifest_path": str(manifest),
        "segments": len(data.get("segments", [])),
        "total_duration": data.get("total_duration"),
        "source": data.get("source"),
    }


@mcp.tool()
def get_segment(manifest_path: str, index: int) -> dict[str, Any]:
    """
    マニフェストから指定セグメントの詳細情報を返す。

    Args:
        manifest_path: process_video が返した manifest.json のパス
        index        : セグメント番号（1 始まり）

    Returns:
        動画パス、文字起こしテキスト、キーフレームパスリストなど
    """
    try:
        data = _load_manifest(manifest_path)
    except FileNotFoundError as e:
        return {"error": str(e)}

    segments = data.get("segments", [])
    seg = next((s for s in segments if s["index"] == index), None)
    if seg is None:
        return {
            "error": f"セグメント {index} が見つかりません（利用可能: 1〜{len(segments)}）"
        }

    result: dict[str, Any] = {
        "index": seg["index"],
        "start": seg["start"],
        "end": seg["end"],
        "duration": round(seg["end"] - seg["start"], 3),
        "video_path": str(_resolve(manifest_path, seg["video"])),
    }

    # 文字起こしテキストを読み込んで返す
    if seg.get("transcript"):
        transcript_file = _resolve(manifest_path, seg["transcript"])
        if transcript_file.exists():
            result["transcript_path"] = str(transcript_file)
            result["transcript_text"] = transcript_file.read_text(encoding="utf-8")

    # キーフレームパスを絶対パスに解決して返す
    if seg.get("key_frames"):
        result["key_frame_paths"] = [
            str(_resolve(manifest_path, f)) for f in seg["key_frames"]
        ]

    return result


@mcp.tool()
def search_transcript(manifest_path: str, query: str) -> list[dict[str, Any]]:
    """
    全セグメントの文字起こしをキーワード検索し、タイムコード付きで返す。

    Args:
        manifest_path: process_video が返した manifest.json のパス
        query        : 検索キーワード

    Returns:
        マッチした箇所のリスト（セグメント番号・絶対タイムスタンプ・テキスト）
    """
    try:
        data = _load_manifest(manifest_path)
    except FileNotFoundError as e:
        return [{"error": str(e)}]

    results: list[dict[str, Any]] = []
    query_lower = query.lower()

    for seg in data.get("segments", []):
        if not seg.get("transcript"):
            continue

        transcript_file = _resolve(manifest_path, seg["transcript"])
        if not transcript_file.exists():
            continue

        content = transcript_file.read_text(encoding="utf-8")
        ext = transcript_file.suffix.lower()

        # SRT / VTT はタイムスタンプ付きでパース、TXT は行単位で検索
        if ext in (".srt", ".vtt"):
            for entry in _parse_srt(content):
                if query_lower in entry["text"].lower():
                    # セグメント開始時刻を加算して動画全体での絶対時刻を求める
                    seg_offset_ms = int(seg["start"] * 1000)
                    abs_ms = _srt_to_ms(entry["start"]) + seg_offset_ms
                    results.append({
                        "segment_index": seg["index"],
                        "absolute_timestamp": _ms_to_timestamp(abs_ms),
                        "segment_timestamp": entry["start"],
                        "text": entry["text"],
                    })
        else:
            for line in content.splitlines():
                if query_lower in line.lower():
                    results.append({
                        "segment_index": seg["index"],
                        "absolute_timestamp": _ms_to_timestamp(int(seg["start"] * 1000)),
                        "segment_timestamp": "00:00:00,000",
                        "text": line.strip(),
                    })

    return results


@mcp.tool()
def list_segments(manifest_path: str) -> list[dict[str, Any]]:
    """
    マニフェスト内の全セグメントの概要を返す。

    Args:
        manifest_path: process_video が返した manifest.json のパス

    Returns:
        各セグメントのインデックス・時間範囲・保有ファイル種別の一覧
    """
    try:
        data = _load_manifest(manifest_path)
    except FileNotFoundError as e:
        return [{"error": str(e)}]

    return [
        {
            "index": seg["index"],
            "start": seg["start"],
            "end": seg["end"],
            "duration": round(seg["end"] - seg["start"], 3),
            "has_transcript": bool(seg.get("transcript")),
            "key_frame_count": len(seg.get("key_frames", [])),
        }
        for seg in data.get("segments", [])
    ]


# ── エントリポイント ────────────────────────────────────────────────────────────


def main() -> None:
    mcp.run()


if __name__ == "__main__":
    main()
