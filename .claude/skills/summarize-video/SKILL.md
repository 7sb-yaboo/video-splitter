# summarize-video

動画ファイルを分析・要約・検索するスキルです。`video-splitter` CLI を Claude Code の
Bash ツール（ターミナル）から直接呼び出して動画を処理します。

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

> **macOS**: v0.1.0 はビルド済みバイナリなし。下記のソースからビルドを参照してください。

ダウンロード後、PATH の通ったディレクトリに配置するか、フルパスで指定してください。

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

### 3. FFmpeg の確認

```bash
ffmpeg -version  # インストール済みであること
```

---

## ワークフロー

### Step 1: 前提確認

Claude Code が自動的に以下を確認します:

- `video-splitter` バイナリが実行可能か
- 動画ファイルのパスが有効か
- Whisper モデルファイルのパスが指定されているか
- 文字起こしが必要か、動画分割のみか

### Step 2: 動画の処理

`video-splitter` を実行して分割・文字起こし・manifest 生成を行います。

```bash
video-splitter /path/to/video.mp4 \
  --output-dir /path/to/output \
  --duration 600 \
  --transcribe \
  --whisper-model /path/to/ggml-medium.bin \
  --language ja \
  --transcribe-format txt \
  --manifest
```

実行後、出力ディレクトリに以下が生成されます:

```
output/
  segment_001.mp4
  segment_001.txt      # 文字起こし（--transcribe-format txt の場合）
  segment_002.mp4
  segment_002.txt
  ...
  manifest.json
```

### Step 3: manifest.json を読んで構造を把握

`manifest.json` を Read ツールで読み込み、セグメント数・総時間・各ファイルパスを確認します。

```json
{
  "source": "/path/to/video.mp4",
  "total_duration": 3672.5,
  "language": "ja",
  "segments": [
    {
      "index": 1,
      "start": 0.0,
      "end": 612.3,
      "video": "segment_001.mp4",
      "transcript": "segment_001.txt",
      "key_frames": []
    }
  ]
}
```

### Step 4: 各セグメントの文字起こしを読む

`transcript` フィールドのパスを Read ツールで順に読み込み、内容を把握します。

長い動画では全セグメントを読み込むとコンテキストが膨らむため、
目的に応じて以下の方針をとります:

- **要約** — 全セグメントを読んで全体像を把握してから要約を生成
- **作業まとめ** — 全セグメントを読んで時系列の箇条書きを生成
- **キーワード検索** — `--search` オプションで絞り込んでから該当箇所のみ読む

### Step 5: 出力生成

ユーザーの要求に応じて以下の形式で出力します:

| 要求 | 出力形式 |
|---|---|
| 動画要約 | 各セグメントの内容を 3〜5 文で要約 → 全体要約を生成 |
| 作業まとめ | 操作・手順・発話を時系列で箇条書き（タイムコード付き） |
| キーワード検索 | 該当箇所をタイムコード付きリストで表示 |

---

## CLI オプション一覧

### 動画処理モード

```
video-splitter <INPUT> [OPTIONS]
```

| オプション | デフォルト | 説明 |
|---|---|---|
| `--output-dir`, `-o` | 入力と同じディレクトリ | 出力先ディレクトリ |
| `--duration`, `-d` | `600` | 目標分割間隔（秒） |
| `--noise-threshold` | `-30.0` | 無音判定閾値（dB） |
| `--silence-duration` | `0.5` | 無音の最短持続時間（秒） |
| `--search-window` | `60.0` | 分割候補の前後探索範囲（秒） |
| `--transcribe` | false | 文字起こしを有効にする |
| `--whisper-model` | — | GGML モデルファイルパス（`--transcribe` 時必須） |
| `--language` | `ja` | 音声言語コード |
| `--transcribe-format` | `txt` | 文字起こし形式（`txt` / `srt` / `vtt`） |
| `--extract-frames` | false | キーフレーム抽出を有効にする |
| `--frames-scene-threshold` | `0.3` | フレーム抽出のシーン感度（0.0〜1.0） |
| `--frames-interval` | `30.0` | フレームのフォールバック間隔（秒） |
| `--split-on-scene` | false | シーン変化点も分割候補に加える |
| `--scene-threshold` | `0.4` | シーン変化検出の閾値（0.0〜1.0） |
| `--manifest` | false | `manifest.json` を生成する |
| `--ffmpeg` | `ffmpeg` | FFmpeg 実行ファイルパス（環境変数 `FFMPEG_PATH` でも可） |
| `--verbose`, `-v` | false | 詳細ログを表示する |

### 検索モード

```
video-splitter <MANIFEST_JSON> --search <QUERY>
```

`manifest.json` のパスを INPUT に指定し、`--search` でキーワードを渡します。
結果は JSON 形式で stdout に出力されます。

```bash
video-splitter output/manifest.json --search "マイグレーション"
```

出力例:

```json
[
  {
    "segment_index": 2,
    "segment_path": "output/segment_002.mp4",
    "start": 632.1,
    "end": 638.4,
    "text": "マイグレーションを実行する前にバックアップを取ってください"
  }
]
```

---

## 使い方の例

### パターン 1: 動画要約

```
meeting.mp4 の内容を要約してください。
Whisper モデルは /models/ggml-medium.bin を使用してください。
```

Claude Code が実行するコマンド:

```bash
video-splitter meeting.mp4 \
  --output-dir meeting_out \
  --transcribe --whisper-model /models/ggml-medium.bin \
  --manifest
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

Claude Code が実行するコマンド（処理済みの場合）:

```bash
video-splitter lecture_out/manifest.json --search "マイグレーション"
```

### パターン 4: 文字起こしのみ（SRT 形式）

```
interview.mp4 を文字起こしして SRT ファイルを生成してください。
要約は不要です。
```

```bash
video-splitter interview.mp4 \
  --transcribe --whisper-model /models/ggml-medium.bin \
  --transcribe-format srt \
  --manifest
```

---

## トラブルシューティング

| 症状 | 原因 | 対処 |
|---|---|---|
| `command not found: video-splitter` | バイナリが PATH にない | フルパスで指定するか PATH に追加 |
| `ffmpeg: command not found` | FFmpeg 未インストール | `brew install ffmpeg` / `apt install ffmpeg` |
| `--whisper-model を指定してください` | モデルパス未指定 | `--whisper-model /path/to/model.bin` を追加 |
| `model file not found` | モデルパスが間違い | 絶対パスで指定（`~` は展開されない場合あり） |
| 文字起こしが遅い | 大型モデル使用 | `ggml-small.bin` か `ggml-medium.bin` に変更 |
| 無音で分割されない | 閾値設定 | `--noise-threshold -40` など絶対値を上げる |
| セグメントが細かすぎる | duration が短い | `--duration 1200`（20 分）など大きくする |
| 動画が分割されない | 動画が duration 以下 | 正常動作。分割不要と判断されている |
