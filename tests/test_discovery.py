from __future__ import annotations

from pathlib import Path

from repotask.discovery.assets import discover_assets
from repotask.discovery.project import discover_project
from repotask.discovery.stacks import detect_stacks


def test_detects_android_compose(tmp_path: Path) -> None:
    (tmp_path / "build.gradle.kts").write_text(
        'plugins { id("com.android.application") }\ncomposeOptions {}\n', encoding="utf-8"
    )
    assert detect_stacks(tmp_path) == ["android", "kotlin", "jetpack-compose"]


def test_detects_flutter_and_dart(tmp_path: Path) -> None:
    (tmp_path / "pubspec.yaml").write_text("name: demo\n", encoding="utf-8")
    assert detect_stacks(tmp_path) == ["flutter", "dart"]


def test_discovers_assets_by_category(git_repo: Path) -> None:
    (git_repo / ".ai/docs/workflow").mkdir(parents=True)
    (git_repo / ".ai/docs/workflow/current.md").write_text("# Flow\n", encoding="utf-8")
    (git_repo / "AGENTS.md").write_text("# Rules\n", encoding="utf-8")
    (git_repo / ".github").mkdir()
    (git_repo / ".github/pull_request_template.md").write_text("# PR\n", encoding="utf-8")
    assets = {(item.relative_path, item.category) for item in discover_assets(git_repo)}
    assert (".ai/docs/workflow/current.md", "workflow") in assets
    assert ("AGENTS.md", "rules") in assets
    assert (".github/pull_request_template.md", "templates") in assets


def test_resolves_project_from_nested_directory(git_repo: Path) -> None:
    nested = git_repo / "a/b"
    nested.mkdir(parents=True)
    assert discover_project(nested).root == git_repo.resolve()

