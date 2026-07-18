"""Dual-surface output: a stable JSON envelope for agents, rich text for humans.

Every command produces the same envelope shape so the agent-facing contract stays
predictable across the whole CLI:

    {"ok": true,  "command": "kb.status", "data": {...}, "warnings": []}
    {"ok": false, "command": "kb.status", "error": {"code": "...", "message": "..."}}
"""

from __future__ import annotations

import functools
import json
import sys
from collections.abc import Callable
from dataclasses import dataclass, field
from typing import Any, TypeVar

import typer
from rich.console import Console, RenderableType

from repotask.errors import RepoTaskError

SCHEMA = "repotask.v2"

Renderer = Callable[[dict[str, Any]], RenderableType]
Command = TypeVar("Command", bound=Callable[..., None])

_stdout = Console()
_stderr = Console(stderr=True)


@dataclass
class OutputState:
    """Process-wide output mode, set once by the CLI root callback."""

    json_mode: bool = False
    warnings: list[str] = field(default_factory=list)

    def warn(self, message: str) -> None:
        self.warnings.append(message)


state = OutputState()


def warn(message: str) -> None:
    state.warn(message)


def emit(command: str, data: dict[str, Any], renderer: Renderer | None = None) -> None:
    """Print a success envelope as JSON, or render the human view."""
    if state.json_mode:
        payload = {
            "schema": SCHEMA,
            "ok": True,
            "command": command,
            "data": data,
            "warnings": state.warnings,
        }
        _stdout.print_json(json.dumps(payload, default=str))
        return
    for message in state.warnings:
        _stderr.print(f"[yellow]warning:[/yellow] {message}")
    if renderer is not None:
        _stdout.print(renderer(data))
    else:
        _stdout.print_json(json.dumps(data, indent=2, default=str))


def emit_error(command: str, message: str, code: str = "error") -> None:
    """Print a failure envelope. Callers are responsible for the exit code."""
    if state.json_mode:
        payload = {
            "schema": SCHEMA,
            "ok": False,
            "command": command,
            "error": {"code": code, "message": message},
            "warnings": state.warnings,
        }
        print(json.dumps(payload, default=str), file=sys.stdout)
        return
    _stderr.print(f"[red]Error:[/red] {message}")


def guard(func: Command) -> Command:
    """Turn an expected `RepoTaskError` into a clean failure envelope and exit code 1.

    Applied per command so unexpected exceptions still surface with a traceback.
    """

    @functools.wraps(func)
    def wrapper(*args: Any, **kwargs: Any) -> None:
        try:
            func(*args, **kwargs)
        except RepoTaskError as error:
            emit_error(func.__name__.replace("_", "-"), str(error))
            raise typer.Exit(1) from error

    return wrapper  # type: ignore[return-value]
