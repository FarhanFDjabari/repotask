"""Load and query the knowledge base layers."""

from __future__ import annotations

import json
import re
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

from repotask.errors import RepoTaskError
from repotask.kb.schema import (
    MANIFEST_NAME,
    Document,
    Manifest,
    Slice,
    load_document,
    load_manifest,
    load_slice,
)
from repotask.kb.source import KnowledgeSource

CONVENTIONS_DIR = "conventions"
RECIPES_DIR = "recipes"
FACTS_DIR = "facts"
SLICES_DIR = "slices"


@dataclass(frozen=True)
class SearchHit:
    document: Document
    score: int
    excerpt: str


@dataclass
class KnowledgeBase:
    source: KnowledgeSource
    manifest: Manifest
    conventions: dict[str, Document] = field(default_factory=dict)
    recipes: dict[str, Document] = field(default_factory=dict)
    slices: dict[str, Slice] = field(default_factory=dict)

    @property
    def documents(self) -> list[Document]:
        return [*self.conventions.values(), *self.recipes.values()]

    def document(self, doc_id: str) -> Document | None:
        return self.conventions.get(doc_id) or self.recipes.get(doc_id)

    def facts_path(self, family: str) -> Path:
        return self.source.path / FACTS_DIR / f"{family}.json"

    def facts(self, family: str) -> list[dict[str, Any]]:
        """Read a generated fact family. Missing families are empty, not an error —
        the project may simply not have been indexed yet."""
        path = self.facts_path(family)
        if not path.is_file():
            return []
        raw = json.loads(path.read_text(encoding="utf-8"))
        entries = raw.get("entries", raw) if isinstance(raw, dict) else raw
        return entries if isinstance(entries, list) else []

    def fact_families(self) -> list[str]:
        directory = self.source.path / FACTS_DIR
        if not directory.is_dir():
            return []
        return sorted(path.stem for path in directory.glob("*.json"))

    def slice_for(self, stacks: list[str]) -> Slice:
        """Merge every slice matching the project stacks, following `extends` chains."""
        seen: set[str] = set()
        conventions: list[str] = []
        recipes: list[str] = []
        facts: list[str] = []
        intents: dict[str, list[str]] = {}

        def visit(name: str) -> None:
            if name in seen:
                return
            seen.add(name)
            current = self.slices.get(name)
            if current is None:
                return
            for parent in current.extends:
                visit(parent)
            _extend(conventions, current.conventions)
            _extend(recipes, current.recipes)
            _extend(facts, current.facts)
            for intent, ids in current.intents.items():
                _extend(intents.setdefault(intent, []), ids)

        for stack in stacks:
            visit(stack)
        return Slice(
            stack="+".join(stacks),
            conventions=conventions,
            recipes=recipes,
            facts=facts,
            intents=intents,
        )

    def search(self, query: str, layer: str | None = None, limit: int = 20) -> list[SearchHit]:
        terms = [term for term in re.split(r"\W+", query.lower()) if term]
        if not terms:
            return []
        hits: list[SearchHit] = []
        for document in self.documents:
            if layer and document.layer != layer:
                continue
            score = _score(document, terms)
            if score:
                hits.append(SearchHit(document, score, _excerpt(document.body, terms)))
        hits.sort(key=lambda hit: (-hit.score, hit.document.id))
        return hits[:limit]


def load(source: KnowledgeSource) -> KnowledgeBase:
    manifest_path = source.path / MANIFEST_NAME
    if not manifest_path.is_file():
        raise RepoTaskError(
            f"{MANIFEST_NAME} not found in {source.path}. This does not look like a RepoTask "
            "knowledge base."
        )
    kb = KnowledgeBase(source=source, manifest=load_manifest(manifest_path))
    kb.conventions = _load_layer(source.path, CONVENTIONS_DIR, "convention")
    kb.recipes = _load_layer(source.path, RECIPES_DIR, "recipe")
    slices_dir = source.path / SLICES_DIR
    if slices_dir.is_dir():
        for path in sorted(slices_dir.glob("*.yaml")):
            current = load_slice(path)
            kb.slices[current.stack] = current
    return kb


def _load_layer(root: Path, directory: str, layer: str) -> dict[str, Document]:
    path = root / directory
    if not path.is_dir():
        return {}
    documents: dict[str, Document] = {}
    for file in sorted(path.rglob("*.md")):
        document = load_document(file, layer, root)  # type: ignore[arg-type]
        if document.id in documents:
            raise RepoTaskError(
                f"Duplicate knowledge document id '{document.id}' in {directory}: "
                f"{documents[document.id].path} and {document.path}"
            )
        documents[document.id] = document
    return documents


def _extend(target: list[str], values: list[str]) -> None:
    for value in values:
        if value not in target:
            target.append(value)


def _score(document: Document, terms: list[str]) -> int:
    """Weight matches by where they land: id and tags beat prose."""
    haystacks = (
        (document.id.lower(), 8),
        (document.title.lower(), 5),
        (" ".join(document.tags).lower(), 5),
        (document.body.lower(), 1),
    )
    total = 0
    for term in terms:
        for text, weight in haystacks:
            if term in text:
                total += weight
    return total


def _excerpt(body: str, terms: list[str], width: int = 200) -> str:
    lowered = body.lower()
    for term in terms:
        position = lowered.find(term)
        if position != -1:
            start = max(0, position - width // 2)
            return body[start : start + width].strip().replace("\n", " ")
    return body[:width].strip().replace("\n", " ")
