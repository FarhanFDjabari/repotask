"""Load RepoTask configuration from the current Git repository."""

from __future__ import annotations

from pathlib import Path
from typing import Any

import yaml
from pydantic import ValidationError

from repotask.config.models import RepoTaskConfig
from repotask.errors import RepoTaskError
from repotask.git import resolve_git_root

CONFIG_PATH = ".repo-task/config.yaml"
LEGACY_CONFIG_PATH = ".repo-task.yml"


def config_path(root: Path) -> Path:
    return root / CONFIG_PATH


def load_yaml_mapping(path: Path) -> dict[str, Any]:
    try:
        raw = yaml.safe_load(path.read_text(encoding="utf-8"))
    except yaml.YAMLError as error:
        raise RepoTaskError(f"Could not parse {path}: {error}") from error
    except OSError as error:
        raise RepoTaskError(f"Could not read {path}: {error}") from error
    if raw is None:
        return {}
    if not isinstance(raw, dict):
        raise RepoTaskError(f"{path} must contain a YAML mapping.")
    return raw


def dump_yaml(data: dict[str, Any]) -> str:
    return yaml.safe_dump(data, sort_keys=False, default_flow_style=False)


def load_config(start: Path | None = None) -> RepoTaskConfig:
    root = resolve_git_root(start)
    path = config_path(root)
    if not path.exists():
        if (root / LEGACY_CONFIG_PATH).exists():
            raise RepoTaskError(
                f"Found legacy {LEGACY_CONFIG_PATH} but no {CONFIG_PATH}. "
                "Run `repo-task migrate` to upgrade to schema version 2."
            )
        raise RepoTaskError(f"{CONFIG_PATH} not found at Git root {root}. Run `repo-task init`.")
    raw = load_yaml_mapping(path)
    try:
        return RepoTaskConfig(root=root, **raw)
    except ValidationError as error:
        raise RepoTaskError(format_validation_error(path, error)) from error


def format_validation_error(path: Path, error: ValidationError) -> str:
    lines = [f"Invalid configuration in {path}:"]
    for item in error.errors():
        location = ".".join(str(part) for part in item["loc"]) or "<root>"
        lines.append(f"  {location}: {item['msg']}")
    return "\n".join(lines)
