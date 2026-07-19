//! `repo-task bug` — the bugfix flow.

use anyhow::{bail, Result};
use serde_json::{json, Value};

use crate::commands::work::{fetch, parse_tickets, read_stdin_or_file};
use crate::config;
use crate::connectors::{self, fallback, mcp_json};
use crate::index::families::SYMBOLS_FAMILY;
use crate::kb;
use crate::kb::slicing::build_pack;
use crate::output;
use crate::workflow::analyze::impact_set;
use crate::workflow::dedupe::{build_impacts, cluster};
use crate::workflow::store;

const TRIAGE_CONTRACT: &str =
    "You have the report, the code it points at, and this project's conventions.
Identify the root cause before proposing a fix, and say which of the impacted symbols you
believe is responsible. If the evidence does not support a single root cause, say what
additional signal you need instead of guessing.";

const MERGE_CONTRACT: &str =
    "These tickets resolve to overlapping code. Shared code is not the same as a
shared root cause: confirm each cluster against the reports before recommending a merge,
name the ticket that should survive, and say what the others add that it would lose.";

/// Fetch a bug report together with the code and conventions it implicates.
pub fn bug_fetch(ticket: &str, system: &str, write: &str, budget: Option<usize>) -> Result<()> {
    let config = config::load()?;
    let item = store::work_item(&config, ticket)?;

    // Without a stored report there is nothing to triage: fetch first, and stop there
    // when the connector handed the call to the agent instead of returning the ticket.
    if !write.is_empty() || !item.source_path().is_file() {
        fetch(ticket, system, write)?;
        if !write.is_empty() || !item.source_path().is_file() {
            return Ok(());
        }
    }

    let kb = kb::open(&config, Some(false))?;
    let report = item.require(store::SOURCE, &format!("repo-task bug fetch {ticket}"))?;
    let symbols = kb.facts(SYMBOLS_FAMILY);
    if symbols.is_empty() {
        bail!("No symbol index found. Run `repo-task index` first.");
    }

    let impact = impact_set(&report, &symbols, 25);
    let paths: Vec<String> = impact
        .files
        .iter()
        .take(20)
        .filter_map(|file| file["path"].as_str().map(String::from))
        .collect();
    let pack = build_pack(
        &kb,
        &config.project.stacks,
        "bugfix",
        budget
            .filter(|value| *value > 0)
            .unwrap_or(config.brief.default_budget),
        &paths,
    );

    let data = json!({
        "ticket": ticket,
        "report": report,
        "impact": impact,
        "context": pack,
        "contract": TRIAGE_CONTRACT,
    });
    item.write_json(
        store::ANALYSIS,
        &json!({"ticket": ticket, "impact": impact}),
    )?;
    item.touch_meta(&[("kind", json!("bug")), ("analyzed", json!(true))])?;
    output::emit("bug.fetch", &data, |value| {
        let mut lines = vec![
            format!("Bug {}", value["ticket"].as_str().unwrap_or("")),
            String::new(),
            "Report".to_string(),
            value["report"]
                .as_str()
                .unwrap_or("")
                .trim_end()
                .to_string(),
            String::new(),
            "Impact".to_string(),
        ];
        lines.extend(super::render_impact(&value["impact"]));
        lines.push(String::new());
        lines.push(super::render_pack(&value["context"]));
        lines.push(String::new());
        lines.push(value["contract"].as_str().unwrap_or("").to_string());
        lines.join("\n")
    });
    Ok(())
}

/// Group open bug tickets that resolve to the same code.
pub fn bug_dedupe(
    system: &str,
    query: &str,
    limit: usize,
    threshold: f64,
    tickets_file: &str,
) -> Result<()> {
    let config = config::load()?;
    let kb = kb::open(&config, Some(false))?;
    let symbols = kb.facts(SYMBOLS_FAMILY);
    if symbols.is_empty() {
        bail!("No symbol index found. Run `repo-task index` first.");
    }

    let tickets: Vec<Value> = if !tickets_file.is_empty() {
        parse_tickets(&read_stdin_or_file(tickets_file)?)?
    } else {
        let name = if system.is_empty() {
            connectors::default_system(&config)?
        } else {
            system.to_string()
        };
        let settings = connectors::connector_config(&config, &name)?;
        let connector = connectors::get(&name)?;
        let attempt = fallback::attempt(settings.allows_rest(), settings.allows_mcp(), || {
            connector.list_tickets(&settings, query, limit)
        })?;
        match attempt {
            fallback::Attempt::Rest(tickets) => tickets
                .into_iter()
                .map(|ticket| serde_json::to_value(ticket).unwrap_or_default())
                .collect(),
            fallback::Attempt::FallBack(reason) => {
                let request = connector.mcp_list(&settings, query, limit)?;
                output::warn(format!("Falling back to an MCP call: {reason}"));
                let data = json!({
                    "mode": "mcp",
                    "reason": reason,
                    "request": mcp_json(&request),
                    "nextStep": "repo-task bug dedupe --tickets -",
                    "instruction": "Call the tool above, then pipe the tickets back as JSON: a \
                                    list of objects with id, title, and body.",
                });
                output::emit("bug.dedupe", &data, |value| {
                    format!(
                        "Call {}.{}, then run: {}",
                        value["request"]["server"].as_str().unwrap_or(""),
                        value["request"]["tool"].as_str().unwrap_or(""),
                        value["nextStep"].as_str().unwrap_or(""),
                    )
                });
                return Ok(());
            }
        }
    };

    let impacts = build_impacts(&tickets, &symbols);
    let clusters = cluster(&impacts, threshold);

    let clustered: Vec<String> = clusters
        .iter()
        .filter_map(|group| group["tickets"].as_array())
        .flatten()
        .filter_map(|member| member["id"].as_str().map(String::from))
        .collect();
    let mut unmatched: Vec<String> = impacts
        .iter()
        .map(|item| item.ticket_id.clone())
        .filter(|id| !clustered.contains(id))
        .collect();
    unmatched.sort();
    unmatched.dedup();

    let data = json!({
        "mode": "analyze",
        "ticketCount": tickets.len(),
        "threshold": threshold,
        "clusters": clusters,
        "unmatched": unmatched,
        "contract": MERGE_CONTRACT,
    });
    output::emit("bug.dedupe", &data, render_clusters);
    Ok(())
}

fn render_clusters(value: &Value) -> String {
    let Some(clusters) = value["clusters"].as_array() else {
        return String::new();
    };
    if clusters.is_empty() {
        return format!(
            "No duplicate candidates among {} tickets.",
            value["ticketCount"].as_u64().unwrap_or(0)
        );
    }
    let mut lines = vec!["Possible duplicate clusters".to_string()];
    for group in clusters {
        lines.push(format!(
            "  score {:.2}",
            group["topScore"].as_f64().unwrap_or(0.0)
        ));
        if let Some(tickets) = group["tickets"].as_array() {
            for ticket in tickets {
                lines.push(format!(
                    "    {} {}",
                    ticket["id"].as_str().unwrap_or(""),
                    ticket["title"].as_str().unwrap_or("")
                ));
            }
        }
        if let Some(paths) = group["sharedPaths"].as_array() {
            for path in paths {
                lines.push(format!("      shared: {}", path.as_str().unwrap_or("")));
            }
        }
    }
    lines.join("\n")
}
