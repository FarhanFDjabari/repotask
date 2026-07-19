//! Artifact store for work in progress: `.repo-task/work/<ticket>/`.
//!
//! The CLI owns these files so each workflow step has a durable input for the next
//! one, and so a new agent session can pick the work up without re-fetching.

use std::path::PathBuf;

use anyhow::{bail, Result};
use serde_json::{json, Value};

use crate::config::RepoTaskConfig;
use crate::kb::source::now_iso;

pub const SOURCE: &str = "source.md";
pub const SUMMARY: &str = "summary.md";
pub const ANALYSIS: &str = "analysis.json";
pub const PLAN: &str = "plan.json";
pub const META: &str = "meta.json";

/// Ticket ids become directory names, so keep them boring. Leading and trailing dots
/// are stripped as well as separators, so nothing that reduces to a relative path
/// fragment survives.
pub fn safe_id(ticket_id: &str) -> Result<String> {
    let mapped: String = ticket_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let cleaned = mapped.trim_matches(['-', '.']).to_string();
    if cleaned.is_empty() {
        bail!("Invalid ticket id: {ticket_id:?}");
    }
    Ok(cleaned)
}

pub struct WorkItem {
    pub root: PathBuf,
    pub ticket_id: String,
}

impl WorkItem {
    pub fn source_path(&self) -> PathBuf {
        self.root.join(SOURCE)
    }

    pub fn read(&self, name: &str) -> Option<String> {
        std::fs::read_to_string(self.root.join(name)).ok()
    }

    pub fn read_json(&self, name: &str) -> Value {
        self.read(name)
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_else(|| json!({}))
    }

    pub fn write(&self, name: &str, content: &str) -> Result<String> {
        std::fs::create_dir_all(&self.root)?;
        let path = self.root.join(name);
        let content = if content.ends_with('\n') {
            content.to_string()
        } else {
            format!("{content}\n")
        };
        std::fs::write(&path, content)?;
        Ok(path.to_string_lossy().to_string())
    }

    pub fn write_json(&self, name: &str, data: &Value) -> Result<String> {
        self.write(name, &serde_json::to_string_pretty(data)?)
    }

    pub fn touch_meta(&self, fields: &[(&str, Value)]) -> Result<()> {
        let mut meta = self.read_json(META);
        let map = meta
            .as_object_mut()
            .expect("read_json always yields an object");
        for (key, value) in fields {
            map.insert((*key).to_string(), value.clone());
        }
        map.entry("ticketId")
            .or_insert_with(|| json!(self.ticket_id));
        map.entry("createdAt").or_insert_with(|| json!(now_iso()));
        map.insert("updatedAt".into(), json!(now_iso()));
        self.write_json(META, &meta)?;
        Ok(())
    }

    pub fn require(&self, name: &str, hint: &str) -> Result<String> {
        match self.read(name) {
            Some(content) => Ok(content),
            None => bail!(
                "{} not found. Run `{hint}` first.",
                self.root.join(name).display()
            ),
        }
    }
}

pub fn work_item(config: &RepoTaskConfig, ticket_id: &str) -> Result<WorkItem> {
    Ok(WorkItem {
        root: config.work_dir().join(safe_id(ticket_id)?),
        ticket_id: ticket_id.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::safe_id;

    #[test]
    fn keeps_ordinary_ticket_ids() {
        assert_eq!(safe_id("ACME-12").unwrap(), "ACME-12");
        assert_eq!(safe_id("bug_42.1").unwrap(), "bug_42.1");
    }

    #[test]
    fn replaces_separators() {
        assert_eq!(safe_id("feature/ACME 12").unwrap(), "feature-ACME-12");
    }

    #[test]
    fn rejects_path_traversal() {
        assert!(safe_id("../..").is_err());
        assert!(safe_id("..").is_err());
        assert!(safe_id("/").is_err());
        assert!(safe_id("").is_err());
    }
}
