---
name: summarize-video
description: >
  video-splitter CLI を使って動画の文字起こし・要約・検索を行うガイド。
  以下の場合に使用:
  (1) 動画・録画の内容を要約・まとめたい,
  (2) 動画の発話を文字起こしして SRT/TXT ファイルを生成したい,
  (3) 動画内のキーワードを検索したい,
  (4) .mp4/.mov/.mkv/.webm ファイルを分析したい。
  Claude Code のターミナル（Bash ツール）から直接 CLI を実行する。MCP サーバーは使用しない。
compatibility: "Requires: video-splitter binary, FFmpeg in PATH, Whisper GGML model file (.bin)"
---

# summarize-video

`video-splitter` CLI を Bash ツールで呼び出し、動画を分割・文字起こし・要約・検索する。

## 前提確認

```bash
video-splitter --version   # PATH にない場合はフルパスで指定
ffmpeg -version
```

初回セットアップが必要な場合は [references/setup.md](references/setup.md) を参照。

## ワークフロー

### Step 1: 情報収集

- 動画ファイルのパスを確認する
- Whisper モデルを確認する（下記フロー）
- 出力先ディレクトリを決める（省略時: 動画と同じ場所）

#### Whisper モデルの確認フロー

1. ユーザーがモデルパスを指定済みかつファイルが存在する → そのまま使用
2. 未指定またはファイルが存在しない → 以下の候補をユーザーに提示する:

| モデル | ファイル名 | サイズ | 特徴 |
|--------|-----------|--------|------|
| tiny | ggml-tiny.bin | ~75MB | 最高速・低精度 |
| base | ggml-base.bin | ~148MB | 高速・普通精度 |
| small | ggml-small.bin | ~488MB | バランス型 |
| **medium（推奨）** | ggml-medium.bin | ~1.5GB | 高精度・日本語に最適 |
| large-v3 | ggml-large-v3.bin | ~3.1GB | 最高精度・低速 |

3. ユーザーが選択したら `scripts/download_model.py` でダウンロードする:

```bash
# スクリプトは保存先パスを stdout に出力する
python .claude/skills/summarize-video/scripts/download_model.py --model <選択名> --quiet
```

4. 出力されたパスを `--whisper-model` に使用する

### Step 2: 動画を処理する

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

完了後: `output/segment_NNN.mp4`、`segment_NNN.txt`、`manifest.json` が生成される。

### Step 3: manifest.json を読む

Read ツールで `manifest.json` を読み込み、セグメント一覧と各 `transcript` パスを確認する。

### Step 4: 文字起こしを読む

| 目的 | 方針 |
|------|------|
| 要約・作業まとめ | 全セグメントの `.txt` を順に Read する |
| キーワード検索 | `--search` で絞り込んでから該当箇所のみ Read する |

**検索モード**:
```bash
video-splitter /path/to/output/manifest.json --search "キーワード"
```

### Step 5: 出力を生成する

| 要求 | 出力形式 |
|------|---------|
| 動画要約 | 各セグメントを 3〜5 文で要約 → 全体要約 |
| 作業まとめ | 操作・発話を時系列箇条書き（タイムコード付き） |
| キーワード検索 | タイムコード付き一覧 |

## 参照

- **全 CLI オプション**: [references/cli-options.md](references/cli-options.md)
- **セットアップ手順**: [references/setup.md](references/setup.md)
- **トラブルシューティング**: [references/troubleshooting.md](references/troubleshooting.md)
