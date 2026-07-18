from __future__ import annotations

from pathlib import Path

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


def test_resolves_project_from_nested_directory(git_repo: Path) -> None:
    nested = git_repo / "a/b"
    nested.mkdir(parents=True)
    assert discover_project(nested).root == git_repo.resolve()

