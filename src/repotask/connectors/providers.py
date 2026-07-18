"""Per-system connectors: Jira, GitHub, GitLab, ClickUp."""

from __future__ import annotations

from typing import Any

from repotask.config.models import ConnectorConfig
from repotask.connectors import secrets
from repotask.connectors.base import McpRequest, Ticket
from repotask.connectors.rest import base_url, get_json
from repotask.errors import RepoTaskError


class JiraConnector:
    name = "jira"

    def fetch_ticket(self, config: ConnectorConfig, ticket_id: str) -> Ticket:
        url = base_url(config.base_url, "", self.name)
        auth = (secrets.require(self.name, "email"), secrets.require(self.name, "token"))
        payload = get_json(
            f"{url}/rest/api/3/issue/{ticket_id}",
            headers={"Accept": "application/json"},
            auth=auth,
        )
        fields = payload.get("fields", {})
        return Ticket(
            id=str(payload.get("key", ticket_id)),
            title=str(fields.get("summary", "")),
            body=_adf_to_text(fields.get("description")),
            status=str((fields.get("status") or {}).get("name", "")),
            url=f"{url}/browse/{payload.get('key', ticket_id)}",
            labels=[str(label) for label in fields.get("labels", [])],
            updated_at=str(fields.get("updated", "")),
            source=self.name,
        )

    def list_tickets(self, config: ConnectorConfig, query: str, limit: int) -> list[Ticket]:
        url = base_url(config.base_url, "", self.name)
        auth = (secrets.require(self.name, "email"), secrets.require(self.name, "token"))
        default_jql = f"project = {config.project} ORDER BY updated DESC" if config.project else ""
        jql = query or default_jql
        if not jql:
            raise RepoTaskError("Jira needs a query or a configured project.")
        payload = get_json(
            f"{url}/rest/api/3/search",
            headers={"Accept": "application/json"},
            params={"jql": jql, "maxResults": limit},
            auth=auth,
        )
        return [
            Ticket(
                id=str(issue.get("key", "")),
                title=str(issue.get("fields", {}).get("summary", "")),
                body=_adf_to_text(issue.get("fields", {}).get("description")),
                status=str((issue.get("fields", {}).get("status") or {}).get("name", "")),
                url=f"{url}/browse/{issue.get('key', '')}",
                labels=[str(label) for label in issue.get("fields", {}).get("labels", [])],
                updated_at=str(issue.get("fields", {}).get("updated", "")),
                source=self.name,
            )
            for issue in payload.get("issues", [])
        ]

    def mcp_fetch(self, config: ConnectorConfig, ticket_id: str) -> McpRequest:
        return McpRequest(
            server=config.mcp_server or "atlassian",
            tool="getJiraIssue",
            arguments={"issueIdOrKey": ticket_id},
            reason=f"Fetch Jira issue {ticket_id} through your own Atlassian connection.",
        )

    def mcp_list(self, config: ConnectorConfig, query: str, limit: int) -> McpRequest:
        return McpRequest(
            server=config.mcp_server or "atlassian",
            tool="searchJiraIssuesUsingJql",
            arguments={"jql": query, "maxResults": limit},
            reason="Search Jira through your own Atlassian connection.",
        )


class GitHubConnector:
    name = "github"

    def fetch_ticket(self, config: ConnectorConfig, ticket_id: str) -> Ticket:
        url = base_url(config.base_url, "https://api.github.com", self.name)
        repository = _require_project(config, self.name, "owner/repo")
        payload = get_json(
            f"{url}/repos/{repository}/issues/{ticket_id}",
            headers=self._headers(),
        )
        return self._ticket(payload)

    def list_tickets(self, config: ConnectorConfig, query: str, limit: int) -> list[Ticket]:
        url = base_url(config.base_url, "https://api.github.com", self.name)
        repository = _require_project(config, self.name, "owner/repo")
        payload = get_json(
            f"{url}/search/issues",
            headers=self._headers(),
            params={"q": f"repo:{repository} is:issue {query}".strip(), "per_page": limit},
        )
        return [self._ticket(item) for item in payload.get("items", [])]

    def mcp_fetch(self, config: ConnectorConfig, ticket_id: str) -> McpRequest:
        owner, _, repo = _require_project(config, self.name, "owner/repo").partition("/")
        return McpRequest(
            server=config.mcp_server or "github",
            tool="get_issue",
            arguments={"owner": owner, "repo": repo, "issue_number": int(ticket_id)},
            reason=f"Fetch GitHub issue #{ticket_id} through your own GitHub connection.",
        )

    def mcp_list(self, config: ConnectorConfig, query: str, limit: int) -> McpRequest:
        repository = _require_project(config, self.name, "owner/repo")
        return McpRequest(
            server=config.mcp_server or "github",
            tool="search_issues",
            arguments={"q": f"repo:{repository} is:issue {query}".strip(), "per_page": limit},
            reason="Search GitHub issues through your own GitHub connection.",
        )

    def _headers(self) -> dict[str, str]:
        headers = {"Accept": "application/vnd.github+json"}
        token = secrets.get(self.name, "token")
        if token:
            headers["Authorization"] = f"Bearer {token}"
        return headers

    def _ticket(self, payload: dict[str, Any]) -> Ticket:
        return Ticket(
            id=str(payload.get("number", "")),
            title=str(payload.get("title", "")),
            body=str(payload.get("body") or ""),
            status=str(payload.get("state", "")),
            url=str(payload.get("html_url", "")),
            labels=[str(label.get("name", "")) for label in payload.get("labels", [])],
            updated_at=str(payload.get("updated_at", "")),
            source=self.name,
        )


class GitLabConnector:
    name = "gitlab"

    def fetch_ticket(self, config: ConnectorConfig, ticket_id: str) -> Ticket:
        url = base_url(config.base_url, "https://gitlab.com/api/v4", self.name)
        project = _quote(_require_project(config, self.name, "group/project"))
        payload = get_json(f"{url}/projects/{project}/issues/{ticket_id}", headers=self._headers())
        return self._ticket(payload)

    def list_tickets(self, config: ConnectorConfig, query: str, limit: int) -> list[Ticket]:
        url = base_url(config.base_url, "https://gitlab.com/api/v4", self.name)
        project = _quote(_require_project(config, self.name, "group/project"))
        payload = get_json(
            f"{url}/projects/{project}/issues",
            headers=self._headers(),
            params={"search": query, "per_page": limit},
        )
        return [self._ticket(item) for item in payload.get("items", [])]

    def mcp_fetch(self, config: ConnectorConfig, ticket_id: str) -> McpRequest:
        return McpRequest(
            server=config.mcp_server or "gitlab",
            tool="get_issue",
            arguments={
                "project_id": _require_project(config, self.name, "group/project"),
                "issue_iid": ticket_id,
            },
            reason=f"Fetch GitLab issue !{ticket_id} through your own GitLab connection.",
        )

    def mcp_list(self, config: ConnectorConfig, query: str, limit: int) -> McpRequest:
        return McpRequest(
            server=config.mcp_server or "gitlab",
            tool="list_issues",
            arguments={
                "project_id": _require_project(config, self.name, "group/project"),
                "search": query,
                "per_page": limit,
            },
            reason="Search GitLab issues through your own GitLab connection.",
        )

    def _headers(self) -> dict[str, str]:
        token = secrets.get(self.name, "token")
        return {"PRIVATE-TOKEN": token} if token else {}

    def _ticket(self, payload: dict[str, Any]) -> Ticket:
        return Ticket(
            id=str(payload.get("iid", "")),
            title=str(payload.get("title", "")),
            body=str(payload.get("description") or ""),
            status=str(payload.get("state", "")),
            url=str(payload.get("web_url", "")),
            labels=[str(label) for label in payload.get("labels", [])],
            updated_at=str(payload.get("updated_at", "")),
            source=self.name,
        )


class ClickUpConnector:
    name = "clickup"

    def fetch_ticket(self, config: ConnectorConfig, ticket_id: str) -> Ticket:
        url = base_url(config.base_url, "https://api.clickup.com/api/v2", self.name)
        payload = get_json(f"{url}/task/{ticket_id}", headers=self._headers())
        return self._ticket(payload)

    def list_tickets(self, config: ConnectorConfig, query: str, limit: int) -> list[Ticket]:
        url = base_url(config.base_url, "https://api.clickup.com/api/v2", self.name)
        list_id = _require_project(config, self.name, "list id")
        payload = get_json(
            f"{url}/list/{list_id}/task",
            headers=self._headers(),
            params={"subtasks": "true"},
        )
        tickets = [self._ticket(item) for item in payload.get("tasks", [])]
        if query:
            lowered = query.lower()
            tickets = [item for item in tickets if lowered in f"{item.title} {item.body}".lower()]
        return tickets[:limit]

    def mcp_fetch(self, config: ConnectorConfig, ticket_id: str) -> McpRequest:
        return McpRequest(
            server=config.mcp_server or "clickup",
            tool="clickup_get_task",
            arguments={"taskId": ticket_id},
            reason=f"Fetch ClickUp task {ticket_id} through your own ClickUp connection.",
        )

    def mcp_list(self, config: ConnectorConfig, query: str, limit: int) -> McpRequest:
        return McpRequest(
            server=config.mcp_server or "clickup",
            tool="clickup_search",
            arguments={"query": query, "limit": limit},
            reason="Search ClickUp through your own ClickUp connection.",
        )

    def _headers(self) -> dict[str, str]:
        return {"Authorization": secrets.require(self.name, "token")}

    def _ticket(self, payload: dict[str, Any]) -> Ticket:
        return Ticket(
            id=str(payload.get("id", "")),
            title=str(payload.get("name", "")),
            body=str(payload.get("description") or payload.get("text_content") or ""),
            status=str((payload.get("status") or {}).get("status", "")),
            url=str(payload.get("url", "")),
            labels=[str(tag.get("name", "")) for tag in payload.get("tags", [])],
            updated_at=str(payload.get("date_updated", "")),
            source=self.name,
        )


def _require_project(config: ConnectorConfig, system: str, shape: str) -> str:
    if not config.project:
        raise RepoTaskError(
            f"Connector '{system}' needs `project` ({shape}) in .repo-task/config.yaml."
        )
    return config.project


def _quote(value: str) -> str:
    from urllib.parse import quote

    return quote(value, safe="")


def _adf_to_text(node: Any) -> str:
    """Flatten Atlassian Document Format into plain text; Jira returns prose as a tree."""
    if node is None:
        return ""
    if isinstance(node, str):
        return node
    if isinstance(node, list):
        return "".join(_adf_to_text(item) for item in node)
    if not isinstance(node, dict):
        return ""
    if node.get("type") == "text":
        return str(node.get("text", ""))
    inner = _adf_to_text(node.get("content"))
    return f"{inner}\n" if node.get("type") in {"paragraph", "heading"} else inner
