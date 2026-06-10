from __future__ import annotations

from pathlib import Path

import pytest

from repotask.config.loader import load_config
from repotask.config.models import RepoTaskConfig
from repotask.errors import RepoTaskError


def valid_config() -> dict:
    return {
        "schema_version": 1,
        "project": {"name": "demo", "stacks": ["python"], "base_branch": "main"},
        "branch": {"feature_pattern": "feature/{task_id}-{slug}"},
        "vcs": {
            "provider": "github",
            "remote": "origin",
            "terminology": "pull-request",
            "create": {"enabled": True, "adapter": "gh"},
        },
        "task_provider": {
            "provider": "jira",
            "display_name": "Jira issue",
            "url_pattern": "https://jira.example/{task_id}",
            "connector_hint": "jira",
        },
        "workflow": {"documents": [], "manual_gates": []},
        "rules": {"files": []},
        "change_request": {"template": "", "title_pattern": "[{task_id}] {title}"},
        "agents": {"roles": {}, "default": [], "conditional": {}, "additional": []},
    }


def test_config_validates_and_formats_provider_url(tmp_path: Path) -> None:
    config = RepoTaskConfig.from_dict(valid_config(), tmp_path)
    assert config.project.stacks == ["python"]
    assert config.task_provider.task_url("ABC-1") == "https://jira.example/ABC-1"


def test_unknown_schema_has_migration_guidance(tmp_path: Path) -> None:
    data = valid_config()
    data["schema_version"] = 99
    with pytest.raises(RepoTaskError, match="migrate"):
        RepoTaskConfig.from_dict(data, tmp_path)


def test_invalid_branch_pattern_names_field(tmp_path: Path) -> None:
    data = valid_config()
    data["branch"]["feature_pattern"] = "feature/{task_id}"
    with pytest.raises(RepoTaskError, match="branch.feature_pattern"):
        RepoTaskConfig.from_dict(data, tmp_path)


def test_yaml_error_reports_line(git_repo: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    (git_repo / ".repo-task.yml").write_text("schema_version: [\n", encoding="utf-8")
    monkeypatch.chdir(git_repo)
    with pytest.raises(RepoTaskError, match="line"):
        load_config()
