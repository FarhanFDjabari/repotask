"""Project facts: curated views over the symbol index, declared in kb.yaml.

A fact family is a filter, not a parser. The knowledge base decides that
"viewmodels" means Kotlin classes named `*ViewModel`; the CLI just applies it.
"""

from __future__ import annotations

import fnmatch
import re
from dataclasses import asdict
from typing import Any

from repotask.errors import RepoTaskError
from repotask.index.runner import IndexResult, Symbol
from repotask.kb.schema import FactFamily

SYMBOLS_FAMILY = "symbols"


def matches(family: FactFamily, symbol: Symbol, pattern: re.Pattern[str] | None) -> bool:
    if family.languages and symbol.language not in family.languages:
        return False
    if family.kinds and symbol.kind not in family.kinds:
        return False
    if family.path_pattern and not fnmatch.fnmatch(symbol.path, family.path_pattern):
        return False
    if pattern is not None and not pattern.search(symbol.name):
        return False
    return True


def collect(family: FactFamily, index: IndexResult) -> list[dict[str, Any]]:
    pattern = _compile(family)
    return [asdict(symbol) for symbol in index.symbols if matches(family, symbol, pattern)]


def build_all(families: list[FactFamily], index: IndexResult) -> dict[str, list[dict[str, Any]]]:
    """Every declared family plus the raw `symbols` family the index always provides."""
    result: dict[str, list[dict[str, Any]]] = {
        SYMBOLS_FAMILY: [asdict(symbol) for symbol in index.symbols]
    }
    for family in families:
        if family.name == SYMBOLS_FAMILY:
            raise RepoTaskError(f"'{SYMBOLS_FAMILY}' is reserved and cannot be redeclared.")
        result[family.name] = collect(family, index)
    return result


def _compile(family: FactFamily) -> re.Pattern[str] | None:
    if not family.name_pattern:
        return None
    try:
        return re.compile(family.name_pattern)
    except re.error as error:
        raise RepoTaskError(
            f"Fact family '{family.name}' has an invalid name_pattern: {error}"
        ) from error
