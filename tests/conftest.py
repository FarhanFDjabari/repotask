from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from repotask.config.writer import dump_yaml


def git(root: Path, *args: str) -> str:
    completed = subprocess.run(
        ["git", *args],
        cwd=root,
        check=True,
        capture_output=True,
        text=True,
    )
    return completed.stdout


@pytest.fixture
def git_repo(tmp_path: Path) -> Path:
    root = tmp_path / "project"
    root.mkdir()
    git(root, "init", "-b", "main")
    git(root, "config", "user.email", "tests@example.com")
    git(root, "config", "user.name", "RepoTask Tests")
    (root / "README.md").write_text("# Test\n", encoding="utf-8")
    git(root, "add", "README.md")
    git(root, "commit", "-m", "initial")
    return root


def write_answers(
    root: Path,
    *,
    stacks: list[str] | None = None,
    vcs: str = "github",
    provider: str = "manual",
    workflow: str = "bundled",
    assets: dict | None = None,
) -> Path:
    data = {
        "project": {
            "name": root.name,
            "stacks": stacks or ["generic"],
            "base_branch": "main",
        },
        "branch": {"feature_pattern": "feature/{task_id}-{slug}"},
        "vcs": {"provider": vcs},
        "task_provider": {
            "provider": provider,
            "display_name": f"{provider} task",
            "url_pattern": "https://tasks.example/{task_id}" if provider != "manual" else "",
            "connector_hint": provider if provider != "manual" else "",
        },
        "workflow": {"mode": workflow},
        "assets": assets
        or {"workflow": [], "template": "", "rules": [], "agents": []},
        "bundled_defaults": True,
    }
    path = root / "answers.yml"
    path.write_text(dump_yaml(data), encoding="utf-8")
    return path
