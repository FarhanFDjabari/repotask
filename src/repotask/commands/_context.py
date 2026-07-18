"""Shared command helpers."""

from __future__ import annotations

from repotask.config import RepoTaskConfig, load_config
from repotask.kb import KnowledgeBase, resolve
from repotask.kb.store import load


def open_knowledge(sync: bool | None = None) -> tuple[RepoTaskConfig, KnowledgeBase]:
    """Load project config and the knowledge base it points at."""
    config = load_config()
    return config, load(resolve(config, sync=sync))
