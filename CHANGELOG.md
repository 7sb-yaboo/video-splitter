# 変更履歴

このプロジェクトの重要な変更はすべてこのファイルに記録されます。

フォーマットは [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) に基づき、
バージョン管理は [Semantic Versioning](https://semver.org/spec/v2.0.0.html) に準拠しています。

## [0.1.1] - 2026-03-04

### 追加

#### summarize-video スキル
- `scripts/download_model.py`: Whisper GGML モデルの一覧表示・自動ダウンロードスクリプト
  - `--list`: 利用可能な5モデル（tiny / base / small / medium / large-v3）を推奨マーク付きで表示
  - `--model <name>`: 指定モデルを `~/.cache/whisper/` へダウンロード（プログレスバー付き）
  - `--dest <dir>`: 保存先ディレクトリを変更
  - `--quiet`: 保存先パスのみ stdout へ出力（Claude による自動利用向け）
  - 既存ファイルはスキップ、ネットワークエラー・中断時は部分ファイルを自動削除

### 変更

#### summarize-video スキル
- `SKILL.md` Step 1 に Whisper モデル確認フローを追加（未指定時はモデル候補を提示しダウンロードまで誘導）
- `references/setup.md` セクション 2 をスクリプト経由の手順に更新（wget コマンド列挙から変更）
- `SKILL.md` の YAML frontmatter を修正

### その他
- `sample_data/` と `tasks/` を `.gitignore` のローカル専用エントリに移動

---

## [0.1.0] - 2026-03-03

### 追加

#### コア分割機能
- FFmpeg `silencedetect` フィルターによる無音区間での動画分割
- `--interval` / `-i`: 目標分割間隔（デフォルト: 600 秒）
- `--silence-threshold` / `-t`: 無音判定の dB 閾値（デフォルト: -30 dB）
- `--silence-duration` / `-d`: 無音と見なす最小継続時間（デフォルト: 0.5 秒）
- `--output-dir` / `-o`: 分割セグメントの出力先ディレクトリ
- アキュレートシークモード（`-ss` を `-i` の後に配置）と `-avoid_negative_ts make_zero` の組み合わせ

#### 文字起こし機能（whisper-rs 組み込み）
- [whisper-rs](https://github.com/tazz4843/whisper-rs) 0.15 による組み込み音声認識（外部バイナリ不要）
- `--model` / `-m`: GGML モデルファイルのパス
- `--language` / `-l`: Whisper に渡す言語コード（デフォルト: `ja`）
- セグメントごとの SRT ファイルを動画ファイルと同じ場所に保存
- `--no-transcribe`: 文字起こしをスキップして動画のみを出力

#### シーン変化検出
- `--split-on-scene`: `showinfo` フィルターで検出したシーン変化点でも分割
- `--scene-threshold`: シーン変化の感度（0.0〜1.0、デフォルト: 0.4）

#### トランスクリプト検索
- `--search <クエリ>`: 出力ディレクトリ内の全 SRT ファイルを横断全文検索
- セグメントパス・タイムスタンプ・マッチテキストを JSON で出力

#### フレーム抽出
- `--extract-frames`: セグメントごとにキーフレームを JPEG で1枚抽出
- フレームは各セグメントフォルダ内の `frames/` サブディレクトリに保存

#### マニフェスト
- 分割完了後に `manifest.json` を自動生成
- 各セグメントのパス・尺・トランスクリプトパス・フレームパスを記録
- `--no-manifest`: マニフェスト生成を抑制

#### MCP サーバー
- `mcp/server.py`: `process_video`・`list_segments`・`get_segment`・`search_transcript` ツールを提供する Python MCP サーバー
- Claude Desktop およびあらゆる MCP 対応クライアントと互換

### 技術的な注意事項

- FFmpeg が `PATH` に存在する必要があります
- Whisper モデルファイル（GGML 形式）は別途ダウンロードが必要です — README を参照
- ソースからビルドする場合は `whisper-rs-sys` のために LLVM/libclang が必要です
  - Windows: `winget install LLVM.LLVM` 後に `LIBCLANG_PATH` を設定
  - Linux: `apt install clang libclang-dev cmake`
  - macOS: `brew install llvm cmake`
