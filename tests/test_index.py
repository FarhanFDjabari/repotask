from __future__ import annotations

from pathlib import Path

import pytest

from repotask.config import load_config
from repotask.errors import RepoTaskError
from repotask.index import build_all, build_index
from repotask.index.families import SYMBOLS_FAMILY
from repotask.index.languages import language_for
from repotask.kb import resolve
from repotask.kb.schema import FactFamily
from repotask.kb.store import load

KOTLIN = """package com.acme.feed

class FeedViewModel(private val repo: FeedRepository) {
    fun load() {}
}

class FeedRepository {
    suspend fun fetch(): List<String> = emptyList()
}
"""

SWIFT = """import Foundation

class FeedViewModel: ObservableObject {
    func load() {}
}
"""

PYTHON = """class Loader:
    def load(self) -> None:
        pass


def helper() -> int:
    return 1
"""


def write(root: Path, relative: str, content: str) -> Path:
    path = root / relative
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")
    return path


def index_project(root: Path):
    write(root, "app/src/main/kotlin/com/acme/feed/FeedViewModel.kt", KOTLIN)
    return build_index(load_config())


def test_language_detection_by_suffix() -> None:
    assert language_for(".kt") == "kotlin"
    assert language_for(".TSX") == "tsx"
    assert language_for(".md") is None


def test_extracts_kotlin_classes_and_functions_with_scope(project: Path) -> None:
    result = index_project(project)

    found = {(symbol.name, symbol.kind, symbol.scope) for symbol in result.symbols}
    assert ("FeedViewModel", "class", "") in found
    assert ("FeedRepository", "class", "") in found
    assert ("load", "function", "FeedViewModel") in found
    assert ("fetch", "function", "FeedRepository") in found


def test_records_line_numbers(project: Path) -> None:
    result = index_project(project)

    viewmodel = next(item for item in result.symbols if item.name == "FeedViewModel")
    assert viewmodel.line == 3


def test_indexes_multiple_languages(project: Path) -> None:
    write(project, "ios/FeedViewModel.swift", SWIFT)
    write(project, "tools/loader.py", PYTHON)

    result = index_project(project)

    assert set(result.languages) == {"kotlin", "swift", "python"}
    assert any(item.name == "Loader" and item.language == "python" for item in result.symbols)


def test_excluded_paths_are_not_indexed(project: Path) -> None:
    write(project, "app/build/generated/Junk.kt", "class Junk {}\n")

    result = index_project(project)

    assert all("build/generated" not in symbol.path for symbol in result.symbols)


def test_oversized_files_are_skipped(project: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    write(project, "app/Huge.kt", "class Huge {}\n" + "// filler\n" * 200_000)

    result = index_project(project)

    assert result.files_skipped >= 1
    assert all(symbol.name != "Huge" for symbol in result.symbols)


def test_fact_family_filters_symbols_by_pattern_and_language(project: Path) -> None:
    result = index_project(project)
    kb = load(resolve(load_config(), sync=True))

    families = build_all(kb.manifest.fact_families, result)

    assert [entry["name"] for entry in families["viewmodels"]] == ["FeedViewModel"]
    assert len(families[SYMBOLS_FAMILY]) == len(result.symbols)


def test_fact_family_cannot_shadow_the_symbols_family(project: Path) -> None:
    result = index_project(project)
    reserved = FactFamily(name=SYMBOLS_FAMILY)

    with pytest.raises(RepoTaskError, match="reserved"):
        build_all([reserved], result)


def test_invalid_name_pattern_is_reported(project: Path) -> None:
    result = index_project(project)
    broken = FactFamily(name="broken", name_pattern="(unclosed")

    with pytest.raises(RepoTaskError, match="invalid name_pattern"):
        build_all([broken], result)


def test_incremental_index_covers_only_named_paths(project: Path) -> None:
    write(project, "app/A.kt", "class Alpha {}\n")
    write(project, "app/B.kt", "class Beta {}\n")

    result = build_index(load_config(), ["app/A.kt"])

    assert {symbol.name for symbol in result.symbols} == {"Alpha"}
