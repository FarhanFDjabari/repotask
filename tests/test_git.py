from __future__ import annotations

from pathlib import Path

from repotask.git import changed_paths, diff
from tests.conftest import git


def test_diff_includes_worktree_sections(git_repo: Path) -> None:
    git(git_repo, "switch", "-c", "feature/test")
    (git_repo / "committed.txt").write_text("one\n", encoding="utf-8")
    git(git_repo, "add", "committed.txt")
    git(git_repo, "commit", "-m", "committed")
    (git_repo / "committed.txt").write_text("two\n", encoding="utf-8")
    (git_repo / "staged.txt").write_text("staged\n", encoding="utf-8")
    git(git_repo, "add", "staged.txt")
    output = diff(git_repo, "main", include_worktree=True)
    assert "# Committed branch diff" in output
    assert "# Staged worktree diff" in output
    assert "# Unstaged worktree diff" in output
    assert changed_paths(git_repo, "main", True) == ["committed.txt", "staged.txt"]

