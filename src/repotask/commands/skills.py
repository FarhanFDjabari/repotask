"""`repo-task skills` — generate the agent-facing entry points."""

from __future__ import annotations

from typing import Any

import typer
from rich.console import RenderableType
from rich.table import Table

from repotask.config import load_config
from repotask.output import emit, guard
from repotask.skills import generator

app = typer.Typer(no_args_is_help=True)


@app.command()
@guard
def sync(
    dry_run: bool = typer.Option(False, "--dry-run", help="Preview without writing."),
) -> None:
    """Write Claude Code skills and the AGENTS.md block for this project."""
    config = load_config()
    results = generator.sync(config, dry_run=dry_run)
    others = generator.find_agent_files(config.root)

    data = {
        "dryRun": dry_run,
        "files": [{"path": item.path, "action": item.action} for item in results],
        "otherAgentFiles": others,
        "note": (
            "Skills reference commands only — no project knowledge is copied into them, "
            "so they stay correct as the knowledge base changes."
        ),
    }
    emit("skills.sync", data, _render)


def _render(data: dict[str, Any]) -> RenderableType:
    table = Table(title="Generated agent skills" + (" (dry run)" if data["dryRun"] else ""))
    table.add_column("file", overflow="fold")
    table.add_column("action")
    for item in data["files"]:
        table.add_row(item["path"], item["action"])
    if data["otherAgentFiles"]:
        table.caption = (
            "Other agent instruction files found: "
            + ", ".join(data["otherAgentFiles"])
            + ". Point them at AGENTS.md."
        )
    return table
