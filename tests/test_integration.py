from __future__ import annotations

import json
from pathlib import Path

import pytest

from repotask.cli import run
from tests.conftest import git, write_answers


def invoke(args: list[str]) -> int:
    return run(args)


@pytest.mark.parametrize(
    ("stacks", "vcs", "provider"),
    [
        (["android", "kotlin", "jetpack-compose"], "gitlab", "clickup"),
        (["ios", "swift", "swiftui"], "github", "jira"),
        (["flutter", "dart"], "bitbucket", "manual"),
        (["generic"], "other", "custom"),
    ],
)
def test_project_matrix_initialization(
    git_repo: Path,
    monkeypatch: pytest.MonkeyPatch,
    stacks: list[str],
    vcs: str,
    provider: str,
) -> None:
    answers = write_answers(git_repo, stacks=stacks, vcs=vcs, provider=provider)
    monkeypatch.chdir(git_repo)
    assert invoke(["init", "--non-interactive", "--answers", str(answers)]) == 0
    config = (git_repo / ".repo-task.yml").read_text(encoding="utf-8")
    assert f"provider: {provider}" in config
    assert f"provider: {vcs}" in config
    assert not (git_repo / ".mobile-task.yml").exists()
    assert not (git_repo / ".task").exists()


def test_start_context_review_cr_status_flow(
    git_repo: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    answers = write_answers(git_repo, stacks=["python"], vcs="github", provider="jira")
    monkeypatch.chdir(git_repo)
    assert invoke(["init", "--non-interactive", "--answers", str(answers)]) == 0
    git(git_repo, "add", ".")
    git(git_repo, "commit", "-m", "initialize RepoTask")

    assert invoke(["start", "ABC-1", "--title", "Fix parser edge case", "--mode", "human"]) == 0
    task_dir = git_repo / ".repo-task/tasks/ABC-1"
    task = json.loads((task_dir / "config.json").read_text(encoding="utf-8"))
    assert task["branch"] == "feature/ABC-1-fix-parser-edge-case"
    assert task["provider"] == "jira"
    assert (task_dir / "agents.json").is_file()

    requirement = git_repo / "requirement.md"
    requirement.write_text("Handle empty input safely.\n", encoding="utf-8")
    assert invoke(["context", "ABC-1", "--from-file", str(requirement)]) == 0
    assert "Handle empty input safely" in (task_dir / "context.md").read_text(encoding="utf-8")

    (git_repo / "feature.py").write_text(
        "def parse(value):\n    return value or ''\n", encoding="utf-8"
    )
    git(git_repo, "add", "feature.py")
    git(git_repo, "commit", "-m", "implement")
    assert invoke(["review", "ABC-1"]) == 0
    assert "## Must fix" in (task_dir / "prompts/review.md").read_text(encoding="utf-8")

    assert invoke(["cr", "ABC-1"]) == 0
    description = (task_dir / "cr-description.md").read_text(encoding="utf-8")
    assert "<!-- repo-task:generated:start -->" in description
    assert "https://tasks.example/ABC-1" in description

    assert invoke(["status", "ABC-1", "--provider"]) == 0
    prompt = (task_dir / "prompts/provider-status.md").read_text(encoding="utf-8")
    assert "READ-ONLY" in prompt
    assert "Do not update provider status" in prompt


def test_review_warns_about_empty_diff(
    git_repo: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    answers = write_answers(git_repo)
    monkeypatch.chdir(git_repo)
    invoke(["init", "--non-interactive", "--answers", str(answers)])
    git(git_repo, "add", ".")
    git(git_repo, "commit", "-m", "initialize")
    invoke(["start", "TEST-1", "--title", "No changes", "--mode", "human"])
    assert invoke(["review", "TEST-1"]) == 1


def test_no_template_workflow_is_supported(
    git_repo: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    answers = write_answers(git_repo, workflow="none")
    monkeypatch.chdir(git_repo)
    assert invoke(["init", "--non-interactive", "--answers", str(answers)]) == 0
    config = (git_repo / ".repo-task.yml").read_text(encoding="utf-8")
    assert "documents: []" in config
    assert ".repo-task/templates/bundled/github.md" in config
