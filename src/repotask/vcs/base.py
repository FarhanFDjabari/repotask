"""Provider-specific local CLI argument generation."""

from __future__ import annotations

import subprocess
from dataclasses import dataclass
from pathlib import Path

from repotask.config.models import VcsConfig
from repotask.errors import RepoTaskError


@dataclass(frozen=True)
class VcsAdapter:
    config: VcsConfig

    def executable(self) -> str | None:
        return None

    def create_args(
        self,
        title: str,
        description_path: Path,
        base_branch: str,
        branch: str,
        draft: bool,
    ) -> list[str]:
        raise RepoTaskError(
            f"Remote change-request creation is not supported for {self.config.provider}."
        )

    def create(
        self,
        root: Path,
        title: str,
        description_path: Path,
        base_branch: str,
        branch: str,
        draft: bool,
    ) -> str:
        executable = self.executable()
        if not executable:
            raise RepoTaskError(
                f"{self.config.provider} supports description generation only in Milestone 1."
            )
        args = self.create_args(title, description_path, base_branch, branch, draft)
        try:
            completed = subprocess.run(
                [executable, *args],
                cwd=root,
                check=False,
                capture_output=True,
                text=True,
            )
        except FileNotFoundError as error:
            raise RepoTaskError(
                f"{executable} is not installed or not on PATH. Install and authenticate it first."
            ) from error
        if completed.returncode != 0:
            detail = completed.stderr.strip() or completed.stdout.strip()
            raise RepoTaskError(detail or f"{executable} {' '.join(args)} failed")
        return completed.stdout.strip()


class GitLabAdapter(VcsAdapter):
    def executable(self) -> str:
        return "glab"

    def create_args(
        self,
        title: str,
        description_path: Path,
        base_branch: str,
        branch: str,
        draft: bool,
    ) -> list[str]:
        args = [
            "mr",
            "create",
            "--title",
            title,
            "--description-file",
            str(description_path),
            "--target-branch",
            base_branch,
            "--source-branch",
            branch,
            "--push",
            "--yes",
        ]
        if draft:
            args.append("--draft")
        return args


class GitHubAdapter(VcsAdapter):
    def executable(self) -> str:
        return "gh"

    def create_args(
        self,
        title: str,
        description_path: Path,
        base_branch: str,
        branch: str,
        draft: bool,
    ) -> list[str]:
        args = [
            "pr",
            "create",
            "--title",
            title,
            "--body-file",
            str(description_path),
            "--base",
            base_branch,
            "--head",
            branch,
        ]
        if draft:
            args.append("--draft")
        return args


def get_vcs_adapter(config: VcsConfig) -> VcsAdapter:
    if config.provider == "gitlab":
        return GitLabAdapter(config)
    if config.provider == "github":
        return GitHubAdapter(config)
    return VcsAdapter(config)
