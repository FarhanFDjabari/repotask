"""Load RepoTask configuration from the current Git repository."""

from __future__ import annotations

from pathlib import Path

import yaml
from yaml.error import MarkedYAMLError

from repotask.config.models import RepoTaskConfig
from repotask.errors import RepoTaskError
from repotask.git import resolve_git_root

CONFIG_NAME = ".repo-task.yml"


def load_config(start: Path | None = None) -> RepoTaskConfig:
    root = resolve_git_root(start)
    path = root / CONFIG_NAME
    if not path.exists():
        raise RepoTaskError(f"{CONFIG_NAME} not found at Git root {root}. Run repo-task init.")
    try:
        raw = yaml.safe_load(path.read_text(encoding="utf-8"))
    except MarkedYAMLError as error:
        mark = getattr(error, "problem_mark", None)
        location = f" at line {mark.line + 1}, column {mark.column + 1}" if mark else ""
        raise RepoTaskError(f"Could not parse {path}{location}: {error.problem}") from error
    except OSError as error:
        raise RepoTaskError(f"Could not read {path}: {error}") from error
    if not isinstance(raw, dict):
        raise RepoTaskError(f"{path} must contain a YAML mapping.")
    return RepoTaskConfig.from_dict(raw, root)

