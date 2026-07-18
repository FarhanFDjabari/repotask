"""Cluster bug tickets that point at the same code.

Two tickets that describe different symptoms but resolve to the same files and
symbols usually share a root cause. The CLI surfaces that overlap as evidence;
the decision to merge stays with a human.
"""

from __future__ import annotations

from dataclasses import dataclass
from itertools import combinations
from typing import Any

from repotask.workflow.analyze import Impact, impact_set

MIN_SIMILARITY = 0.34


@dataclass(frozen=True)
class TicketImpact:
    ticket_id: str
    title: str
    impact: Impact

    @property
    def paths(self) -> set[str]:
        return {item["path"] for item in self.impact.files}

    @property
    def symbols(self) -> set[str]:
        return {f"{item.get('path')}:{item.get('name')}" for item in self.impact.symbols}


def jaccard(left: set[str], right: set[str]) -> float:
    if not left or not right:
        return 0.0
    union = left | right
    return len(left & right) / len(union)


def similarity(left: TicketImpact, right: TicketImpact) -> tuple[float, dict[str, Any]]:
    """Blend file and symbol overlap; symbol agreement is the stronger signal."""
    path_score = jaccard(left.paths, right.paths)
    symbol_score = jaccard(left.symbols, right.symbols)
    score = round(0.4 * path_score + 0.6 * symbol_score, 3)
    evidence = {
        "sharedPaths": sorted(left.paths & right.paths),
        "sharedSymbols": sorted(
            {item.split(":", 1)[1] for item in left.symbols & right.symbols}
        ),
        "pathSimilarity": round(path_score, 3),
        "symbolSimilarity": round(symbol_score, 3),
    }
    return score, evidence


def build_impacts(
    tickets: list[dict[str, str]], symbols: list[dict[str, Any]]
) -> list[TicketImpact]:
    return [
        TicketImpact(
            ticket_id=ticket["id"],
            title=ticket.get("title", ""),
            impact=impact_set(f"{ticket.get('title', '')}\n{ticket.get('body', '')}", symbols),
        )
        for ticket in tickets
    ]


def cluster(
    impacts: list[TicketImpact], threshold: float = MIN_SIMILARITY
) -> list[dict[str, Any]]:
    """Single-linkage clustering over the similarity graph."""
    pairs = []
    for left, right in combinations(impacts, 2):
        score, evidence = similarity(left, right)
        if score >= threshold:
            pairs.append((left.ticket_id, right.ticket_id, score, evidence))

    parent = {item.ticket_id: item.ticket_id for item in impacts}

    def find(node: str) -> str:
        while parent[node] != node:
            parent[node] = parent[parent[node]]
            node = parent[node]
        return node

    for left_id, right_id, _score, _evidence in pairs:
        left_root, right_root = find(left_id), find(right_id)
        if left_root != right_root:
            parent[right_root] = left_root

    by_id = {item.ticket_id: item for item in impacts}
    groups: dict[str, list[str]] = {}
    for ticket_id in parent:
        groups.setdefault(find(ticket_id), []).append(ticket_id)

    clusters = []
    for members in groups.values():
        if len(members) < 2:
            continue
        member_pairs = [
            {
                "tickets": [left_id, right_id],
                "score": score,
                "evidence": evidence,
            }
            for left_id, right_id, score, evidence in pairs
            if left_id in members and right_id in members
        ]
        shared_paths = set.intersection(*(by_id[item].paths for item in members))
        clusters.append(
            {
                "tickets": [
                    {"id": item, "title": by_id[item].title} for item in sorted(members)
                ],
                "sharedPaths": sorted(shared_paths),
                "pairs": sorted(member_pairs, key=lambda pair: -pair["score"]),
                "topScore": max(pair["score"] for pair in member_pairs),
            }
        )
    return sorted(clusters, key=lambda item: -item["topScore"])
