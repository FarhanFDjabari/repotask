//! Spike command surface: `index` and `brief`.

use anyhow::{bail, Result};
use serde_json::{json, Value};

use crate::config;
use crate::git;
use crate::index::families::{build_all, SYMBOLS_DESCRIPTION, SYMBOLS_FAMILY};
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

    output::emit("brief", &data, super::render_pack);
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

/// Read a curated project fact family produced by `index`.
pub fn fact(family: &str, query: &str, limit: usize) -> Result<()> {
    let config = config::load()?;
    let kb = kb::open(&config, Some(false))?;
    let available = kb.fact_families();

    if family.is_empty() {
        let mut families: Vec<Value> = kb
            .manifest
            .fact_families
            .iter()
            .map(|item| {
                json!({
                    "name": item.name,
                    "description": item.description,
                    "indexed": available.contains(&item.name),
                })
            })
            .collect();
        // `symbols` is generated by every index rather than declared in kb.yaml, so it
        // would otherwise be missing from the one listing that advertises what `fact`
        // accepts — while the not-indexed error already names it.
        if available.iter().any(|name| name == SYMBOLS_FAMILY) {
            families.push(json!({
                "name": SYMBOLS_FAMILY,
                "description": SYMBOLS_DESCRIPTION,
                "indexed": true,
            }));
        }
        let data = json!({
            "families": families,
            "available": available,
        });
        output::emit("fact", &data, |value| {
            let mut lines = vec!["Fact families".to_string()];
            if let Some(families) = value["families"].as_array() {
                for item in families {
                    lines.push(format!(
                        "  {:<16} {:<4} {}",
                        item["name"].as_str().unwrap_or(""),
                        if item["indexed"].as_bool().unwrap_or(false) {
                            "yes"
                        } else {
                            "no"
                        },
                        item["description"].as_str().unwrap_or(""),
                    ));
                }
            }
            lines.join("\n")
        });
        return Ok(());
    }

    if !available.iter().any(|name| name == family) {
        bail!(
            "Fact family '{family}' has not been indexed. Available: {}. Run `repo-task index`.",
            if available.is_empty() {
                "none".to_string()
            } else {
                available.join(", ")
            }
        );
    }
    let mut entries = kb.facts(family);
    if !query.is_empty() {
        let lowered = query.to_lowercase();
        entries.retain(|entry| {
            entry["name"]
                .as_str()
                .unwrap_or_default()
                .to_lowercase()
                .contains(&lowered)
        });
    }
    let count = entries.len();
    entries.truncate(limit);

    let data = json!({"family": family, "count": count, "entries": entries});
    output::emit("fact", &data, |value| {
        let Some(entries) = value["entries"].as_array() else {
            return String::new();
        };
        if entries.is_empty() {
            return format!(
                "No entries in '{}'.",
                value["family"].as_str().unwrap_or("")
            );
        }
        let mut lines = vec![format!(
            "{} ({})",
            value["family"].as_str().unwrap_or(""),
            value["count"]
        )];
        for entry in entries {
            lines.push(format!(
                "  {:<28} {:<10} {}:{}",
                entry["name"].as_str().unwrap_or(""),
                entry["kind"].as_str().unwrap_or(""),
                entry["path"].as_str().unwrap_or(""),
                entry["line"],
            ));
        }
        lines.join("\n")
    });
    Ok(())
}
