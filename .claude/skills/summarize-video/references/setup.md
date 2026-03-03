# セットアップ手順

## 1. video-splitter バイナリの取得

**GitHub Releases からダウンロード（推奨）**:
`https://github.com/7sb-yaboo/video-splitter/releases/latest`

| プラットフォーム | ファイル名 |
|---|---|
| Windows (x64) | `video-splitter-x86_64-windows.exe` |
| Linux (x64) | `video-splitter-x86_64-linux` |

> **macOS**: v0.1.0 はビルド済みバイナリなし。ソースからビルドが必要。

ダウンロード後、PATH の通ったディレクトリに配置するか、フルパスで指定する。

**ソースからビルド（macOS / 開発者向け）**:

```bash
# 前提: LLVM/libclang, CMake, Rust (stable), FFmpeg が必要
git clone https://github.com/7sb-yaboo/video-splitter.git
cd video-splitter
cargo build --release
# バイナリ: target/release/video-splitter
```

## 2. Whisper モデルの取得

`scripts/download_model.py` を使って対話的にダウンロードできます（推奨）:

```bash
# モデル一覧を確認
python .claude/skills/summarize-video/scripts/download_model.py --list

# medium モデルをダウンロード（~/.cache/whisper/ に保存）
python .claude/skills/summarize-video/scripts/download_model.py --model medium
```

手動でダウンロードする場合は `https://huggingface.co/ggerganov/whisper.cpp/` から
目的のモデルファイル（`.bin`）を取得してください。

## 3. FFmpeg の確認

```bash
# macOS
brew install ffmpeg

# Ubuntu/Debian
sudo apt install ffmpeg

# Windows: https://ffmpeg.org/download.html からインストール後 PATH に追加
```
