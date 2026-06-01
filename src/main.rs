use clap::Parser;
use std::path::{Path, PathBuf};
use std::process::Command;
use anyhow::{Context, Result};

/// 動画ファイルの容量を削減するCLIツール
#[derive(Parser, Debug)]
#[command(name = "video-compressor")]
#[command(about = "動画ファイルを圧縮して容量を削減します", long_about = None)]
struct Args {
    /// 入力動画ファイルのパス
    input: PathBuf,

    /// 出力動画ファイルのパス（省略時は入力ファイル名_compressed）
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// 品質設定（CRF値: 0-51、小さいほど高品質、デフォルトは23）
    #[arg(short, long, default_value = "23")]
    quality: u8,

    /// エンコーディングプリセット（ultrafast, superfast, veryfast, faster, fast, medium, slow, slower, veryslow）
    #[arg(short, long, default_value = "medium")]
    preset: String,

    /// ビデオコーデック（libx264 または libx265）
    #[arg(short = 'c', long, default_value = "libx264")]
    codec: String,

    /// オーディオビットレート（例: 128k）
    #[arg(short = 'a', long, default_value = "128k")]
    audio_bitrate: String,

    /// 解像度の幅を指定（例: 1280）アスペクト比は維持されます
    #[arg(short = 'w', long)]
    width: Option<u32>,

    /// 目標ファイルサイズ（MB単位、例: 10）
    #[arg(short = 't', long)]
    target_size: Option<f64>,
}

fn main() -> Result<()> {
    let args = Args::parse();

    // FFmpegがインストールされているか確認
    check_ffmpeg_installed()?;

    // 出力ファイルパスを決定（デフォルトは{filename}_compressed.{ext}）
    let final_output_path = args.output.unwrap_or_else(|| {
        let parent = args.input.parent().unwrap_or(std::path::Path::new("."));
        let file_stem = args.input.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("output");
        let extension = args.input.extension()
            .and_then(|e| e.to_str())
            .unwrap_or("mp4");
        parent.join(format!("{}_compressed.{}", file_stem, extension))
    });

    // 一時ファイルパスを生成（拡張子を維持して、ファイル名に_tmpを追加）
    let temp_output_path = {
        let parent = final_output_path.parent().unwrap_or(std::path::Path::new("."));
        let file_stem = final_output_path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("output");
        let extension = final_output_path.extension()
            .and_then(|e| e.to_str())
            .unwrap_or("mp4");
        parent.join(format!("{}_tmp.{}", file_stem, extension))
    };

    println!("入力ファイル: {}", args.input.display());
    println!("出力ファイル: {}", final_output_path.display());

    // 目標サイズモードか通常モードかで処理を分岐
    let status = if let Some(target_size_mb) = args.target_size {
        println!("目標サイズ: {} MB", target_size_mb);
        println!("コーデック: {}", args.codec);
        println!("\n圧縮を開始します...\n");

        // 動画の長さを取得
        let duration = get_video_duration(&args.input)?;
        println!("動画の長さ: {:.1}秒", duration);

        // 音声ビットレートをパース（例: "128k" -> 128.0）
        let audio_bitrate_kbps: f64 = args.audio_bitrate
            .trim_end_matches('k')
            .trim_end_matches('K')
            .parse()
            .unwrap_or(128.0);

        // 再試行ループ（最大3回）
        let mut current_target = target_size_mb;
        let mut attempt = 1;
        let max_attempts = 3;

        loop {
            // ビットレートを計算
            let video_bitrate = calculate_video_bitrate(current_target, duration, audio_bitrate_kbps);
            println!("\n試行 {}/{}: 目標{}MB, ビデオビットレート={}kbps",
                    attempt, max_attempts, current_target, video_bitrate);

            // FFmpegコマンドを構築（ビットレートモード）
            let mut cmd = Command::new("ffmpeg");
            cmd.arg("-i")
                .arg(&args.input)
                .arg("-c:v")
                .arg(&args.codec)
                .arg("-b:v")
                .arg(format!("{}k", video_bitrate))
                .arg("-maxrate")
                .arg(format!("{}k", video_bitrate))
                .arg("-bufsize")
                .arg(format!("{}k", video_bitrate * 2))
                .arg("-preset")
                .arg(&args.preset)
                .arg("-c:a")
                .arg("aac")
                .arg("-b:a")
                .arg(&args.audio_bitrate);

            // 解像度の指定がある場合
            if let Some(width) = args.width {
                cmd.arg("-vf")
                    .arg(format!("scale={}:-2", width));
            }

            cmd.arg("-y")
                .arg(&temp_output_path);

            // FFmpegを実行
            let status = cmd
                .status()
                .context("FFmpegの実行に失敗しました")?;

            if !status.success() {
                break status;
            }

            // 出力ファイルのサイズをチェック
            let output_size_mb = std::fs::metadata(&temp_output_path)
                .context("出力ファイルの情報取得に失敗しました")?
                .len() as f64 / 1024.0 / 1024.0;

            println!("結果: {:.2} MB", output_size_mb);

            // 目標サイズ以下なら成功
            if output_size_mb <= target_size_mb {
                println!("✓ 目標サイズ内に収まりました！");
                break status;
            }

            // 最大試行回数に達したら終了
            if attempt >= max_attempts {
                println!("\n⚠ 警告: {}回試行しましたが、目標サイズ({} MB)を超えています({:.2} MB)",
                        max_attempts, target_size_mb, output_size_mb);
                break status;
            }

            // 目標を90%に調整して再試行
            current_target *= 0.9;
            attempt += 1;
            println!("目標サイズを超えたため、再試行します...");

            // 一時ファイルを削除
            let _ = std::fs::remove_file(&temp_output_path);
        }
    } else {
        println!("品質設定: CRF {}", args.quality);
        println!("コーデック: {}", args.codec);
        println!("\n圧縮を開始します...\n");

        // 通常モード（CRFベース）
        let mut cmd = Command::new("ffmpeg");
        cmd.arg("-i")
            .arg(&args.input)
            .arg("-c:v")
            .arg(&args.codec)
            .arg("-crf")
            .arg(args.quality.to_string())
            .arg("-preset")
            .arg(&args.preset)
            .arg("-c:a")
            .arg("aac")
            .arg("-b:a")
            .arg(&args.audio_bitrate);

        // 解像度の指定がある場合
        if let Some(width) = args.width {
            cmd.arg("-vf")
                .arg(format!("scale={}:-2", width));
        }

        cmd.arg("-y")
            .arg(&temp_output_path);

        // FFmpegを実行
        cmd.status()
            .context("FFmpegの実行に失敗しました")?
    };

    if status.success() {
        // 元のファイルサイズを取得
        let input_size_bytes = std::fs::metadata(&args.input)
            .ok()
            .map(|m| m.len());

        // 一時ファイルを最終出力パスにリネーム
        std::fs::rename(&temp_output_path, &final_output_path)
            .context("ファイルのリネームに失敗しました")?;

        println!("\n✓ 圧縮が完了しました！");
        println!("出力ファイル: {}", final_output_path.display());

        // ファイルサイズの比較
        if let (Some(input_size), Ok(output_meta)) = (
            input_size_bytes,
            std::fs::metadata(&final_output_path)
        ) {
            let input_size_mb = input_size as f64 / 1024.0 / 1024.0;
            let output_size_mb = output_meta.len() as f64 / 1024.0 / 1024.0;
            let reduction = ((input_size_mb - output_size_mb) / input_size_mb) * 100.0;

            println!("\n元のサイズ: {:.2} MB", input_size_mb);
            println!("圧縮後: {:.2} MB", output_size_mb);
            println!("削減率: {:.1}%", reduction);
        }

        match copy_file_to_clipboard(&final_output_path) {
            Ok(()) => {
                println!("\n📋 動画ファイルをクリップボードにコピーしました");
            }
            Err(e) => {
                eprintln!("\n警告: クリップボードへのコピーに失敗しました");
                eprintln!("エラー: {}", e);
            }
        }

        Ok(())
    } else {
        // FFmpegが失敗した場合、一時ファイルをクリーンアップ
        let _ = std::fs::remove_file(&temp_output_path);
        anyhow::bail!("FFmpegによる圧縮に失敗しました")
    }
}

/// FFmpegがインストールされているか確認
fn check_ffmpeg_installed() -> Result<()> {
    Command::new("ffmpeg")
        .arg("-version")
        .output()
        .context("FFmpegが見つかりません。FFmpegをインストールしてください。\n\nインストール方法:\n  macOS: brew install ffmpeg\n  Ubuntu/Debian: sudo apt install ffmpeg\n  Windows: https://ffmpeg.org/download.html")?;
    Ok(())
}

/// ffprobeを使って動画の長さ（秒）を取得
fn get_video_duration(path: &PathBuf) -> Result<f64> {
    let output = Command::new("ffprobe")
        .arg("-v")
        .arg("error")
        .arg("-show_entries")
        .arg("format=duration")
        .arg("-of")
        .arg("default=noprint_wrappers=1:nokey=1")
        .arg(path)
        .output()
        .context("ffprobeの実行に失敗しました")?;

    let duration_str = String::from_utf8_lossy(&output.stdout);
    let duration = duration_str
        .trim()
        .parse::<f64>()
        .context("動画の長さの取得に失敗しました")?;

    Ok(duration)
}

/// 目標ファイルサイズからビデオビットレート（kbps）を計算
fn calculate_video_bitrate(target_size_mb: f64, duration_sec: f64, audio_bitrate_kbps: f64) -> u32 {
    // 目標サイズをビットに変換
    let target_size_bits = target_size_mb * 8.0 * 1024.0 * 1024.0;

    // 音声分のビットを計算
    let audio_bits = audio_bitrate_kbps * 1000.0 * duration_sec;

    // ビデオに使えるビット数
    let video_bits = target_size_bits - audio_bits;

    // ビデオビットレート（kbps）を計算（10%のマージンを持たせる）
    let video_bitrate_kbps = (video_bits / duration_sec / 1000.0) * 0.9;

    video_bitrate_kbps.max(100.0) as u32
}

fn copy_file_to_clipboard(path: &Path) -> Result<()> {
    let absolute_path = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let swift_code = r#"
import AppKit
import Foundation

let filePath = CommandLine.arguments[1]
let fileURL = URL(fileURLWithPath: filePath)
let pasteboard = NSPasteboard.general

pasteboard.clearContents()

guard pasteboard.writeObjects([fileURL as NSURL]) else {
    fputs("NSPasteboard.writeObjects returned false\n", stderr)
    exit(1)
}
"#;

    let output = Command::new("swift")
        .arg("-")
        .arg(absolute_path)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            if let Some(mut stdin) = child.stdin.take() {
                stdin.write_all(swift_code.as_bytes())?;
            }
            child.wait_with_output()
        })
        .context("Swiftによるクリップボード操作の起動に失敗しました")?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("{}", stderr.trim());
    }
}
