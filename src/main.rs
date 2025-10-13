use clap::Parser;
use std::path::PathBuf;
use std::process::Command;
use anyhow::{Context, Result};

/// 動画ファイルの容量を削減するCLIツール
#[derive(Parser, Debug)]
#[command(name = "video-compressor")]
#[command(about = "動画ファイルを圧縮して容量を削減します", long_about = None)]
struct Args {
    /// 入力動画ファイルのパス
    input: PathBuf,

    /// 出力動画ファイルのパス（省略時は入力ファイルを上書き）
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
}

fn main() -> Result<()> {
    let args = Args::parse();

    // FFmpegがインストールされているか確認
    check_ffmpeg_installed()?;

    // 出力ファイルパスを決定（デフォルトは入力ファイルと同じ）
    let final_output_path = args.output.unwrap_or_else(|| args.input.clone());

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
    println!("品質設定: CRF {}", args.quality);
    println!("コーデック: {}", args.codec);
    println!("\n圧縮を開始します...\n");

    // FFmpegコマンドを構築
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

    cmd.arg("-y") // 出力ファイルを上書き
        .arg(&temp_output_path);

    // FFmpegを実行
    let status = cmd
        .status()
        .context("FFmpegの実行に失敗しました")?;

    if status.success() {
        // 元のファイルサイズを取得（削除前に）
        let input_size_bytes = std::fs::metadata(&args.input)
            .ok()
            .map(|m| m.len());

        // 入力ファイルと出力ファイルが同じ場合、元のファイルを削除
        if args.input == final_output_path {
            std::fs::remove_file(&args.input)
                .context("元のファイルの削除に失敗しました")?;
        }

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

        // クリップボードに動画ファイルをコピー（macOS）
        let absolute_path = std::fs::canonicalize(&final_output_path)
            .unwrap_or(final_output_path.clone());

        // AppleScriptを使ってファイルをクリップボードにコピー
        let script = format!(
            "set the clipboard to (read (POSIX file \"{}\") as «class furl»)",
            absolute_path.display()
        );

        let copy_result = Command::new("osascript")
            .arg("-e")
            .arg(&script)
            .output();

        match copy_result {
            Ok(output) if output.status.success() => {
                println!("\n📋 動画ファイルをクリップボードにコピーしました");
            }
            Ok(output) => {
                eprintln!("\n警告: クリップボードへのコピーに失敗しました");
                if !output.stderr.is_empty() {
                    eprintln!("エラー: {}", String::from_utf8_lossy(&output.stderr));
                }
            }
            Err(e) => {
                eprintln!("\n警告: クリップボードへのコピーに失敗しました: {}", e);
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
