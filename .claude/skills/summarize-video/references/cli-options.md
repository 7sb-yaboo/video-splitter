# CLI オプション一覧

## 動画処理モード

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

## 検索モード

```
video-splitter <MANIFEST_JSON> --search <QUERY>
```

`manifest.json` のパスを INPUT に指定し、`--search` でキーワードを渡す。
結果は JSON 形式で stdout に出力される。

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

## manifest.json の構造

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
