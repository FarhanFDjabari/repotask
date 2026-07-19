//! Shared HTTP plumbing for REST connectors.
//!
//! `ureq` rather than `reqwest`: these are a handful of blocking GETs, and pulling in
//! an async runtime for them would cost more binary than the feature is worth.

use anyhow::{bail, Result};
use serde_json::Value;

pub fn base_url(configured: &str, fallback: &str, system: &str) -> Result<String> {
    let url = if configured.is_empty() {
        fallback
    } else {
        configured
    };
    let url = url.trim_end_matches('/');
    if url.is_empty() {
        bail!("Connector '{system}' needs a base_url in .repo-task/config.yaml.");
    }
    Ok(url.to_string())
}

/// GET and decode JSON, converting transport and status failures into user errors.
pub fn get_json(
    url: &str,
    headers: &[(&str, String)],
    params: &[(&str, String)],
    basic_auth: Option<(&str, &str)>,
) -> Result<Value> {
    let mut request = ureq::get(url);
    for (key, value) in headers {
        request = request.header(*key, value);
    }
    if let Some((user, password)) = basic_auth {
        let encoded = base64(format!("{user}:{password}").as_bytes());
        request = request.header("Authorization", &format!("Basic {encoded}"));
    }
    for (key, value) in params {
        request = request.query(*key, value);
    }

    match request.call() {
        Ok(mut response) => {
            let body = response.body_mut().read_to_string()?;
            serde_json::from_str(&body)
                .map_err(|error| anyhow::anyhow!("{url} did not return JSON: {error}"))
        }
        Err(ureq::Error::StatusCode(code)) => match code {
            401 | 403 => {
                bail!("{url} rejected the credentials ({code}). Check your token scope.")
            }
            404 => bail!("Not found: {url}"),
            other => bail!("{url} returned {other}"),
        },
        Err(error) => bail!("Request to {url} failed: {error}"),
    }
}

/// Small base64 encoder; avoids a dependency for one Authorization header.
fn base64(input: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let bytes = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let packed = ((bytes[0] as u32) << 16) | ((bytes[1] as u32) << 8) | bytes[2] as u32;
        out.push(ALPHABET[(packed >> 18) as usize & 63] as char);
        out.push(ALPHABET[(packed >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            ALPHABET[(packed >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[packed as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_basic_auth_pairs() {
        assert_eq!(base64(b"user:pass"), "dXNlcjpwYXNz");
        assert_eq!(base64(b"a"), "YQ==");
        assert_eq!(base64(b"ab"), "YWI=");
        assert_eq!(base64(b"abc"), "YWJj");
    }

    #[test]
    fn base_url_prefers_the_configured_value() {
        assert_eq!(
            base_url("https://acme.example/", "https://fallback", "jira").unwrap(),
            "https://acme.example"
        );
        assert_eq!(
            base_url("", "https://fallback", "github").unwrap(),
            "https://fallback"
        );
    }

    #[test]
    fn base_url_without_a_fallback_is_an_error() {
        let error = base_url("", "", "jira").unwrap_err().to_string();

        assert!(error.contains("base_url"), "{error}");
    }
}
