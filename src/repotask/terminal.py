"""Minimal terminal helpers without third-party dependencies."""

from __future__ import annotations

import os
import sys
from collections.abc import Iterable

COLORS = {
    "red": "31",
    "green": "32",
    "yellow": "33",
    "bold": "1",
}


def color(text: str, style: str) -> str:
    if not _supports_color() or style not in COLORS:
        return text
    return f"\033[{COLORS[style]}m{text}\033[0m"


def print_error(message: str) -> None:
    print(f"{color('Error:', 'red')} {message}", file=sys.stderr)


def print_table(title: str, headers: Iterable[str], rows: Iterable[Iterable[str]]) -> None:
    headers = [str(header) for header in headers]
    rows = [[str(cell) for cell in row] for row in rows]
    widths = [len(header) for header in headers]
    for row in rows:
        for index, cell in enumerate(row):
            widths[index] = max(widths[index], len(cell))
    separator = "+-" + "-+-".join("-" * width for width in widths) + "-+"
    print(title)
    print(separator)
    header_row = " | ".join(header.ljust(widths[index]) for index, header in enumerate(headers))
    print(f"| {header_row} |")
    print(separator)
    for row in rows:
        print("| " + " | ".join(cell.ljust(widths[index]) for index, cell in enumerate(row)) + " |")
    print(separator)


def prompt_text(message: str, default: str = "") -> str:
    suffix = f" [{default}]" if default else ""
    response = input(f"{message}{suffix}: ")
    return response if response else default


def _supports_color() -> bool:
    return sys.stdout.isatty() and os.environ.get("NO_COLOR") is None
