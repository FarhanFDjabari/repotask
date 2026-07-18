from __future__ import annotations

from pathlib import Path

from repotask.config import load_config
from repotask.kb import resolve
from repotask.kb.budget import estimate, fit
from repotask.kb.slicing import build_pack, rank
from repotask.kb.store import load


def open_kb():
    config = load_config()
    return config, load(resolve(config, sync=True))


def test_fit_leaves_short_text_untouched() -> None:
    body, truncated = fit("short body", 100)

    assert body == "short body"
    assert truncated is False


def test_fit_keeps_the_result_inside_the_budget() -> None:
    body, truncated = fit("word " * 500, 40, command="recipe", doc_id="x")

    assert truncated is True
    assert estimate(body) <= 40


def test_ranking_drops_documents_for_other_stacks(project: Path) -> None:
    config, kb = open_kb()

    ranked = rank(kb, config.project.stacks, "add pagination")

    assert [item.document.id for item in ranked] == ["pagination", "android-architecture"]


def test_ranking_prefers_the_intent_slice(project: Path) -> None:
    config, kb = open_kb()

    ranked = rank(kb, config.project.stacks, "feature")

    assert ranked[0].score >= 100
    assert "slice:feature" in ranked[0].reasons


def test_pack_never_exceeds_its_budget(project: Path) -> None:
    config, kb = open_kb()

    for budget in (4, 12, 30, 60, 6000):
        pack = build_pack(kb, config.project.stacks, "add pagination", budget)
        assert pack.used <= budget, f"budget {budget} exceeded: {pack.used}"


def test_pack_excludes_knowledge_for_other_stacks(project: Path) -> None:
    config, kb = open_kb()

    pack = build_pack(kb, config.project.stacks, "add pagination", 6000)

    ids = {document["id"] for document in pack.documents}
    assert "android-architecture" in ids
    assert "ios-architecture" not in ids


def test_pack_records_omitted_documents_when_budget_is_tight(project: Path) -> None:
    config, kb = open_kb()

    pack = build_pack(kb, config.project.stacks, "add pagination", 12)

    assert pack.documents == []
    assert "pagination" in pack.omitted


def test_pack_includes_indexed_facts(project: Path) -> None:
    from repotask.commands.facts import index as index_command
    from repotask.output import state
    from tests.test_index import KOTLIN, write

    write(project, "app/FeedViewModel.kt", KOTLIN)
    open_kb()  # first sync, so the index has a knowledge base to write into
    state.json_mode = True
    try:
        index_command(changed_only=False)
    finally:
        state.json_mode = False

    config, kb = open_kb()
    pack = build_pack(kb, config.project.stacks, "add pagination", 6000)

    assert [entry["name"] for entry in pack.facts["viewmodels"]] == ["FeedViewModel"]


def test_changed_paths_lift_documents_that_declare_them(project: Path, kb_repo: Path) -> None:
    from tests.conftest import commit_all

    (kb_repo / "conventions/room.md").write_text(
        "---\nid: room\ntitle: Room Schemas\nstacks: [android]\n"
        'applies_to: ["**/schemas/*.json"]\n---\nSchemas are immutable once committed.\n',
        encoding="utf-8",
    )
    commit_all(kb_repo, "add room convention")
    config, kb = open_kb()

    without = {
        item.document.id: item for item in rank(kb, config.project.stacks, "bump database version")
    }
    with_paths = {
        item.document.id: item
        for item in rank(kb, config.project.stacks, "bump database version", ["app/schemas/2.json"])
    }

    assert with_paths["room"].score > without["room"].score
    assert any(reason.startswith("paths:") for reason in with_paths["room"].reasons)
