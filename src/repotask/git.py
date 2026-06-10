"""Git operations used by RepoTask."""

from __future__ import annotations

import subprocess
from pathlib import Path

from repotask.errors import RepoTaskError


def run_git(args: list[str], root: Path | None = None, check: bool = True) -> str:
    try:
        completed = subprocess.run(
            ["git", *args],
            cwd=root,
            check=False,
            capture_output=True,
            text=True,
        )
    except FileNotFoundError as error:
        raise RepoTaskError("git is not installed or not on PATH.") from error
    if check and completed.returncode != 0:
        detail = completed.stderr.strip() or completed.stdout.strip()
        raise RepoTaskError(detail or f"git {' '.join(args)} failed")
    return completed.stdout


def resolve_git_root(start: Path | None = None) -> Path:
    location = (start or Path.cwd()).resolve()
    try:
        output = run_git(["rev-parse", "--show-toplevel"], location)
    except RepoTaskError as error:
        raise RepoTaskError(f"{location} is not inside a Git repository.") from error
    return Path(output.strip()).resolve()


def current_branch(root: Path) -> str:
    return run_git(["branch", "--show-current"], root).strip()


def assert_clean_worktree(root: Path) -> None:
    if run_git(["status", "--porcelain"], root).strip():
        raise RepoTaskError(
            "Git working tree must be clean before creating a task branch. "
            "Commit, stash, or remove current changes first."
        )


def verify_branch(root: Path, branch: str) -> None:
    try:
        run_git(["rev-parse", "--verify", branch], root)
    except RepoTaskError as error:
        raise RepoTaskError(f"Base branch does not exist locally: {branch}") from error


def create_branch(root: Path, branch: str, base_branch: str) -> None:
    verify_branch(root, base_branch)
    try:
        run_git(["switch", "-c", branch, base_branch], root)
    except RepoTaskError as error:
        raise RepoTaskError(
            f"Could not create branch {branch} from {base_branch}: {error}"
        ) from error


def diff(root: Path, base_branch: str, include_worktree: bool = False) -> str:
    committed = run_git(["diff", f"{base_branch}...HEAD"], root)
    if not include_worktree:
        return committed
    sections = [
        ("Committed branch diff", committed),
        ("Staged worktree diff", run_git(["diff", "--cached"], root)),
        ("Unstaged worktree diff", run_git(["diff"], root)),
    ]
    return "\n\n".join(
        f"# {label}\n\n{content.rstrip()}" for label, content in sections if content.strip()
    )


def changed_paths(root: Path, base_branch: str, include_worktree: bool = False) -> list[str]:
    commands = [["diff", "--name-only", f"{base_branch}...HEAD"]]
    if include_worktree:
        commands.extend([["diff", "--cached", "--name-only"], ["diff", "--name-only"]])
    paths: set[str] = set()
    for command in commands:
        paths.update(line.strip() for line in run_git(command, root).splitlines() if line.strip())
    return sorted(paths)


def diff_summary(root: Path, base_branch: str) -> str:
    sections = []
    for label, args in [
        ("Name status", ["diff", "--name-status", f"{base_branch}...HEAD"]),
        ("Stat", ["diff", "--stat", f"{base_branch}...HEAD"]),
    ]:
        try:
            output = run_git(args, root)
        except RepoTaskError as error:
            output = f"Unavailable: {error}"
        sections.append(f"## {label}\n\n{output.strip() or 'No changes detected.'}")
    return "\n\n".join(sections)


def has_uncommitted_changes(root: Path) -> bool:
    return bool(run_git(["status", "--porcelain"], root).strip())


def has_tracked_worktree_changes(root: Path) -> bool:
    staged = run_git(["diff", "--cached", "--name-only"], root)
    unstaged = run_git(["diff", "--name-only"], root)
    return bool(staged.strip() or unstaged.strip())


def untracked_files(root: Path) -> list[str]:
    output = run_git(["ls-files", "--others", "--exclude-standard"], root)
    return [line for line in output.splitlines() if line.strip()]


def safe_git_output(root: Path, args: list[str]) -> str:
    try:
        return run_git(args, root)
    except RepoTaskError as error:
        return f"unavailable: {error}"
