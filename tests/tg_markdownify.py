#!/usr/bin/env python3
"""Convert a markdown file to Telegram-friendly blocks using telegramify-markdown (async)."""

import argparse
import asyncio
import sys
from pathlib import Path

import telegramify_markdown
from telegramify_markdown.interpreters import InterpreterChain, TextInterpreter
from telegramify_markdown.type import ContentTypes


def main() -> int:
    parser = argparse.ArgumentParser(description="Async Telegram markdown converter.")
    parser.add_argument("input_file", help="Path to the source markdown/text file.")
    parser.add_argument("output_file", help="Path to write the converted output.")
    args = parser.parse_args()

    md = Path(args.input_file).read_text(encoding="utf-8")

    interpreter_chain = InterpreterChain([TextInterpreter()])

    async def run():
        return await telegramify_markdown.telegramify(
            content=md,
            interpreters_use=interpreter_chain,
            latex_escape=True,
            normalize_whitespace=True,
            max_word_count=999999,
        )

    boxes = asyncio.run(run())

    blocks = [
        item.content
        for item in boxes
        if item.content_type == ContentTypes.TEXT and item.content is not None
    ]

    output = "\n=========\n".join(blocks)
    Path(args.output_file).parent.mkdir(parents=True, exist_ok=True)
    Path(args.output_file).write_text(output, encoding="utf-8")
    return 0


if __name__ == "__main__":
    sys.exit(main())
