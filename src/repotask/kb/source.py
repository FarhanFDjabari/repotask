"""Resolve the knowledge base to a directory on disk.

Two sources, one schema:
  * remote git repository  -> cloned into ~/.repo-task/kb/<slug> and pinned to a ref
  * local project directory -> used in place, no syncing

Remote wins when both are configured.
"""

from __future__ import annotations

import hashlib
import os
import re
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path

from repotask.config.models import KnowledgeConfig, RepoTaskConfig
from repotask.errors import RepoTaskError
from repotask.git import run_git

CACHE_ENV = "REPOTASK_CACHE_DIR"


def cache_root() -> Path:
    override = os.environ.get(CACHE_ENV)
    return Path(override).expanduser() if override else Path.home() / ".repo-task/kb"


def slug_for(remote: str) -> str:
    """Readable directory name plus a hash, so similar remotes never collide."""
    tail = re.sub(r"[^a-zA-Z0-9]+", "-", remote.rstrip("/").split("/")[-1]).strip("-").lower()
    digest = hashlib.sha256(remote.encode("utf-8")).hexdigest()[:10]
    return f"{tail or 'kb'}-{digest}"


@dataclass(frozen=True)
class KnowledgeSource:
    path: Path
    kind: str  # "remote" | "local"
    remote: str = ""
    ref: str = ""
    revision: str = ""
    synced_at: str = ""

    @property
    def exists(self) -> bool:
        return self.path.is_dir()


def resolve(config: RepoTaskConfig, sync: bool | None = None) -> KnowledgeSource:
    """Return the knowledge base location, syncing the remote when allowed."""
    knowledge = config.knowledge
    if knowledge.remote:
        should_sync = knowledge.auto_sync if sync is None else sync
        return _resolve_remote(knowledge, should_sync)
    local = (config.root / knowledge.local).resolve()
    if not local.is_dir():
        raise RepoTaskError(
            f"Knowledge base not found at {local}. Configure `knowledge.remote` or create the "
            "directory, then run `repo-task kb sync`."
        )
    return KnowledgeSource(path=local, kind="local")


def _resolve_remote(knowledge: KnowledgeConfig, should_sync: bool) -> KnowledgeSource:
    path = cache_root() / slug_for(knowledge.remote)
    if not path.is_dir():
        if not should_sync:
            raise RepoTaskError(
                f"Knowledge base has never been synced to {path}. Run `repo-task kb sync`."
            )
        _clone(knowledge.remote, path)
    elif should_sync:
        _fetch(path)
    _checkout(path, knowledge.ref)
    return KnowledgeSource(
        path=path,
        kind="remote",
        remote=knowledge.remote,
        ref=knowledge.ref,
        revision=run_git(["rev-parse", "HEAD"], path).strip(),
        synced_at=_read_stamp(path, should_sync),
    )


def _clone(remote: str, path: Path) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    try:
        run_git(["clone", "--quiet", "--", remote, str(path)])
    except RepoTaskError as error:
        raise RepoTaskError(f"Could not clone knowledge base {remote}: {error}") from error


def _fetch(path: Path) -> None:
    try:
        run_git(["fetch", "--quiet", "--prune", "origin"], path)
    except RepoTaskError as error:
        raise RepoTaskError(f"Could not fetch knowledge base updates: {error}") from error


def _checkout(path: Path, ref: str) -> None:
    """Pin the worktree to `ref`, preferring the remote-tracking branch when one exists."""
    if ref.startswith("-"):
        raise RepoTaskError(f"Invalid knowledge base ref: {ref}")
    target = ref
    try:
        run_git(["rev-parse", "--verify", f"origin/{ref}"], path)
        target = f"origin/{ref}"
    except RepoTaskError:
        pass
    try:
        run_git(["checkout", "--quiet", "--detach", target], path)
    except RepoTaskError as error:
        raise RepoTaskError(f"Knowledge base ref not found: {ref} ({error})") from error


def _read_stamp(path: Path, synced_now: bool) -> str:
    stamp = path.parent / f"{path.name}.synced"
    if synced_now:
        value = datetime.now(timezone.utc).isoformat(timespec="seconds")
        stamp.write_text(value, encoding="utf-8")
        return value
    return stamp.read_text(encoding="utf-8").strip() if stamp.is_file() else ""
