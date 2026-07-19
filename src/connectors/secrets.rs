//! Credential lookup for REST connectors.
//!
//! Credentials never live in the project: they come from the environment first, then
//! `~/.repo-task/secrets.yaml`, which is outside version control by construction.

use std::collections::BTreeMap;
use std::path::PathBuf;

use anyhow::{bail, Result};

pub const SECRETS_ENV: &str = "REPOTASK_SECRETS_FILE";

pub fn secrets_path() -> PathBuf {
    if let Ok(override_path) = std::env::var(SECRETS_ENV) {
        return PathBuf::from(override_path);
    }
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".repo-task/secrets.yaml")
}

fn load() -> BTreeMap<String, BTreeMap<String, String>> {
    let path = secrets_path();
    let Ok(text) = std::fs::read_to_string(&path) else {
        return BTreeMap::new();
    };
    serde_norway::from_str(&text).unwrap_or_default()
}

fn env_name(system: &str, key: &str) -> String {
    format!(
        "REPOTASK_{}_{}",
        system.to_uppercase().replace('-', "_"),
        key.to_uppercase()
    )
}

/// Look up `key` for `system`; environment wins over the secrets file.
pub fn get(system: &str, key: &str) -> String {
    if let Ok(value) = std::env::var(env_name(system, key)) {
        if !value.is_empty() {
            return value;
        }
    }
    load()
        .get(system)
        .and_then(|values| values.get(key))
        .cloned()
        .unwrap_or_default()
}

pub fn require(system: &str, key: &str) -> Result<String> {
    let value = get(system, key);
    if value.is_empty() {
        bail!(
            "Missing credential '{key}' for '{system}'. Set {} or add it under '{system}:' in {}.",
            env_name(system, key),
            secrets_path().display()
        );
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn environment_variable_names_are_predictable() {
        assert_eq!(env_name("jira", "token"), "REPOTASK_JIRA_TOKEN");
        assert_eq!(
            env_name("github-enterprise", "token"),
            "REPOTASK_GITHUB_ENTERPRISE_TOKEN"
        );
    }

    #[test]
    fn missing_credentials_name_both_sources() {
        let error = require("jira", "token").unwrap_err().to_string();

        assert!(error.contains("REPOTASK_JIRA_TOKEN"), "{error}");
        assert!(error.contains("secrets.yaml"), "{error}");
    }
}
