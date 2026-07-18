"""`repo-task bug` — the bugfix flow."""

from __future__ import annotations

import json
from typing import Any

import typer
from rich.console import RenderableType
from rich.table import Table

from repotask.commands._context import open_knowledge
from repotask.commands.work import _read_stdin_or_file
from repotask.config import load_config
from repotask.connectors import connector_config, default_system, get_connector
from repotask.errors import RepoTaskError
from repotask.index.families import SYMBOLS_FAMILY
from repotask.kb.slicing import build_pack
from repotask.output import emit, guard
from repotask.workflow import store
from repotask.workflow.analyze import impact_set
from repotask.workflow.dedupe import MIN_SIMILARITY, build_impacts, cluster

app = typer.Typer(no_args_is_help=True)

TRIAGE_CONTRACT = """You have the report, the code it points at, and this project's conventions.
Identify the root cause before proposing a fix, and say which of the impacted symbols you
believe is responsible. If the evidence does not support a single root cause, say what
additional signal you need instead of guessing."""

MERGE_CONTRACT = """These tickets resolve to overlapping code. Shared code is not the same as a
shared root cause: confirm each cluster against the reports before recommending a merge,
name the ticket that should survive, and say what the others add that it would lose."""


@app.command("fetch")
@guard
def bug_fetch(
    ticket: str = typer.Argument(..., help="Bug ticket id."),
    system: str = typer.Option("", "--system", help="Connector name."),
    write: str = typer.Option("", "--write", help="Store this content ('-' reads stdin)."),
    budget: int = typer.Option(0, "--budget", help="Token ceiling for the attached context."),
) -> None:
    """Fetch a bug report together with the code and conventions it implicates."""
    from repotask.commands.work import fetch as fetch_command

    if write or not _has_source(ticket):
        fetch_command(ticket=ticket, system=system, write=write)
        if write or not _has_source(ticket):
            return

    config, kb = open_knowledge(sync=False)
    item = store.work_item(config, ticket)
    report = item.require(store.SOURCE, f"repo-task bug fetch {ticket}")
    symbols = kb.facts(SYMBOLS_FAMILY)
    if not symbols:
        raise RepoTaskError("No symbol index found. Run `repo-task index` first.")

    impact = impact_set(report, symbols)
    pack = build_pack(
        kb,
        config.project.stacks,
        "bugfix",
        budget or config.brief.default_budget,
        [item["path"] for item in impact.files[:20]],
    )
    data = {
        "ticket": ticket,
        "report": report,
        "impact": impact.as_dict(),
        "context": pack.as_dict(),
        "contract": TRIAGE_CONTRACT,
    }
    item.write_json(store.ANALYSIS, {"ticket": ticket, "impact": impact.as_dict()})
    item.touch_meta(kind="bug", analyzed=True)
    emit("bug.fetch", data, lambda payload: payload["contract"])


def _has_source(ticket: str) -> bool:
    config = load_config()
    return store.work_item(config, ticket).source_path.is_file()


@app.command("dedupe")
@guard
def bug_dedupe(
    system: str = typer.Option("", "--system", help="Connector name."),
    query: str = typer.Option("", "--query", help="Provider query selecting the tickets."),
    limit: int = typer.Option(50, "--limit", help="Maximum tickets to compare."),
    threshold: float = typer.Option(
        MIN_SIMILARITY, "--threshold", help="Minimum similarity to group two tickets."
    ),
    tickets_file: str = typer.Option(
        "", "--tickets", help="Read tickets as JSON instead of calling a connector ('-' is stdin)."
    ),
) -> None:
    """Group open bug tickets that resolve to the same code.

    In `mcp` mode the CLI returns the search call for the agent to make; the agent
    pipes the resulting tickets back with `--tickets -`.
    """
    config, kb = open_knowledge(sync=False)
    symbols = kb.facts(SYMBOLS_FAMILY)
    if not symbols:
        raise RepoTaskError("No symbol index found. Run `repo-task index` first.")

    if tickets_file:
        tickets = _parse_tickets(_read_stdin_or_file(tickets_file))
    else:
        name = system or default_system(config)
        settings = connector_config(config, name)
        connector = get_connector(name)
        if settings.mode == "mcp":
            request = connector.mcp_list(settings, query, limit)
            emit(
                "bug.dedupe",
                {
                    "mode": "mcp",
                    "request": request.as_dict(),
                    "nextStep": "repo-task bug dedupe --tickets -",
                    "instruction": (
                        "Call the tool above, then pipe the tickets back as JSON: a list of "
                        "objects with id, title, and body."
                    ),
                },
                lambda data: (
                    f"Call {data['request']['server']}.{data['request']['tool']}, then run: "
                    f"{data['nextStep']}"
                ),
            )
            return
        tickets = [ticket.as_dict() for ticket in connector.list_tickets(settings, query, limit)]

    impacts = build_impacts(tickets, symbols)
    clusters = cluster(impacts, threshold)
    unmatched = sorted(
        {item.ticket_id for item in impacts}
        - {member["id"] for group in clusters for member in group["tickets"]}
    )
    emit(
        "bug.dedupe",
        {
            "mode": "analyze",
            "ticketCount": len(tickets),
            "threshold": threshold,
            "clusters": clusters,
            "unmatched": unmatched,
            "contract": MERGE_CONTRACT,
        },
        _render_clusters,
    )


def _parse_tickets(text: str) -> list[dict[str, str]]:
    try:
        payload = json.loads(text)
    except ValueError as error:
        raise RepoTaskError(f"Could not parse tickets as JSON: {error}") from error
    items = payload.get("tickets", payload) if isinstance(payload, dict) else payload
    if not isinstance(items, list):
        raise RepoTaskError("Tickets must be a JSON list of objects with id, title, and body.")
    tickets = []
    for item in items:
        if not isinstance(item, dict) or not item.get("id"):
            raise RepoTaskError("Every ticket needs at least an 'id'.")
        tickets.append(
            {
                "id": str(item["id"]),
                "title": str(item.get("title", "")),
                "body": str(item.get("body", "")),
            }
        )
    return tickets


def _render_clusters(data: dict[str, Any]) -> RenderableType:
    if data.get("mode") != "analyze":
        return str(data)
    if not data["clusters"]:
        return f"No duplicate candidates among {data['ticketCount']} tickets."
    table = Table(title="Possible duplicate clusters")
    table.add_column("score", justify="right")
    table.add_column("tickets", overflow="fold")
    table.add_column("shared code", overflow="fold")
    for group in data["clusters"]:
        table.add_row(
            f"{group['topScore']:.2f}",
            "\n".join(f"{item['id']} {item['title']}".strip() for item in group["tickets"]),
            "\n".join(group["sharedPaths"]) or "-",
        )
    return table
