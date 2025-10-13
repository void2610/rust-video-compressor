# video-compressor

動画ファイルを圧縮して容量を削減するRust製CLIツール

## 機能

- FFmpegを使用した高品質な動画圧縮
- デフォルトで入力ファイルを上書き（安全な一時ファイル処理）
- 圧縮後のファイルを自動的にmacOSクリップボードにコピー
- ファイルサイズの比較表示（元のサイズ、圧縮後、削減率）
- カスタマイズ可能な品質設定（CRF値）
- 解像度の変更オプション
- 複数のコーデックとプリセットに対応

## インストール

```bash
cargo install --path .
```

## 前提条件

- FFmpegがインストールされている必要があります
  ```bash
  brew install ffmpeg
  ```

## 使い方

### 基本的な使用方法

```bash
# デフォルト：入力ファイルを上書き
video-compressor video.mp4

# 別のファイルに出力
video-compressor video.mp4 -o compressed.mp4
```

### オプション

- `-o, --output <PATH>`: 出力ファイルパス（省略時は入力ファイルを上書き）
- `-q, --quality <CRF>`: 品質設定（0-51、デフォルトは23、小さいほど高品質）
- `-p, --preset <PRESET>`: エンコーディングプリセット（デフォルトはmedium）
  - `ultrafast`, `superfast`, `veryfast`, `faster`, `fast`, `medium`, `slow`, `slower`, `veryslow`
- `-c, --codec <CODEC>`: ビデオコーデック（デフォルトはlibx264）
  - `libx264`, `libx265`
- `-a, --audio-bitrate <BITRATE>`: オーディオビットレート（デフォルトは128k）
- `-w, --width <WIDTH>`: 解像度の幅（アスペクト比は維持される）

### 使用例

```bash
# 高品質で圧縮
video-compressor video.mp4 -q 18

# 解像度を1280pxに変更
video-compressor video.mp4 -w 1280

# H.265コーデックを使用
video-compressor video.mp4 -c libx265

# より速い圧縮（品質は低下）
video-compressor video.mp4 -p fast -q 28
```

## Automatorとの統合

macOSのAutomatorから実行する場合、以下のようなシェルスクリプトを使用します：

```bash
# PATHを設定（重要）
export PATH="/opt/homebrew/bin:/usr/local/bin:$HOME/.cargo/bin:$PATH"

# ファイルのディレクトリに移動
dir=$(dirname "$1")
cd $dir

# 圧縮を実行
video-compressor "$1"
```

## 技術仕様

### 処理フロー

1. FFmpegのインストール確認
2. 出力ファイルパスの決定（デフォルトは入力と同じ）
3. 一時ファイル（`{filename}_tmp.{ext}`）に圧縮
4. 圧縮成功後、元のファイルを削除（入力=出力の場合）
5. 一時ファイルを最終出力パスにリネーム
6. ファイルサイズの比較表示
7. Swiftを使用してクリップボードにファイルをコピー

### クリップボード機能

macOSのCocoaフレームワーク（NSPasteboard）を使用して、動画ファイル自体をクリップボードにコピーします。これにより、メッセージアプリやSlackなどに直接貼り付けることができます。

## 開発

### ビルド

```bash
cargo build --release
```

### テスト

```bash
cargo test
```

## 依存関係

- `clap`: コマンドライン引数のパース
- `anyhow`: エラーハンドリング
- FFmpeg（外部依存）
- Swift（クリップボード機能、macOS標準）

## ライセンス

MIT
