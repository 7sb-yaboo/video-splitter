#!/usr/bin/env python3
"""Whisper GGML モデルの一覧表示・ダウンロードスクリプト。

使い方:
    python scripts/download_model.py --list
    python scripts/download_model.py --model medium
    python scripts/download_model.py --model medium --dest /path/to/dir
    python scripts/download_model.py --model medium --quiet
"""

import argparse
import sys
import urllib.error
import urllib.request
from pathlib import Path

# Windows での UTF-8 出力を保証する
if sys.stdout.encoding and sys.stdout.encoding.lower() not in ("utf-8", "utf8"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
if sys.stderr.encoding and sys.stderr.encoding.lower() not in ("utf-8", "utf8"):
    sys.stderr.reconfigure(encoding="utf-8", errors="replace")

BASE_URL = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/"

MODELS = {
    "tiny": {
        "filename": "ggml-tiny.bin",
        "size": "~75MB",
        "desc": "最高速・低精度",
    },
    "base": {
        "filename": "ggml-base.bin",
        "size": "~148MB",
        "desc": "高速・普通精度",
    },
    "small": {
        "filename": "ggml-small.bin",
        "size": "~488MB",
        "desc": "バランス型",
    },
    "medium": {
        "filename": "ggml-medium.bin",
        "size": "~1.5GB",
        "desc": "高精度（日本語に最適）",
        "recommended": True,
    },
    "large-v3": {
        "filename": "ggml-large-v3.bin",
        "size": "~3.1GB",
        "desc": "最高精度・低速",
    },
}

DEFAULT_DEST = Path.home() / ".cache" / "whisper"


def list_models() -> None:
    """モデル一覧を表形式で表示する。"""
    print(f"{'モデル':<12} {'ファイル名':<22} {'サイズ':<8} 説明")
    print("-" * 70)
    for name, info in MODELS.items():
        tag = " （推奨）" if info.get("recommended") else ""
        print(f"{name:<12} {info['filename']:<22} {info['size']:<8} {info['desc']}{tag}")


def _progress_hook(block_num: int, block_size: int, total_size: int) -> None:
    """ダウンロード進捗をターミナルに表示する。"""
    if total_size <= 0:
        downloaded = block_num * block_size
        print(f"\r  {downloaded // 1024 // 1024} MB ダウンロード済み...", end="", flush=True)
        return
    downloaded = min(block_num * block_size, total_size)
    percent = downloaded / total_size * 100
    bar_len = 40
    filled = int(bar_len * downloaded / total_size)
    bar = "=" * filled + "-" * (bar_len - filled)
    mb_done = downloaded // 1024 // 1024
    mb_total = total_size // 1024 // 1024
    print(f"\r  [{bar}] {percent:.1f}%  {mb_done}/{mb_total} MB", end="", flush=True)


def download_model(model: str, dest: Path, quiet: bool) -> Path:
    """モデルをダウンロードして保存先の絶対パスを返す。

    既にファイルが存在する場合はダウンロードをスキップする。
    """
    if model not in MODELS:
        print(f"エラー: 不明なモデル '{model}'。--list で確認してください。", file=sys.stderr)
        sys.exit(1)

    info = MODELS[model]
    dest.mkdir(parents=True, exist_ok=True)
    dest_file = dest / info["filename"]

    if dest_file.exists():
        if not quiet:
            print(f"スキップ: {dest_file} は既に存在します。")
        print(str(dest_file))
        return dest_file

    url = BASE_URL + info["filename"]
    if not quiet:
        print(f"ダウンロード: {url}")
        print(f"保存先: {dest_file}  ({info['size']})")

    try:
        if quiet:
            urllib.request.urlretrieve(url, dest_file)
        else:
            urllib.request.urlretrieve(url, dest_file, reporthook=_progress_hook)
            print()  # 改行
    except (urllib.error.URLError, KeyboardInterrupt) as exc:
        # 中途半端なファイルを削除
        if dest_file.exists():
            dest_file.unlink()
        if isinstance(exc, KeyboardInterrupt):
            print("\n中断されました。", file=sys.stderr)
        else:
            print(f"\nエラー: ダウンロードに失敗しました: {exc}", file=sys.stderr)
        sys.exit(1)

    if not quiet:
        print(f"完了: {dest_file}")
    print(str(dest_file))
    return dest_file


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Whisper GGML モデルの一覧表示・ダウンロード"
    )
    parser.add_argument("--list", action="store_true", help="利用可能なモデル一覧を表示して終了")
    parser.add_argument("--model", metavar="NAME", help="ダウンロードするモデル名")
    parser.add_argument(
        "--dest",
        metavar="DIR",
        type=Path,
        default=DEFAULT_DEST,
        help=f"保存先ディレクトリ (デフォルト: {DEFAULT_DEST})",
    )
    parser.add_argument(
        "--quiet",
        action="store_true",
        help="保存先パスのみを stdout に出力する（Claude が利用する）",
    )
    args = parser.parse_args()

    if args.list:
        list_models()
        return

    if not args.model:
        parser.print_help()
        sys.exit(1)

    download_model(args.model, args.dest, args.quiet)


if __name__ == "__main__":
    main()
