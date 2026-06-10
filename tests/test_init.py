from __future__ import annotations

import json
from pathlib import Path

import pytest

from repotask.commands.init import initialize
from repotask.errors import RepoTaskError
from tests.conftest import write_answers


def test_noninteractive_requires_explicit_provider(
    git_repo: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    path = git_repo / "answers.yml"
    path.write_text("project:\n  stacks: [generic]\n", encoding="utf-8")
    monkeypatch.chdir(git_repo)
    with pytest.raises(RepoTaskError, match="never infers"):
        initialize(path, non_interactive=True, dry_run=False, force=False)


def test_dry_run_writes_nothing(git_repo: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    answers = write_answers(git_repo)
    monkeypatch.chdir(git_repo)
    initialize(answers, non_interactive=True, dry_run=True, force=False)
    assert not (git_repo / ".repo-task.yml").exists()


def test_init_imports_assets_and_is_idempotent(
    git_repo: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    (git_repo / "docs/workflow").mkdir(parents=True)
    (git_repo / "docs/workflow/current.md").write_text("# Existing\n", encoding="utf-8")
    (git_repo / "AGENTS.md").write_text("# Project rules\n", encoding="utf-8")
    answers = write_answers(
        git_repo,
        assets={
            "workflow": ["docs/workflow/current.md"],
            "template": "",
            "rules": ["AGENTS.md"],
            "agents": [],
        },
        workflow="combined",
    )
    monkeypatch.chdir(git_repo)
    initialize(answers, non_interactive=True, dry_run=False, force=False)
    initialize(answers, non_interactive=True, dry_run=False, force=False)
    assert (git_repo / ".repo-task/workflow/imported/docs/workflow/current.md").is_file()
    assert (git_repo / ".repo-task/rules/imported/AGENTS.md").is_file()
    assert (git_repo / ".gitignore").read_text(encoding="utf-8").count(
        ".repo-task/tasks/"
    ) == 1
    manifest = json.loads(
        (git_repo / ".repo-task/setup-manifest.json").read_text(encoding="utf-8")
    )
    assert all("sha256" in entry for entry in manifest["files"])


def test_force_backs_up_owned_file(git_repo: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    answers = write_answers(git_repo)
    monkeypatch.chdir(git_repo)
    initialize(answers, non_interactive=True, dry_run=False, force=False)
    config_path = git_repo / ".repo-task.yml"
    config_path.write_text("user modification\n", encoding="utf-8")
    initialize(answers, non_interactive=True, dry_run=False, force=True)
    backups = list((git_repo / ".repo-task/backups").glob("*/.repo-task.yml"))
    assert len(backups) == 1
    assert backups[0].read_text(encoding="utf-8") == "user modification\n"


def test_missing_selected_asset_fails(git_repo: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    answers = write_answers(
        git_repo,
        assets={"workflow": [], "template": "", "rules": ["missing.md"], "agents": []},
    )
    monkeypatch.chdir(git_repo)
    with pytest.raises(RepoTaskError, match="does not exist"):
        initialize(answers, non_interactive=True, dry_run=False, force=False)

