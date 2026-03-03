mod silence;
mod split;
mod transcribe;

use anyhow::{bail, Result};
use clap::Parser;
use std::path::PathBuf;

/// 動画ファイルを指定時間ごとに分割するツール（無音区間で自然に分割）
#[derive(Parser, Debug)]
#[command(name = "video-splitter", version, about, long_about = None)]
struct Cli {
    /// 入力動画ファイル
    input: PathBuf,

    /// 分割間隔（秒）
    #[arg(short, long, default_value_t = 600.0)]
    duration: f64,

    /// 出力先ディレクトリ（デフォルト: 入力ファイルと同じディレクトリ）
    #[arg(short, long)]
    output_dir: Option<PathBuf>,

    /// 無音判定閾値 (dB)
    #[arg(long, default_value_t = -30.0)]
    noise_threshold: f64,

    /// 無音の最短持続時間（秒）
    #[arg(long, default_value_t = 0.5)]
    silence_duration: f64,

    /// ターゲット時刻の前後探索範囲（秒）
    #[arg(long, default_value_t = 60.0)]
    search_window: f64,

    /// FFmpeg 実行ファイルパス
    #[arg(long, default_value = "ffmpeg", env = "FFMPEG_PATH")]
    ffmpeg: String,

    /// 詳細ログを表示する
    #[arg(short, long)]
    verbose: bool,

    /// 文字起こしを有効にする（--whisper-model が必須）
    #[arg(long)]
    transcribe: bool,

    /// whisper.cpp 実行ファイルパス
    #[arg(long, default_value = "whisper-cpp", env = "WHISPER_PATH")]
    whisper: String,

    /// モデルファイルパス（--transcribe 時は必須）
    #[arg(long)]
    whisper_model: Option<PathBuf>,

    /// 音声言語コード
    #[arg(long, default_value = "ja")]
    language: String,

    /// 文字起こし出力形式（txt / srt / vtt）
    #[arg(long, default_value = "txt")]
    transcribe_format: String,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    run(cli)
}

fn run(cli: Cli) -> Result<()> {
    // 入力ファイルの存在確認
    if !cli.input.exists() {
        bail!("入力ファイルが見つかりません: {}", cli.input.display());
    }

    let input_str = cli
        .input
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("Invalid input path"))?;

    // 出力先ディレクトリの決定
    let output_dir = match cli.output_dir {
        Some(dir) => dir,
        None => cli
            .input
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."))
            .to_path_buf(),
    };

    if !output_dir.exists() {
        std::fs::create_dir_all(&output_dir)
            .map_err(|e| anyhow::anyhow!("出力ディレクトリの作成に失敗しました: {}", e))?;
    }

    // FFmpeg の存在確認
    split::validate_ffmpeg(&cli.ffmpeg)?;

    // 文字起こしのバリデーション
    if cli.transcribe {
        if cli.whisper_model.is_none() {
            bail!("--transcribe を使用する場合は --whisper-model でモデルファイルパスを指定してください");
        }
        transcribe::validate_whisper(&cli.whisper)?;
    }

    // 動画の総時間取得
    println!("Analyzing video: {}", cli.input.display());
    let total_duration = split::get_video_duration(&cli.ffmpeg, input_str)?;
    let total_minutes = total_duration / 60.0;
    println!("  Total duration: {:.1}s ({:.1} min)", total_duration, total_minutes);

    // 分割不要チェック
    if total_duration <= cli.duration {
        println!(
            "動画の長さ（{:.1}s）が分割間隔（{:.1}s）以下のため、分割は不要です。",
            total_duration, cli.duration
        );
        return Ok(());
    }

    // 無音区間の検出
    println!("Detecting silence in: {}", cli.input.display());
    let silence_intervals = silence::detect_silence(
        &cli.ffmpeg,
        input_str,
        cli.noise_threshold,
        cli.silence_duration,
        cli.verbose,
    )?;
    println!("  Found {} silence interval(s)", silence_intervals.len());

    // 分割ポイントの決定
    let mut split_points: Vec<f64> = Vec::new();
    let mut target = cli.duration;

    while target < total_duration {
        let point = silence::find_nearest_split_point(&silence_intervals, target, cli.search_window)
            .unwrap_or(target);
        split_points.push(point);
        target = point + cli.duration;
    }

    println!("Split points ({} total):", split_points.len());
    for (i, &point) in split_points.iter().enumerate() {
        println!("  {:>3}.  {:.3}s ({:.1} min)", i + 1, point, point / 60.0);
    }

    // セグメントの構築
    let segments = split::build_segments(&split_points, total_duration, &cli.input, &output_dir);
    let total_segments = segments.len();
    println!("Splitting into {} segment(s)...", total_segments);

    // 文字起こし: 動画全体を1回だけ whisper で処理し SRT エントリを取得する
    // セグメントごとの個別実行ではなく、タイムスタンプで後からスライスする
    let srt_entries = if cli.transcribe {
        let model = cli.whisper_model.as_deref().unwrap();
        println!("Transcribing full video (1 pass)...");
        let entries =
            transcribe::transcribe_full(&cli.ffmpeg, &cli.whisper, model, input_str, &cli.language, cli.verbose)?;
        println!("  -> OK: {} entries", entries.len());
        Some(entries)
    } else {
        None
    };

    // 各セグメントをカット、文字起こしが有効な場合は SRT をスライスして書き出す
    for segment in &segments {
        println!(
            "[{}/{}] Cutting: {} ({:.1}s - {:.1}s)",
            segment.index,
            total_segments,
            segment.output_path.file_name().unwrap().to_string_lossy(),
            segment.start,
            segment.end,
        );

        split::cut_segment(&cli.ffmpeg, input_str, segment, cli.verbose)?;
        println!("  -> OK: {}", segment.output_path.display());

        if let Some(ref entries) = srt_entries {
            let sliced = transcribe::slice_srt(entries, segment.start, segment.end);
            let transcript_path = segment.output_path.with_extension(&cli.transcribe_format);
            transcribe::write_transcript(&sliced, &transcript_path, &cli.transcribe_format)?;
            println!("  -> OK: {} ({} entries)", transcript_path.display(), sliced.len());
        }
    }

    println!("Done! {} file(s) created in: {}", total_segments, output_dir.display());
    Ok(())
}
