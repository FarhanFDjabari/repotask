//! Project setup: `init`, `doctor`, `migrate`.

use std::path::Path;

use anyhow::{bail, Result};
use serde_json::{json, Map, Value};

use crate::config::{self, CONFIG_PATH, LEGACY_CONFIG_PATH, SUPPORTED_SCHEMA_VERSION};
use crate::discovery;
use crate::git;
use crate::kb;
use crate::output;

const GITIGNORE_ENTRIES: &[&str] = &[".repo-task/work/", ".repo-task/cache/"];

const V1_SECTIONS: &[&str] = &[
    "vcs",
    "task_provider",
    "workflow",
    "rules",
    "change_request",
    "agents",
    "branch",
];

fn config_document(
    name: &str,
    stacks: &[String],
    base_branch: &str,
    remote: &str,
    local: &str,
) -> Value {
    let mut knowledge = Map::new();
    knowledge.insert("ref".into(), json!("main"));
    if remote.is_empty() {
        knowledge.insert("local".into(), json!(local));
    } else {
        knowledge.insert("remote".into(), json!(remote));
    }
    json!({
        "schema_version": SUPPORTED_SCHEMA_VERSION,
        "project": {"name": name, "stacks": stacks, "base_branch": base_branch},
        "knowledge": Value::Object(knowledge),
        "connectors": {},
    })
}

fn next_steps(has_remote: bool) -> Vec<String> {
    let mut steps: Vec<String> = Vec::new();
    // A local knowledge base is scaffolded in place; only a remote one needs cloning.
    if has_remote {
        steps.push("repo-task kb sync".into());
    } else {
        steps.push("repo-task kb init".into());
    }
    steps.push("repo-task index".into());
    steps.push("repo-task skills sync".into());
    steps
}

pub fn init(
    remote: &str,
    local: &str,
    stacks: &[String],
    force: bool,
    dry_run: bool,
) -> Result<()> {
    let discovered = discovery::discover()?;
    let path = config::config_path(&discovered.root);
    if path.exists() && !force && !dry_run {
        bail!("{CONFIG_PATH} already exists. Pass --force to overwrite.");
    }

    let chosen: Vec<String> = if stacks.is_empty() {
        discovered.stacks.clone()
    } else {
        stacks.to_vec()
    };
    let document = config_document(
        &discovered.project_name,
        &chosen,
        &discovered.base_branch,
        remote,
        local,
    );
    let content = serde_norway::to_string(&document)?;

    let mut created: Vec<String> = Vec::new();
    if !dry_run {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, &content)?;
        created.push(CONFIG_PATH.into());
        if remote.is_empty() {
            std::fs::create_dir_all(discovered.root.join(local))?;
            created.push(local.into());
        }
        created.extend(ensure_gitignore(&discovered.root)?);
    }

    let data = json!({
        "root": discovered.root.to_string_lossy(),
        "config": document,
        "created": created,
        "dryRun": dry_run,
        "nextSteps": next_steps(!remote.is_empty()),
    });
    output::emit("init", &data, move |value| {
        let prefix = if value["dryRun"].as_bool().unwrap_or(false) {
            "Would write"
        } else {
            "Wrote"
        };
        let steps = value["nextSteps"]
            .as_array()
            .map(|items| {
                items
                    .iter()
                    .enumerate()
                    .map(|(index, step)| {
                        format!("  {}. {}", index + 1, step.as_str().unwrap_or(""))
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default();
        format!("{prefix} {CONFIG_PATH}\n\n{content}\nNext:\n{steps}")
    });
    Ok(())
}

fn ensure_gitignore(root: &Path) -> Result<Vec<String>> {
    let path = root.join(".gitignore");
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    let missing: Vec<&str> = GITIGNORE_ENTRIES
        .iter()
        .filter(|entry| !existing.lines().any(|line| line == **entry))
        .copied()
        .collect();
    if missing.is_empty() {
        return Ok(Vec::new());
    }
    let separator = if existing.is_empty() || existing.ends_with('\n') {
        ""
    } else {
        "\n"
    };
    std::fs::write(
        &path,
        format!("{existing}{separator}{}\n", missing.join("\n")),
    )?;
    Ok(vec![".gitignore".into()])
}

/// Returns false when a check failed, so the caller can set a non-zero exit code.
pub fn doctor() -> Result<bool> {
    let mut checks: Vec<Value> = Vec::new();
    let mut record = |name: &str, ok: bool, detail: String| {
        checks.push(json!({"name": name, "ok": ok, "detail": detail}));
    };

    let config = match config::load() {
        Ok(config) => {
            record(
                "config",
                true,
                config::config_path(&config.root).to_string_lossy().into(),
            );
            config
        }
        Err(error) => {
            record("config", false, error.to_string());
            let data = json!({"ok": false, "checks": checks});
            output::emit("doctor", &data, render_doctor);
            return Ok(false);
        }
    };

    record("stacks", true, config.project.stacks.join(", "));
    match kb::open(&config, Some(false)) {
        Ok(kb) => {
            record(
                "knowledge",
                true,
                format!("{}: {}", kb.source.kind, kb.source.path.display()),
            );
            record(
                "layers",
                !kb.conventions.is_empty() || !kb.recipes.is_empty(),
                format!(
                    "{} conventions, {} recipes, {} fact families",
                    kb.conventions.len(),
                    kb.recipes.len(),
                    kb.fact_families().len()
                ),
            );
            let slices: Vec<String> = kb.slices.keys().cloned().collect();
            record(
                "slices",
                !slices.is_empty(),
                if slices.is_empty() {
                    "no slices/*.yaml found".into()
                } else {
                    slices.join(", ")
                },
            );
        }
        Err(error) => record("knowledge", false, error.to_string()),
    }

    let ok = checks
        .iter()
        .all(|check| check["ok"].as_bool().unwrap_or(false));
    let data = json!({"ok": ok, "checks": checks});
    output::emit("doctor", &data, render_doctor);
    Ok(ok)
}

fn render_doctor(value: &Value) -> String {
    let Some(checks) = value["checks"].as_array() else {
        return String::new();
    };
    let mut lines = vec![output::style::heading("repo-task doctor")];
    for check in checks {
        let ok = check["ok"].as_bool().unwrap_or(false);
        lines.push(format!(
            "  {:<10} {} {}",
            check["name"].as_str().unwrap_or(""),
            if ok {
                output::style::pass("ok  ")
            } else {
                output::style::fail("FAIL")
            },
            check["detail"].as_str().unwrap_or(""),
        ));
    }
    lines.join("\n")
}

pub fn migrate(dry_run: bool) -> Result<()> {
    let root = git::resolve_root(None)?;
    let legacy = root.join(LEGACY_CONFIG_PATH);
    if !legacy.is_file() {
        bail!(
            "No {LEGACY_CONFIG_PATH} found at {}; nothing to migrate.",
            root.display()
        );
    }
    let target = config::config_path(&root);
    if target.exists() && !dry_run {
        bail!("{CONFIG_PATH} already exists; remove it before migrating.");
    }

    let old: Value = serde_norway::from_str(&std::fs::read_to_string(&legacy)?)?;
    let project = &old["project"];
    let stacks: Vec<String> = project["stacks"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_else(|| vec!["generic".into()]);
    let fallback_name = root
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let document = config_document(
        project["name"].as_str().unwrap_or(&fallback_name),
        &stacks,
        project["base_branch"].as_str().unwrap_or("main"),
        "",
        ".repo-task/knowledge",
    );
    let content = serde_norway::to_string(&document)?;
    if !dry_run {
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&target, &content)?;
    }

    let mut dropped: Vec<&str> = old
        .as_object()
        .map(|map| {
            V1_SECTIONS
                .iter()
                .filter(|key| map.contains_key(**key))
                .copied()
                .collect()
        })
        .unwrap_or_default();
    dropped.sort();

    let data = json!({
        "from": LEGACY_CONFIG_PATH,
        "to": CONFIG_PATH,
        "config": document,
        "droppedSections": dropped,
        "dryRun": dry_run,
        "nextSteps": [
            format!("Review and delete {LEGACY_CONFIG_PATH}"),
            "Move your rules/*.md into the knowledge base as conventions/".to_string(),
            "repo-task kb sync".to_string(),
        ],
    });
    output::emit("migrate", &data, move |value| {
        let dropped = value["droppedSections"]
            .as_array()
            .map(|items| {
                let names: Vec<&str> = items.iter().filter_map(|item| item.as_str()).collect();
                if names.is_empty() {
                    "none".to_string()
                } else {
                    names.join(", ")
                }
            })
            .unwrap_or_else(|| "none".into());
        format!(
            "{} {} -> {}\n\n{content}\nDropped v1 sections (now knowledge-base concerns): {dropped}",
            if value["dryRun"].as_bool().unwrap_or(false) { "Would migrate" } else { "Migrated" },
            value["from"].as_str().unwrap_or(""),
            value["to"].as_str().unwrap_or(""),
        )
    });
    Ok(())
}
