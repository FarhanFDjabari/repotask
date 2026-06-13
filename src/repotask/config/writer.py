"""Configuration serialization."""

from __future__ import annotations

from typing import Any

from repotask.config.simple_yaml import dump_yaml as _dump_yaml


def dump_yaml(data: dict[str, Any]) -> str:
    return _dump_yaml(data)
