# video-splitter

動画ファイルを指定した時間間隔ごとに自動分割する CLI ツールです。
FFmpeg の `silencedetect` フィルターで無音区間を検出し、ターゲット時刻に最も近い無音区間で分割するため、話の途中でぶつ切りになりません。
オプションで各セグメントの文字起こし（[whisper.cpp](https://github.com/ggerganov/whisper.cpp) 利用）も同時に実行できます。

## 特徴

- 指定した間隔（デフォルト 10 分）ごとに動画を分割
- 無音区間を検出して自然な区切り位置で分割（会話・講義の途中で切れない）
- 候補となる無音区間がなければターゲット時刻でそのまま分割
- コーデックコピー (`-c copy`) による高速・無劣化処理
- 出力ファイル名は `{元のファイル名}_{連番3桁}.{拡張子}` 形式
- **`--transcribe` で各セグメントの文字起こしを自動生成**（`.txt` / `.srt` / `.vtt`）

### 文字起こしの仕組み

文字起こしは **動画全体を1回だけ** whisper.cpp で処理し、得られた SRT をセグメントの時間範囲で切り出す方式を採用しています。

```
動画ファイル
  ├─ [1回] whisper → 全体 SRT（タイムスタンプ付き）
  └─ [N回] FFmpeg → セグメント動画 × N
                         ↓
               SRT をタイムスタンプでスライス
                         ↓
               セグメントごとの文字起こしファイル × N
```

これにより、セグメント境界で文脈が途切れず、whisper の実行は1回で済むため処理が効率的です。
MCP ツール等からセグメント動画と文字起こしをセットで参照することで、AI による作業フローの把握・要約が行えます。

## 必要環境

- [Rust](https://www.rust-lang.org/tools/install) 1.70 以上
- [FFmpeg](https://ffmpeg.org/download.html)（PATH が通っているか `--ffmpeg` で指定）
- [whisper.cpp](https://github.com/ggerganov/whisper.cpp)（`--transcribe` を使う場合のみ）

## インストール

```bash
git clone <このリポジトリのURL>
cd video-splitter
cargo build --release
```

ビルド後のバイナリは `target/release/video-splitter` (Windows では `.exe`) に生成されます。

## 使い方

```
video-splitter [OPTIONS] <INPUT>
```

### 基本例

```bash
# lecture.mp4 を 10 分ごとに分割（デフォルト）
video-splitter lecture.mp4

# 30 分ごとに分割、出力先を指定
video-splitter lecture.mp4 --duration 1800 --output-dir ./output
```

### 文字起こしを同時に実行する

```bash
# 分割 + 各セグメントの文字起こし（.txt）を生成
video-splitter lecture.mp4 --transcribe --whisper-model models/ggml-base.bin

# タイムスタンプ付き SRT 形式で出力（AI 参照用に推奨）
video-splitter lecture.mp4 --transcribe --whisper-model models/ggml-base.bin --transcribe-format srt

# 英語の動画を文字起こし
video-splitter lecture.mp4 --transcribe --whisper-model models/ggml-base.bin --language en
```

### 実行例

```
$ video-splitter lecture.mp4 --transcribe --whisper-model models/ggml-base.bin --transcribe-format srt

Analyzing video: lecture.mp4
  Total duration: 3612.5s (60.2 min)
Detecting silence in: lecture.mp4
  Found 47 silence interval(s)
Split points (6 total):
    1.  598.450s (10.0 min)
    2. 1201.230s (20.0 min)
    ...
Splitting into 7 segment(s)...
Transcribing full video (1 pass)...
  -> OK: 312 entries
[1/7] Cutting: lecture_001.mp4 (0.0s - 598.5s)
  -> OK: /path/to/lecture_001.mp4
  -> OK: /path/to/lecture_001.srt (47 entries)
...
Done! 7 file(s) created in: /path/to/
```

## オプション一覧

### 分割オプション

| オプション | デフォルト | 説明 |
|-----------|-----------|------|
| `INPUT` | （必須）| 入力動画ファイルパス |
| `-d, --duration` | `600.0` | 分割間隔（秒）|
| `-o, --output-dir` | 入力と同じディレクトリ | 出力先ディレクトリ |
| `--noise-threshold` | `-30.0` | 無音判定閾値 (dB)。値を大きくすると無音と判定されやすくなる |
| `--silence-duration` | `0.5` | 無音として認識する最短持続時間（秒）|
| `--search-window` | `60.0` | ターゲット時刻の前後を探索する範囲（秒）|
| `--ffmpeg` | `"ffmpeg"` | FFmpeg 実行ファイルのパス（環境変数 `FFMPEG_PATH` でも指定可）|
| `-v, --verbose` | `false` | FFmpeg / whisper.cpp コマンドの詳細ログを表示する |
| `-h, --help` | | ヘルプを表示 |
| `-V, --version` | | バージョンを表示 |

### 文字起こしオプション

| オプション | デフォルト | 説明 |
|-----------|-----------|------|
| `--transcribe` | `false` | 文字起こしを有効にする（`--whisper-model` が必須）|
| `--whisper` | `"whisper-cpp"` | whisper.cpp 実行ファイルのパス（環境変数 `WHISPER_PATH` でも指定可）|
| `--whisper-model` | なし | モデルファイルパス（`--transcribe` 時は必須）|
| `--language` | `"ja"` | 音声言語コード（`ja` / `en` / `auto` など）|
| `--transcribe-format` | `"txt"` | 出力形式（`txt` / `srt` / `vtt`）|

#### 出力形式の選び方

| 形式 | 内容 | 用途 |
|------|------|------|
| `txt` | プレーンテキスト | 読みやすさ優先・軽量 |
| `srt` | タイムスタンプ付き字幕 | AI による作業フロー分析・映像との対応付けに推奨 |
| `vtt` | WebVTT 形式 | ブラウザ / 動画プレイヤーでの字幕表示 |

## 調整のヒント

### 無音が検出されにくい場合
音量が大きめの動画では `--noise-threshold` を下げる（より静かな区間のみを無音とみなす）。

```bash
video-splitter lecture.mp4 --noise-threshold -40
```

### 短い無音区間も利用したい場合
`--silence-duration` を小さくする。

```bash
video-splitter lecture.mp4 --silence-duration 0.3
```

### FFmpeg がインストールされていない / PATH が通っていない場合
`--ffmpeg` オプションまたは環境変数 `FFMPEG_PATH` でフルパスを指定する。

```bash
video-splitter lecture.mp4 --ffmpeg /usr/local/bin/ffmpeg
# または
FFMPEG_PATH=/usr/local/bin/ffmpeg video-splitter lecture.mp4
```

### whisper.cpp がインストールされていない / PATH が通っていない場合
`--whisper` オプションまたは環境変数 `WHISPER_PATH` でフルパスを指定する。

```bash
video-splitter lecture.mp4 --transcribe --whisper-model models/ggml-base.bin \
  --whisper /usr/local/bin/whisper-cpp
# または
WHISPER_PATH=/usr/local/bin/whisper-cpp video-splitter lecture.mp4 \
  --transcribe --whisper-model models/ggml-base.bin
```

### 文字起こし精度を上げたい場合
より大きなモデル（`ggml-small` / `ggml-medium` / `ggml-large`）を使う。モデルは [whisper.cpp のリリースページ](https://github.com/ggerganov/whisper.cpp) からダウンロードできます。

```bash
video-splitter lecture.mp4 --transcribe --whisper-model models/ggml-large-v3.bin
```

## テスト

```bash
cargo test
```

## ライセンス

MIT
