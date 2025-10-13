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
    #[arg(short, long)]
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
}

fn main() -> Result<()> {
    let args = Args::parse();

    // FFmpegがインストールされているか確認
    check_ffmpeg_installed()?;

    // 出力ファイルパスを決定
    let output_path = match args.output {
        Some(path) => path,
        None => {
            let input_stem = args.input
                .file_stem()
                .context("入力ファイル名の取得に失敗しました")?
                .to_str()
                .context("ファイル名の変換に失敗しました")?;
            let input_ext = args.input
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("mp4");
            let parent = args.input.parent().unwrap_or(std::path::Path::new("."));
            parent.join(format!("{}_compressed.{}", input_stem, input_ext))
        }
    };

    println!("入力ファイル: {}", args.input.display());
    println!("出力ファイル: {}", output_path.display());
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
        .arg(&output_path);

    // FFmpegを実行
    let status = cmd
        .status()
        .context("FFmpegの実行に失敗しました")?;

    if status.success() {
        println!("\n✓ 圧縮が完了しました！");
        println!("出力ファイル: {}", output_path.display());

        // ファイルサイズの比較
        if let (Ok(input_meta), Ok(output_meta)) = (
            std::fs::metadata(&args.input),
            std::fs::metadata(&output_path)
        ) {
            let input_size = input_meta.len() as f64 / 1024.0 / 1024.0;
            let output_size = output_meta.len() as f64 / 1024.0 / 1024.0;
            let reduction = ((input_size - output_size) / input_size) * 100.0;

            println!("\n元のサイズ: {:.2} MB", input_size);
            println!("圧縮後: {:.2} MB", output_size);
            println!("削減率: {:.1}%", reduction);
        }

        Ok(())
    } else {
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
