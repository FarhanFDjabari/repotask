//! Spike command surface: `index` and `brief`.

use anyhow::{bail, Result};
use serde_json::{json, Value};

use crate::config;
use crate::git;
use crate::index::families::{build_all, SYMBOLS_FAMILY};
use crate::index::runner::build_index;
use crate::kb;
use crate::kb::slicing::build_pack;
use crate::kb::store::FACTS_DIR;
use crate::output;

pub fn index(changed_only: bool) -> Result<()> {
    let config = config::load()?;
    let kb = kb::open(&config, Some(false))?;

    let paths = git::changed_paths(&config.root, &config.project.base_branch, true);
    if changed_only && paths.is_empty() {
        bail!(
            "No files changed against {}; nothing to index.",
            config.project.base_branch
        );
    }
    let result = build_index(&config, changed_only.then_some(paths.as_slice()))?;
    let families = build_all(&kb.manifest.fact_families, &result)?;

    let mut written: Vec<String> = Vec::new();
    let mut counts = serde_json::Map::new();
    for (name, entries) in &families {
        let path = kb.facts_path(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let document = json!({
            "family": name,
            "project": config.project.name,
            "count": entries.len(),
            "entries": entries,
        });
        std::fs::write(
            &path,
            format!("{}\n", serde_json::to_string_pretty(&document)?),
        )?;
        written.push(format!("{FACTS_DIR}/{name}.json"));
        counts.insert(name.clone(), json!(entries.len()));
    }

    let data = json!({
        "filesScanned": result.files_scanned,
        "filesSkipped": result.files_skipped,
        "languages": result.languages,
        "symbols": result.symbols.len(),
        "families": counts,
        "written": written,
        "knowledgeBase": kb.source.path.to_string_lossy(),
        "nextSteps": if kb.source.kind == "remote" {
            vec!["repo-task kb propose"]
        } else {
            vec!["Commit the facts/ changes"]
        },
    });

    output::emit("index", &data, |value| {
        let mut lines = vec!["Indexed project facts".to_string()];
        if let Some(families) = value["families"].as_object() {
            for (name, count) in families {
                lines.push(format!("  {name:<16} {count}"));
            }
        }
        lines.join("\n")
    });
    Ok(())
}

pub fn brief(intent: &str, paths: &[String], changed: bool, budget: Option<usize>) -> Result<()> {
    let config = config::load()?;
    let kb = kb::open(&config, Some(false))?;

    let mut relevant: Vec<String> = paths.to_vec();
    if changed {
        relevant.extend(git::changed_paths(
            &config.root,
            &config.project.base_branch,
            true,
        ));
    }
    let limit = budget
        .filter(|value| *value > 0)
        .unwrap_or(config.brief.default_budget)
        .max(1);
    let pack = build_pack(&kb, &config.project.stacks, intent, limit, &relevant);

    let mut data = serde_json::to_value(&pack)?;
    data["stacks"] = json!(config.project.stacks);
    data["paths"] = json!(relevant);

    output::emit("brief", &data, render_brief);
    Ok(())
}

pub fn symbol(query: &str, kind: Option<&str>, limit: usize) -> Result<()> {
    let config = config::load()?;
    let kb = kb::open(&config, Some(false))?;
    let entries = kb.facts(SYMBOLS_FAMILY);
    if entries.is_empty() {
        bail!("No symbol index found. Run `repo-task index` first.");
    }
    let pattern = regex::RegexBuilder::new(query)
        .case_insensitive(true)
        .build()
        .map_err(|error| anyhow::anyhow!("Invalid search pattern: {error}"))?;

    let hits: Vec<&Value> = entries
        .iter()
        .filter(|entry| {
            let name = entry["name"].as_str().unwrap_or_default();
            let matches_kind = kind.is_none_or(|value| entry["kind"].as_str() == Some(value));
            pattern.is_match(name) && matches_kind
        })
        .take(limit)
        .collect();

    let data = json!({"query": query, "kind": kind.unwrap_or_default(), "hits": hits});
    output::emit("symbol", &data, |value| {
        let Some(hits) = value["hits"].as_array() else {
            return String::new();
        };
        if hits.is_empty() {
            return format!(
                "No symbol matched '{}'.",
                value["query"].as_str().unwrap_or("")
            );
        }
        hits.iter()
            .map(|hit| {
                format!(
                    "{:<28} {:<10} {}:{}",
                    hit["name"].as_str().unwrap_or(""),
                    hit["kind"].as_str().unwrap_or(""),
                    hit["path"].as_str().unwrap_or(""),
                    hit["line"]
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    });
    Ok(())
}

fn render_brief(value: &Value) -> String {
    let mut lines = vec![
        format!("Context pack: {}", value["intent"].as_str().unwrap_or("")),
        format!("Budget: {} / {} tokens", value["used"], value["budget"]),
    ];
    if let Some(documents) = value["documents"].as_array() {
        for document in documents {
            lines.push(format!(
                "\n# {}  ({}, matched on {})\n{}",
                document["title"].as_str().unwrap_or(""),
                document["path"].as_str().unwrap_or(""),
                document["reasons"]
                    .as_array()
                    .map(|items| items
                        .iter()
                        .filter_map(|item| item.as_str())
                        .collect::<Vec<_>>()
                        .join(", "))
                    .unwrap_or_default(),
                document["body"].as_str().unwrap_or(""),
            ));
        }
    }
    if let Some(omitted) = value["omitted"].as_array() {
        if !omitted.is_empty() {
            lines.push(format!("\nOmitted: {omitted:?}"));
        }
    }
    lines.join("\n")
}
