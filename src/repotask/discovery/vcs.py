"""VCS provider recommendations."""

from __future__ import annotations

from pathlib import Path
from urllib.parse import urlparse

from repotask.git import run_git


def detect_vcs_provider(root: Path) -> str:
    remote = run_git(["remote", "get-url", "origin"], root, check=False).strip().lower()
    host = urlparse(remote).hostname or remote
    if "gitlab" in host:
        return "gitlab"
    if "github" in host:
        return "github"
    if "bitbucket" in host:
        return "bitbucket"
    return "other"


def detect_base_branch(root: Path) -> str:
    symbolic = run_git(
        ["symbolic-ref", "--quiet", "--short", "refs/remotes/origin/HEAD"], root, check=False
    ).strip()
    if symbolic.startswith("origin/"):
        return symbolic.removeprefix("origin/")
    for candidate in ("main", "master"):
        if run_git(["rev-parse", "--verify", candidate], root, check=False).strip():
            return candidate
    branch = run_git(["branch", "--show-current"], root, check=False).strip()
    return branch or "main"

