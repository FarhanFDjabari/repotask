from __future__ import annotations

from pathlib import Path

from repotask.config import load_config
from repotask.skills import generator
from repotask.skills.generator import AGENTS_FILE, BEGIN_MARKER, END_MARKER, SKILLS


def test_every_skill_declares_frontmatter_with_triggers() -> None:
    for name in SKILLS:
        content = generator.template(name)
        assert content.startswith("---\n")
        assert f"name: {name}\n" in content
        assert "description:" in content


def test_skills_reference_commands_without_inlining_knowledge() -> None:
    for name in SKILLS:
        content = generator.template(name)
        assert "repo-task --json" in content


def test_sync_creates_skill_files_and_agents_block(project: Path) -> None:
    results = generator.sync(load_config())

    paths = {item.path for item in results}
    assert f".claude/skills/{SKILLS[0]}/SKILL.md" in paths
    assert AGENTS_FILE in paths
    assert (project / ".claude/skills/repotask-search/SKILL.md").is_file()
    assert BEGIN_MARKER in (project / AGENTS_FILE).read_text(encoding="utf-8")


def test_sync_is_idempotent(project: Path) -> None:
    generator.sync(load_config())

    results = generator.sync(load_config())

    assert {item.action for item in results} == {"unchanged"}


def test_sync_preserves_hand_written_agents_content(project: Path) -> None:
    (project / AGENTS_FILE).write_text("# House rules\n\nBe kind.\n", encoding="utf-8")

    generator.sync(load_config())

    content = (project / AGENTS_FILE).read_text(encoding="utf-8")
    assert content.startswith("# House rules")
    assert BEGIN_MARKER in content


def test_sync_replaces_only_the_marked_block(project: Path) -> None:
    (project / AGENTS_FILE).write_text(
        f"# Top\n\n{BEGIN_MARKER}\nstale content\n{END_MARKER}\n\n# Bottom\n", encoding="utf-8"
    )

    generator.sync(load_config())

    content = (project / AGENTS_FILE).read_text(encoding="utf-8")
    assert "stale content" not in content
    assert content.startswith("# Top")
    assert content.rstrip().endswith("# Bottom")
    assert content.count(BEGIN_MARKER) == 1


def test_agents_block_lists_the_project_stacks(project: Path) -> None:
    block = generator.agents_block(load_config())

    assert "android, kotlin" in block


def test_dry_run_writes_nothing(project: Path) -> None:
    results = generator.sync(load_config(), dry_run=True)

    assert all(item.action == "created" for item in results)
    assert not (project / ".claude/skills").exists()


def test_other_agent_files_are_reported(project: Path) -> None:
    (project / "CLAUDE.md").write_text("# legacy\n", encoding="utf-8")

    assert generator.find_agent_files(project) == ["CLAUDE.md"]
