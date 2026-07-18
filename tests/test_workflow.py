from __future__ import annotations

from pathlib import Path

import pytest

from repotask.config import load_config
from repotask.errors import RepoTaskError
from repotask.workflow.analyze import (
    impact_set,
    keywords,
    layer_of,
    module_of,
    split_increments,
)
from repotask.workflow.dedupe import build_impacts, cluster, jaccard
from repotask.workflow.store import safe_id, work_item

SYMBOLS = [
    {"name": "FeedViewModel", "kind": "class", "path": "app/src/main/kotlin/feed/FeedViewModel.kt"},
    {"name": "FeedRepository", "kind": "class", "path": "data/src/main/kotlin/feed/FeedRepo.kt"},
    {"name": "LoginScreen", "kind": "class", "path": "app/src/main/kotlin/login/LoginScreen.kt"},
]


def test_keywords_prefer_identifiers_over_prose() -> None:
    found = keywords("The FeedViewModel should page through results using `PagingSource`.")

    assert "FeedViewModel" in found
    assert "PagingSource" in found
    assert "should" not in found


def test_keywords_are_deduplicated_case_insensitively() -> None:
    found = keywords("Feed feed FEED FeedViewModel")

    assert len([term for term in found if term.lower() == "feed"]) == 1


def test_module_uses_the_segment_before_src() -> None:
    assert module_of("app/src/main/kotlin/com/acme/Feed.kt") == "app"
    assert module_of("core/data/src/main/kotlin/Repo.kt") == "data"


def test_module_falls_back_past_source_roots() -> None:
    assert module_of("lib/features/feed/view.dart") == "features"
    assert module_of("main.py") == "."


def test_layer_prefers_the_most_specific_marker() -> None:
    assert layer_of("app/ui/FeedViewModel.kt") == "presentation"
    assert layer_of("app/domain/FeedModel.kt") == "domain"
    assert layer_of("app/data/FeedDao.kt") == "data"
    assert layer_of("tools/script.sh") == "other"


def test_impact_set_matches_symbols_and_groups_by_module() -> None:
    impact = impact_set("FeedViewModel stops paging after the second FeedRepository call", SYMBOLS)

    paths = [item["path"] for item in impact.files]
    assert "app/src/main/kotlin/feed/FeedViewModel.kt" in paths
    assert "data/src/main/kotlin/feed/FeedRepo.kt" in paths
    assert "app/src/main/kotlin/login/LoginScreen.kt" not in paths
    assert set(impact.modules) == {"app", "data"}


def test_impact_set_is_empty_without_usable_terms() -> None:
    impact = impact_set("it is not ok", SYMBOLS)

    assert impact.files == []


def test_split_orders_data_before_presentation() -> None:
    impact = impact_set("FeedViewModel and FeedRepository", SYMBOLS)

    increments = split_increments(impact)

    assert [item["layer"] for item in increments] == ["data", "presentation"]
    assert increments[0]["step"] == 1


def test_split_chunks_large_groups() -> None:
    symbols = [
        {"name": f"Feed{index}", "kind": "class", "path": f"app/src/main/kotlin/ui/Feed{index}.kt"}
        for index in range(10)
    ]
    impact = impact_set(" ".join(f"Feed{index}" for index in range(10)), symbols)

    increments = split_increments(impact, max_files=4)

    assert len(increments) == 3
    assert all(len(item["paths"]) <= 4 for item in increments)


def test_jaccard_handles_empty_sets() -> None:
    assert jaccard(set(), {"a"}) == 0.0
    assert jaccard({"a"}, {"a"}) == 1.0


def test_dedupe_groups_tickets_that_share_code() -> None:
    tickets = [
        {"id": "BUG-1", "title": "Feed stops loading", "body": "FeedViewModel never emits"},
        {"id": "BUG-2", "title": "Feed spinner", "body": "FeedViewModel stays loading"},
        {"id": "BUG-3", "title": "Login typo", "body": "LoginScreen copy is wrong"},
    ]

    clusters = cluster(build_impacts(tickets, SYMBOLS))

    assert len(clusters) == 1
    assert {item["id"] for item in clusters[0]["tickets"]} == {"BUG-1", "BUG-2"}


def test_dedupe_reports_shared_evidence() -> None:
    tickets = [
        {"id": "A", "title": "", "body": "FeedViewModel breaks"},
        {"id": "B", "title": "", "body": "FeedViewModel is stuck"},
    ]

    clusters = cluster(build_impacts(tickets, SYMBOLS))

    assert clusters[0]["sharedPaths"] == ["app/src/main/kotlin/feed/FeedViewModel.kt"]
    assert clusters[0]["pairs"][0]["evidence"]["sharedSymbols"] == ["FeedViewModel"]


def test_dedupe_respects_the_threshold() -> None:
    tickets = [
        {"id": "A", "title": "", "body": "FeedViewModel and FeedRepository"},
        {"id": "B", "title": "", "body": "FeedViewModel only"},
    ]

    assert cluster(build_impacts(tickets, SYMBOLS), threshold=0.99) == []


def test_safe_id_rejects_path_traversal() -> None:
    assert safe_id("ACME-12") == "ACME-12"
    assert safe_id("feature/ACME 12") == "feature-ACME-12"
    with pytest.raises(RepoTaskError):
        safe_id("../..")


def test_work_item_requires_a_fetched_source(project: Path) -> None:
    item = work_item(load_config(), "ACME-1")

    with pytest.raises(RepoTaskError, match="repo-task fetch"):
        item.require("source.md", "repo-task fetch ACME-1")


def test_work_item_round_trips_artifacts(project: Path) -> None:
    item = work_item(load_config(), "ACME-1")

    item.write("source.md", "# Ticket")
    item.write_json("analysis.json", {"ticket": "ACME-1"})
    item.touch_meta(kind="feature")

    assert item.read("source.md") == "# Ticket\n"
    assert item.read_json("analysis.json")["ticket"] == "ACME-1"
    assert item.read_json("meta.json")["kind"] == "feature"
