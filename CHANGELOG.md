# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-03-03

### Added

#### Core splitting
- Silence-based video splitting using FFmpeg `silencedetect` filter
- `--interval` / `-i`: target split interval (default: 600 s)
- `--silence-threshold` / `-t`: dB threshold for silence detection (default: -30 dB)
- `--silence-duration` / `-d`: minimum silence duration to consider (default: 0.5 s)
- `--output-dir` / `-o`: output directory for split segments
- Accurate seek mode (`-ss` after `-i`) with `-avoid_negative_ts make_zero`

#### Transcription (whisper-rs, built-in)
- Built-in speech-to-text via [whisper-rs](https://github.com/tazz4843/whisper-rs) 0.15 (no external binary needed)
- `--model` / `-m`: path to GGML model file
- `--language` / `-l`: language code passed to Whisper (default: `ja`)
- Per-segment SRT generation stored alongside each video segment
- `--no-transcribe`: skip transcription and produce video-only segments

#### Scene change detection
- `--split-on-scene`: additionally split at scene changes detected via `showinfo` filter
- `--scene-threshold`: sensitivity for scene change (0.0–1.0, default: 0.4)

#### Transcript search
- `--search <QUERY>`: full-text search across all SRT transcripts in output directory
- JSON output with segment path, timestamp, and matched text

#### Frame extraction
- `--extract-frames`: extract one keyframe per segment as JPEG
- Frames stored in `frames/` sub-directory of each segment folder

#### Manifest
- `manifest.json` generated automatically after splitting
- Lists all segments with path, duration, transcript path, and frame paths
- `--no-manifest`: suppress manifest generation

#### MCP server
- `mcp/server.py`: Python MCP server exposing `process_video`, `list_segments`,
  `get_segment`, and `search_transcript` tools
- Compatible with Claude Desktop and any MCP-capable client

### Technical notes

- Requires FFmpeg in `PATH`
- Whisper model files (GGML format) must be downloaded separately — see README
- Building from source requires LLVM/libclang for `whisper-rs-sys`
  - Windows: `winget install LLVM.LLVM` then set `LIBCLANG_PATH`
  - Linux: `apt install clang libclang-dev cmake`
  - macOS: `brew install llvm cmake`
