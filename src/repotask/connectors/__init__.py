"""Connectors to external ticket and document systems."""

from __future__ import annotations

from repotask.config.models import ConnectorConfig, RepoTaskConfig
from repotask.connectors.base import Connector, McpRequest, Ticket
from repotask.connectors.providers import (
    ClickUpConnector,
    GitHubConnector,
    GitLabConnector,
    JiraConnector,
)
from repotask.errors import RepoTaskError

CONNECTORS: dict[str, Connector] = {
    connector.name: connector  # type: ignore[misc]
    for connector in (
        JiraConnector(),
        GitHubConnector(),
        GitLabConnector(),
        ClickUpConnector(),
    )
}


def get_connector(system: str) -> Connector:
    connector = CONNECTORS.get(system)
    if connector is None:
        raise RepoTaskError(
            f"Unknown system '{system}'. Available: {', '.join(sorted(CONNECTORS))}."
        )
    return connector


def connector_config(config: RepoTaskConfig, system: str) -> ConnectorConfig:
    settings = config.connectors.get(system)
    if settings is None:
        raise RepoTaskError(
            f"Connector '{system}' is not configured. Add it under `connectors:` in "
            ".repo-task/config.yaml."
        )
    if settings.mode == "off":
        raise RepoTaskError(f"Connector '{system}' is disabled.")
    return settings


def default_system(config: RepoTaskConfig) -> str:
    """The single enabled connector, when the user has not named one."""
    enabled = [name for name, settings in config.connectors.items() if settings.mode != "off"]
    if not enabled:
        raise RepoTaskError(
            "No connectors are configured. Add one under `connectors:` in .repo-task/config.yaml."
        )
    if len(enabled) > 1:
        raise RepoTaskError(
            f"Several connectors are enabled ({', '.join(sorted(enabled))}). Pass --system."
        )
    return enabled[0]


__all__ = [
    "CONNECTORS",
    "Connector",
    "McpRequest",
    "Ticket",
    "connector_config",
    "default_system",
    "get_connector",
]
