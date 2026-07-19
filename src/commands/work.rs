//! Feature workflow: `fetch` -> `summarize` -> `analyze` -> `split`.
//!
//! None of these steps call a model. Each one gathers the inputs for a decision and
//! states the contract the agent should satisfy, then stores what the agent writes back.

use std::io::Read;

use anyhow::{bail, Result};
use serde_json::{json, Value};

use crate::config;
use crate::connectors::{self, fallback, mcp_json};
use crate::index::families::SYMBOLS_FAMILY;
use crate::kb;
use crate::kb::slicing::build_pack;
use crate::output;
use crate::workflow::analyze::{impact_set, split_increments, Impact};
use crate::workflow::store;

const SUMMARY_CONTRACT: &str =
    "Condense the source above into the context this project actually needs.
Keep only what affects these stacks: {stacks}. Drop platform notes, market copy, and
requirements that belong to other clients. Preserve every acceptance criterion that
survives that filter. Write the result back with:

    repo-task summarize {ticket} --write -

Use these sections: ## Problem, ## Scope, ## Acceptance criteria, ## Out of scope,
## Open questions.";

const EFFORT_RUBRIC: &str = "Estimate effort from the evidence, not from the prose:
- S: one module, one layer, no schema or API change
- M: one module across layers, or two modules in one layer
- L: several modules, a schema/migration, or a public API change
- XL: cross-cutting change, or the impact set is too diffuse to bound — split it first
State the band, the two or three facts that drove it, and what would change it.";

const SPLIT_CONTRACT: &str =
    "Turn these increments into steps that each land as one reviewable change.
Every step must build and be independently revertible. Merge steps that cannot stand alone,
and split any step whose diff would be hard to review. Keep data-layer work ahead of the UI
that consumes it.";

/// `-` means stdin, which is how an agent writes its output back.
pub fn read_stdin_or_file(value: &str) -> Result<String> {
    if value == "-" {
        let mut buffer = String::new();
        std::io::stdin().read_to_string(&mut buffer)?;
        if buffer.trim().is_empty() {
            bail!("Nothing was provided on stdin.");
        }
        return Ok(buffer);
    }
    std::fs::read_to_string(value).map_err(|_| anyhow::anyhow!("File not found: {value}"))
}

pub fn fetch(ticket: &str, system: &str, write: &str) -> Result<()> {
    let config = config::load()?;
    let item = store::work_item(&config, ticket)?;

    if !write.is_empty() {
        let content = read_stdin_or_file(write)?;
        let stored = item.write(store::SOURCE, &content)?;
        item.touch_meta(&[("source", json!("agent"))])?;
        let data =
            json!({"ticket": ticket, "stored": stored, "bytes": content.len(), "mode": "write"});
        output::emit("fetch", &data, |value| {
            format!(
                "Stored {} bytes in {}",
                value["bytes"],
                value["stored"].as_str().unwrap_or("")
            )
        });
        return Ok(());
    }

    let name = if system.is_empty() {
        connectors::default_system(&config)?
    } else {
        system.to_string()
    };
    let settings = connectors::connector_config(&config, &name)?;
    let connector = connectors::get(&name)?;

    // REST first: the CLI fetches and stores the ticket, so the agent never pays
    // context for the raw payload. MCP is the fallback for when the CLI cannot call.
    let attempt = fallback::attempt(settings.allows_rest(), settings.allows_mcp(), || {
        connector.fetch_ticket(&settings, ticket)
    })?;

    let ticket_data = match attempt {
        fallback::Attempt::Rest(ticket_data) => ticket_data,
        fallback::Attempt::FallBack(reason) => {
            let request = connector.mcp_fetch(&settings, ticket)?;
            output::warn(format!("Falling back to an MCP call: {reason}"));
            let data = json!({
                "ticket": ticket,
                "mode": "mcp",
                "reason": reason,
                "request": mcp_json(&request),
                "nextStep": format!("repo-task fetch {ticket} --write -"),
                "instruction": "Call the tool above yourself, then pipe the ticket text back \
                                into the next step so later commands can use it.",
            });
            output::emit("fetch", &data, |value| {
                format!(
                    "Call {}.{} with {}, then run: {}",
                    value["request"]["server"].as_str().unwrap_or(""),
                    value["request"]["tool"].as_str().unwrap_or(""),
                    value["request"]["arguments"],
                    value["nextStep"].as_str().unwrap_or(""),
                )
            });
            return Ok(());
        }
    };
    let stored = item.write(store::SOURCE, &ticket_data.as_markdown())?;
    item.touch_meta(&[
        ("source", json!(name)),
        ("title", json!(ticket_data.title)),
        ("url", json!(ticket_data.url)),
    ])?;
    let data =
        json!({"ticket": ticket, "mode": "rest", "stored": stored, "ticketData": ticket_data});
    output::emit("fetch", &data, |value| {
        format!(
            "Fetched {} -> {}",
            value["ticketData"]["title"].as_str().unwrap_or(""),
            value["stored"].as_str().unwrap_or("")
        )
    });
    Ok(())
}

pub fn summarize(ticket: &str, write: &str, budget: Option<usize>) -> Result<()> {
    let config = config::load()?;
    let item = store::work_item(&config, ticket)?;

    if !write.is_empty() {
        let content = read_stdin_or_file(write)?;
        let stored = item.write(store::SUMMARY, &content)?;
        item.touch_meta(&[("summarized", json!(true))])?;
        let data = json!({"ticket": ticket, "stored": stored, "mode": "write"});
        output::emit("summarize", &data, |value| {
            format!(
                "Stored summary in {}",
                value["stored"].as_str().unwrap_or("")
            )
        });
        return Ok(());
    }

    let kb = kb::open(&config, Some(false))?;
    let source = item.require(store::SOURCE, &format!("repo-task fetch {ticket}"))?;
    let limit = budget
        .filter(|value| *value > 0)
        .unwrap_or(config.brief.default_budget);
    let pack = build_pack(&kb, &config.project.stacks, "summarize", limit, &[]);
    let contract = SUMMARY_CONTRACT
        .replace("{stacks}", &config.project.stacks.join(", "))
        .replace("{ticket}", ticket);

    let data = json!({
        "ticket": ticket,
        "mode": "prepare",
        "stacks": config.project.stacks,
        "source": source,
        "context": pack,
        "contract": contract,
        "nextStep": format!("repo-task summarize {ticket} --write -"),
    });
    output::emit("summarize", &data, |value| {
        let mut lines = vec![
            format!(
                "Summarize {}  (stacks: {})",
                value["ticket"].as_str().unwrap_or(""),
                value["stacks"]
                    .as_array()
                    .map(|items| items
                        .iter()
                        .filter_map(|item| item.as_str())
                        .collect::<Vec<_>>()
                        .join(", "))
                    .unwrap_or_default(),
            ),
            String::new(),
            "Source".to_string(),
            value["source"]
                .as_str()
                .unwrap_or("")
                .trim_end()
                .to_string(),
            String::new(),
        ];
        lines.push(super::render_pack(&value["context"]));
        lines.push(String::new());
        lines.push(value["contract"].as_str().unwrap_or("").to_string());
        lines.join("\n")
    });
    Ok(())
}

pub fn analyze(ticket: &str, limit: usize) -> Result<()> {
    let config = config::load()?;
    let kb = kb::open(&config, Some(false))?;
    let item = store::work_item(&config, ticket)?;

    let summary = item.read(store::SUMMARY);
    let text = match &summary {
        Some(text) => text.clone(),
        None => item.require(store::SOURCE, &format!("repo-task fetch {ticket}"))?,
    };

    let symbols = kb.facts(SYMBOLS_FAMILY);
    if symbols.is_empty() {
        bail!("No symbol index found. Run `repo-task index` first.");
    }

    let impact = impact_set(&text, &symbols, 25);
    let data = json!({
        "ticket": ticket,
        "basedOn": if summary.is_some() { store::SUMMARY } else { store::SOURCE },
        "impact": {
            "terms": impact.terms,
            "files": impact.files.iter().take(limit).collect::<Vec<_>>(),
            "symbols": impact.symbols.iter().take(limit).collect::<Vec<_>>(),
            "modules": impact.modules,
        },
        "counts": {
            "files": impact.files.len(),
            "symbols": impact.symbols.len(),
            "modules": impact.modules.len(),
        },
        "rubric": EFFORT_RUBRIC,
        "nextStep": format!("repo-task split {ticket}"),
    });
    item.write_json(store::ANALYSIS, &data)?;
    item.touch_meta(&[("analyzed", json!(true))])?;
    output::emit("analyze", &data, |value| {
        let mut lines = vec![format!(
            "Impact for {}",
            value["ticket"].as_str().unwrap_or("")
        )];
        if let Some(files) = value["impact"]["files"].as_array() {
            for file in files {
                lines.push(format!(
                    "  {:<12} {:<13} {}",
                    file["module"].as_str().unwrap_or(""),
                    file["layer"].as_str().unwrap_or(""),
                    file["path"].as_str().unwrap_or(""),
                ));
            }
        }
        lines.join("\n")
    });
    Ok(())
}

pub fn split(ticket: &str, max_files: usize) -> Result<()> {
    let config = config::load()?;
    let item = store::work_item(&config, ticket)?;
    let analysis = item.read_json(store::ANALYSIS);
    if analysis.get("impact").is_none() {
        bail!("No analysis found. Run `repo-task analyze {ticket}` first.");
    }

    let impact = Impact {
        terms: analysis["impact"]["terms"]
            .as_array()
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default(),
        files: analysis["impact"]["files"]
            .as_array()
            .cloned()
            .unwrap_or_default(),
        symbols: analysis["impact"]["symbols"]
            .as_array()
            .cloned()
            .unwrap_or_default(),
        modules: serde_json::from_value(analysis["impact"]["modules"].clone()).unwrap_or_default(),
    };
    let increments = split_increments(&impact, max_files);
    let data = json!({"ticket": ticket, "increments": increments, "contract": SPLIT_CONTRACT});
    item.write_json(store::PLAN, &data)?;
    item.touch_meta(&[("split", json!(true))])?;
    output::emit("split", &data, |value| {
        let mut lines = vec![format!(
            "Proposed increments for {}",
            value["ticket"].as_str().unwrap_or("")
        )];
        if let Some(increments) = value["increments"].as_array() {
            for increment in increments {
                lines.push(format!(
                    "  {}. {}",
                    increment["step"],
                    increment["title"].as_str().unwrap_or("")
                ));
                if let Some(paths) = increment["paths"].as_array() {
                    for path in paths {
                        lines.push(format!("       {}", path.as_str().unwrap_or("")));
                    }
                }
            }
        }
        lines.join("\n")
    });
    Ok(())
}

/// Shared by the bug flow, which accepts the same JSON ticket list.
pub fn parse_tickets(text: &str) -> Result<Vec<Value>> {
    let payload: Value = serde_json::from_str(text)
        .map_err(|error| anyhow::anyhow!("Could not parse tickets as JSON: {error}"))?;
    let items = match &payload {
        Value::Object(map) => map.get("tickets").cloned().unwrap_or(payload.clone()),
        other => other.clone(),
    };
    let Value::Array(items) = items else {
        bail!("Tickets must be a JSON list of objects with id, title, and body.");
    };
    items
        .into_iter()
        .map(|item| {
            let id = item["id"]
                .as_str()
                .map(String::from)
                .or_else(|| item["id"].as_i64().map(|value| value.to_string()));
            let Some(id) = id.filter(|value| !value.is_empty()) else {
                bail!("Every ticket needs at least an 'id'.");
            };
            Ok(json!({
                "id": id,
                "title": item["title"].as_str().unwrap_or_default(),
                "body": item["body"].as_str().unwrap_or_default(),
            }))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::parse_tickets;

    #[test]
    fn accepts_a_bare_list() {
        let tickets = parse_tickets(r#"[{"id": "A", "title": "t", "body": "b"}]"#).unwrap();

        assert_eq!(tickets.len(), 1);
        assert_eq!(tickets[0]["id"], "A");
    }

    #[test]
    fn accepts_a_wrapped_list() {
        let tickets = parse_tickets(r#"{"tickets": [{"id": "A"}]}"#).unwrap();

        assert_eq!(tickets[0]["title"], "");
    }

    #[test]
    fn accepts_numeric_ids() {
        let tickets = parse_tickets(r#"[{"id": 42}]"#).unwrap();

        assert_eq!(tickets[0]["id"], "42");
    }

    #[test]
    fn rejects_tickets_without_an_id() {
        assert!(parse_tickets(r#"[{"title": "no id"}]"#).is_err());
    }

    #[test]
    fn rejects_malformed_json() {
        assert!(parse_tickets("not json").is_err());
    }
}
