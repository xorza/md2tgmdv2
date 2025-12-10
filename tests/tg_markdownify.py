#!/usr/bin/env python3
"""
Convert a text/markdown file into Telegram-friendly Markdown using
the telegramify-markdown package.

Usage:
  python tg_markdownify.py input.txt output.txt
  python tg_markdownify.py README.md out.md --parse-mode MarkdownV2
"""

import argparse
import inspect
import sys
from pathlib import Path


def get_converter():
    """Locate a usable conversion function from telegramify_markdown."""
    try:
        import telegramify_markdown as tmd  # type: ignore
    except ImportError:
        sys.exit(
            "telegramify-markdown is not installed. "
            "Install it with 'pip install telegramify-markdown'."
        )

    candidates = [
        getattr(tmd, "markdownify", None),
        getattr(tmd, "telegramify", None),
        getattr(tmd, "telegramify_markdown", None),
        getattr(tmd, "escape_markdown", None),
    ]
    for fn in candidates:
        if callable(fn):
            return fn

    sys.exit("No conversion function found inside telegramify_markdown.")


def convert_text(text: str, parse_mode: str) -> str:
    """Call the converter with compatible keyword arguments if available."""
    fn = get_converter()
    params = inspect.signature(fn).parameters
    kwargs = {}
    if "parse_mode" in params:
        kwargs["parse_mode"] = parse_mode
    return fn(text, **kwargs)


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Transform an input file into Telegram-friendly Markdown."
    )
    parser.add_argument("input_file", help="Path to the source text/markdown file.")
    parser.add_argument("output_file", help="Path where converted text will be saved.")
    parser.add_argument(
        "--parse-mode",
        default="MarkdownV2",
        help="Parse mode passed to telegramify-markdown when supported.",
    )
    args = parser.parse_args()

    input_path = Path(args.input_file)
    output_path = Path(args.output_file)

    text = input_path.read_text(encoding="utf-8")
    converted = convert_text(text, args.parse_mode)

    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(converted, encoding="utf-8")


if __name__ == "__main__":
    main()
