//! CLI command groups.

use serde_json::Value;

pub mod bug;
pub mod connect;
pub mod design;
pub mod facts;
pub mod kb;
pub mod query;
pub mod setup;
pub mod skills;
pub mod work;

/// Render a `ContextPack` for humans: the budget line, then each document in full.
///
/// Shared by `brief` and by the commands that hand back a pack alongside other
/// evidence (`summarize`, `bug fetch`), so the pack reads the same everywhere.
pub(crate) fn render_pack(value: &Value) -> String {
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

/// Render an impact set's files and symbols as indented rows.
pub(crate) fn render_impact(value: &Value) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    if let Some(files) = value["files"].as_array() {
        for file in files {
            lines.push(format!(
                "  {:<12} {:<13} {}",
                file["module"].as_str().unwrap_or(""),
                file["layer"].as_str().unwrap_or(""),
                file["path"].as_str().unwrap_or(""),
            ));
        }
    }
    if let Some(symbols) = value["symbols"].as_array() {
        for symbol in symbols {
            lines.push(format!(
                "  {:<28} {:<10} {}:{}",
                symbol["name"].as_str().unwrap_or(""),
                symbol["kind"].as_str().unwrap_or(""),
                symbol["path"].as_str().unwrap_or(""),
                symbol["line"],
            ));
        }
    }
    lines
}
