"""Configuration serialization."""

from __future__ import annotations

from typing import Any

import yaml


def dump_yaml(data: dict[str, Any]) -> str:
    return yaml.safe_dump(data, sort_keys=False, allow_unicode=False, width=100)

