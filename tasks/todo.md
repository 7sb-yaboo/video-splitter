# 動画分割 CLI ツール 実装タスク

## チェックリスト

- [x] `tasks/todo.md` 作成
- [x] `Cargo.toml` 作成
- [x] `src/main.rs` 実装（CLI引数定義・全体フロー）
- [x] `src/silence.rs` 実装（無音検出・分割ポイント選択）
- [x] `src/split.rs` 実装（動画尺取得・セグメント構築・FFmpegカット）
- [x] `cargo build` でコンパイルエラーなし確認
- [x] `cargo run -- --help` で使い方表示確認
- [x] ユニットテスト全6件パス確認
- [x] FFmpeg 未インストール時のエラーメッセージ確認

## レビューセクション

### 実装完了 (2026-03-03)

- `cargo build` 成功（クレート取得・コンパイル正常）
- `cargo run -- --help` で全オプションが日本語で表示される
- ユニットテスト 6 件すべて pass
- FFmpeg 未インストール時: インストール案内 URL 付きエラーが表示される
- 入力ファイル不存在時: 日本語エラーが表示される

### 修正対応
- clap の `env` feature を `Cargo.toml` に追加（`derive` のみでは `#[arg(env=...)]` 不可）

---

## 文字起こし機能追加チェックリスト (2026-03-03)

- [x] `src/transcribe.rs` 新規作成（whisper.cpp 呼び出し専用モジュール）
- [x] `src/main.rs` に新 CLI オプション追加（--transcribe / --whisper / --whisper-model / --language / --transcribe-format）
- [x] バリデーション実装（--transcribe && --whisper-model なし → エラー）
- [x] `cargo build` でコンパイルエラーなし確認
- [x] `cargo run -- --help` で新オプションが表示されること確認
- [x] `--transcribe` のみ指定（`--whisper-model` なし）でエラーメッセージ確認
- [x] 存在しない whisper パスを `--whisper` に指定した場合のエラー確認
- [x] 既存ユニットテスト 6 件すべて引き続き pass 確認

## レビューセクション（文字起こし機能）

- `src/transcribe.rs`: validate_whisper / transcribe_segment を実装
  - ffmpeg で 16kHz モノラル PCM WAV に変換 → whisper.cpp で文字起こし → 一時 WAV 削除
  - 出力形式フラグ: txt=`-otxt` / srt=`-osrt` / vtt=`-ovtt`
  - `-of` に拡張子なしパスを渡す（whisper.cpp の仕様に準拠）
- `src/main.rs`: 既存フローを維持しつつ、cut_segment 直後に transcribe_segment を呼び出す構造
