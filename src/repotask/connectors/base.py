"""Connector contract for external ticket and document systems.

Two backends per system:
  * `rest` — the CLI calls the API itself. Cheap, deterministic, no agent tokens.
  * `mcp`  — the CLI calls nothing and instead returns the MCP tool and arguments
             for the *agent* to invoke, reusing the user's existing connection.

Which one runs is a per-system configuration choice, not a code path the agent picks.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any, Protocol

from repotask.config.models import ConnectorConfig
from repotask.errors import RepoTaskError


@dataclass(frozen=True)
class Ticket:
    """A ticket or PRD normalized across providers."""

    id: str
    title: str = ""
    body: str = ""
    status: str = ""
    url: str = ""
    labels: list[str] = field(default_factory=list)
    updated_at: str = ""
    source: str = ""

    def as_dict(self) -> dict[str, Any]:
        return {
            "id": self.id,
            "title": self.title,
            "body": self.body,
            "status": self.status,
            "url": self.url,
            "labels": self.labels,
            "updatedAt": self.updated_at,
            "source": self.source,
        }

    def as_markdown(self) -> str:
        header = [f"# {self.title or self.id}", "", f"- id: {self.id}"]
        if self.status:
            header.append(f"- status: {self.status}")
        if self.url:
            header.append(f"- url: {self.url}")
        if self.labels:
            header.append(f"- labels: {', '.join(self.labels)}")
        return "\n".join([*header, "", self.body.strip(), ""])


@dataclass(frozen=True)
class McpRequest:
    """An instruction for the agent to make the call itself."""

    server: str
    tool: str
    arguments: dict[str, Any]
    reason: str

    def as_dict(self) -> dict[str, Any]:
        return {
            "server": self.server,
            "tool": self.tool,
            "arguments": self.arguments,
            "reason": self.reason,
        }


class Connector(Protocol):
    """A provider that can fetch tickets over REST."""

    name: str

    def fetch_ticket(self, config: ConnectorConfig, ticket_id: str) -> Ticket: ...

    def list_tickets(self, config: ConnectorConfig, query: str, limit: int) -> list[Ticket]: ...

    def mcp_fetch(self, config: ConnectorConfig, ticket_id: str) -> McpRequest: ...

    def mcp_list(self, config: ConnectorConfig, query: str, limit: int) -> McpRequest: ...


def require_mode(system: str, config: ConnectorConfig, expected: str) -> None:
    if config.mode == "off":
        raise RepoTaskError(f"Connector '{system}' is disabled in .repo-task/config.yaml.")
    if config.mode != expected:
        raise RepoTaskError(
            f"Connector '{system}' is configured for '{config.mode}' mode, not '{expected}'."
        )
