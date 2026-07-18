"""Project fact commands: `index`, `symbol`, `fact`."""

from __future__ import annotations

import json
import re
from typing import Any

import typer
from rich.console import RenderableType
from rich.table import Table

from repotask.commands._context import open_knowledge
from repotask.errors import RepoTaskError
from repotask.files import write_text
from repotask.git import changed_paths as git_changed_paths
from repotask.index import build_all, build_index
from repotask.index.families import SYMBOLS_FAMILY
from repotask.kb.store import FACTS_DIR
from repotask.output import emit, guard


@guard
def index(
    changed_only: bool = typer.Option(
        False, "--changed-only", help="Index only files changed against the base branch."
    ),
) -> None:
    """Scan the project and write fact families into the knowledge base."""
    config, kb = open_knowledge(sync=False)
    paths = git_changed_paths(config.root, config.project.base_branch, include_worktree=True)
    if changed_only and not paths:
        raise RepoTaskError(
            f"No files changed against {config.project.base_branch}; nothing to index."
        )
    result = build_index(config, paths if changed_only else None)
    families = build_all(kb.manifest.fact_families, result)

    written: list[str] = []
    for name, entries in families.items():
        path = kb.facts_path(name)
        write_text(
            path,
            json.dumps(
                {
                    "family": name,
                    "project": config.project.name,
                    "count": len(entries),
                    "entries": entries,
                },
                indent=2,
            )
            + "\n",
        )
        written.append(f"{FACTS_DIR}/{name}.json")

    data = {
        "filesScanned": result.files_scanned,
        "filesSkipped": result.files_skipped,
        "languages": result.languages,
        "symbols": len(result.symbols),
        "families": {name: len(entries) for name, entries in families.items()},
        "written": written,
        "knowledgeBase": str(kb.source.path),
        "nextSteps": (
            ["repo-task kb propose"]
            if kb.source.kind == "remote"
            else ["Commit the facts/ changes"]
        ),
    }
    emit("index", data, _render_index)


def _render_index(data: dict[str, Any]) -> RenderableType:
    table = Table(title="Indexed project facts")
    table.add_column("family")
    table.add_column("entries", justify="right")
    for name, count in sorted(data["families"].items()):
        table.add_row(name, str(count))
    return table


@guard
def symbol(
    query: str = typer.Argument(..., help="Substring or regular expression to match."),
    kind: str = typer.Option("", "--kind", help="Restrict to one declaration kind."),
    limit: int = typer.Option(50, "--limit"),
) -> None:
    """Search indexed declarations by name."""
    _config, kb = open_knowledge(sync=False)
    entries = kb.facts(SYMBOLS_FAMILY)
    if not entries:
        raise RepoTaskError("No symbol index found. Run `repo-task index` first.")
    try:
        pattern = re.compile(query, re.IGNORECASE)
    except re.error as error:
        raise RepoTaskError(f"Invalid search pattern: {error}") from error

    hits = [
        entry
        for entry in entries
        if pattern.search(str(entry.get("name", ""))) and (not kind or entry.get("kind") == kind)
    ][:limit]
    emit("symbol", {"query": query, "kind": kind, "hits": hits}, _render_symbols)


def _render_symbols(data: dict[str, Any]) -> RenderableType:
    if not data["hits"]:
        return f"No symbol matched '{data['query']}'."
    table = Table(title=f"Symbols matching '{data['query']}'")
    for column in ("name", "kind", "path", "line"):
        table.add_column(column, overflow="fold")
    for hit in data["hits"]:
        table.add_row(hit["name"], hit["kind"], hit["path"], str(hit["line"]))
    return table


@guard
def fact(
    family: str = typer.Argument("", help="Fact family name. Omit to list available families."),
    query: str = typer.Option("", "--query", help="Filter entries by name substring."),
    limit: int = typer.Option(100, "--limit"),
) -> None:
    """Read a curated project fact family produced by `index`."""
    _config, kb = open_knowledge(sync=False)
    available = kb.fact_families()
    if not family:
        emit(
            "fact",
            {
                "families": [
                    {
                        "name": item.name,
                        "description": item.description,
                        "indexed": item.name in available,
                    }
                    for item in kb.manifest.fact_families
                ],
                "available": available,
            },
            _render_families,
        )
        return

    if family not in available:
        raise RepoTaskError(
            f"Fact family '{family}' has not been indexed. Available: "
            f"{', '.join(available) or 'none'}. Run `repo-task index`."
        )
    entries = kb.facts(family)
    if query:
        lowered = query.lower()
        entries = [entry for entry in entries if lowered in str(entry.get("name", "")).lower()]
    emit(
        "fact",
        {"family": family, "count": len(entries), "entries": entries[:limit]},
        _render_fact,
    )


def _render_families(data: dict[str, Any]) -> RenderableType:
    table = Table(title="Fact families")
    table.add_column("name")
    table.add_column("indexed")
    table.add_column("description", overflow="fold")
    for item in data["families"]:
        table.add_row(item["name"], "yes" if item["indexed"] else "no", item["description"])
    return table


def _render_fact(data: dict[str, Any]) -> RenderableType:
    if not data["entries"]:
        return f"No entries in '{data['family']}'."
    table = Table(title=f"{data['family']} ({data['count']})")
    for column in ("name", "kind", "path", "line"):
        table.add_column(column, overflow="fold")
    for entry in data["entries"]:
        table.add_row(
            str(entry.get("name", "")),
            str(entry.get("kind", "")),
            str(entry.get("path", "")),
            str(entry.get("line", "")),
        )
    return table
