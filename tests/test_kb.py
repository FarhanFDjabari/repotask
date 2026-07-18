from __future__ import annotations

from pathlib import Path

import pytest

from repotask.config import load_config
from repotask.errors import RepoTaskError
from repotask.kb import resolve
from repotask.kb.schema import parse_frontmatter
from repotask.kb.source import slug_for
from repotask.kb.store import load
from tests.conftest import commit_all


def open_kb(sync: bool = True):
    config = load_config()
    return config, load(resolve(config, sync=sync))


def test_parse_frontmatter_splits_yaml_from_body() -> None:
    front, body = parse_frontmatter("---\nid: a\ntags: [x]\n---\nBody text\n")

    assert front == {"id": "a", "tags": ["x"]}
    assert body == "Body text"


def test_parse_frontmatter_without_fence_returns_whole_text() -> None:
    front, body = parse_frontmatter("Just markdown\n")

    assert front == {}
    assert body == "Just markdown\n"


def test_parse_frontmatter_rejects_unterminated_fence() -> None:
    with pytest.raises(RepoTaskError, match="closing"):
        parse_frontmatter("---\nid: a\nBody\n")


def test_slug_is_stable_and_collision_resistant() -> None:
    assert slug_for("https://git.example/team/kb") == slug_for("https://git.example/team/kb")
    assert slug_for("https://git.example/a/kb") != slug_for("https://git.example/b/kb")


def test_remote_source_clones_and_pins_revision(project: Path) -> None:
    _config, kb = open_kb()

    assert kb.source.kind == "remote"
    assert len(kb.source.revision) == 40
    assert kb.source.synced_at


def test_sync_pulls_new_knowledge_documents(project: Path, kb_repo: Path) -> None:
    _config, first = open_kb()
    assert "caching" not in first.recipes

    (kb_repo / "recipes/caching.md").write_text(
        "---\nid: caching\ntitle: Caching\nstacks: [android]\n---\nUse a repository cache.\n",
        encoding="utf-8",
    )
    commit_all(kb_repo, "add caching recipe")

    _config, second = open_kb()
    assert "caching" in second.recipes


def test_resolve_without_sync_fails_before_first_clone(project: Path) -> None:
    config = load_config()
    with pytest.raises(RepoTaskError, match="never been synced"):
        resolve(config, sync=False)


def test_layers_load_with_ids_and_bodies(project: Path) -> None:
    _config, kb = open_kb()

    assert set(kb.conventions) == {"android-architecture", "ios-architecture"}
    assert set(kb.recipes) == {"pagination"}
    assert "Paging 3" in kb.recipes["pagination"].body
    assert kb.recipes["pagination"].path == "recipes/pagination.md"


def test_slice_for_stacks_selects_only_matching_stack(project: Path) -> None:
    _config, kb = open_kb()

    resolved = kb.slice_for(["android", "kotlin"])

    assert resolved.conventions == ["android-architecture"]
    assert "ios-architecture" not in resolved.conventions
    assert resolved.facts == ["viewmodels"]


def test_search_ranks_id_and_tag_matches_above_body(project: Path) -> None:
    _config, kb = open_kb()

    hits = kb.search("pagination")

    assert hits[0].document.id == "pagination"


def test_search_can_restrict_to_a_layer(project: Path) -> None:
    _config, kb = open_kb()

    hits = kb.search("mvvm", layer="recipe")

    assert hits == []


def test_missing_manifest_is_a_clear_error(project: Path, kb_repo: Path) -> None:
    (kb_repo / "kb.yaml").unlink()
    commit_all(kb_repo, "remove manifest")

    with pytest.raises(RepoTaskError, match="kb.yaml not found"):
        open_kb()


def test_facts_are_empty_until_indexed(project: Path) -> None:
    _config, kb = open_kb()

    assert kb.facts("viewmodels") == []
    assert kb.fact_families() == []
