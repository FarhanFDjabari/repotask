//! Figma: design structure over REST, distilled for code generation.
//!
//! Figma is not a ticket system, so it does not implement `Connector`. What an agent
//! needs from a design is the structure — frame names, component names, text content,
//! layout, and the variables behind colours and spacing. That arrives as JSON, and a
//! raw Figma node tree is enormous: a single screen easily runs to hundreds of
//! kilobytes of transform matrices and vector geometry that say nothing about how to
//! build it. Filtering that down is exactly what the CLI is for.
//!
//! Images are the exception. The CLI cannot look at a frame, so `image` returns the
//! rendered URL and lets the agent view it directly rather than moving bytes through
//! a process that cannot use them.

use anyhow::{bail, Result};
use serde_json::{json, Map, Value};

use crate::config::ConnectorConfig;
use crate::connectors::rest::{base_url, get_json};
use crate::connectors::{require_project, secrets, McpRequest};

const API: &str = "https://api.figma.com/v1";

fn headers() -> Result<Vec<(&'static str, String)>> {
    Ok(vec![("X-Figma-Token", secrets::require("figma", "token")?)])
}

/// Node types that carry build-relevant meaning. Everything else — vectors, groups,
/// boolean operations — is drawing detail the agent does not need to reproduce.
const MEANINGFUL: &[&str] = &[
    "FRAME",
    "COMPONENT",
    "COMPONENT_SET",
    "INSTANCE",
    "TEXT",
    "SECTION",
];

/// Walk a Figma node tree, keeping the structure and dropping the geometry.
fn distil(node: &Value, depth: usize, max_depth: usize) -> Option<Value> {
    let node_type = node["type"].as_str().unwrap_or_default();
    if depth > max_depth {
        return None;
    }
    if !MEANINGFUL.contains(&node_type) && depth > 0 {
        // Not meaningful itself, but its children might be: flatten through it.
        let children = distil_children(node, depth, max_depth);
        return (!children.is_empty()).then(|| json!(children)).map(flatten);
    }

    let mut kept = Map::new();
    kept.insert("name".into(), node["name"].clone());
    kept.insert("type".into(), json!(node_type));
    if let Some(id) = node["id"].as_str() {
        kept.insert("id".into(), json!(id));
    }
    if node_type == "TEXT" {
        if let Some(text) = node["characters"].as_str() {
            kept.insert("text".into(), json!(text));
        }
    }
    if let Some(component_id) = node["componentId"].as_str() {
        kept.insert("componentId".into(), json!(component_id));
    }
    if let Some(layout) = node["layoutMode"].as_str() {
        kept.insert("layout".into(), json!(layout));
    }

    let children = distil_children(node, depth, max_depth);
    if !children.is_empty() {
        kept.insert("children".into(), json!(children));
    }
    Some(Value::Object(kept))
}

fn distil_children(node: &Value, depth: usize, max_depth: usize) -> Vec<Value> {
    node["children"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|child| distil(child, depth + 1, max_depth))
                .collect()
        })
        .unwrap_or_default()
}

/// A node that flattened to a single child stands in for that child.
fn flatten(value: Value) -> Value {
    match &value {
        Value::Array(items) if items.len() == 1 => items[0].clone(),
        _ => value,
    }
}

/// Read a file key out of `--file`, which takes either the bare key or a pasted link.
///
/// A link is what the user actually has to hand, and every Figma editor puts the key
/// in the same position: `figma.com/<editor>/<key>/<name>`.
pub fn file_key(input: &str) -> Result<String> {
    let Some((_, path)) = input.split_once("figma.com/") else {
        return Ok(input.to_string());
    };
    let mut segments = path.split('/');
    let editor = segments.next().unwrap_or_default();
    let key = segments
        .next()
        .unwrap_or_default()
        .split(['?', '#'])
        .next()
        .unwrap_or_default();
    if !matches!(editor, "design" | "file" | "board" | "slides" | "proto") || key.is_empty() {
        bail!("Could not read a file key from '{input}'. Expected figma.com/design/<key>/<name>.");
    }
    Ok(key.to_string())
}

pub fn file(config: &ConnectorConfig, depth: usize) -> Result<Value> {
    let key = require_project(config, "figma", "file key")?;
    let url = base_url(&config.base_url, API, "figma")?;
    let payload = get_json(
        &format!("{url}/files/{key}"),
        &headers()?,
        &[("depth", "4".into())],
        None,
    )?;

    let pages: Vec<Value> = payload["document"]["children"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|page| distil(page, 0, depth))
                .collect()
        })
        .unwrap_or_default();

    Ok(json!({
        "name": payload["name"],
        "lastModified": payload["lastModified"],
        "pages": pages,
        "components": component_summary(&payload["components"]),
    }))
}

pub fn node(config: &ConnectorConfig, node_id: &str, depth: usize) -> Result<Value> {
    let key = require_project(config, "figma", "file key")?;
    let url = base_url(&config.base_url, API, "figma")?;
    let payload = get_json(
        &format!("{url}/files/{key}/nodes"),
        &headers()?,
        &[("ids", node_id.to_string())],
        None,
    )?;

    let nodes: Vec<Value> = payload["nodes"]
        .as_object()
        .map(|map| {
            map.values()
                .filter_map(|entry| distil(&entry["document"], 0, depth))
                .collect()
        })
        .unwrap_or_default();
    Ok(json!({"nodes": nodes}))
}

/// Design tokens: the variables behind colours, spacing, and typography.
pub fn variables(config: &ConnectorConfig) -> Result<Value> {
    let key = require_project(config, "figma", "file key")?;
    let url = base_url(&config.base_url, API, "figma")?;
    let payload = get_json(
        &format!("{url}/files/{key}/variables/local"),
        &headers()?,
        &[],
        None,
    )?;

    let variables: Vec<Value> = payload["meta"]["variables"]
        .as_object()
        .map(|map| {
            map.values()
                .map(|item| {
                    json!({
                        "name": item["name"],
                        "type": item["resolvedType"],
                        "values": item["valuesByMode"],
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(json!({"variables": variables}))
}

/// Rendered frame URLs. The CLI returns links rather than bytes: it cannot look at a
/// design, and the agent can.
pub fn image(config: &ConnectorConfig, node_id: &str, scale: &str) -> Result<Value> {
    let key = require_project(config, "figma", "file key")?;
    let url = base_url(&config.base_url, API, "figma")?;
    let payload = get_json(
        &format!("{url}/images/{key}"),
        &headers()?,
        &[
            ("ids", node_id.to_string()),
            ("scale", scale.to_string()),
            ("format", "png".into()),
        ],
        None,
    )?;
    Ok(json!({
        "images": payload["images"],
        "instruction": "Open these URLs to view the frames. They expire, so fetch them when you \
                        are ready to look rather than storing them.",
    }))
}

fn component_summary(components: &Value) -> Vec<Value> {
    components
        .as_object()
        .map(|map| {
            map.iter()
                .map(|(id, item)| {
                    json!({
                        "id": id,
                        "name": item["name"],
                        "description": item["description"],
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Name the node the way the target server expects, or not at all.
///
/// The hosted server takes a single `nodeId` on every tool. The desktop bridge reads
/// the current selection instead, so only its screenshot tool names nodes, and it takes
/// a list. A hint carrying an argument the server does not declare is rejected outright,
/// which costs the agent the call and a retry — the fallback is meant to save.
fn node_argument(dialect: &str, tool: &str, node_id: &str) -> Option<(String, Value)> {
    if node_id.is_empty() {
        return None;
    }
    match dialect {
        "bridge" if tool == "get_screenshot" => Some(("nodeIds".into(), json!([node_id]))),
        "bridge" => None,
        _ => Some(("nodeId".into(), json!(node_id))),
    }
}

/// Carry the flags the caller already gave the CLI, where the server declares them.
///
/// Only the bridge does: it takes `depth` when walking the tree and `scale` when
/// rendering. The hosted server's nearest equivalent is `maxDimension`, a pixel cap
/// rather than a multiplier, so `--scale` cannot be handed to it unchanged.
fn tuning(tool: &str, depth: usize, scale: &str) -> Result<Vec<(String, Value)>> {
    match tool {
        "get_design_context" => Ok(vec![("depth".into(), json!(depth))]),
        "get_screenshot" => {
            let Ok(parsed) = scale.parse::<f64>() else {
                bail!("--scale '{scale}' is not a number.");
            };
            Ok(vec![("scale".into(), json!(parsed))])
        }
        _ => Ok(Vec::new()),
    }
}

pub fn mcp_hint(
    config: &ConnectorConfig,
    verb: &str,
    node_id: &str,
    depth: usize,
    scale: &str,
) -> Result<McpRequest> {
    let dialect = config.mcp_dialect.as_str();
    if !["figma", "bridge"].contains(&dialect) {
        bail!("Connector 'figma' has mcp_dialect '{dialect}'; use figma or bridge.");
    }
    let server = if config.mcp_server.is_empty() {
        "figma"
    } else {
        &config.mcp_server
    };
    let tool = match verb {
        "image" => "get_screenshot",
        "variables" => "get_variable_defs",
        _ => "get_design_context",
    };
    // Naming the file keeps the hint pointed at the same design the REST path would
    // have read, rather than whatever the user happens to have open. Both servers call
    // it `fileKey` and reject it empty, so an argument without a value is left out
    // instead of sent blank.
    let mut arguments = Map::new();
    if let Some((name, value)) = node_argument(dialect, tool, node_id) {
        arguments.insert(name, value);
    }
    if dialect == "bridge" {
        for (name, value) in tuning(tool, depth, scale)? {
            arguments.insert(name, value);
        }
    }
    if !config.project.is_empty() {
        arguments.insert("fileKey".into(), json!(config.project));
    }
    Ok(McpRequest {
        server: server.to_string(),
        tool: tool.to_string(),
        arguments: Value::Object(arguments),
        reason: format!("Read the Figma {verb} through your own Figma connection."),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_structure_and_drops_geometry() {
        let node = json!({
            "id": "1:2",
            "name": "Checkout",
            "type": "FRAME",
            "layoutMode": "VERTICAL",
            "absoluteBoundingBox": {"x": 0.0, "y": 0.0},
            "fills": [{"type": "SOLID", "color": {"r": 0.1}}],
            "children": [
                {"id": "1:3", "name": "Title", "type": "TEXT", "characters": "Pay now",
                 "style": {"fontFamily": "Inter"}},
                {"id": "1:4", "name": "decor", "type": "VECTOR", "fillGeometry": [1, 2, 3]}
            ]
        });

        let distilled = distil(&node, 0, 5).unwrap();

        assert_eq!(distilled["name"], "Checkout");
        assert_eq!(distilled["layout"], "VERTICAL");
        assert!(distilled.get("fills").is_none(), "geometry must be dropped");
        assert!(distilled.get("absoluteBoundingBox").is_none());
        let children = distilled["children"].as_array().unwrap();
        assert_eq!(
            children.len(),
            1,
            "the vector child carries no build meaning"
        );
        assert_eq!(children[0]["text"], "Pay now");
    }

    #[test]
    fn keeps_component_instances_and_their_ids() {
        let node = json!({
            "id": "1:2", "name": "Root", "type": "FRAME",
            "children": [
                {"id": "1:5", "name": "PrimaryButton", "type": "INSTANCE", "componentId": "9:1"}
            ]
        });

        let distilled = distil(&node, 0, 5).unwrap();

        let child = &distilled["children"][0];
        assert_eq!(child["name"], "PrimaryButton");
        assert_eq!(child["componentId"], "9:1");
    }

    #[test]
    fn flattens_through_meaningless_wrappers() {
        let node = json!({
            "id": "1:2", "name": "Root", "type": "FRAME",
            "children": [
                {"id": "1:6", "name": "Group", "type": "GROUP", "children": [
                    {"id": "1:7", "name": "Label", "type": "TEXT", "characters": "Hi"}
                ]}
            ]
        });

        let distilled = distil(&node, 0, 5).unwrap();

        assert_eq!(
            distilled["children"][0]["name"], "Label",
            "the group is not build-relevant"
        );
    }

    #[test]
    fn respects_the_depth_limit() {
        let node = json!({
            "id": "1", "name": "A", "type": "FRAME",
            "children": [{"id": "2", "name": "B", "type": "FRAME",
                "children": [{"id": "3", "name": "C", "type": "FRAME"}]}]
        });

        let distilled = distil(&node, 0, 1).unwrap();

        assert_eq!(distilled["children"][0]["name"], "B");
        assert!(
            distilled["children"][0].get("children").is_none(),
            "depth 2 is beyond the limit"
        );
    }

    #[test]
    fn summarizes_the_component_catalogue() {
        let components = json!({
            "9:1": {"name": "Button/Primary", "description": "Main call to action"},
        });

        let summary = component_summary(&components);

        assert_eq!(summary[0]["name"], "Button/Primary");
        assert_eq!(summary[0]["id"], "9:1");
    }

    #[test]
    fn mcp_hints_map_verbs_to_figma_tools() {
        let config = ConnectorConfig {
            project: "abc".into(),
            ..Default::default()
        };

        assert_eq!(
            mcp_hint(&config, "image", "1:2", 0, "2").unwrap().tool,
            "get_screenshot"
        );
        assert_eq!(
            mcp_hint(&config, "variables", "1:2", 0, "2").unwrap().tool,
            "get_variable_defs"
        );
        assert_eq!(
            mcp_hint(&config, "file", "1:2", 3, "2").unwrap().tool,
            "get_design_context"
        );
    }

    #[test]
    fn mcp_hints_name_the_file_they_meant() {
        let config = ConnectorConfig {
            project: "abc".into(),
            ..Default::default()
        };

        let hint = mcp_hint(&config, "node", "1:2", 5, "2").unwrap();

        assert_eq!(hint.arguments["fileKey"], "abc");
        assert_eq!(hint.arguments["nodeId"], "1:2");
    }

    #[test]
    fn mcp_hints_omit_arguments_that_have_no_value() {
        let hint = mcp_hint(&ConnectorConfig::default(), "file", "", 3, "2").unwrap();

        assert_eq!(
            hint.arguments,
            json!({}),
            "an empty argument is rejected by the server, so it is not sent"
        );
    }

    fn bridge() -> ConnectorConfig {
        ConnectorConfig {
            project: "abc".into(),
            mcp_dialect: "bridge".into(),
            ..Default::default()
        }
    }

    #[test]
    fn the_bridge_takes_a_list_of_nodes_to_screenshot() {
        let hint = mcp_hint(&bridge(), "image", "1:2", 0, "3").unwrap();

        assert_eq!(
            hint.arguments,
            json!({"nodeIds": ["1:2"], "scale": 3.0, "fileKey": "abc"})
        );
    }

    #[test]
    fn the_bridge_reads_its_other_tools_off_the_selection() {
        for verb in ["file", "node"] {
            let hint = mcp_hint(&bridge(), verb, "1:2", 5, "2").unwrap();

            assert_eq!(
                hint.arguments,
                json!({"depth": 5, "fileKey": "abc"}),
                "{verb} names no node on the bridge"
            );
        }

        let hint = mcp_hint(&bridge(), "variables", "1:2", 0, "2").unwrap();

        assert_eq!(
            hint.arguments,
            json!({"fileKey": "abc"}),
            "variables takes neither a node nor a depth"
        );
    }

    #[test]
    fn the_hosted_dialect_takes_neither_depth_nor_scale() {
        let config = ConnectorConfig {
            project: "abc".into(),
            ..Default::default()
        };

        let hint = mcp_hint(&config, "image", "1:2", 5, "3").unwrap();

        assert_eq!(
            hint.arguments,
            json!({"nodeId": "1:2", "fileKey": "abc"}),
            "the hosted server declares maxDimension, which --scale is not"
        );
    }

    #[test]
    fn a_scale_that_is_not_a_number_is_rejected() {
        let error = mcp_hint(&bridge(), "image", "1:2", 0, "2x")
            .unwrap_err()
            .to_string();

        assert!(error.contains("--scale '2x' is not a number"), "{error}");
    }

    #[test]
    fn an_unknown_dialect_is_rejected() {
        let config = ConnectorConfig {
            mcp_dialect: "sketch".into(),
            ..Default::default()
        };

        let error = mcp_hint(&config, "file", "1:2", 3, "2")
            .unwrap_err()
            .to_string();

        assert!(error.contains("use figma or bridge"), "{error}");
    }

    #[test]
    fn a_bare_key_is_left_alone() {
        assert_eq!(file_key("AbC123XyZ890").unwrap(), "AbC123XyZ890");
    }

    #[test]
    fn a_link_yields_its_key() {
        let key = file_key("https://www.figma.com/design/AbC123XyZ890/Checkout-Flow").unwrap();

        assert_eq!(key, "AbC123XyZ890");
    }

    #[test]
    fn a_key_is_read_before_the_query_string() {
        let key = file_key("https://www.figma.com/design/AbC123XyZ890?node-id=1-2").unwrap();

        assert_eq!(key, "AbC123XyZ890");
    }

    #[test]
    fn every_editor_puts_the_key_in_the_same_place() {
        for editor in ["design", "file", "board", "slides", "proto"] {
            let link = format!("https://www.figma.com/{editor}/AbC123XyZ890/Name");

            assert_eq!(file_key(&link).unwrap(), "AbC123XyZ890", "{editor}");
        }
    }

    #[test]
    fn a_link_without_a_key_is_rejected() {
        let error = file_key("https://www.figma.com/design/")
            .unwrap_err()
            .to_string();

        assert!(error.contains("figma.com/design/<key>"), "{error}");
    }
}
