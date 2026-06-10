"""Compose rules in configured order and run stack profile detectors."""

from __future__ import annotations

import fnmatch
from pathlib import Path

from repotask.config.models import RepoTaskConfig
from repotask.files import read_optional


def compose_rules(config: RepoTaskConfig) -> list[tuple[str, str]]:
    result = []
    for relative in config.rules.files:
        content = read_optional(config.root / relative)
        if content is not None:
            result.append((relative, content))
    return result


def _match(path: str, pattern: str) -> bool:
    return fnmatch.fnmatch(path, pattern) or fnmatch.fnmatch(Path(path).name, pattern)


def detector_callouts(config: RepoTaskConfig, changed_paths: list[str]) -> list[str]:
    callouts: list[str] = []
    if "android" in config.project.stacks:
        room_paths = [
            path
            for path in changed_paths
            if _match(path, "**/schemas/*.json") or _match(path, "*Database.kt")
        ]
        if room_paths:
            files = "\n".join(f"- `{path}`" for path in room_paths)
            callouts.append(
                f"""# Room Schema Change Detected

Changed files:

{files}

Verify these Android/Room profile rules and treat violations as **Must fix**:

- Existing committed schema files are immutable; only a new schema should be added.
- The database version is bumped before schema generation.
- A matching migration is registered and data preservation is verified."""
            )
    return callouts

