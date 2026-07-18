"""Deterministic impact analysis and increment splitting.

Neither step estimates or decides anything: they turn ticket text into evidence
(which symbols and files the words point at, grouped into shippable slices) and
hand that to the agent with the rubric it should apply.
"""

from __future__ import annotations

import re
from collections import Counter, defaultdict
from dataclasses import dataclass, field
from typing import Any

STOP_WORDS = {
    "the", "and", "for", "with", "that", "this", "from", "into", "when", "then", "than",
    "should", "would", "could", "must", "will", "shall", "user", "users", "app", "page",
    "screen", "feature", "bug", "issue", "ticket", "task", "add", "remove", "update",
    "change", "fix", "make", "need", "needs", "want", "have", "has", "not", "but", "all",
    "any", "our", "their", "them", "they", "you", "your", "are", "was", "were", "been",
}

# Layer order for incremental delivery: data lands before the UI that consumes it.
LAYER_ORDER = (
    ("data", ("data", "repository", "repo", "network", "api", "db", "database", "dao")),
    ("domain", ("domain", "usecase", "use_case", "interactor", "model", "entity")),
    ("presentation", ("ui", "view", "screen", "compose", "widget", "viewmodel", "presenter")),
)
DEFAULT_LAYER = "other"


@dataclass
class Impact:
    terms: list[str]
    files: list[dict[str, Any]] = field(default_factory=list)
    symbols: list[dict[str, Any]] = field(default_factory=list)
    modules: dict[str, int] = field(default_factory=dict)

    def as_dict(self) -> dict[str, Any]:
        return {
            "terms": self.terms,
            "files": self.files,
            "symbols": self.symbols,
            "modules": self.modules,
        }


def keywords(text: str, limit: int = 25) -> list[str]:
    """Pull likely code-bearing terms out of prose: identifiers first, then salient words."""
    identifiers = re.findall(r"\b[A-Z][a-zA-Z0-9]*(?:[A-Z][a-zA-Z0-9]*)+\b", text)
    backticked = re.findall(r"`([^`]+)`", text)
    words = [
        word
        for word in re.findall(r"\b[a-zA-Z][a-zA-Z0-9_]{3,}\b", text)
        if word.lower() not in STOP_WORDS
    ]
    ordered: list[str] = []
    for term in [*identifiers, *backticked, *words]:
        cleaned = term.strip()
        if cleaned and cleaned.lower() not in {item.lower() for item in ordered}:
            ordered.append(cleaned)
    return ordered[:limit]


def impact_set(text: str, symbols: list[dict[str, Any]], term_limit: int = 25) -> Impact:
    """Match ticket vocabulary against the symbol index."""
    terms = keywords(text, term_limit)
    if not terms:
        return Impact(terms=[])

    patterns = [(term, re.compile(re.escape(term), re.IGNORECASE)) for term in terms]
    file_hits: Counter[str] = Counter()
    file_terms: dict[str, set[str]] = defaultdict(set)
    matched: list[dict[str, Any]] = []

    for entry in symbols:
        name = str(entry.get("name", ""))
        hits = [term for term, pattern in patterns if pattern.search(name)]
        if not hits:
            continue
        path = str(entry.get("path", ""))
        file_hits[path] += len(hits)
        file_terms[path].update(hits)
        matched.append({**entry, "matchedTerms": hits})

    files = [
        {
            "path": path,
            "score": score,
            "terms": sorted(file_terms[path]),
            "module": module_of(path),
            "layer": layer_of(path),
        }
        for path, score in file_hits.most_common()
    ]
    modules: Counter[str] = Counter(item["module"] for item in files)
    return Impact(terms=terms, files=files, symbols=matched, modules=dict(modules))


SOURCE_ROOT_DIRS = {"src", "main", "lib", "app", "java", "kotlin", "swift", "dart", "sources"}


def module_of(path: str) -> str:
    """The Gradle/SPM-style module a file belongs to.

    `app/src/main/kotlin/...` is the `app` module: the segment before `src` wins.
    Layouts without a `src` fall back to the first segment that is not a language
    or source-root directory.
    """
    parts = [part for part in path.split("/") if part]
    if len(parts) < 2:
        return "."
    if "src" in parts:
        index = parts.index("src")
        if index > 0:
            return parts[index - 1]
        parts = parts[index + 1 :]
    for part in parts[:-1]:
        if part not in SOURCE_ROOT_DIRS:
            return part
    return parts[0]


def layer_of(path: str) -> str:
    """Longest matching marker wins, so `viewmodel` beats `model`."""
    lowered = path.lower()
    best_layer = DEFAULT_LAYER
    best_length = 0
    for layer, markers in LAYER_ORDER:
        for marker in markers:
            if marker in lowered and len(marker) > best_length:
                best_layer = layer
                best_length = len(marker)
    return best_layer


def split_increments(impact: Impact, max_files: int = 8) -> list[dict[str, Any]]:
    """Group impacted files into reviewable steps: by module, then by layer, data first."""
    order = {name: position for position, (name, _) in enumerate(LAYER_ORDER)}
    order[DEFAULT_LAYER] = len(order)

    grouped: dict[tuple[str, str], list[dict[str, Any]]] = defaultdict(list)
    for item in impact.files:
        grouped[(item["module"], item["layer"])].append(item)

    increments: list[dict[str, Any]] = []
    for (module, layer), items in sorted(
        grouped.items(), key=lambda entry: (order[entry[0][1]], entry[0][0])
    ):
        for offset in range(0, len(items), max_files):
            chunk = items[offset : offset + max_files]
            suffix = "" if len(items) <= max_files else f" (part {offset // max_files + 1})"
            increments.append(
                {
                    "step": len(increments) + 1,
                    "title": f"{module}: {layer} changes{suffix}",
                    "module": module,
                    "layer": layer,
                    "paths": [item["path"] for item in chunk],
                    "terms": sorted({term for item in chunk for term in item["terms"]}),
                }
            )
    return increments
