"""Load configured workflow documents."""

from __future__ import annotations

from repotask.config.models import RepoTaskConfig
from repotask.files import read_optional


def load_workflow_documents(config: RepoTaskConfig) -> list[tuple[str, str]]:
    documents = []
    for relative in config.workflow.documents:
        content = read_optional(config.root / relative)
        if content is not None:
            documents.append((relative, content))
    return documents

