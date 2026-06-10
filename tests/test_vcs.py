from pathlib import Path

from repotask.config.models import VcsConfig
from repotask.vcs.base import GitHubAdapter, GitLabAdapter


def test_gitlab_arguments() -> None:
    adapter = GitLabAdapter(VcsConfig(provider="gitlab"))
    args = adapter.create_args("Title", Path("body.md"), "main", "feature/x", True)
    assert args[:2] == ["mr", "create"]
    assert "--target-branch" in args
    assert "--source-branch" in args
    assert "--draft" in args


def test_github_arguments() -> None:
    adapter = GitHubAdapter(VcsConfig(provider="github"))
    args = adapter.create_args("Title", Path("body.md"), "main", "feature/x", False)
    assert args[:2] == ["pr", "create"]
    assert args[args.index("--base") + 1] == "main"
    assert args[args.index("--head") + 1] == "feature/x"
    assert "--draft" not in args

