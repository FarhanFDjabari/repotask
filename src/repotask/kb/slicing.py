"""Select and rank the knowledge that applies to one piece of work.

Ranking inputs, highest weight first:
  * the document is named by the stack slice for this intent
  * the document is named by the stack slice at all
  * the document declares one of the project stacks
  * intent words hit the document's tags, title, or id
  * the document's `applies_to` globs hit the changed paths

Documents that match nothing are dropped, not merely ranked low: an Android task
should never pay for the iOS conventions.
"""

from __future__ import annotations

import fnmatch
import re
from dataclasses import dataclass, field
from typing import Any

from repotask.kb.budget import estimate, fit
from repotask.kb.schema import Document
from repotask.kb.store import KnowledgeBase

WEIGHT_INTENT_SLICE = 100
WEIGHT_SLICE = 50
WEIGHT_STACK = 20
WEIGHT_TAG = 10
WEIGHT_TITLE = 5
WEIGHT_PATH = 15

FACT_ENTRY_LIMIT = 200
FACT_BUDGET_SHARE = 0.4


@dataclass(frozen=True)
class RankedDocument:
    document: Document
    score: int
    reasons: list[str]


@dataclass
class ContextPack:
    intent: str
    budget: int
    used: int = 0
    documents: list[dict[str, Any]] = field(default_factory=list)
    facts: dict[str, list[dict[str, Any]]] = field(default_factory=dict)
    omitted: list[str] = field(default_factory=list)

    def as_dict(self) -> dict[str, Any]:
        return {
            "intent": self.intent,
            "budget": self.budget,
            "used": self.used,
            "documents": self.documents,
            "facts": self.facts,
            "omitted": self.omitted,
        }


def rank(
    kb: KnowledgeBase,
    stacks: list[str],
    intent: str,
    changed_paths: list[str] | None = None,
) -> list[RankedDocument]:
    slice_ = kb.slice_for(stacks)
    intent_ids = set(slice_.intents.get(intent, []))
    slice_ids = set(slice_.conventions) | set(slice_.recipes)
    words = _words(intent)
    paths = changed_paths or []

    ranked: list[RankedDocument] = []
    for document in kb.documents:
        score = 0
        reasons: list[str] = []
        if document.id in intent_ids:
            score += WEIGHT_INTENT_SLICE
            reasons.append(f"slice:{intent}")
        if document.id in slice_ids:
            score += WEIGHT_SLICE
            reasons.append("slice")
        if document.stacks and set(document.stacks) & set(stacks):
            score += WEIGHT_STACK
            reasons.append("stack")
        tag_hits = words & {tag.lower() for tag in document.tags}
        if tag_hits:
            score += WEIGHT_TAG * len(tag_hits)
            reasons.append("tags:" + ",".join(sorted(tag_hits)))
        if words & _words(f"{document.id} {document.title}"):
            score += WEIGHT_TITLE
            reasons.append("title")
        path_hits = _path_hits(document, paths)
        if path_hits:
            score += WEIGHT_PATH
            reasons.append("paths:" + ",".join(path_hits[:3]))
        if score:
            ranked.append(RankedDocument(document, score, reasons))

    ranked.sort(key=lambda item: (-item.score, item.document.layer, item.document.id))
    return ranked


def build_pack(
    kb: KnowledgeBase,
    stacks: list[str],
    intent: str,
    budget: int,
    changed_paths: list[str] | None = None,
) -> ContextPack:
    """Assemble a budgeted context pack: ranked documents first, then project facts."""
    pack = ContextPack(intent=intent, budget=budget)
    ranked = rank(kb, stacks, intent, changed_paths)

    fact_families = kb.slice_for(stacks).facts
    available = {name: kb.facts(name)[:FACT_ENTRY_LIMIT] for name in fact_families}
    available = {name: entries for name, entries in available.items() if entries}
    facts, facts_cost, dropped = _fit_facts(available, budget)
    pack.omitted.extend(f"facts:{name}" for name in dropped)

    remaining = max(0, budget - facts_cost)
    for item in ranked:
        document = item.document
        if remaining <= 0:
            pack.omitted.append(document.id)
            continue
        body, truncated = fit(
            document.body,
            remaining,
            command=document.layer,
            doc_id=document.id,
        )
        if not body:
            pack.omitted.append(document.id)
            continue
        used = estimate(body)
        remaining -= used
        pack.used += used
        pack.documents.append(
            {
                "id": document.id,
                "title": document.title,
                "layer": document.layer,
                "path": document.path,
                "score": item.score,
                "reasons": item.reasons,
                "truncated": truncated,
                "body": body,
            }
        )

    pack.facts = facts
    pack.used += facts_cost
    return pack


def _fit_facts(
    families: dict[str, list[dict[str, Any]]], budget: int
) -> tuple[dict[str, list[dict[str, Any]]], int, list[str]]:
    """Cap facts at a share of the budget so documents always keep room."""
    allowance = int(budget * FACT_BUDGET_SHARE)
    kept: dict[str, list[dict[str, Any]]] = {}
    dropped: list[str] = []
    used = 0
    for name, entries in families.items():
        taken: list[dict[str, Any]] = []
        for entry in entries:
            cost = estimate(str(entry))
            if used + cost > allowance:
                break
            taken.append(entry)
            used += cost
        if taken:
            kept[name] = taken
        if len(taken) < len(entries):
            dropped.append(name)
    return kept, used, dropped


def _words(text: str) -> set[str]:
    return {word for word in re.split(r"\W+", text.lower()) if len(word) > 2}


def _path_hits(document: Document, paths: list[str]) -> list[str]:
    return [
        pattern
        for pattern in document.applies_to
        if any(fnmatch.fnmatch(path, pattern) for path in paths)
    ]
