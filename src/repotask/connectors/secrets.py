"""Credential lookup for REST connectors.

Credentials never live in the project: they come from the environment first, then
`~/.repo-task/secrets.yaml`, which is outside version control by construction.
"""

from __future__ import annotations

import os
from functools import lru_cache
from pathlib import Path

import yaml

from repotask.errors import RepoTaskError

SECRETS_ENV = "REPOTASK_SECRETS_FILE"


def secrets_path() -> Path:
    override = os.environ.get(SECRETS_ENV)
    return Path(override).expanduser() if override else Path.home() / ".repo-task/secrets.yaml"


@lru_cache(maxsize=1)
def _load() -> dict[str, dict[str, str]]:
    path = secrets_path()
    if not path.is_file():
        return {}
    raw = yaml.safe_load(path.read_text(encoding="utf-8")) or {}
    if not isinstance(raw, dict):
        raise RepoTaskError(f"{path} must contain a YAML mapping of system -> credentials.")
    return {
        str(system): {str(key): str(value) for key, value in values.items()}
        for system, values in raw.items()
        if isinstance(values, dict)
    }


def get(system: str, key: str) -> str:
    """Look up `key` for `system`; environment wins over the secrets file."""
    env_name = f"REPOTASK_{system.upper().replace('-', '_')}_{key.upper()}"
    return os.environ.get(env_name) or _load().get(system, {}).get(key, "")


def require(system: str, key: str) -> str:
    value = get(system, key)
    if not value:
        env_name = f"REPOTASK_{system.upper().replace('-', '_')}_{key.upper()}"
        raise RepoTaskError(
            f"Missing credential '{key}' for '{system}'. Set {env_name} or add it under "
            f"'{system}:' in {secrets_path()}."
        )
    return value


def reset_cache() -> None:
    _load.cache_clear()
