"""Artifact store for work in progress: `.repo-task/work/<ticket>/`.

The CLI owns these files so each workflow step has a durable input for the next
one, and so a new agent session can pick the work up without re-fetching.
"""

from __future__ import annotations

import json
import re
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

from repotask.config.models import RepoTaskConfig
from repotask.errors import RepoTaskError
from repotask.files import read_optional, write_text

SOURCE = "source.md"
SUMMARY = "summary.md"
ANALYSIS = "analysis.json"
PLAN = "plan.json"
META = "meta.json"


def now() -> str:
    return datetime.now(timezone.utc).isoformat(timespec="seconds")


def safe_id(ticket_id: str) -> str:
    """Ticket ids become directory names, so keep them boring.

    Leading and trailing dots are stripped as well as separators, so nothing that
    reduces to a relative path fragment survives.
    """
    cleaned = re.sub(r"[^A-Za-z0-9._-]+", "-", ticket_id).strip("-.")
    if not cleaned:
        raise RepoTaskError(f"Invalid ticket id: {ticket_id!r}")
    return cleaned


@dataclass(frozen=True)
class WorkItem:
    root: Path
    ticket_id: str

    @property
    def source_path(self) -> Path:
        return self.root / SOURCE

    def read(self, name: str) -> str | None:
        return read_optional(self.root / name)

    def read_json(self, name: str) -> dict[str, Any]:
        text = self.read(name)
        return json.loads(text) if text else {}

    def write(self, name: str, content: str) -> str:
        write_text(self.root / name, content if content.endswith("\n") else content + "\n")
        return str(self.root / name)

    def write_json(self, name: str, data: dict[str, Any]) -> str:
        return self.write(name, json.dumps(data, indent=2))

    def touch_meta(self, **fields: Any) -> None:
        meta = self.read_json(META)
        meta.update(fields)
        meta.setdefault("ticketId", self.ticket_id)
        meta.setdefault("createdAt", now())
        meta["updatedAt"] = now()
        self.write_json(META, meta)

    def require(self, name: str, hint: str) -> str:
        content = self.read(name)
        if content is None:
            raise RepoTaskError(f"{self.root / name} not found. Run `{hint}` first.")
        return content


def work_item(config: RepoTaskConfig, ticket_id: str) -> WorkItem:
    return WorkItem(root=config.work_dir / safe_id(ticket_id), ticket_id=ticket_id)


def list_work(config: RepoTaskConfig) -> list[WorkItem]:
    if not config.work_dir.is_dir():
        return []
    return [
        WorkItem(root=path, ticket_id=path.name)
        for path in sorted(config.work_dir.iterdir())
        if path.is_dir()
    ]
