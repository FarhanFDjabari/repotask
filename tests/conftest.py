from __future__ import annotations

import subprocess
from pathlib import Path

import pytest
import yaml

from repotask.config.loader import CONFIG_PATH
from repotask.kb.source import CACHE_ENV

CONVENTION_ANDROID = """---
id: android-architecture
title: Android Architecture
stacks: [android, kotlin]
tags: [mvvm, hilt, module]
---
Feature modules use MVVM with Hilt. UI state is exposed as StateFlow.
"""

CONVENTION_IOS = """---
id: ios-architecture
title: iOS Architecture
stacks: [ios, swift]
tags: [mvvm, swiftui]
---
SwiftUI views bind to ObservableObject view models.
"""

RECIPE_PAGINATION = """---
id: pagination
title: Implement Pagination
stacks: [android]
tags: [paging, list]
---
Use Paging 3 with a RemoteMediator backed by Room.
"""


def git(root: Path, *args: str) -> str:
    completed = subprocess.run(
        ["git", *args],
        cwd=root,
        check=True,
        capture_output=True,
        text=True,
    )
    return completed.stdout


def init_repo(root: Path) -> Path:
    root.mkdir(parents=True, exist_ok=True)
    git(root, "init", "-b", "main")
    git(root, "config", "user.email", "tests@example.com")
    git(root, "config", "user.name", "RepoTask Tests")
    return root


def commit_all(root: Path, message: str = "initial") -> None:
    git(root, "add", "-A")
    git(root, "commit", "-m", message)


@pytest.fixture
def git_repo(tmp_path: Path) -> Path:
    root = init_repo(tmp_path / "project")
    (root / "README.md").write_text("# Test\n", encoding="utf-8")
    commit_all(root)
    return root


@pytest.fixture
def kb_repo(tmp_path: Path) -> Path:
    """A real git repository holding a minimal three-layer knowledge base."""
    root = init_repo(tmp_path / "kb")
    (root / "kb.yaml").write_text(
        yaml.safe_dump(
            {
                "schema_version": 1,
                "name": "test-kb",
                "default_budget": 500,
                "fact_families": [
                    {
                        "name": "viewmodels",
                        "description": "Android ViewModels",
                        "stacks": ["android"],
                        "languages": ["kotlin"],
                        "kinds": ["class"],
                        "name_pattern": ".*ViewModel$",
                    }
                ],
            },
            sort_keys=False,
        ),
        encoding="utf-8",
    )
    for directory, name, content in (
        ("conventions", "android-architecture.md", CONVENTION_ANDROID),
        ("conventions", "ios-architecture.md", CONVENTION_IOS),
        ("recipes", "pagination.md", RECIPE_PAGINATION),
    ):
        path = root / directory / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding="utf-8")
    slices = root / "slices"
    slices.mkdir()
    (slices / "android.yaml").write_text(
        yaml.safe_dump(
            {
                "stack": "android",
                "conventions": ["android-architecture"],
                "recipes": ["pagination"],
                "facts": ["viewmodels"],
                "intents": {"feature": ["android-architecture", "pagination"]},
            },
            sort_keys=False,
        ),
        encoding="utf-8",
    )
    commit_all(root)
    return root


@pytest.fixture
def project(tmp_path: Path, kb_repo: Path, monkeypatch: pytest.MonkeyPatch) -> Path:
    """An Android-ish project wired to the fixture knowledge base."""
    root = init_repo(tmp_path / "app")
    (root / "build.gradle.kts").write_text("// app\n", encoding="utf-8")
    config = root / CONFIG_PATH
    config.parent.mkdir(parents=True, exist_ok=True)
    config.write_text(
        yaml.safe_dump(
            {
                "schema_version": 2,
                "project": {
                    "name": "app",
                    "stacks": ["android", "kotlin"],
                    "base_branch": "main",
                },
                "knowledge": {"remote": f"file://{kb_repo}", "ref": "main"},
            },
            sort_keys=False,
        ),
        encoding="utf-8",
    )
    commit_all(root)
    monkeypatch.setenv(CACHE_ENV, str(tmp_path / "cache"))
    monkeypatch.chdir(root)
    return root
