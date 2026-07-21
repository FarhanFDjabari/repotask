//! Dual-surface output: a stable JSON envelope for agents, plain text for humans.
//!
//! The envelope is the agent-facing contract and is byte-compatible with the
//! Python implementation it replaces:
//!
//! ```json
//! {"schema": "repotask.v2", "ok": true, "command": "brief", "data": {}, "warnings": []}
//! ```

use std::cell::RefCell;

use owo_colors::OwoColorize;
use serde::Serialize;
use serde_json::Value;

pub const SCHEMA: &str = "repotask.v2";

/// Styling for the human surface only. Callers must pad *before* styling: a padded
/// string keeps its column width, whereas padding a styled one counts the escape
/// bytes and misaligns the row.
pub mod style {
    use super::OwoColorize;

    pub fn heading(text: &str) -> String {
        text.bold().to_string()
    }

    pub fn id(text: &str) -> String {
        text.cyan().to_string()
    }

    pub fn pass(text: &str) -> String {
        text.green().to_string()
    }

    pub fn fail(text: &str) -> String {
        text.red().bold().to_string()
    }

    pub fn dim(text: &str) -> String {
        text.dimmed().to_string()
    }
}

thread_local! {
    static STATE: RefCell<State> = const { RefCell::new(State::new()) };
}

struct State {
    json_mode: bool,
    warnings: Vec<String>,
}

impl State {
    const fn new() -> Self {
        Self {
            json_mode: false,
            warnings: Vec::new(),
        }
    }
}

pub fn set_json_mode(enabled: bool) {
    STATE.with(|state| state.borrow_mut().json_mode = enabled);
}

pub fn json_mode() -> bool {
    STATE.with(|state| state.borrow().json_mode)
}

pub fn warn(message: impl Into<String>) {
    STATE.with(|state| state.borrow_mut().warnings.push(message.into()));
}

fn warnings() -> Vec<String> {
    STATE.with(|state| state.borrow().warnings.clone())
}

#[derive(Serialize)]
struct Envelope<'a> {
    schema: &'a str,
    ok: bool,
    command: &'a str,
    data: &'a Value,
    warnings: Vec<String>,
}

#[derive(Serialize)]
struct ErrorEnvelope<'a> {
    schema: &'a str,
    ok: bool,
    command: &'a str,
    error: ErrorBody<'a>,
    warnings: Vec<String>,
}

#[derive(Serialize)]
struct ErrorBody<'a> {
    code: &'a str,
    message: &'a str,
}

/// Print a success envelope as JSON, or the human rendering.
pub fn emit(command: &str, data: &Value, human: impl FnOnce(&Value) -> String) {
    if json_mode() {
        let envelope = Envelope {
            schema: SCHEMA,
            ok: true,
            command,
            data,
            warnings: warnings(),
        };
        println!(
            "{}",
            serde_json::to_string_pretty(&envelope).unwrap_or_default()
        );
        return;
    }
    for message in warnings() {
        anstream::eprintln!("{} {message}", style::fail("warning:"));
    }
    anstream::println!("{}", human(data));
}

/// Print a failure envelope. The caller owns the exit code.
pub fn emit_error(command: &str, message: &str, code: &str) {
    if json_mode() {
        let envelope = ErrorEnvelope {
            schema: SCHEMA,
            ok: false,
            command,
            error: ErrorBody { code, message },
            warnings: warnings(),
        };
        println!("{}", serde_json::to_string(&envelope).unwrap_or_default());
        return;
    }
    anstream::eprintln!("{} {message}", style::fail("Error:"));
}
