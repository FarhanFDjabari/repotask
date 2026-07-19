//! Per-system connectors: Jira, GitHub, GitLab, ClickUp.

use anyhow::{bail, Result};
use serde_json::{json, Value};

use crate::config::ConnectorConfig;
use crate::connectors::rest::{base_url, get_json};
use crate::connectors::secrets;
use crate::connectors::{require_project, Connector, McpRequest, Ticket};

fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

fn text(value: &Value) -> String {
    value.as_str().unwrap_or_default().to_string()
}

fn fallback(configured: &str, default: &str) -> String {
    if configured.is_empty() {
        default.to_string()
    } else {
        configured.to_string()
    }
}

pub struct Jira;

impl Jira {
    fn auth(&self) -> Result<(String, String)> {
        Ok((
            secrets::require("jira", "email")?,
            secrets::require("jira", "token")?,
        ))
    }

    fn ticket(&self, url: &str, payload: &Value) -> Ticket {
        let fields = &payload["fields"];
        let key = text(&payload["key"]);
        Ticket {
            id: key.clone(),
            title: text(&fields["summary"]),
            body: adf_to_text(&fields["description"]),
            status: text(&fields["status"]["name"]),
            url: format!("{url}/browse/{key}"),
            labels: strings(&fields["labels"]),
            updated_at: text(&fields["updated"]),
            source: "jira".into(),
        }
    }
}

impl Connector for Jira {
    fn name(&self) -> &'static str {
        "jira"
    }

    fn fetch_ticket(&self, config: &ConnectorConfig, ticket_id: &str) -> Result<Ticket> {
        let url = base_url(&config.base_url, "", self.name())?;
        let (email, token) = self.auth()?;
        let payload = get_json(
            &format!("{url}/rest/api/3/issue/{ticket_id}"),
            &[("Accept", "application/json".into())],
            &[],
            Some((&email, &token)),
        )?;
        let mut ticket = self.ticket(&url, &payload);
        if ticket.id.is_empty() {
            ticket.id = ticket_id.to_string();
        }
        Ok(ticket)
    }

    fn list_tickets(
        &self,
        config: &ConnectorConfig,
        query: &str,
        limit: usize,
    ) -> Result<Vec<Ticket>> {
        let url = base_url(&config.base_url, "", self.name())?;
        let (email, token) = self.auth()?;
        let jql = if !query.is_empty() {
            query.to_string()
        } else if !config.project.is_empty() {
            format!("project = {} ORDER BY updated DESC", config.project)
        } else {
            bail!("Jira needs a query or a configured project.");
        };
        let payload = get_json(
            &format!("{url}/rest/api/3/search"),
            &[("Accept", "application/json".into())],
            &[("jql", jql), ("maxResults", limit.to_string())],
            Some((&email, &token)),
        )?;
        Ok(payload["issues"]
            .as_array()
            .map(|issues| {
                issues
                    .iter()
                    .map(|issue| self.ticket(&url, issue))
                    .collect()
            })
            .unwrap_or_default())
    }

    fn mcp_fetch(&self, config: &ConnectorConfig, ticket_id: &str) -> Result<McpRequest> {
        Ok(McpRequest {
            server: fallback(&config.mcp_server, "atlassian"),
            tool: "getJiraIssue".into(),
            arguments: json!({"issueIdOrKey": ticket_id}),
            reason: format!("Fetch Jira issue {ticket_id} through your own Atlassian connection."),
        })
    }

    fn mcp_list(&self, config: &ConnectorConfig, query: &str, limit: usize) -> Result<McpRequest> {
        Ok(McpRequest {
            server: fallback(&config.mcp_server, "atlassian"),
            tool: "searchJiraIssuesUsingJql".into(),
            arguments: json!({"jql": query, "maxResults": limit}),
            reason: "Search Jira through your own Atlassian connection.".into(),
        })
    }
}

pub struct GitHub;

impl GitHub {
    fn headers(&self) -> Vec<(&'static str, String)> {
        let mut headers = vec![("Accept", "application/vnd.github+json".to_string())];
        let token = secrets::get("github", "token");
        if !token.is_empty() {
            headers.push(("Authorization", format!("Bearer {token}")));
        }
        headers
    }

    fn ticket(&self, payload: &Value) -> Ticket {
        Ticket {
            id: payload["number"]
                .as_i64()
                .map(|value| value.to_string())
                .unwrap_or_default(),
            title: text(&payload["title"]),
            body: text(&payload["body"]),
            status: text(&payload["state"]),
            url: text(&payload["html_url"]),
            labels: payload["labels"]
                .as_array()
                .map(|items| items.iter().map(|item| text(&item["name"])).collect())
                .unwrap_or_default(),
            updated_at: text(&payload["updated_at"]),
            source: "github".into(),
        }
    }
}

impl Connector for GitHub {
    fn name(&self) -> &'static str {
        "github"
    }

    fn fetch_ticket(&self, config: &ConnectorConfig, ticket_id: &str) -> Result<Ticket> {
        let url = base_url(&config.base_url, "https://api.github.com", self.name())?;
        let repository = require_project(config, self.name(), "owner/repo")?;
        let payload = get_json(
            &format!("{url}/repos/{repository}/issues/{ticket_id}"),
            &self.headers(),
            &[],
            None,
        )?;
        Ok(self.ticket(&payload))
    }

    fn list_tickets(
        &self,
        config: &ConnectorConfig,
        query: &str,
        limit: usize,
    ) -> Result<Vec<Ticket>> {
        let url = base_url(&config.base_url, "https://api.github.com", self.name())?;
        let repository = require_project(config, self.name(), "owner/repo")?;
        let payload = get_json(
            &format!("{url}/search/issues"),
            &self.headers(),
            &[
                (
                    "q",
                    format!("repo:{repository} is:issue {query}")
                        .trim()
                        .to_string(),
                ),
                ("per_page", limit.to_string()),
            ],
            None,
        )?;
        Ok(payload["items"]
            .as_array()
            .map(|items| items.iter().map(|item| self.ticket(item)).collect())
            .unwrap_or_default())
    }

    fn mcp_fetch(&self, config: &ConnectorConfig, ticket_id: &str) -> Result<McpRequest> {
        let repository = require_project(config, self.name(), "owner/repo")?;
        let (owner, repo) = repository
            .split_once('/')
            .unwrap_or((repository.as_str(), ""));
        Ok(McpRequest {
            server: fallback(&config.mcp_server, "github"),
            tool: "get_issue".into(),
            arguments: json!({
                "owner": owner,
                "repo": repo,
                "issue_number": ticket_id.parse::<i64>().unwrap_or_default(),
            }),
            reason: format!("Fetch GitHub issue #{ticket_id} through your own GitHub connection."),
        })
    }

    fn mcp_list(&self, config: &ConnectorConfig, query: &str, limit: usize) -> Result<McpRequest> {
        let repository = require_project(config, self.name(), "owner/repo")?;
        Ok(McpRequest {
            server: fallback(&config.mcp_server, "github"),
            tool: "search_issues".into(),
            arguments: json!({
                "q": format!("repo:{repository} is:issue {query}").trim(),
                "per_page": limit,
            }),
            reason: "Search GitHub issues through your own GitHub connection.".into(),
        })
    }
}

pub struct GitLab;

impl GitLab {
    fn headers(&self) -> Vec<(&'static str, String)> {
        let token = secrets::get("gitlab", "token");
        if token.is_empty() {
            Vec::new()
        } else {
            vec![("PRIVATE-TOKEN", token)]
        }
    }

    fn ticket(&self, payload: &Value) -> Ticket {
        Ticket {
            id: payload["iid"]
                .as_i64()
                .map(|value| value.to_string())
                .unwrap_or_default(),
            title: text(&payload["title"]),
            body: text(&payload["description"]),
            status: text(&payload["state"]),
            url: text(&payload["web_url"]),
            labels: strings(&payload["labels"]),
            updated_at: text(&payload["updated_at"]),
            source: "gitlab".into(),
        }
    }
}

impl Connector for GitLab {
    fn name(&self) -> &'static str {
        "gitlab"
    }

    fn fetch_ticket(&self, config: &ConnectorConfig, ticket_id: &str) -> Result<Ticket> {
        let url = base_url(&config.base_url, "https://gitlab.com/api/v4", self.name())?;
        let project = url_encode(&require_project(config, self.name(), "group/project")?);
        let payload = get_json(
            &format!("{url}/projects/{project}/issues/{ticket_id}"),
            &self.headers(),
            &[],
            None,
        )?;
        Ok(self.ticket(&payload))
    }

    fn list_tickets(
        &self,
        config: &ConnectorConfig,
        query: &str,
        limit: usize,
    ) -> Result<Vec<Ticket>> {
        let url = base_url(&config.base_url, "https://gitlab.com/api/v4", self.name())?;
        let project = url_encode(&require_project(config, self.name(), "group/project")?);
        let payload = get_json(
            &format!("{url}/projects/{project}/issues"),
            &self.headers(),
            &[
                ("search", query.to_string()),
                ("per_page", limit.to_string()),
            ],
            None,
        )?;
        Ok(payload
            .as_array()
            .map(|items| items.iter().map(|item| self.ticket(item)).collect())
            .unwrap_or_default())
    }

    fn mcp_fetch(&self, config: &ConnectorConfig, ticket_id: &str) -> Result<McpRequest> {
        Ok(McpRequest {
            server: fallback(&config.mcp_server, "gitlab"),
            tool: "get_issue".into(),
            arguments: json!({
                "project_id": require_project(config, self.name(), "group/project")?,
                "issue_iid": ticket_id,
            }),
            reason: format!("Fetch GitLab issue !{ticket_id} through your own GitLab connection."),
        })
    }

    fn mcp_list(&self, config: &ConnectorConfig, query: &str, limit: usize) -> Result<McpRequest> {
        Ok(McpRequest {
            server: fallback(&config.mcp_server, "gitlab"),
            tool: "list_issues".into(),
            arguments: json!({
                "project_id": require_project(config, self.name(), "group/project")?,
                "search": query,
                "per_page": limit,
            }),
            reason: "Search GitLab issues through your own GitLab connection.".into(),
        })
    }
}

pub struct ClickUp;

impl ClickUp {
    fn headers(&self) -> Result<Vec<(&'static str, String)>> {
        Ok(vec![(
            "Authorization",
            secrets::require("clickup", "token")?,
        )])
    }

    fn ticket(&self, payload: &Value) -> Ticket {
        let body = if payload["description"].is_string() {
            text(&payload["description"])
        } else {
            text(&payload["text_content"])
        };
        Ticket {
            id: text(&payload["id"]),
            title: text(&payload["name"]),
            body,
            status: text(&payload["status"]["status"]),
            url: text(&payload["url"]),
            labels: payload["tags"]
                .as_array()
                .map(|items| items.iter().map(|item| text(&item["name"])).collect())
                .unwrap_or_default(),
            updated_at: text(&payload["date_updated"]),
            source: "clickup".into(),
        }
    }
}

impl Connector for ClickUp {
    fn name(&self) -> &'static str {
        "clickup"
    }

    fn fetch_ticket(&self, config: &ConnectorConfig, ticket_id: &str) -> Result<Ticket> {
        let url = base_url(
            &config.base_url,
            "https://api.clickup.com/api/v2",
            self.name(),
        )?;
        let payload = get_json(
            &format!("{url}/task/{ticket_id}"),
            &self.headers()?,
            &[],
            None,
        )?;
        Ok(self.ticket(&payload))
    }

    fn list_tickets(
        &self,
        config: &ConnectorConfig,
        query: &str,
        limit: usize,
    ) -> Result<Vec<Ticket>> {
        let url = base_url(
            &config.base_url,
            "https://api.clickup.com/api/v2",
            self.name(),
        )?;
        let list_id = require_project(config, self.name(), "list id")?;
        let payload = get_json(
            &format!("{url}/list/{list_id}/task"),
            &self.headers()?,
            &[("subtasks", "true".into())],
            None,
        )?;
        let mut tickets: Vec<Ticket> = payload["tasks"]
            .as_array()
            .map(|items| items.iter().map(|item| self.ticket(item)).collect())
            .unwrap_or_default();
        if !query.is_empty() {
            let lowered = query.to_lowercase();
            tickets.retain(|ticket| {
                format!("{} {}", ticket.title, ticket.body)
                    .to_lowercase()
                    .contains(&lowered)
            });
        }
        tickets.truncate(limit);
        Ok(tickets)
    }

    fn mcp_fetch(&self, config: &ConnectorConfig, ticket_id: &str) -> Result<McpRequest> {
        Ok(McpRequest {
            server: fallback(&config.mcp_server, "clickup"),
            tool: "clickup_get_task".into(),
            arguments: json!({"taskId": ticket_id}),
            reason: format!("Fetch ClickUp task {ticket_id} through your own ClickUp connection."),
        })
    }

    fn mcp_list(&self, config: &ConnectorConfig, query: &str, limit: usize) -> Result<McpRequest> {
        Ok(McpRequest {
            server: fallback(&config.mcp_server, "clickup"),
            tool: "clickup_search".into(),
            arguments: json!({"query": query, "limit": limit}),
            reason: "Search ClickUp through your own ClickUp connection.".into(),
        })
    }
}

fn url_encode(value: &str) -> String {
    value
        .bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (byte as char).to_string()
            }
            other => format!("%{other:02X}"),
        })
        .collect()
}

/// Flatten Atlassian Document Format into plain text; Jira returns prose as a tree.
fn adf_to_text(node: &Value) -> String {
    match node {
        Value::String(text) => text.clone(),
        Value::Array(items) => items.iter().map(adf_to_text).collect(),
        Value::Object(map) => {
            if map.get("type").and_then(|value| value.as_str()) == Some("text") {
                return map
                    .get("text")
                    .and_then(|value| value.as_str())
                    .unwrap_or("")
                    .to_string();
            }
            let inner = adf_to_text(map.get("content").unwrap_or(&Value::Null));
            match map.get("type").and_then(|value| value.as_str()) {
                Some("paragraph") | Some("heading") => format!("{inner}\n"),
                _ => inner,
            }
        }
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(project: &str) -> ConnectorConfig {
        ConnectorConfig {
            mode: "mcp".into(),
            project: project.into(),
            ..Default::default()
        }
    }

    #[test]
    fn flattens_atlassian_document_format() {
        let doc = json!({
            "type": "doc",
            "content": [
                {"type": "paragraph", "content": [{"type": "text", "text": "Hello"}]},
                {"type": "paragraph", "content": [{"type": "text", "text": "World"}]}
            ]
        });

        assert_eq!(adf_to_text(&doc), "Hello\nWorld\n");
    }

    #[test]
    fn adf_handles_a_plain_string_description() {
        assert_eq!(adf_to_text(&json!("Legacy text")), "Legacy text");
        assert_eq!(adf_to_text(&Value::Null), "");
    }

    #[test]
    fn encodes_gitlab_project_paths() {
        assert_eq!(url_encode("group/project"), "group%2Fproject");
        assert_eq!(url_encode("simple-name"), "simple-name");
    }

    #[test]
    fn github_mcp_request_splits_owner_and_repo() {
        let request = GitHub.mcp_fetch(&config("acme/app"), "42").unwrap();

        assert_eq!(request.tool, "get_issue");
        assert_eq!(request.arguments["owner"], "acme");
        assert_eq!(request.arguments["repo"], "app");
        assert_eq!(request.arguments["issue_number"], 42);
    }

    #[test]
    fn mcp_requests_fall_back_to_a_default_server() {
        assert_eq!(
            Jira.mcp_fetch(&config(""), "ACME-1").unwrap().server,
            "atlassian"
        );
        assert_eq!(
            ClickUp.mcp_fetch(&config(""), "abc").unwrap().server,
            "clickup"
        );
    }

    #[test]
    fn mcp_requests_respect_a_configured_server() {
        let mut settings = config("acme/app");
        settings.mcp_server = "work-github".into();

        assert_eq!(
            GitHub.mcp_fetch(&settings, "1").unwrap().server,
            "work-github"
        );
    }

    #[test]
    fn connectors_needing_a_project_say_so() {
        let error = GitHub.mcp_fetch(&config(""), "1").unwrap_err().to_string();

        assert!(error.contains("owner/repo"), "{error}");
    }

    #[test]
    fn github_tickets_normalize_the_provider_payload() {
        let payload = json!({
            "number": 7,
            "title": "Feed breaks",
            "body": "Steps",
            "state": "open",
            "html_url": "https://github.com/acme/app/issues/7",
            "labels": [{"name": "bug"}],
            "updated_at": "2026-01-01T00:00:00Z",
        });

        let ticket = GitHub.ticket(&payload);

        assert_eq!(ticket.id, "7");
        assert_eq!(ticket.labels, vec!["bug"]);
        assert_eq!(ticket.source, "github");
    }
}
