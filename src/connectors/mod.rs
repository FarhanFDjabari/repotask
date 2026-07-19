//! Connectors to external ticket and document systems.
//!
//! Two backends per system:
//!
//! * `rest` — the CLI calls the API itself. Cheap, deterministic, no agent tokens.
//! * `mcp` — the CLI calls nothing and instead returns the MCP tool and arguments for
//!   the *agent* to invoke, reusing the user's existing connection.
//!
//! Which one runs is a per-system configuration choice, not a path the agent picks.

pub mod providers;
pub mod rest;
pub mod secrets;

use anyhow::{bail, Result};
use serde::Serialize;
use serde_json::{json, Value};

use crate::config::{ConnectorConfig, RepoTaskConfig};

/// A ticket or PRD normalized across providers.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Ticket {
    pub id: String,
    pub title: String,
    pub body: String,
    pub status: String,
    pub url: String,
    pub labels: Vec<String>,
    pub updated_at: String,
    pub source: String,
}

impl Ticket {
    pub fn as_markdown(&self) -> String {
        let heading = if self.title.is_empty() {
            &self.id
        } else {
            &self.title
        };
        let mut lines = vec![
            format!("# {heading}"),
            String::new(),
            format!("- id: {}", self.id),
        ];
        if !self.status.is_empty() {
            lines.push(format!("- status: {}", self.status));
        }
        if !self.url.is_empty() {
            lines.push(format!("- url: {}", self.url));
        }
        if !self.labels.is_empty() {
            lines.push(format!("- labels: {}", self.labels.join(", ")));
        }
        lines.push(String::new());
        lines.push(self.body.trim().to_string());
        lines.push(String::new());
        lines.join("\n")
    }
}

/// An instruction for the agent to make the call itself.
#[derive(Debug, Clone, Serialize)]
pub struct McpRequest {
    pub server: String,
    pub tool: String,
    pub arguments: Value,
    pub reason: String,
}

pub trait Connector {
    fn name(&self) -> &'static str;
    fn fetch_ticket(&self, config: &ConnectorConfig, ticket_id: &str) -> Result<Ticket>;
    fn list_tickets(
        &self,
        config: &ConnectorConfig,
        query: &str,
        limit: usize,
    ) -> Result<Vec<Ticket>>;
    fn mcp_fetch(&self, config: &ConnectorConfig, ticket_id: &str) -> Result<McpRequest>;
    fn mcp_list(&self, config: &ConnectorConfig, query: &str, limit: usize) -> Result<McpRequest>;
}

pub const CONNECTOR_NAMES: &[&str] = &["jira", "github", "gitlab", "clickup"];

pub fn get(system: &str) -> Result<Box<dyn Connector>> {
    Ok(match system {
        "jira" => Box::new(providers::Jira),
        "github" => Box::new(providers::GitHub),
        "gitlab" => Box::new(providers::GitLab),
        "clickup" => Box::new(providers::ClickUp),
        other => bail!(
            "Unknown system '{other}'. Available: {}.",
            CONNECTOR_NAMES.join(", ")
        ),
    })
}

pub fn connector_config(config: &RepoTaskConfig, system: &str) -> Result<ConnectorConfig> {
    let Some(settings) = config.connectors.get(system) else {
        bail!(
            "Connector '{system}' is not configured. Add it under `connectors:` in \
             .repo-task/config.yaml."
        );
    };
    if settings.mode == "off" {
        bail!("Connector '{system}' is disabled.");
    }
    Ok(settings.clone())
}

/// The single enabled connector, when the user has not named one.
pub fn default_system(config: &RepoTaskConfig) -> Result<String> {
    let mut enabled: Vec<&String> = config
        .connectors
        .iter()
        .filter(|(_, settings)| settings.mode != "off")
        .map(|(name, _)| name)
        .collect();
    enabled.sort();
    match enabled.len() {
        0 => bail!(
            "No connectors are configured. Add one under `connectors:` in .repo-task/config.yaml."
        ),
        1 => Ok(enabled[0].clone()),
        _ => {
            let names: Vec<&str> = enabled.iter().map(|name| name.as_str()).collect();
            bail!(
                "Several connectors are enabled ({}). Pass --system.",
                names.join(", ")
            )
        }
    }
}

pub fn mcp_json(request: &McpRequest) -> Value {
    json!({
        "server": request.server,
        "tool": request.tool,
        "arguments": request.arguments,
        "reason": request.reason,
    })
}

pub fn require_project(config: &ConnectorConfig, system: &str, shape: &str) -> Result<String> {
    if config.project.is_empty() {
        bail!("Connector '{system}' needs `project` ({shape}) in .repo-task/config.yaml.");
    }
    Ok(config.project.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn config_with(modes: &[(&str, &str)]) -> RepoTaskConfig {
        let mut connectors: BTreeMap<String, ConnectorConfig> = BTreeMap::new();
        for (name, mode) in modes {
            connectors.insert(
                (*name).to_string(),
                ConnectorConfig {
                    mode: (*mode).to_string(),
                    base_url: String::new(),
                    project: String::new(),
                    mcp_server: String::new(),
                },
            );
        }
        RepoTaskConfig {
            schema_version: 2,
            project: crate::config::ProjectConfig {
                name: "app".into(),
                stacks: vec!["android".into()],
                base_branch: "main".into(),
            },
            knowledge: Default::default(),
            index: Default::default(),
            brief: Default::default(),
            connectors,
            root: std::path::PathBuf::from("."),
        }
    }

    #[test]
    fn unknown_systems_list_the_available_ones() {
        let error = match get("bugzilla") {
            Err(error) => error.to_string(),
            Ok(_) => panic!("expected an unknown-system error"),
        };

        assert!(error.contains("jira"), "{error}");
    }

    #[test]
    fn default_system_picks_the_only_enabled_connector() {
        let config = config_with(&[("jira", "rest"), ("github", "off")]);

        assert_eq!(default_system(&config).unwrap(), "jira");
    }

    #[test]
    fn default_system_refuses_to_guess_between_two() {
        let config = config_with(&[("jira", "rest"), ("github", "mcp")]);

        let error = default_system(&config).unwrap_err().to_string();

        assert!(error.contains("--system"), "{error}");
    }

    #[test]
    fn unconfigured_connectors_say_where_to_add_them() {
        let config = config_with(&[]);

        let error = connector_config(&config, "jira").unwrap_err().to_string();

        assert!(error.contains(".repo-task/config.yaml"), "{error}");
    }

    #[test]
    fn ticket_markdown_carries_the_metadata_agents_need() {
        let ticket = Ticket {
            id: "ACME-1".into(),
            title: "Paginate the feed".into(),
            body: "Body text".into(),
            status: "In Progress".into(),
            url: "https://example/ACME-1".into(),
            labels: vec!["mobile".into()],
            ..Ticket::default()
        };

        let markdown = ticket.as_markdown();

        assert!(markdown.starts_with("# Paginate the feed"));
        assert!(markdown.contains("- id: ACME-1"));
        assert!(markdown.contains("- status: In Progress"));
        assert!(markdown.contains("- labels: mobile"));
        assert!(markdown.contains("Body text"));
    }

    #[test]
    fn ticket_markdown_falls_back_to_the_id_without_a_title() {
        let ticket = Ticket {
            id: "ACME-2".into(),
            ..Ticket::default()
        };

        assert!(ticket.as_markdown().starts_with("# ACME-2"));
    }
}
