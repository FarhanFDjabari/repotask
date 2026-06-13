"""Access bundled files in source and frozen builds."""

from __future__ import annotations

from importlib.resources import files
from importlib.resources.abc import Traversable
from pathlib import Path


def _bundled_resource(*parts: str) -> Traversable:
    resource = files("repotask.bundled")
    for part in parts:
        resource = resource.joinpath(part)
    return resource


def bundled_path(*parts: str) -> Path:
    return Path(str(_bundled_resource(*parts)))


def read_bundled(*parts: str) -> str:
    return _bundled_resource(*parts).read_text(encoding="utf-8")
