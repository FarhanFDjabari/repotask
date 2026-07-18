"""`repo-task brief` — one budgeted context pack for a piece of work."""

from __future__ import annotations

from typing import Any

import typer
from rich.console import Group, RenderableType
from rich.markdown import Markdown
from rich.table import Table

from repotask.commands._context import open_knowledge
from repotask.git import changed_paths as git_changed_paths
from repotask.kb.slicing import build_pack
from repotask.output import emit, guard


@guard
def brief(
    intent: str = typer.Argument(
        ..., help="What you are about to do, e.g. 'add pagination to the feed'."
    ),
    paths: list[str] = typer.Option(
        [], "--path", help="Relevant file paths. Repeatable. Sharpens ranking."
    ),
    changed: bool = typer.Option(
        False, "--changed", help="Use files changed against the base branch as the paths."
    ),
    budget: int = typer.Option(0, "--budget", help="Token ceiling. Defaults to the project value."),
) -> None:
    """Assemble the conventions, recipes, and project facts that apply to this work.

    This is the command an agent should call first: it replaces reading the whole
    knowledge base with the slice that actually applies.
    """
    config, kb = open_knowledge(sync=False)
    relevant = list(paths)
    if changed:
        relevant.extend(
            git_changed_paths(config.root, config.project.base_branch, include_worktree=True)
        )
    limit = budget or config.brief.default_budget or kb.manifest.default_budget
    pack = build_pack(kb, config.project.stacks, intent, limit, relevant)

    data = pack.as_dict()
    data["stacks"] = config.project.stacks
    data["paths"] = relevant
    emit("brief", data, _render)


def _render(data: dict[str, Any]) -> RenderableType:
    header = Table(title=f"Context pack: {data['intent']}", show_header=False)
    header.add_row("Stacks", ", ".join(data["stacks"]))
    header.add_row("Budget", f"{data['used']} / {data['budget']} tokens")
    header.add_row("Documents", str(len(data["documents"])))
    header.add_row(
        "Facts",
        ", ".join(f"{name} ({len(entries)})" for name, entries in data["facts"].items()) or "none",
    )
    if data["omitted"]:
        header.add_row("Omitted", ", ".join(data["omitted"]))

    parts: list[RenderableType] = [header]
    for document in data["documents"]:
        reasons = ", ".join(document["reasons"])
        parts.append(
            Markdown(
                f"# {document['title']}\n\n_{document['path']} — matched on {reasons}_\n\n"
                f"{document['body']}"
            )
        )
    return Group(*parts)
