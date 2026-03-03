# video-splitter

動画ファイルを指定した時間間隔ごとに自動分割する CLI ツールです。
FFmpeg の `silencedetect` フィルターで無音区間を検出し、ターゲット時刻に最も近い無音区間で分割するため、話の途中でぶつ切りになりません。
オプションで文字起こし（[whisper.cpp](https://github.com/ggerganov/whisper.cpp)）とキーフレーム抽出も実行でき、MCP サーバー経由で AI に作業フローを分析させることを主な用途としています。

## 特徴

- 指定した間隔（デフォルト 10 分）ごとに動画を分割
- 無音区間を検出して自然な区切り位置で分割（会話・講義の途中で切れない）
- 候補となる無音区間がなければターゲット時刻でそのまま分割
- コーデックコピー (`-c copy`) による高速・無劣化処理
- 出力ファイル名は `{元のファイル名}_{連番3桁}.{拡張子}` 形式
- **`--transcribe`** で全体を1回だけ whisper 処理 → SRT をセグメント単位でスライス
- **`--extract-frames`** でシーン変化点のキーフレームを JPEG 抽出
- **`--manifest`** で動画・文字起こし・フレームパスをまとめた `manifest.json` を生成
- **MCP サーバー**（`mcp/server.py`）経由で AI エージェントから直接呼び出し可能

## システム全体の処理フロー

```
動画ファイル
  │
  ├─ [1回] whisper → 全体 SRT（タイムスタンプ付き）
  ├─ [N回] FFmpeg  → セグメント動画 × N
  │                    ├─ SRT をタイムスタンプでスライス → セグメント SRT × N
  │                    └─ FFmpeg シーン変化検出 → キーフレーム JPEG × N
  │
  └─ manifest.json（全セグメントの動画・SRT・フレームパスを統合）
       │
       └─ MCP サーバー → AI エージェント（作業フロー分析・要約）
```

## 必要環境

- [Rust](https://www.rust-lang.org/tools/install) 1.70 以上
- [FFmpeg](https://ffmpeg.org/download.html)（PATH が通っているか `--ffmpeg` で指定）
- [whisper.cpp](https://github.com/ggerganov/whisper.cpp)（`--transcribe` 使用時のみ）
- Python 3.10 以上 ＋ `mcp` パッケージ（MCP サーバー使用時のみ）

## インストール

```bash
git clone <このリポジトリのURL>
cd video-splitter
cargo build --release
```

ビルド後のバイナリは `target/release/video-splitter`（Windows では `.exe`）に生成されます。

## CLI の使い方

```
video-splitter [OPTIONS] <INPUT>
```

### 基本例

```bash
# 10 分ごとに分割（デフォルト）
video-splitter lecture.mp4

# 分割 + 文字起こし（SRT）+ キーフレーム + manifest.json を一括生成
video-splitter lecture.mp4 \
  --transcribe --whisper-model models/ggml-base.bin \
  --extract-frames \
  --manifest
```

### 実行例

```
$ video-splitter lecture.mp4 --transcribe --whisper-model models/ggml-base.bin \
    --extract-frames --manifest --transcribe-format srt

Analyzing video: lecture.mp4
  Total duration: 3612.5s (60.2 min)
Detecting silence in: lecture.mp4
  Found 47 silence interval(s)
Split points (6 total):
    1.  598.450s (10.0 min)
    ...
Splitting into 7 segment(s)...
Transcribing full video (1 pass)...
  -> OK: 312 entries
[1/7] Cutting: lecture_001.mp4 (0.0s - 598.5s)
  -> OK: /out/lecture_001.mp4
  -> OK: /out/lecture_001.srt (47 entries)
  -> OK: /out/lecture_001_frames/ (12 frames)
...
Manifest: /out/manifest.json
Done! 7 file(s) created in: /out
```

### 生成される manifest.json の構造

```json
{
  "source": "lecture.mp4",
  "total_duration": 3612.5,
  "language": "ja",
  "segments": [
    {
      "index": 1,
      "start": 0.0,
      "end": 598.5,
      "video": "lecture_001.mp4",
      "transcript": "lecture_001.srt",
      "key_frames": [
        "lecture_001_frames/frame_0001.jpg",
        "lecture_001_frames/frame_0008.jpg"
      ]
    }
  ]
}
```

## MCP サーバーのセットアップ

### インストール

```bash
cd mcp
pip install -e .
```

### MCP ツール一覧

| ツール | 説明 |
|--------|------|
| `process_video` | 動画の分割・文字起こし・フレーム抽出を実行し `manifest.json` パスを返す |
| `list_segments` | マニフェスト内の全セグメント概要を返す |
| `get_segment` | 指定セグメントの動画パス・文字起こしテキスト・フレームパスを返す |
| `search_transcript` | キーワードで全セグメントを横断検索し、タイムコード付きで返す |

### Claude Desktop への登録

`claude_desktop_config.json` に以下を追加します：

```json
{
  "mcpServers": {
    "video-splitter": {
      "command": "python",
      "args": ["/absolute/path/to/video-splitter/mcp/server.py"],
      "env": {
        "VIDEO_SPLITTER_BIN": "/absolute/path/to/video-splitter/target/release/video-splitter"
      }
    }
  }
}
```

### 利用例（AI との対話）

```
ユーザー: lecture.mp4 の作業フローをまとめてください
AI: process_video("lecture.mp4", transcribe=True, extract_frames=True, ...) を呼び出す
    → manifest.json 生成
    list_segments(manifest_path) → セグメント一覧取得
    get_segment(manifest_path, 1) → 動画パス + SRT テキスト + フレームパスを取得
    ... 各セグメントを順に参照して作業フローを生成
```

## CLI オプション一覧

### 分割オプション

| オプション | デフォルト | 説明 |
|-----------|-----------|------|
| `INPUT` | （必須）| 入力動画ファイルパス |
| `-d, --duration` | `600.0` | 分割間隔（秒）|
| `-o, --output-dir` | 入力と同じディレクトリ | 出力先ディレクトリ |
| `--noise-threshold` | `-30.0` | 無音判定閾値 (dB) |
| `--silence-duration` | `0.5` | 無音として認識する最短持続時間（秒）|
| `--search-window` | `60.0` | ターゲット時刻の前後を探索する範囲（秒）|
| `--ffmpeg` | `"ffmpeg"` | FFmpeg パス（環境変数 `FFMPEG_PATH` でも指定可）|
| `-v, --verbose` | `false` | 詳細ログを表示する |

### 文字起こしオプション

| オプション | デフォルト | 説明 |
|-----------|-----------|------|
| `--transcribe` | `false` | 文字起こしを有効にする（`--whisper-model` が必須）|
| `--whisper` | `"whisper-cpp"` | whisper.cpp パス（環境変数 `WHISPER_PATH` でも指定可）|
| `--whisper-model` | なし | モデルファイルパス |
| `--language` | `"ja"` | 音声言語コード（`ja` / `en` / `auto` など）|
| `--transcribe-format` | `"txt"` | 出力形式（`txt` / `srt` / `vtt`）|

### キーフレーム抽出オプション

| オプション | デフォルト | 説明 |
|-----------|-----------|------|
| `--extract-frames` | `false` | キーフレーム抽出を有効にする |
| `--frames-scene-threshold` | `0.3` | シーン変化の感度（0.0〜1.0）|
| `--frames-interval` | `30.0` | シーン変化未検出時のフォールバック間隔（秒、0.0 = 無効）|

### マニフェストオプション

| オプション | デフォルト | 説明 |
|-----------|-----------|------|
| `--manifest` | `false` | `manifest.json` を生成する |

## テスト

```bash
cargo test
```

## ライセンス

MIT
