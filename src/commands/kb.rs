//! `repo-task kb` — knowledge base source management.

use anyhow::{bail, Result};
use serde_json::{json, Value};

use crate::config;
use crate::git;
use crate::kb;
use crate::kb::seed::scaffold;
use crate::kb::source::now_iso;
use crate::kb::store::{KnowledgeBase, FACTS_DIR};
use crate::output;

fn source_data(stacks: &[String], kb: &KnowledgeBase) -> Value {
    let slice = kb.slice_for(stacks);
    json!({
        "source": {
            "kind": kb.source.kind,
            "path": kb.source.path.to_string_lossy(),
            "remote": kb.source.remote,
            "ref": kb.source.r#ref,
            "revision": kb.source.revision,
            "syncedAt": kb.source.synced_at,
        },
        "manifest": {
            "name": kb.manifest.name,
            "schemaVersion": kb.manifest.schema_version,
            "defaultBudget": kb.manifest.default_budget,
        },
        "counts": {
            "conventions": kb.conventions.len(),
            "recipes": kb.recipes.len(),
            "slices": kb.slices.len(),
            "factFamilies": kb.fact_families().len(),
        },
        "stacks": stacks,
        "resolvedSlice": {
            "conventions": slice.conventions,
            "recipes": slice.recipes,
            "facts": slice.facts,
        },
    })
}

fn render(value: &Value) -> String {
    let source = &value["source"];
    let counts = &value["counts"];
    let mut lines = vec![
        format!(
            "Knowledge base: {}",
            value["manifest"]["name"].as_str().unwrap_or("")
        ),
        format!("  Kind           {}", source["kind"].as_str().unwrap_or("")),
        format!("  Path           {}", source["path"].as_str().unwrap_or("")),
    ];
    if !source["remote"].as_str().unwrap_or("").is_empty() {
        lines.push(format!(
            "  Remote         {} @ {}",
            source["remote"].as_str().unwrap_or(""),
            source["ref"].as_str().unwrap_or("")
        ));
        let revision = source["revision"].as_str().unwrap_or("");
        lines.push(format!(
            "  Revision       {}",
            &revision[..revision.len().min(12)]
        ));
        let synced = source["syncedAt"].as_str().unwrap_or("");
        lines.push(format!(
            "  Synced         {}",
            if synced.is_empty() { "never" } else { synced }
        ));
    }
    lines.push(format!("  Conventions    {}", counts["conventions"]));
    lines.push(format!("  Recipes        {}", counts["recipes"]));
    lines.push(format!("  Fact families  {}", counts["factFamilies"]));
    lines.push(format!(
        "  Project stacks {}",
        value["stacks"]
            .as_array()
            .map(|items| items
                .iter()
                .filter_map(|item| item.as_str())
                .collect::<Vec<_>>()
                .join(", "))
            .unwrap_or_default()
    ));
    lines.join("\n")
}

pub fn sync() -> Result<()> {
    let config = config::load()?;
    let kb = kb::open(&config, Some(true))?;
    output::emit("kb.sync", &source_data(&config.project.stacks, &kb), render);
    Ok(())
}

pub fn status() -> Result<()> {
    let config = config::load()?;
    let kb = kb::open(&config, Some(false))?;
    output::emit(
        "kb.status",
        &source_data(&config.project.stacks, &kb),
        render,
    );
    Ok(())
}

/// Scaffold a starter knowledge base. The seed is a starting point, not an answer:
/// the conventions it writes are placeholders for this project's real decisions.
pub fn init(path: &str, force: bool) -> Result<()> {
    let config = config::load()?;
    let relative = if path.is_empty() {
        config.knowledge.local.as_str()
    } else {
        path
    };
    let target = config.root.join(relative);
    let written = scaffold(&target, &config.project.stacks, force)?;

    let data = json!({
        "path": target.to_string_lossy(),
        "stacks": config.project.stacks,
        "written": written,
        "nextSteps": [
            "Edit the conventions to match this project",
            "repo-task index",
            "repo-task skills sync",
        ],
    });
    output::emit("kb.init", &data, |value| {
        let count = value["written"]
            .as_array()
            .map(|items| items.len())
            .unwrap_or(0);
        let steps = value["nextSteps"]
            .as_array()
            .map(|items| {
                items
                    .iter()
                    .map(|step| format!("  - {}", step.as_str().unwrap_or("")))
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default();
        format!(
            "Scaffolded {count} files into {}\n{steps}",
            value["path"].as_str().unwrap_or("")
        )
    });
    Ok(())
}

/// Stage generated facts in the knowledge base worktree and print PR instructions.
/// Never pushes: a human opens the PR/MR so the knowledge base stays reviewed.
pub fn propose(message: &str, branch: &str) -> Result<()> {
    let config = config::load()?;
    let kb = kb::open(&config, Some(false))?;
    if kb.source.kind != "remote" {
        bail!(
            "`kb propose` applies to a remote knowledge base. A local knowledge base is \
             committed with the project itself."
        );
    }
    let path = &kb.source.path;
    if git::run(&["status", "--porcelain", "--", FACTS_DIR], Some(path))?
        .trim()
        .is_empty()
    {
        bail!("No fact changes to propose. Run `repo-task index` first.");
    }

    let stamp: String = now_iso()
        .chars()
        .filter(|c| c.is_ascii_digit())
        .take(14)
        .collect();
    let target = if branch.is_empty() {
        format!("facts/{}-{}", config.project.name, stamp)
    } else {
        branch.to_string()
    };
    git::run(&["checkout", "-B", &target], Some(path))?;
    git::run(&["add", "--", FACTS_DIR], Some(path))?;
    git::run(&["commit", "-m", message], Some(path))?;

    let data = json!({
        "branch": target,
        "path": path.to_string_lossy(),
        "remote": kb.source.remote,
        "commit": git::run(&["rev-parse", "HEAD"], Some(path))?.trim(),
        "nextSteps": [
            format!("git -C {} push -u origin {target}", path.display()),
            "Open a pull/merge request against the knowledge base default branch.".to_string(),
        ],
    });
    output::emit("kb.propose", &data, |value| {
        let steps = value["nextSteps"]
            .as_array()
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.as_str())
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default();
        format!(
            "Committed facts on {}\n{steps}",
            value["branch"].as_str().unwrap_or("")
        )
    });
    Ok(())
}
