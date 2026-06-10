from __future__ import annotations

from pathlib import Path

from repotask.agents.service import assign_agents, assigned_for_phase
from repotask.config.models import RepoTaskConfig
from tests.test_config import valid_config


def agent_config(tmp_path: Path) -> RepoTaskConfig:
    data = valid_config()
    data["agents"] = {
        "roles": {
            "requirement-analyst": "a.md",
            "diff-reviewer": "b.md",
            "qa-planner": "c.md",
        },
        "default": ["requirement-analyst", "diff-reviewer", "qa-planner"],
        "conditional": {
            "ui-reviewer": {
                "keywords": ["compose"],
                "paths": ["*.tsx"],
                "phases": ["investigate", "review"],
            }
        },
        "additional": ["unknown.md"],
    }
    return RepoTaskConfig.from_dict(data, tmp_path)


def test_keyword_and_path_assignment(tmp_path: Path) -> None:
    config = agent_config(tmp_path)
    result = assign_agents(config, "Compose screen", "", ["src/App.tsx"])
    assert result["conditionalAgents"] == ["ui-reviewer"]
    assert result["matchedKeywords"]["ui-reviewer"] == ["compose"]
    assert result["matchedPaths"]["ui-reviewer"] == ["*.tsx"]


def test_unknown_additional_agent_is_not_assigned(tmp_path: Path) -> None:
    config = agent_config(tmp_path)
    result = assign_agents(config, "plain task", "")
    assert "unknown" not in result["assignedAgents"]


def test_phase_selection_keeps_conditional_specialist(tmp_path: Path) -> None:
    config = agent_config(tmp_path)
    snapshot = {
        "assignedAgents": ["requirement-analyst", "diff-reviewer", "qa-planner", "ui-reviewer"],
        "conditionalAgents": ["ui-reviewer"],
    }
    assert assigned_for_phase(snapshot, "review", config) == [
        "diff-reviewer",
        "qa-planner",
        "ui-reviewer",
    ]

