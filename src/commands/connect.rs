//! `repo-task connect <system> <verb>` — call a config-declared external system.
//!
//! REST first, so the CLI performs the call and hands the agent only the projected
//! result. When the CLI cannot make the call, it returns the declared MCP tool for
//! the agent to run through its own connection.

use anyhow::{bail, Result};
use serde_json::json;

use crate::config;
use crate::connectors::{connector_config, declared, fallback, mcp_json};
use crate::output;

pub fn connect(system: &str, verb_name: &str, args: &[String]) -> Result<()> {
    let config = config::load()?;
    let settings = connector_config(&config, system)?;

    if verb_name.is_empty() {
        let mut verbs: Vec<&String> = settings.verbs.keys().collect();
        verbs.sort();
        let data = json!({
            "system": system,
            "mode": settings.mode,
            "verbs": settings.verbs.iter().map(|(name, verb)| json!({
                "verb": name,
                "path": verb.path,
                "fields": verb.fields,
                "mcpTool": verb.mcp_tool,
            })).collect::<Vec<_>>(),
        });
        output::emit("connect", &data, |value| {
            let names: Vec<&str> = value["verbs"]
                .as_array()
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|item| item["verb"].as_str())
                        .collect()
                })
                .unwrap_or_default();
            if names.is_empty() {
                format!("No verbs declared for '{system}'.")
            } else {
                format!("Verbs for {system}: {}", names.join(", "))
            }
        });
        return Ok(());
    }

    let Some(verb) = settings.verbs.get(verb_name) else {
        let mut available: Vec<&str> = settings.verbs.keys().map(String::as_str).collect();
        available.sort();
        bail!(
            "'{system}' has no verb '{verb_name}'. Declared: {}.",
            if available.is_empty() {
                "none".to_string()
            } else {
                available.join(", ")
            }
        );
    };
    let arguments = declared::parse_args(args)?;
    // Build before attempting: an unfilled placeholder is the caller's mistake, and
    // retrying it through the agent would send the same incomplete arguments.
    let request = declared::build(system, &settings, verb, &arguments)?;

    let attempt = fallback::attempt(settings.allows_rest(), settings.allows_mcp(), || {
        declared::send(system, &settings, verb, &request)
    })?;

    match attempt {
        fallback::Attempt::Rest(result) => {
            let data = json!({
                "system": system,
                "verb": verb_name,
                "mode": "rest",
                "arguments": arguments,
                "result": result,
            });
            output::emit("connect", &data, |value| {
                serde_json::to_string_pretty(&value["result"]).unwrap_or_default()
            });
        }
        fallback::Attempt::FallBack(reason) => {
            let request = declared::mcp_request(system, &settings, verb_name, verb, &arguments)?;
            output::warn(format!("Falling back to an MCP call: {reason}"));
            let data = json!({
                "system": system,
                "verb": verb_name,
                "mode": "mcp",
                "reason": reason,
                "request": mcp_json(&request),
                "instruction": "Call the tool above through your own connection. The CLI could \
                                not make the REST call itself.",
            });
            output::emit("connect", &data, |value| {
                format!(
                    "Call {}.{} with {}",
                    value["request"]["server"].as_str().unwrap_or(""),
                    value["request"]["tool"].as_str().unwrap_or(""),
                    value["request"]["arguments"],
                )
            });
        }
    }
    Ok(())
}
