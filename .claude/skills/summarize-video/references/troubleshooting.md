# トラブルシューティング

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
