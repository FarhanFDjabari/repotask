//! Token budgeting for context packs.
//!
//! Estimation is deliberately crude — roughly four characters per token. The goal
//! is a predictable ceiling on what the agent receives, not an exact token count.

pub const CHARS_PER_TOKEN: usize = 4;

fn truncation_note(command: &str, id: &str) -> String {
    format!("\n\n[truncated by repo-task: read the full document with `repo-task {command} {id}`]")
}

pub fn estimate(text: &str) -> usize {
    (text.len() / CHARS_PER_TOKEN).max(1)
}

/// Trim `text` to roughly `tokens`, cutting at a paragraph boundary when possible.
/// Returns the text and whether it was truncated.
pub fn fit(text: &str, tokens: usize, command: &str, id: &str) -> (String, bool) {
    if tokens == 0 {
        return (String::new(), true);
    }
    let limit = tokens * CHARS_PER_TOKEN;
    if text.len() <= limit {
        return (text.to_string(), false);
    }
    // The note is part of what the agent receives, so it has to fit inside the budget too.
    let note = truncation_note(command, id);
    let Some(limit) = limit.checked_sub(note.len()).filter(|value| *value > 0) else {
        return (String::new(), true);
    };

    let mut end = limit.min(text.len());
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    let mut cut = &text[..end];
    if let Some(boundary) = cut.rfind("\n\n") {
        if boundary > limit / 2 {
            cut = &cut[..boundary];
        }
    }
    (format!("{}{}", cut.trim_end(), note), true)
}
