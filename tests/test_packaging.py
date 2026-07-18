from __future__ import annotations

from pathlib import Path

import tomllib

import repotask

PYPROJECT = Path(__file__).resolve().parents[1] / "pyproject.toml"


def test_package_version_matches_pyproject() -> None:
    metadata = tomllib.loads(PYPROJECT.read_text(encoding="utf-8"))

    assert repotask.__version__ == metadata["project"]["version"]
