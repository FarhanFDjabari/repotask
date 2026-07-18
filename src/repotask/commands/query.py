"""Direct knowledge lookups: `convention`, `recipe`, `search`."""

from __future__ import annotations

from typing import Any

import typer
from rich.console import Group, RenderableType
from rich.markdown import Markdown
from rich.table import Table

from repotask.commands._context import open_knowledge
from repotask.errors import RepoTaskError
from repotask.kb.schema import Document
from repotask.kb.store import KnowledgeBase
from repotask.output import emit, guard


def _document_data(document: Document) -> dict[str, Any]:
    return {
        "id": document.id,
        "title": document.title,
        "layer": document.layer,
        "stacks": document.stacks,
        "tags": document.tags,
        "path": document.path,
        "body": document.body,
    }


def _render_documents(data: dict[str, Any]) -> RenderableType:
    parts: list[RenderableType] = []
    for item in data["documents"]:
        parts.append(Markdown(f"# {item['title']}\n\n_{item['path']}_\n\n{item['body']}"))
    return Group(*parts) if parts else "No matching documents."


def _lookup(kb: KnowledgeBase, layer: str, topic: str, stacks: list[str]) -> list[Document]:
    """Exact id first; otherwise fall back to a stack-filtered search."""
    pool = kb.conventions if layer == "convention" else kb.recipes
    exact = pool.get(topic)
    if exact is not None:
        return [exact]
    hits = kb.search(topic, layer=layer)
    scoped = [
        hit.document
        for hit in hits
        if not hit.document.stacks or set(hit.document.stacks) & set(stacks)
    ]
    return scoped or [hit.document for hit in hits]


@guard
def convention(
    topic: str = typer.Argument(..., help="Convention id or search topic."),
    limit: int = typer.Option(3, "--limit", help="Maximum documents to return."),
) -> None:
    """Read architecture and stack conventions for this project."""
    config, kb = open_knowledge()
    documents = _lookup(kb, "convention", topic, config.project.stacks)[:limit]
    if not documents:
        raise RepoTaskError(
            f"No convention matched '{topic}'. Try `repo-task search {topic}`."
        )
    emit(
        "convention",
        {"topic": topic, "documents": [_document_data(item) for item in documents]},
        _render_documents,
    )


@guard
def recipe(
    task: str = typer.Argument(..., help="Recipe id or the task you want a playbook for."),
    limit: int = typer.Option(3, "--limit", help="Maximum documents to return."),
) -> None:
    """Read the project playbook for a specific task."""
    config, kb = open_knowledge()
    documents = _lookup(kb, "recipe", task, config.project.stacks)[:limit]
    if not documents:
        raise RepoTaskError(f"No recipe matched '{task}'. Try `repo-task search {task}`.")
    emit(
        "recipe",
        {"task": task, "documents": [_document_data(item) for item in documents]},
        _render_documents,
    )


@guard
def search(
    query: str = typer.Argument(..., help="Keywords to search across the knowledge base."),
    layer: str = typer.Option("", "--layer", help="Restrict to `convention` or `recipe`."),
    limit: int = typer.Option(20, "--limit"),
) -> None:
    """Search conventions and recipes; returns ids to read with `convention` or `recipe`."""
    if layer and layer not in {"convention", "recipe"}:
        raise RepoTaskError("--layer must be `convention` or `recipe`.")
    _config, kb = open_knowledge()
    hits = kb.search(query, layer=layer or None, limit=limit)
    data = {
        "query": query,
        "hits": [
            {
                "id": hit.document.id,
                "title": hit.document.title,
                "layer": hit.document.layer,
                "stacks": hit.document.stacks,
                "tags": hit.document.tags,
                "score": hit.score,
                "excerpt": hit.excerpt,
            }
            for hit in hits
        ],
    }
    emit("search", data, _render_hits)


def _render_hits(data: dict[str, Any]) -> RenderableType:
    if not data["hits"]:
        return f"No knowledge matched '{data['query']}'."
    table = Table(title=f"Knowledge matching '{data['query']}'")
    table.add_column("id")
    table.add_column("layer")
    table.add_column("title")
    table.add_column("excerpt", overflow="fold")
    for hit in data["hits"]:
        table.add_row(hit["id"], hit["layer"], hit["title"], hit["excerpt"])
    return table
