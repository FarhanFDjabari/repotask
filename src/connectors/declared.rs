//! Config-declared REST calls.
//!
//! A system with no built-in connector — and often no MCP server either — is still
//! reachable by declaring the call in `.repo-task/config.yaml`. The CLI performs it
//! and projects the response down to the declared fields, so the agent receives the
//! distilled result rather than the whole payload.

use anyhow::{bail, Result};
use serde_json::{json, Map, Value};

use crate::config::{ConnectorConfig, VerbConfig};
use crate::connectors::rest::{base_url, get_json};
use crate::connectors::{secrets, McpRequest};

/// Fill `{name}` placeholders from `--arg name=value`, percent-encoding each value so
/// a caller cannot smuggle a path segment or query string into the URL.
pub fn render(template: &str, args: &Map<String, Value>) -> Result<String> {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        let Some(end) = rest[start..].find('}').map(|offset| start + offset) else {
            bail!("Unclosed placeholder in '{template}'");
        };
        let name = &rest[start + 1..end];
        let Some(value) = args.get(name) else {
            bail!("Missing --arg {name} for '{template}'");
        };
        out.push_str(&rest[..start]);
        out.push_str(&encode(&scalar(value)));
        rest = &rest[end + 1..];
    }
    out.push_str(rest);
    Ok(out)
}

fn scalar(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

fn encode(value: &str) -> String {
    value
        .bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (byte as char).to_string()
            }
            other => format!("%{other:02X}"),
        })
        .collect()
}

/// Keep only the declared dotted paths. `data.items.name` walks objects and maps over
/// arrays, so one path can project a field out of every element of a list.
pub fn project(value: &Value, fields: &[String]) -> Value {
    if fields.is_empty() {
        return value.clone();
    }
    let mut out = Map::new();
    for field in fields {
        if let Some(found) = pluck(value, &field.split('.').collect::<Vec<_>>()) {
            out.insert(field.clone(), found);
        }
    }
    Value::Object(out)
}

fn pluck(value: &Value, path: &[&str]) -> Option<Value> {
    let Some((head, tail)) = path.split_first() else {
        return Some(value.clone());
    };
    match value {
        Value::Array(items) => {
            let collected: Vec<Value> = items.iter().filter_map(|item| pluck(item, path)).collect();
            (!collected.is_empty()).then_some(Value::Array(collected))
        }
        Value::Object(map) => map.get(*head).and_then(|next| pluck(next, tail)),
        _ => None,
    }
}

fn auth_headers(system: &str, config: &ConnectorConfig) -> Vec<(&'static str, String)> {
    if config.auth_header.is_empty() {
        return Vec::new();
    }
    let token = secrets::get(system, "token");
    if token.is_empty() {
        return Vec::new();
    }
    let format = if config.auth_format.is_empty() {
        "{token}"
    } else {
        &config.auth_format
    };
    // The header name comes from user config, so it must outlive this call as owned
    // data; leaking one small string per process is simpler than threading lifetimes.
    let name: &'static str = Box::leak(config.auth_header.clone().into_boxed_str());
    vec![(name, format.replace("{token}", &token))]
}

/// A fully rendered request.
///
/// Building is separate from sending on purpose: a missing argument or a malformed
/// path is the caller's mistake, and handing it to the agent to retry over MCP would
/// just repeat it with the same broken inputs. Only sending is worth falling back on.
pub struct Request {
    pub url: String,
    pub query: Vec<(String, String)>,
}

pub fn build(
    system: &str,
    config: &ConnectorConfig,
    verb: &VerbConfig,
    args: &Map<String, Value>,
) -> Result<Request> {
    let path = render(&verb.path, args)?;
    let url = format!(
        "{}/{}",
        base_url(&config.base_url, "", system)?,
        path.trim_start_matches('/')
    );
    let query = verb
        .query
        .iter()
        .map(|(key, template)| Ok((key.clone(), render(template, args)?)))
        .collect::<Result<Vec<_>>>()?;
    Ok(Request { url, query })
}

pub fn send(
    system: &str,
    config: &ConnectorConfig,
    verb: &VerbConfig,
    request: &Request,
) -> Result<Value> {
    let query: Vec<(&str, String)> = request
        .query
        .iter()
        .map(|(key, value)| (key.as_str(), value.clone()))
        .collect();
    let payload = get_json(&request.url, &auth_headers(system, config), &query, None)?;
    Ok(project(&payload, &verb.fields))
}

pub fn mcp_request(
    system: &str,
    config: &ConnectorConfig,
    verb_name: &str,
    verb: &VerbConfig,
    args: &Map<String, Value>,
) -> Result<McpRequest> {
    if verb.mcp_tool.is_empty() {
        bail!(
            "Verb '{verb_name}' on '{system}' has no `mcp_tool` to fall back to. Add one, or \
             configure the REST call so the CLI can make it."
        );
    }
    Ok(McpRequest {
        server: if config.mcp_server.is_empty() {
            system.to_string()
        } else {
            config.mcp_server.clone()
        },
        tool: verb.mcp_tool.clone(),
        arguments: Value::Object(args.clone()),
        reason: format!("Run '{verb_name}' on {system} through your own connection."),
    })
}

/// Parse repeated `--arg key=value` pairs.
pub fn parse_args(pairs: &[String]) -> Result<Map<String, Value>> {
    let mut args = Map::new();
    for pair in pairs {
        let Some((key, value)) = pair.split_once('=') else {
            bail!("Arguments are `key=value`; got '{pair}'.");
        };
        args.insert(key.to_string(), json!(value));
    }
    Ok(args)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(pairs: &[(&str, &str)]) -> Map<String, Value> {
        pairs
            .iter()
            .map(|(key, value)| ((*key).to_string(), json!(value)))
            .collect()
    }

    #[test]
    fn renders_placeholders() {
        let path = render("/files/{key}/nodes", &args(&[("key", "abc123")])).unwrap();

        assert_eq!(path, "/files/abc123/nodes");
    }

    #[test]
    fn encodes_argument_values() {
        let path = render("/search/{query}", &args(&[("query", "a/b c")])).unwrap();

        assert_eq!(
            path, "/search/a%2Fb%20c",
            "an argument must not add path segments"
        );
    }

    #[test]
    fn missing_arguments_are_named() {
        let error = render("/files/{key}", &args(&[])).unwrap_err().to_string();

        assert!(error.contains("--arg key"), "{error}");
    }

    #[test]
    fn unclosed_placeholders_are_rejected() {
        assert!(render("/files/{key", &args(&[])).is_err());
    }

    #[test]
    fn projection_keeps_only_declared_fields() {
        let payload = json!({"name": "Checkout", "huge": {"nested": [1, 2, 3]}, "id": "7"});

        let projected = project(&payload, &["name".into(), "id".into()]);

        assert_eq!(projected, json!({"name": "Checkout", "id": "7"}));
    }

    #[test]
    fn projection_maps_over_arrays() {
        let payload = json!({"items": [{"name": "a", "drop": 1}, {"name": "b", "drop": 2}]});

        let projected = project(&payload, &["items.name".into()]);

        assert_eq!(projected, json!({"items.name": ["a", "b"]}));
    }

    #[test]
    fn projection_without_fields_keeps_everything() {
        let payload = json!({"a": 1});

        assert_eq!(project(&payload, &[]), payload);
    }

    #[test]
    fn projection_skips_absent_paths() {
        let projected = project(&json!({"a": 1}), &["missing".into()]);

        assert_eq!(projected, json!({}));
    }

    #[test]
    fn parses_argument_pairs() {
        let parsed = parse_args(&["key=abc".into(), "node=1:2".into()]).unwrap();

        assert_eq!(parsed["key"], "abc");
        assert_eq!(parsed["node"], "1:2");
    }

    #[test]
    fn rejects_arguments_without_a_value() {
        assert!(parse_args(&["broken".into()]).is_err());
    }

    #[test]
    fn a_verb_without_an_mcp_tool_says_so() {
        let config = ConnectorConfig {
            base_url: "https://x".into(),
            ..Default::default()
        };
        let verb = VerbConfig {
            path: "/a".into(),
            ..VerbConfig::default()
        };

        let error = mcp_request("figma", &config, "file", &verb, &args(&[]))
            .unwrap_err()
            .to_string();

        assert!(error.contains("mcp_tool"), "{error}");
    }
}
