//! Project configuration (schema version 2).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::git;

pub const SUPPORTED_SCHEMA_VERSION: u32 = 2;
pub const CONFIG_PATH: &str = ".repo-task/config.yaml";
pub const LEGACY_CONFIG_PATH: &str = ".repo-task.yml";

pub const STACKS: &[&str] = &[
    "generic",
    "android",
    "kotlin",
    "jetpack-compose",
    "ios",
    "swift",
    "swiftui",
    "flutter",
    "dart",
    "react-native",
    "typescript",
    "web",
    "python",
    "go",
    "rust",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectConfig {
    pub name: String,
    pub stacks: Vec<String>,
    #[serde(default = "default_base_branch")]
    pub base_branch: String,
}

fn default_base_branch() -> String {
    "main".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KnowledgeConfig {
    #[serde(default)]
    pub remote: String,
    #[serde(default = "default_ref")]
    pub r#ref: String,
    #[serde(default = "default_local")]
    pub local: String,
    #[serde(default = "default_true")]
    pub auto_sync: bool,
}

fn default_ref() -> String {
    "main".into()
}

fn default_local() -> String {
    ".repo-task/knowledge".into()
}

fn default_true() -> bool {
    true
}

impl Default for KnowledgeConfig {
    fn default() -> Self {
        Self {
            remote: String::new(),
            r#ref: default_ref(),
            local: default_local(),
            auto_sync: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IndexConfig {
    #[serde(default = "default_exclude")]
    pub exclude: Vec<String>,
    #[serde(default = "default_max_file_bytes")]
    pub max_file_bytes: u64,
}

fn default_exclude() -> Vec<String> {
    [
        "**/build/**",
        "**/node_modules/**",
        "**/.git/**",
        "**/Pods/**",
    ]
    .iter()
    .map(|value| value.to_string())
    .collect()
}

fn default_max_file_bytes() -> u64 {
    512_000
}

impl Default for IndexConfig {
    fn default() -> Self {
        Self {
            exclude: default_exclude(),
            max_file_bytes: default_max_file_bytes(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BriefConfig {
    #[serde(default = "default_budget")]
    pub default_budget: usize,
}

fn default_budget() -> usize {
    6000
}

impl Default for BriefConfig {
    fn default() -> Self {
        Self {
            default_budget: default_budget(),
        }
    }
}

/// One externally declared REST call.
///
/// Declared in the project config so a system with no built-in connector — and no
/// MCP server — is still reachable without shipping Rust for it.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerbConfig {
    /// Path appended to the connector's `base_url`. `{name}` placeholders are filled
    /// from `--arg name=value` and percent-encoded. Omitted by a verb that only exists
    /// over MCP, which declares `mcp_tool` instead.
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub query: BTreeMap<String, String>,
    /// Dotted paths to keep from the response, e.g. `data.items.name`. Empty keeps
    /// everything — the point of projecting is that the agent sees the distilled
    /// result rather than the whole payload.
    #[serde(default)]
    pub fields: Vec<String>,
    /// MCP tool to fall back to when the REST call cannot be made.
    #[serde(default)]
    pub mcp_tool: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConnectorConfig {
    /// `auto` (REST when it can, MCP hint otherwise), `rest`, `mcp`, or `off`.
    #[serde(default = "default_mode")]
    pub mode: String,
    #[serde(default)]
    pub base_url: String,
    #[serde(default)]
    pub project: String,
    #[serde(default)]
    pub mcp_server: String,
    /// Which server's argument names the MCP hint should use. Servers exposing the
    /// same tool disagree on its arguments, and the CLI never connects to one, so it
    /// cannot discover the shape and has to be told.
    #[serde(default = "default_dialect")]
    pub mcp_dialect: String,
    /// Header name carrying the credential, when the system is config-declared.
    #[serde(default)]
    pub auth_header: String,
    /// Format for the credential value; `{token}` is substituted.
    #[serde(default)]
    pub auth_format: String,
    #[serde(default)]
    pub verbs: BTreeMap<String, VerbConfig>,
}

impl Default for ConnectorConfig {
    fn default() -> Self {
        Self {
            mode: default_mode(),
            base_url: String::new(),
            project: String::new(),
            mcp_server: String::new(),
            mcp_dialect: default_dialect(),
            auth_header: String::new(),
            auth_format: String::new(),
            verbs: BTreeMap::new(),
        }
    }
}

impl ConnectorConfig {
    pub fn allows_rest(&self) -> bool {
        self.mode == "auto" || self.mode == "rest"
    }

    pub fn allows_mcp(&self) -> bool {
        self.mode == "auto" || self.mode == "mcp"
    }
}

/// REST first: the CLI fetches and distils, so the agent only pays for the result.
fn default_mode() -> String {
    "auto".into()
}

/// The hosted server is the one most agents are already connected to.
fn default_dialect() -> String {
    "figma".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepoTaskConfig {
    pub schema_version: u32,
    pub project: ProjectConfig,
    #[serde(default)]
    pub knowledge: KnowledgeConfig,
    #[serde(default)]
    pub index: IndexConfig,
    #[serde(default)]
    pub brief: BriefConfig,
    #[serde(default)]
    pub connectors: BTreeMap<String, ConnectorConfig>,

    #[serde(skip)]
    pub root: PathBuf,
}

impl RepoTaskConfig {
    pub fn work_dir(&self) -> PathBuf {
        self.root.join(".repo-task/work")
    }

    fn validate(&self) -> Result<()> {
        if self.schema_version != SUPPORTED_SCHEMA_VERSION {
            bail!(
                "unsupported schema_version {}; this build supports {}. Run `repo-task migrate`.",
                self.schema_version,
                SUPPORTED_SCHEMA_VERSION
            );
        }
        if self.project.stacks.is_empty() {
            bail!("Invalid configuration field 'project.stacks': at least one stack is required");
        }
        let unknown: Vec<&str> = self
            .project
            .stacks
            .iter()
            .filter(|stack| !STACKS.contains(&stack.as_str()))
            .map(String::as_str)
            .collect();
        if !unknown.is_empty() {
            bail!(
                "Invalid configuration field 'project.stacks': unsupported values {}",
                unknown.join(", ")
            );
        }
        if !self.knowledge.remote.is_empty()
            && !["https://", "ssh://", "git@", "file://", "/"]
                .iter()
                .any(|prefix| self.knowledge.remote.starts_with(prefix))
        {
            bail!(
                "Invalid configuration field 'knowledge.remote': {}",
                self.knowledge.remote
            );
        }
        for (system, connector) in &self.connectors {
            for (verb, settings) in &connector.verbs {
                if !settings.path.is_empty() {
                    continue;
                }
                let field = format!("connectors.{system}.verbs.{verb}");
                if settings.mcp_tool.is_empty() {
                    bail!(
                        "Invalid configuration field '{field}': declare `path` for a REST call, \
                         or `mcp_tool` for one the agent runs over MCP"
                    );
                }
                if connector.mode == "rest" {
                    bail!("Invalid configuration field '{field}': mode 'rest' needs a `path`");
                }
            }
        }
        Ok(())
    }
}

pub fn config_path(root: &Path) -> PathBuf {
    root.join(CONFIG_PATH)
}

pub fn load_from(root: &Path) -> Result<RepoTaskConfig> {
    let path = config_path(root);
    if !path.exists() {
        if root.join(LEGACY_CONFIG_PATH).exists() {
            bail!(
                "Found legacy {LEGACY_CONFIG_PATH} but no {CONFIG_PATH}. \
                 Run `repo-task migrate` to upgrade to schema version 2."
            );
        }
        bail!(
            "{CONFIG_PATH} not found at Git root {}. Run `repo-task init`.",
            root.display()
        );
    }
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("Could not read {}", path.display()))?;
    let mut config: RepoTaskConfig = serde_norway::from_str(&text)
        .with_context(|| format!("Invalid configuration in {}", path.display()))?;
    config.root = root.to_path_buf();
    config.validate()?;
    Ok(config)
}

pub fn load() -> Result<RepoTaskConfig> {
    let root = git::resolve_root(None)?;
    load_from(&root)
}
