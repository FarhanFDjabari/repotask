from __future__ import annotations

from pathlib import Path

from repotask.config.models import RepoTaskConfig
from repotask.rules import compose_rules, detector_callouts
from tests.test_config import valid_config


def test_rules_preserve_common_stack_project_order(tmp_path: Path) -> None:
    for name in ("common.md", "python.md", "project.md"):
        (tmp_path / name).write_text(f"# {name}\n", encoding="utf-8")
    data = valid_config()
    data["rules"]["files"] = ["common.md", "python.md", "project.md"]
    config = RepoTaskConfig.from_dict(data, tmp_path)
    assert [path for path, _content in compose_rules(config)] == [
        "common.md",
        "python.md",
        "project.md",
    ]


def test_room_detector_only_runs_for_android(tmp_path: Path) -> None:
    data = valid_config()
    data["project"]["stacks"] = ["android"]
    config = RepoTaskConfig.from_dict(data, tmp_path)
    callouts = detector_callouts(config, ["app/schemas/2.json"])
    assert "Room Schema Change Detected" in callouts[0]

    data["project"]["stacks"] = ["generic"]
    generic = RepoTaskConfig.from_dict(data, tmp_path)
    assert detector_callouts(generic, ["app/schemas/2.json"]) == []

