"""Feature workflow: `fetch` -> `summarize` -> `analyze` -> `split`.

None of these steps call a model. Each one gathers the inputs for a decision and
states the contract the agent should satisfy, then stores what the agent writes back.
"""

from __future__ import annotations

import sys
from typing import Any

import typer
from rich.console import RenderableType
from rich.table import Table

from repotask.commands._context import open_knowledge
from repotask.config import load_config
from repotask.connectors import connector_config, default_system, get_connector
from repotask.errors import RepoTaskError
from repotask.index.families import SYMBOLS_FAMILY
from repotask.kb.slicing import build_pack
from repotask.output import emit, guard
from repotask.workflow import store
from repotask.workflow.analyze import impact_set, split_increments

SUMMARY_CONTRACT = """Condense the source below into the context this project actually needs.
Keep only what affects these stacks: {stacks}. Drop platform notes, market copy, and
requirements that belong to other clients. Preserve every acceptance criterion that
survives that filter. Write the result back with:

    repo-task summarize {ticket} --write -

Use these sections: ## Problem, ## Scope, ## Acceptance criteria, ## Out of scope,
## Open questions."""

EFFORT_RUBRIC = """Estimate effort from the evidence, not from the prose:
- S: one module, one layer, no schema or API change
- M: one module across layers, or two modules in one layer
- L: several modules, a schema/migration, or a public API change
- XL: cross-cutting change, or the impact set is too diffuse to bound — split it first
State the band, the two or three facts that drove it, and what would change it."""

SPLIT_CONTRACT = """Turn these increments into steps that each land as one reviewable change.
Every step must build and be independently revertible. Merge steps that cannot stand alone,
and split any step whose diff would be hard to review. Keep data-layer work ahead of the UI
that consumes it."""


def _read_stdin_or_file(value: str) -> str:
    """`-` means stdin, which is how an agent writes its output back."""
    if value == "-":
        content = sys.stdin.read()
        if not content.strip():
            raise RepoTaskError("Nothing was provided on stdin.")
        return content
    from pathlib import Path

    path = Path(value)
    if not path.is_file():
        raise RepoTaskError(f"File not found: {value}")
    return path.read_text(encoding="utf-8")


@guard
def fetch(
    ticket: str = typer.Argument(..., help="Ticket or issue id."),
    system: str = typer.Option("", "--system", help="Connector name. Defaults to the only one."),
    write: str = typer.Option(
        "", "--write", help="Store this content as the source ('-' reads stdin)."
    ),
) -> None:
    """Pull a ticket or PRD into the local work directory.

    In `mcp` mode nothing is fetched: the CLI returns the tool call for the agent to
    make, and the agent stores the result with `--write -`.
    """
    config = load_config()
    item = store.work_item(config, ticket)

    if write:
        content = _read_stdin_or_file(write)
        path = item.write(store.SOURCE, content)
        item.touch_meta(source="agent")
        emit(
            "fetch",
            {"ticket": ticket, "stored": path, "bytes": len(content), "mode": "write"},
            lambda data: f"Stored {data['bytes']} bytes in {data['stored']}",
        )
        return

    name = system or default_system(config)
    settings = connector_config(config, name)
    connector = get_connector(name)

    if settings.mode == "mcp":
        request = connector.mcp_fetch(settings, ticket)
        emit(
            "fetch",
            {
                "ticket": ticket,
                "mode": "mcp",
                "request": request.as_dict(),
                "nextStep": f"repo-task fetch {ticket} --write -",
                "instruction": (
                    "Call the tool above yourself, then pipe the ticket text back into the "
                    "next step so later commands can use it."
                ),
            },
            lambda data: (
                f"Call {data['request']['server']}.{data['request']['tool']} with "
                f"{data['request']['arguments']}, then run: {data['nextStep']}"
            ),
        )
        return

    ticket_data = connector.fetch_ticket(settings, ticket)
    path = item.write(store.SOURCE, ticket_data.as_markdown())
    item.touch_meta(source=name, title=ticket_data.title, url=ticket_data.url)
    emit(
        "fetch",
        {"ticket": ticket, "mode": "rest", "stored": path, "ticketData": ticket_data.as_dict()},
        lambda data: f"Fetched {data['ticketData']['title']} -> {data['stored']}",
    )


@guard
def summarize(
    ticket: str = typer.Argument(..., help="Ticket id already fetched."),
    write: str = typer.Option(
        "", "--write", help="Store this content as the summary ('-' reads stdin)."
    ),
    budget: int = typer.Option(0, "--budget", help="Token ceiling for the attached context."),
) -> None:
    """Return the source plus the project's own context, filtered to this project's stacks."""
    config, kb = open_knowledge(sync=False)
    item = store.work_item(config, ticket)

    if write:
        content = _read_stdin_or_file(write)
        path = item.write(store.SUMMARY, content)
        item.touch_meta(summarized=True)
        emit(
            "summarize",
            {"ticket": ticket, "stored": path, "mode": "write"},
            lambda data: f"Stored summary in {data['stored']}",
        )
        return

    source = item.require(store.SOURCE, f"repo-task fetch {ticket}")
    limit = budget or config.brief.default_budget
    pack = build_pack(kb, config.project.stacks, "summarize", limit)
    emit(
        "summarize",
        {
            "ticket": ticket,
            "mode": "prepare",
            "stacks": config.project.stacks,
            "source": source,
            "context": pack.as_dict(),
            "contract": SUMMARY_CONTRACT.format(
                stacks=", ".join(config.project.stacks), ticket=ticket
            ),
            "nextStep": f"repo-task summarize {ticket} --write -",
        },
        lambda data: data["contract"],
    )


@guard
def analyze(
    ticket: str = typer.Argument(..., help="Ticket id with a stored summary or source."),
    limit: int = typer.Option(40, "--limit", help="Maximum impacted files to report."),
) -> None:
    """Compute the impact set from the indexed project facts."""
    config, kb = open_knowledge(sync=False)
    item = store.work_item(config, ticket)
    text = item.read(store.SUMMARY) or item.require(store.SOURCE, f"repo-task fetch {ticket}")

    symbols = kb.facts(SYMBOLS_FAMILY)
    if not symbols:
        raise RepoTaskError("No symbol index found. Run `repo-task index` first.")

    impact = impact_set(text, symbols)
    data = {
        "ticket": ticket,
        "basedOn": store.SUMMARY if item.read(store.SUMMARY) else store.SOURCE,
        "impact": {
            **impact.as_dict(),
            "files": impact.files[:limit],
            "symbols": impact.symbols[:limit],
        },
        "counts": {
            "files": len(impact.files),
            "symbols": len(impact.symbols),
            "modules": len(impact.modules),
        },
        "rubric": EFFORT_RUBRIC,
        "nextStep": f"repo-task split {ticket}",
    }
    item.write_json(store.ANALYSIS, data)
    item.touch_meta(analyzed=True)
    emit("analyze", data, _render_analysis)


def _render_analysis(data: dict[str, Any]) -> RenderableType:
    table = Table(title=f"Impact for {data['ticket']}")
    table.add_column("path", overflow="fold")
    table.add_column("module")
    table.add_column("layer")
    table.add_column("terms", overflow="fold")
    for item in data["impact"]["files"]:
        table.add_row(item["path"], item["module"], item["layer"], ", ".join(item["terms"]))
    return table


@guard
def split(
    ticket: str = typer.Argument(..., help="Ticket id that has been analyzed."),
    max_files: int = typer.Option(8, "--max-files", help="Largest number of files per step."),
) -> None:
    """Group the impact set into incremental, independently reviewable steps."""
    config = load_config()
    item = store.work_item(config, ticket)
    analysis = item.read_json(store.ANALYSIS)
    if not analysis:
        raise RepoTaskError(f"No analysis found. Run `repo-task analyze {ticket}` first.")

    from repotask.workflow.analyze import Impact

    impact = Impact(
        terms=analysis["impact"]["terms"],
        files=analysis["impact"]["files"],
        symbols=analysis["impact"]["symbols"],
        modules=analysis["impact"]["modules"],
    )
    increments = split_increments(impact, max_files)
    data = {
        "ticket": ticket,
        "increments": increments,
        "contract": SPLIT_CONTRACT,
    }
    item.write_json(store.PLAN, data)
    item.touch_meta(split=True)
    emit("split", data, _render_split)


def _render_split(data: dict[str, Any]) -> RenderableType:
    table = Table(title=f"Proposed increments for {data['ticket']}")
    table.add_column("#", justify="right")
    table.add_column("title")
    table.add_column("paths", overflow="fold")
    for increment in data["increments"]:
        table.add_row(str(increment["step"]), increment["title"], "\n".join(increment["paths"]))
    return table
