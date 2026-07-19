//! `repo-task skills` — generate the agent-facing entry points.

use anyhow::Result;
use serde_json::{json, Value};

use crate::config;
use crate::output;
use crate::skills;

pub fn sync(dry_run: bool) -> Result<()> {
    let config = config::load()?;
    let results = skills::sync(&config, dry_run)?;
    let others = skills::find_agent_files(&config.root);

    let data = json!({
        "dryRun": dry_run,
        "files": results
            .iter()
            .map(|item| json!({"path": item.path, "action": item.action}))
            .collect::<Vec<_>>(),
        "otherAgentFiles": others,
        "note": "Skills reference commands only — no project knowledge is copied into them, \
                 so they stay correct as the knowledge base changes.",
    });
    output::emit("skills.sync", &data, |value| {
        let mut lines = vec![format!(
            "Generated agent skills{}",
            if value["dryRun"].as_bool().unwrap_or(false) {
                " (dry run)"
            } else {
                ""
            }
        )];
        if let Some(files) = value["files"].as_array() {
            for file in files {
                lines.push(format!(
                    "  {:<9} {}",
                    file["action"].as_str().unwrap_or(""),
                    file["path"].as_str().unwrap_or("")
                ));
            }
        }
        if let Some(others) = value["otherAgentFiles"].as_array() {
            if !others.is_empty() {
                let names: Vec<&str> = others.iter().filter_map(|item| item.as_str()).collect();
                lines.push(format!(
                    "\nOther agent instruction files found: {}. Point them at AGENTS.md.",
                    names.join(", ")
                ));
            }
        }
        lines.join("\n")
    });
    Ok(())
}
