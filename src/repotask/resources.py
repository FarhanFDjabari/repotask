"""Access bundled files in source and frozen builds."""

from __future__ import annotations

from importlib.resources import files
from pathlib import Path


def bundled_path(*parts: str) -> Path:
    return Path(str(files("repotask.bundled").joinpath(*parts)))


def read_bundled(*parts: str) -> str:
    return files("repotask.bundled").joinpath(*parts).read_text(encoding="utf-8")

