//! `repo-task design` — read design structure, and map it to this project's code.
//!
//! The fetching half is REST-first for the same reason as tickets: the CLI distils a
//! Figma node tree down to structure, so the agent pays context for the result rather
//! than the payload. The mapping half is knowledge, and lives in the knowledge base
//! next to the code components the index already knows about.

use anyhow::Result;
use serde_json::json;

use crate::config;
use crate::connectors::{connector_config, fallback, figma, mcp_json};
use crate::index::families::SYMBOLS_FAMILY;
use crate::kb;
use crate::kb::slicing::build_pack;
use crate::output;
use crate::workflow::analyze::impact_set;

const IMPLEMENT_CONTRACT: &str =
    "You have the design structure, this project's matching code components, and the
conventions for building a screen. Reuse the components the index already lists rather than
creating near-duplicates, and name new ones the way their siblings are named. When the design
and the code disagree about a component's shape, say so instead of silently following one.";

pub fn design(verb: &str, node: &str, depth: usize, scale: &str, file: &str) -> Result<()> {
    let config = config::load()?;
    let mut settings = connector_config(&config, "figma")?;
    // `--file` reaches a file the config does not name, which is most of them: the
    // configured key is the project's own design file, not every file the user can open.
    if !file.is_empty() {
        settings.project = figma::file_key(file)?;
    }

    let attempt = fallback::attempt(
        settings.allows_rest(),
        settings.allows_mcp(),
        || match verb {
            "file" => figma::file(&settings, depth),
            "node" => figma::node(&settings, node, depth),
            "variables" => figma::variables(&settings),
            "image" => figma::image(&settings, node, scale),
            other => {
                anyhow::bail!("Unknown design verb '{other}'. Use file, node, variables, or image.")
            }
        },
    )?;

    let structure = match attempt {
        fallback::Attempt::Rest(value) => value,
        fallback::Attempt::FallBack(reason) => {
            let request = figma::mcp_hint(&settings, verb, node);
            output::warn(format!("Falling back to an MCP call: {reason}"));
            let data = json!({
                "verb": verb,
                "mode": "mcp",
                "reason": reason,
                "request": mcp_json(&request),
                "instruction": "Read the design through your own Figma connection, then run \
                                `repo-task design map` to line it up with this project's code.",
            });
            output::emit("design", &data, |value| {
                format!(
                    "Call {}.{} with {}",
                    value["request"]["server"].as_str().unwrap_or(""),
                    value["request"]["tool"].as_str().unwrap_or(""),
                    value["request"]["arguments"],
                )
            });
            return Ok(());
        }
    };

    let data = json!({"verb": verb, "mode": "rest", "design": structure});
    output::emit("design", &data, |value| {
        serde_json::to_string_pretty(&value["design"]).unwrap_or_default()
    });
    Ok(())
}

/// Line design component names up against the indexed code components.
///
/// This is the half the agent cannot get from Figma: which of *this project's*
/// components already implements a given design component, and what the project's
/// conventions say about building the ones that do not exist yet.
pub fn map(names: &[String], budget: Option<usize>) -> Result<()> {
    let config = config::load()?;
    let kb = kb::open(&config, Some(false))?;
    let symbols = kb.facts(SYMBOLS_FAMILY);
    if symbols.is_empty() {
        anyhow::bail!("No symbol index found. Run `repo-task index` first.");
    }

    // Design component names are often `Button/Primary`; the segments are what match
    // code identifiers, so search on the whole path and each of its parts.
    let vocabulary = names
        .iter()
        .flat_map(|name| {
            let mut parts: Vec<String> = name
                .split(['/', '.'])
                .map(|part| part.trim().to_string())
                .collect();
            parts.push(name.clone());
            parts
        })
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ");

    let impact = impact_set(&vocabulary, &symbols, 40);
    let matched: Vec<&str> = impact
        .files
        .iter()
        .filter_map(|file| file["path"].as_str())
        .collect();

    let pack = build_pack(
        &kb,
        &config.project.stacks,
        "feature",
        budget
            .filter(|value| *value > 0)
            .unwrap_or(config.brief.default_budget),
        &impact
            .files
            .iter()
            .filter_map(|f| f["path"].as_str().map(String::from))
            .collect::<Vec<_>>(),
    );

    let unmatched: Vec<&String> = names
        .iter()
        .filter(|name| {
            let lowered = name.to_lowercase();
            !impact.symbols.iter().any(|symbol| {
                symbol["name"]
                    .as_str()
                    .map(|found| {
                        lowered.split(['/', '.']).any(|part| {
                            !part.trim().is_empty() && found.to_lowercase().contains(part.trim())
                        })
                    })
                    .unwrap_or(false)
            })
        })
        .collect();

    let data = json!({
        "designComponents": names,
        "existingCode": impact.symbols,
        "files": matched,
        "unmatched": unmatched,
        "context": pack,
        "contract": IMPLEMENT_CONTRACT,
    });
    output::emit("design.map", &data, |value| {
        let mut lines = vec!["Design component -> code".to_string()];
        if let Some(symbols) = value["existingCode"].as_array() {
            for symbol in symbols {
                lines.push(format!(
                    "  {:<28} {}",
                    symbol["name"].as_str().unwrap_or(""),
                    symbol["path"].as_str().unwrap_or(""),
                ));
            }
        }
        if let Some(unmatched) = value["unmatched"].as_array() {
            if !unmatched.is_empty() {
                let names: Vec<&str> = unmatched.iter().filter_map(|item| item.as_str()).collect();
                lines.push(format!("\nNo code found for: {}", names.join(", ")));
            }
        }
        lines.join("\n")
    });
    Ok(())
}
