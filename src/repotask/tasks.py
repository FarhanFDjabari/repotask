"""Task workspace paths and metadata."""

from __future__ import annotations

import re
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

from repotask.config.models import RepoTaskConfig
from repotask.errors import RepoTaskError
from repotask.files import read_json


def validate_task_id(task_id: str) -> str:
    value = task_id.strip()
    if not value:
        raise RepoTaskError("TASK_ID must not be empty.")
    if value in {".", ".."} or "/" in value or "\\" in value or "\x00" in value:
        raise RepoTaskError("TASK_ID must be an opaque identifier, not a filesystem path.")
    return value


def task_directory(config: RepoTaskConfig, task_id: str) -> Path:
    return config.root / ".repo-task/tasks" / validate_task_id(task_id)


def require_task_directory(config: RepoTaskConfig, task_id: str) -> Path:
    path = task_directory(config, task_id)
    if not path.is_dir():
        raise RepoTaskError(f"Task workspace not found: {path}. Run repo-task start first.")
    return path


def read_task_config(config: RepoTaskConfig, task_id: str) -> dict[str, Any]:
    task_dir = require_task_directory(config, task_id)
    try:
        return read_json(task_dir / "config.json")
    except ValueError as error:
        raise RepoTaskError(f"Could not parse {task_dir / 'config.json'}: {error}") from error


def slugify(title: str) -> str:
    normalized = re.sub(r"[^a-z0-9]+", "-", title.lower()).strip("-")
    return normalized or "task"


def now_iso() -> str:
    return datetime.now(UTC).replace(microsecond=0).isoformat()

