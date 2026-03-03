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

## 2. Whisper モデルファイルの取得

```bash
# 小型モデル（高速・精度普通）
wget https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin

# 中型モデル（バランス型・推奨）
wget https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.bin

# 大型モデル（最高精度・低速）
wget https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3.bin
```

## 3. FFmpeg の確認

```bash
# macOS
brew install ffmpeg

# Ubuntu/Debian
sudo apt install ffmpeg

# Windows: https://ffmpeg.org/download.html からインストール後 PATH に追加
```
