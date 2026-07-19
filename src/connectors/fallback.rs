//! REST-first execution with an MCP fallback.
//!
//! REST is the cheaper path: the CLI performs the call and distils the response, so
//! the agent only pays context for the result rather than the whole payload. MCP is
//! the fallback for when the CLI *cannot* make the call — no credential, no network,
//! a server that is down — because the agent's own connection may still succeed.
//!
//! The distinction that matters: fall back when the CLI could not get an answer, not
//! when it got one it did not like. A 404 is a real answer — the ticket does not
//! exist — and retrying it through the agent wastes tokens to reach the same place.

use anyhow::Result;

/// Why a REST attempt failed, which decides whether the MCP hint is worth offering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestFailure {
    /// No credential configured, transport error, or a 5xx. The agent may do better.
    Unavailable,
    /// The server answered definitively (404, 400). The agent would get the same.
    Definitive,
}

/// Classify an error raised by the REST layer.
///
/// The REST layer reports status codes in its message; matching on that keeps the
/// classification in one place rather than threading a status through every provider.
pub fn classify(error: &anyhow::Error) -> RestFailure {
    let message = error.to_string();
    if message.contains("Not found:") || message.contains("returned 400") {
        return RestFailure::Definitive;
    }
    RestFailure::Unavailable
}

pub enum Attempt<T> {
    /// The REST call succeeded.
    Rest(T),
    /// REST could not be made; the caller should emit the MCP hint. Carries the
    /// reason so the envelope can say why the costlier path was chosen.
    FallBack(String),
}

/// Run `rest` when the mode allows it, deciding what to do with any failure.
///
/// * mode `rest` — surface every failure; the user asked for REST specifically.
/// * mode `mcp` — never attempt REST.
/// * mode `auto` — attempt REST, fall back on `Unavailable`, surface `Definitive`.
pub fn attempt<T>(
    allows_rest: bool,
    allows_mcp: bool,
    rest: impl FnOnce() -> Result<T>,
) -> Result<Attempt<T>> {
    if !allows_rest {
        return Ok(Attempt::FallBack(
            "connector is configured for mcp mode".into(),
        ));
    }
    match rest() {
        Ok(value) => Ok(Attempt::Rest(value)),
        Err(error) => {
            if !allows_mcp || classify(&error) == RestFailure::Definitive {
                return Err(error);
            }
            Ok(Attempt::FallBack(format!(
                "REST call unavailable ({error})"
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::{anyhow, bail};

    #[test]
    fn missing_credentials_are_recoverable() {
        let error = anyhow!("Missing credential 'token' for 'clickup'.");

        assert_eq!(classify(&error), RestFailure::Unavailable);
    }

    #[test]
    fn server_errors_are_recoverable() {
        assert_eq!(
            classify(&anyhow!("https://x returned 503")),
            RestFailure::Unavailable
        );
        assert_eq!(
            classify(&anyhow!("Request to https://x failed")),
            RestFailure::Unavailable
        );
    }

    #[test]
    fn a_missing_resource_is_definitive() {
        assert_eq!(
            classify(&anyhow!("Not found: https://x/issue/1")),
            RestFailure::Definitive
        );
    }

    #[test]
    fn auth_rejection_is_recoverable_because_the_agent_may_hold_a_better_session() {
        let error = anyhow!("https://x rejected the credentials (401). Check your token scope.");

        assert_eq!(classify(&error), RestFailure::Unavailable);
    }

    #[test]
    fn auto_mode_returns_the_rest_result() {
        let attempt = attempt(true, true, || Ok(42)).unwrap();

        assert!(matches!(attempt, Attempt::Rest(42)));
    }

    #[test]
    fn auto_mode_falls_back_when_rest_is_unavailable() {
        let attempt = attempt(true, true, || -> Result<i32> {
            bail!("Missing credential 'token'")
        })
        .unwrap();

        match attempt {
            Attempt::FallBack(reason) => assert!(reason.contains("Missing credential")),
            Attempt::Rest(_) => panic!("expected a fallback"),
        }
    }

    #[test]
    fn auto_mode_surfaces_a_definitive_answer() {
        let result = attempt(true, true, || -> Result<i32> {
            bail!("Not found: https://x")
        });

        assert!(
            result.is_err(),
            "a 404 must not be retried through the agent"
        );
    }

    #[test]
    fn rest_mode_surfaces_every_failure() {
        let result = attempt(true, false, || -> Result<i32> {
            bail!("Missing credential")
        });

        assert!(
            result.is_err(),
            "an explicit rest mode must not silently fall back"
        );
    }

    #[test]
    fn mcp_mode_never_attempts_rest() {
        let attempt = attempt(false, true, || -> Result<i32> { panic!("must not run") }).unwrap();

        assert!(matches!(attempt, Attempt::FallBack(_)));
    }
}
