# summarize-video

動画ファイルを分析・要約・検索するスキルです。`video-splitter` の MCP サーバーを通じて
Claude が直接動画を処理できます。

## トリガー条件

以下のようなリクエストで自動的にこのスキルを使用します:

- 「〜の動画を要約して」
- 「動画の作業内容をまとめて」
- 「動画で〜を検索して」
- 「〜.mp4 / .mov / .mkv / .webm を分析して」
- 「録画の内容を教えて」

---

## セットアップ

### 1. video-splitter バイナリの取得

**GitHub Releases からダウンロード（推奨）**:

```
https://github.com/7sb-yaboo/video-splitter/releases/latest
```

| プラットフォーム | ファイル名 |
|---|---|
| Windows (x64) | `video-splitter-x86_64-windows.exe` |
| Linux (x64) | `video-splitter-x86_64-linux` |
| macOS (Apple Silicon) | `video-splitter-aarch64-macos` |

**ソースからビルド**:

```bash
# 前提: LLVM/libclang, CMake, Rust (stable), FFmpeg が必要
git clone https://github.com/7sb-yaboo/video-splitter.git
cd video-splitter
cargo build --release
# バイナリ: target/release/video-splitter
```

### 2. Whisper モデルファイルの取得

```bash
# 小型モデル（高速・精度普通）
wget https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin

# 中型モデル（バランス型・推奨）
wget https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.bin

# 大型モデル（最高精度・低速）
wget https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3.bin
```

### 3. MCP サーバーの設定

**Claude Desktop の `claude_desktop_config.json`**:

```json
{
  "mcpServers": {
    "video-splitter": {
      "command": "python",
      "args": ["/path/to/video-splitter/mcp/server.py"],
      "env": {
        "VIDEO_SPLITTER_BIN": "/path/to/video-splitter",
        "WHISPER_MODEL": "/path/to/ggml-medium.bin"
      }
    }
  }
}
```

> **Windows の場合**: パスにバックスラッシュを使用、または `\\` でエスケープ。
> **macOS/Linux の場合**: フルパスを指定。`~` は展開されないため注意。

### 4. FFmpeg の確認

```bash
ffmpeg -version  # インストール済みであること
```

---

## ワークフロー

### Step 1: 前提確認

Claude が自動的に以下を確認します:

- MCP サーバー (`video-splitter`) が設定・接続されているか
- 動画ファイルのパスが有効か
- モデルファイルのパス（指定がない場合は環境変数 `WHISPER_MODEL` を使用）
- 文字起こしが必要か、動画分割のみか

### Step 2: 動画の処理

`process_video()` ツールを呼び出して分割・文字起こし・フレーム抽出を実行します。

```
process_video(
  video_path: "/path/to/video.mp4",
  output_dir: "/path/to/output",       # 省略時: 動画と同じディレクトリ
  transcribe: true,                     # 文字起こしを行う
  extract_frames: true,                 # キーフレームを抽出
  whisper_model: "/path/to/model.bin",  # モデルファイルパス
  language: "ja",                       # 言語コード
  interval: 600,                        # 分割間隔（秒）デフォルト: 600
  silence_threshold: -30,               # 無音検出閾値（dB）
  silence_duration: 0.5                 # 無音最小継続時間（秒）
)
```

返り値から `manifest_path` を取得します。

### Step 3: セグメント一覧の確認

`list_segments()` でセグメント数・総時間を把握します。

### Step 4: セグメントの分析

- `get_segment(segment_id)` で各セグメントの文字起こしテキストとキーフレーム画像を取得
- 長い動画は `has_transcript: true` のセグメントを優先

### Step 5: 出力生成

ユーザーの要求に応じて以下の形式で出力します:

| 要求 | 出力形式 |
|---|---|
| 動画要約 | 各セグメントの内容を 3〜5 文で要約 → 全体要約を生成 |
| 作業まとめ | 操作・手順・発話を時系列で箇条書き（タイムコード付き） |
| キーワード検索 | `search_transcript()` で該当箇所を抽出してタイムコード付きリスト |

---

## 使い方の例

### パターン 1: 動画要約

```
meeting.mp4 の内容を要約してください。
Whisper モデルは /models/ggml-medium.bin を使用してください。
```

### パターン 2: 作業まとめ（画面録画など）

```
tutorial-recording.mp4 の作業内容を時系列でまとめてください。
操作手順と発話内容を箇条書きにしてタイムコードを付けてください。
```

### パターン 3: キーワード検索

```
lecture.mp4 の中で「マイグレーション」について話している箇所を全て教えてください。
```

### パターン 4: 文字起こしのみ

```
interview.mp4 を文字起こしして SRT ファイルを生成してください。
要約は不要です。
```

---

## MCP ツール一覧

### `process_video`

動画を分割・文字起こし・フレーム抽出します。

| 引数 | 型 | 必須 | 説明 |
|---|---|---|---|
| `video_path` | string | ✓ | 入力動画ファイルのパス |
| `output_dir` | string | | 出力ディレクトリ（省略時: 動画と同じ場所） |
| `transcribe` | bool | | 文字起こしを行う（デフォルト: true） |
| `extract_frames` | bool | | キーフレームを抽出（デフォルト: false） |
| `whisper_model` | string | | GGML モデルファイルパス |
| `language` | string | | 言語コード（デフォルト: "ja"） |
| `interval` | int | | 目標分割間隔（秒、デフォルト: 600） |
| `silence_threshold` | float | | 無音検出 dB 閾値（デフォルト: -30） |
| `silence_duration` | float | | 無音最小継続時間（秒、デフォルト: 0.5） |

### `list_segments`

manifest.json から全セグメントの一覧を返します。

| 引数 | 型 | 必須 | 説明 |
|---|---|---|---|
| `manifest_path` | string | ✓ | manifest.json のパス |

### `get_segment`

特定セグメントの詳細（文字起こし・フレームパス）を返します。

| 引数 | 型 | 必須 | 説明 |
|---|---|---|---|
| `manifest_path` | string | ✓ | manifest.json のパス |
| `segment_id` | int | ✓ | セグメント番号（0 始まり） |

### `search_transcript`

全セグメントの SRT を横断検索します。

| 引数 | 型 | 必須 | 説明 |
|---|---|---|---|
| `manifest_path` | string | ✓ | manifest.json のパス |
| `query` | string | ✓ | 検索キーワードまたは正規表現 |

---

## 出力形式

### manifest.json の構造

```json
{
  "segments": [
    {
      "id": 0,
      "path": "output/segment_000.mp4",
      "start": 0.0,
      "end": 612.3,
      "duration": 612.3,
      "transcript_path": "output/segment_000.srt",
      "has_transcript": true,
      "frame_paths": ["output/frames/segment_000_frame.jpg"]
    }
  ]
}
```

### search_transcript の出力

```json
[
  {
    "segment_id": 2,
    "segment_path": "output/segment_002.mp4",
    "timestamp_start": "00:10:32,400",
    "timestamp_end": "00:10:38,100",
    "text": "マイグレーションを実行する前にバックアップを取ってください"
  }
]
```

---

## トラブルシューティング

| 症状 | 原因 | 対処 |
|---|---|---|
| `MCP server not found` | サーバーが起動していない | Claude Desktop を再起動、設定パスを確認 |
| `ffmpeg: command not found` | FFmpeg 未インストール | `brew install ffmpeg` / `apt install ffmpeg` |
| `model file not found` | モデルパスが間違い | 絶対パスで指定、`~` は使わない |
| 文字起こしが遅い | 大型モデル使用 | `ggml-small.bin` か `ggml-medium.bin` に変更 |
| 無音で分割されない | 閾値設定 | `silence_threshold: -40` など絶対値を上げる |
| セグメントが細かすぎる | interval が短い | `interval: 1200`（20分）など大きくする |
