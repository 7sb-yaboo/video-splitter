# MCP 化ロードマップ

**目的**: video-splitter を MCP サーバーとしてリリースし、AI が動画内の作業内容を理解して作業フローを生成できるようにする。

---

## フェーズ概要

```
Phase 1  キーフレーム抽出         ← CLI 機能追加（視覚情報の取得）
Phase 2  統合JSONマニフェスト      ← CLI 機能追加（MCP連携の基盤）
Phase 3  MCP サーバー化           ← 本体（ツール定義・プロトコル実装）
Phase 4  分割ポイント強化         ← オプション改善（シーン変化も分割点に）
Phase 5  transcript 横断検索      ← オプション機能（大量セグメント対応）
```

---

## Phase 1: キーフレーム抽出

**目的**: AI が映像を「見る」ための代表フレームを静止画として出力する。
作業フロー動画（操作画面・手元作業）は音声より映像の方が情報量が多いため、テキストだけでは作業内容の把握に限界がある。

### タスク

- [x] `src/frames.rs` 新規作成
  - `extract_key_frames(ffmpeg, segment, threshold, interval_sec, verbose) -> Result<Vec<PathBuf>>`
  - シーン変化検出: `ffmpeg -vf "select=gt(scene,{threshold})" -vsync vfr frame_%04d.jpg`
  - フォールバック（変化が少ない動画向け）: 一定間隔サンプリング `fps=1/{interval_sec}`
- [x] `src/main.rs` に CLI オプション追加
  - `--extract-frames` (bool, default: false)
  - `--frames-scene-threshold` (f64, default: 0.3) シーン変化の感度
  - `--frames-interval` (f64, default: 30.0) フォールバック間隔
- [x] 出力先: `{output_dir}/{segment_stem}_frames/frame_0001.jpg, ...`
- [x] `cargo build` + `cargo test` パス確認（11 tests passed）

### 出力イメージ

```
[1/7] Cutting:     lecture_001.mp4  (0.0s - 598.5s)  -> OK
      Transcribing: lecture_001.srt  (47 entries)      -> OK
      Key frames:   lecture_001_frames/ (12 frames)    -> OK
```

---

## Phase 2: 統合 JSON マニフェスト

**目的**: セグメントごとの動画・文字起こし・フレームパスを1つの JSON にまとめ、MCP ツールが1コールで全コンテキストを取得できるようにする。

### タスク

- [x] `src/manifest.rs` 新規作成
  - `Manifest` / `SegmentMeta` 構造体定義（serde Serialize）
  - `write_manifest(path, meta) -> Result<()>`
  - `to_relative(path, base) -> String`（パスを manifest からの相対パスに変換）
- [x] `Cargo.toml` に `serde` / `serde_json` を追加
- [x] `src/main.rs` に CLI オプション追加
  - `--manifest` (bool, default: false) 処理完了後に JSON を出力
  - 出力先: `{output_dir}/manifest.json`
- [x] `cargo build` + `cargo test` パス確認（11 tests passed）

### manifest.json スキーマ

```json
{
  "source": "lecture.mp4",
  "total_duration": 3612.5,
  "language": "ja",
  "created_at": "2026-03-03T12:00:00Z",
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

---

## Phase 3: MCP サーバー化

**目的**: CLI ツールを MCP サーバーとして公開し、AI エージェントから直接呼び出せるようにする。

### 実装方針の選択肢

| 方針 | メリット | デメリット |
|------|---------|-----------|
| **A: Python ラッパー**（推奨） | MCP SDK が充実、実装量が少ない | Rust CLI を別途インストールが必要 |
| B: Rust ネイティブ | 単一バイナリ、高速 | MCP ライブラリが未成熟 |

→ **Phase 3 は Python（`mcp` ライブラリ）で薄いラッパーを実装し、内部で Rust CLI を呼び出す方針を推奨。**

### タスク

- [x] `mcp/` ディレクトリ作成（Python プロジェクト）
  - `mcp/pyproject.toml`
  - `mcp/server.py` — MCP サーバー本体
- [x] MCP ツール実装（4本）
  - `process_video` — 分割・文字起こし・フレーム抽出を一括実行し `manifest.json` パスを返す
  - `list_segments` — 全セグメントの概要一覧を返す
  - `get_segment` — 指定セグメントの動画パス・SRT テキスト・フレームパスを返す
  - `search_transcript` — キーワードで全セグメントを横断検索しタイムコード付きで返す
- [x] `claude_desktop_config.json` への登録例を README に記載
- [x] README.md に MCP セクション追加

### ツール入出力イメージ

```
process_video("lecture.mp4", {transcribe: true, extract_frames: true})
→ { "manifest": "/path/to/manifest.json", "segments": 7 }

get_segment("/path/to/manifest.json", 1)
→ { "video": "...", "transcript": "...", "key_frames": ["..."] }

search_transcript("/path/to/manifest.json", "ファイルを保存")
→ [{ "segment": 2, "timestamp": "00:03:24", "text": "...ファイルを保存して..." }]
```

---

## Phase 4: 分割ポイント強化（オプション）

**目的**: 無音区間に加えてシーン変化（画面切り替わり）も分割点として使えるようにし、作業工程の区切りをより正確に捉える。

### タスク

- [ ] `src/scene.rs` 新規作成
  - `detect_scene_changes(ffmpeg, input, threshold, verbose) -> Result<Vec<f64>>`
  - `ffmpeg -i input.mp4 -vf "select=gt(scene\,{t}),showinfo" -f null -` の stderr をパース
- [ ] `src/main.rs` に CLI オプション追加
  - `--split-on-scene` (bool, default: false)
  - `--scene-threshold` (f64, default: 0.4)
- [ ] 分割ポイントのマージロジック: 無音区間 ∪ シーン変化点（近傍の重複を除去）
- [ ] `cargo test` パス確認

---

## Phase 5: transcript 横断検索（オプション）

**目的**: 大量セグメントに対してキーワード検索し、該当箇所をタイムコード付きで返す。Phase 3 の `search_transcript` ツールの基盤となる。

### タスク

- [ ] `src/search.rs` 新規作成
  - `search(manifest, query) -> Result<Vec<SearchHit>>`
  - `SearchHit { segment_index, start_ms, end_ms, context_text }`
- [ ] CLI オプション追加（`--search <query>`）または MCP ツール専用として実装
- [ ] 検索結果のソート（関連度 / 時系列）

---

## 優先順位まとめ

| フェーズ | 依存関係 | 着手可否 |
|---------|---------|---------|
| Phase 1（フレーム抽出） | なし | ✅ 即着手可能 |
| Phase 2（マニフェスト） | Phase 1 の後が望ましい | ✅ 独立して着手可能 |
| Phase 3（MCP化） | **Phase 1・2 完了後** | Phase 1・2 待ち |
| Phase 4（シーン分割） | Phase 1 と並行可 | ✅ 独立して着手可能 |
| Phase 5（検索） | Phase 2 完了後 | Phase 2 待ち |
