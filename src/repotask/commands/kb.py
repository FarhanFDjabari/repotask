"""`repo-task kb` — knowledge base source management."""

from __future__ import annotations

from typing import Any

import typer
from rich.console import RenderableType
from rich.table import Table

from repotask.commands._context import open_knowledge
from repotask.config import load_config
from repotask.errors import RepoTaskError
from repotask.git import run_git
from repotask.kb import resolve
from repotask.kb.store import FACTS_DIR, load
from repotask.output import emit, guard

app = typer.Typer(no_args_is_help=True)


def _source_data(config_stacks: list[str], kb: Any) -> dict[str, Any]:
    slice_ = kb.slice_for(config_stacks)
    return {
        "source": {
            "kind": kb.source.kind,
            "path": str(kb.source.path),
            "remote": kb.source.remote,
            "ref": kb.source.ref,
            "revision": kb.source.revision,
            "syncedAt": kb.source.synced_at,
        },
        "manifest": {
            "name": kb.manifest.name,
            "schemaVersion": kb.manifest.schema_version,
            "defaultBudget": kb.manifest.default_budget,
        },
        "counts": {
            "conventions": len(kb.conventions),
            "recipes": len(kb.recipes),
            "slices": len(kb.slices),
            "factFamilies": len(kb.fact_families()),
        },
        "stacks": config_stacks,
        "resolvedSlice": {
            "conventions": slice_.conventions,
            "recipes": slice_.recipes,
            "facts": slice_.facts,
        },
    }


def _render(data: dict[str, Any]) -> RenderableType:
    table = Table(title=f"Knowledge base: {data['manifest']['name']}", show_header=False)
    source = data["source"]
    table.add_row("Kind", source["kind"])
    table.add_row("Path", source["path"])
    if source["remote"]:
        table.add_row("Remote", f"{source['remote']} @ {source['ref']}")
        table.add_row("Revision", source["revision"][:12])
        table.add_row("Synced", source["syncedAt"] or "never")
    counts = data["counts"]
    table.add_row("Conventions", str(counts["conventions"]))
    table.add_row("Recipes", str(counts["recipes"]))
    table.add_row("Fact families", str(counts["factFamilies"]))
    table.add_row("Project stacks", ", ".join(data["stacks"]))
    return table


@app.command()
@guard
def sync() -> None:
    """Clone or fetch the knowledge base and pin it to the configured ref."""
    config = load_config()
    kb = load(resolve(config, sync=True))
    emit("kb.sync", _source_data(config.project.stacks, kb), _render)


@app.command()
@guard
def status() -> None:
    """Show the resolved knowledge base without touching the network."""
    config, kb = open_knowledge(sync=False)
    emit("kb.status", _source_data(config.project.stacks, kb), _render)


@app.command()
@guard
def propose(
    message: str = typer.Option("chore: refresh project facts", "--message", "-m"),
    branch: str = typer.Option("", "--branch", help="Branch name; defaults to a timestamped one."),
) -> None:
    """Stage generated facts in the knowledge base worktree and print PR instructions.

    Never pushes: a human opens the PR/MR so the knowledge base stays reviewed.
    """
    from datetime import datetime, timezone

    config, kb = open_knowledge(sync=False)
    if kb.source.kind != "remote":
        raise RepoTaskError(
            "`kb propose` applies to a remote knowledge base. A local knowledge base is "
            "committed with the project itself."
        )
    path = kb.source.path
    if not run_git(["status", "--porcelain", "--", FACTS_DIR], path).strip():
        raise RepoTaskError("No fact changes to propose. Run `repo-task index` first.")

    stamp = datetime.now(timezone.utc).strftime("%Y%m%d-%H%M%S")
    target = branch or f"facts/{config.project.name}-{stamp}"
    run_git(["checkout", "-B", target], path)
    run_git(["add", "--", FACTS_DIR], path)
    run_git(["commit", "-m", message], path)

    data = {
        "branch": target,
        "path": str(path),
        "remote": kb.source.remote,
        "commit": run_git(["rev-parse", "HEAD"], path).strip(),
        "nextSteps": [
            f"git -C {path} push -u origin {target}",
            "Open a pull/merge request against the knowledge base default branch.",
        ],
    }
    emit(
        "kb.propose",
        data,
        lambda payload: "\n".join(
            [f"Committed facts on {payload['branch']}", *payload["nextSteps"]]
        ),
    )
