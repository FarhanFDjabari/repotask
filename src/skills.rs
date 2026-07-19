//! Generate agent skill files that reference repo-task commands.
//!
//! Skills stay thin on purpose: they tell the agent which command to run, never what
//! the project's conventions are. Inlined knowledge goes stale the moment the
//! knowledge base moves, and costs tokens on every session whether or not it is needed.

use std::path::Path;

use anyhow::Result;
use include_dir::{include_dir, Dir};

use crate::config::RepoTaskConfig;

static TEMPLATES: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/skills-templates");

pub const SKILLS: &[&str] = &[
    "repotask-search",
    "repotask-feature",
    "repotask-bugfix",
    "repotask-review",
    "repotask-design",
];
pub const CLAUDE_SKILLS_DIR: &str = ".claude/skills";
pub const AGENTS_FILE: &str = "AGENTS.md";
pub const BEGIN_MARKER: &str = "<!-- repo-task:begin -->";
pub const END_MARKER: &str = "<!-- repo-task:end -->";

const OTHER_AGENT_FILES: &[&str] = &[
    "CLAUDE.md",
    ".cursorrules",
    ".github/copilot-instructions.md",
    "GEMINI.md",
];

pub struct GeneratedFile {
    pub path: String,
    pub action: &'static str,
}

pub fn template(name: &str) -> Result<&'static str> {
    TEMPLATES
        .get_file(format!("{name}.md"))
        .and_then(|file| file.contents_utf8())
        .ok_or_else(|| anyhow::anyhow!("Bundled skill template missing: {name}"))
}

pub fn agents_block(config: &RepoTaskConfig) -> String {
    let stacks = config.project.stacks.join(", ");
    format!(
        r#"{BEGIN_MARKER}
## Project knowledge: use the `repo-task` CLI

This repository's architecture conventions, task playbooks, and code index live in a
knowledge base that the `repo-task` CLI reads. Query it instead of inferring
conventions from the source — it is reviewed and current.

Stacks: {stacks}

Start every task with one budgeted context pack:

```bash
repo-task --json brief "<what you are about to do>" --changed
```

Then, as needed:

| Need | Command |
| --- | --- |
| Find knowledge by keyword | `repo-task --json search "<keywords>"` |
| Architecture or stack rule | `repo-task --json convention <id>` |
| How this project does a task | `repo-task --json recipe <id>` |
| Curated project facts | `repo-task --json fact <family>` |
| Where a declaration lives | `repo-task --json symbol <name>` |
| Ticket to reviewable steps | `repo-task --json fetch\|summarize\|analyze\|split <ticket>` |
| Bug triage and duplicates | `repo-task --json bug fetch <ticket>`, `repo-task --json bug dedupe` |
| Design structure and mapping | `repo-task --json design node <id>`, `repo-task --json design map <names>` |
| Any declared external system | `repo-task --json connect <system> <verb> --arg k=v` |

Prefer these over calling an external API yourself: the CLI performs the call and returns
the distilled result, so you pay context for the answer rather than the whole payload. When
it cannot make the call it hands you the tool to run through your own connection.

Every command returns `{{"ok": ..., "command": ..., "data": ..., "warnings": []}}`.
Read `data`; on failure `error.message` says what to do next.

Cite the document `id` behind any decision you justify with a project convention.
{END_MARKER}"#
    )
}

/// Map of relative path -> content for every file `skills sync` owns.
pub fn render_all(config: &RepoTaskConfig) -> Result<Vec<(String, String)>> {
    let mut files: Vec<(String, String)> = Vec::new();
    for name in SKILLS {
        files.push((
            format!("{CLAUDE_SKILLS_DIR}/{name}/SKILL.md"),
            template(name)?.to_string(),
        ));
    }
    files.push((AGENTS_FILE.to_string(), agents_block(config)));
    Ok(files)
}

pub fn sync(config: &RepoTaskConfig, dry_run: bool) -> Result<Vec<GeneratedFile>> {
    let mut results: Vec<GeneratedFile> = Vec::new();
    for (relative, content) in render_all(config)? {
        let path = config.root.join(&relative);
        let existing = std::fs::read_to_string(&path).ok();
        let content = if relative == AGENTS_FILE {
            merge_agents(existing.as_deref(), &content)
        } else {
            content
        };
        if existing.as_deref() == Some(content.as_str()) {
            results.push(GeneratedFile {
                path: relative,
                action: "unchanged",
            });
            continue;
        }
        if !dry_run {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&path, &content)?;
        }
        let action = if existing.is_some() {
            "updated"
        } else {
            "created"
        };
        results.push(GeneratedFile {
            path: relative,
            action,
        });
    }
    Ok(results)
}

/// Replace only our marked block so hand-written AGENTS.md content survives.
fn merge_agents(existing: Option<&str>, block: &str) -> String {
    let Some(existing) = existing else {
        return format!("{block}\n");
    };
    if let (Some(start), Some(end)) = (existing.find(BEGIN_MARKER), existing.find(END_MARKER)) {
        let head = &existing[..start];
        let tail = &existing[end + END_MARKER.len()..];
        return format!("{head}{block}{tail}");
    }
    let separator = if existing.ends_with("\n\n") {
        ""
    } else if existing.ends_with('\n') {
        "\n"
    } else {
        "\n\n"
    };
    format!("{existing}{separator}{block}\n")
}

/// Other harnesses' instruction files, reported so the user can point them here.
pub fn find_agent_files(root: &Path) -> Vec<String> {
    OTHER_AGENT_FILES
        .iter()
        .filter(|name| root.join(name).is_file())
        .map(|name| (*name).to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_skill_declares_frontmatter_and_references_commands() {
        for name in SKILLS {
            let content = template(name).expect("template is bundled");
            assert!(content.starts_with("---\n"), "{name} needs frontmatter");
            assert!(content.contains(&format!("name: {name}\n")));
            assert!(content.contains("description:"));
            assert!(
                content.contains("repo-task --json"),
                "{name} must reference commands"
            );
        }
    }

    #[test]
    fn merge_creates_the_block_when_the_file_is_absent() {
        let merged = merge_agents(None, "BLOCK");

        assert_eq!(merged, "BLOCK\n");
    }

    #[test]
    fn merge_appends_below_hand_written_content() {
        let merged = merge_agents(Some("# House rules\n\nBe kind.\n"), "BLOCK");

        assert!(merged.starts_with("# House rules"));
        assert!(merged.ends_with("BLOCK\n"));
    }

    #[test]
    fn merge_replaces_only_the_marked_block() {
        let existing = format!("# Top\n\n{BEGIN_MARKER}\nstale\n{END_MARKER}\n\n# Bottom\n");

        let merged = merge_agents(
            Some(&existing),
            &format!("{BEGIN_MARKER}\nfresh\n{END_MARKER}"),
        );

        assert!(!merged.contains("stale"));
        assert!(merged.contains("fresh"));
        assert!(merged.starts_with("# Top"));
        assert!(merged.trim_end().ends_with("# Bottom"));
    }

    #[test]
    fn merge_is_idempotent() {
        let block = format!("{BEGIN_MARKER}\nfresh\n{END_MARKER}");
        let once = merge_agents(Some("# Top\n"), &block);

        let twice = merge_agents(Some(&once), &block);

        assert_eq!(once, twice);
    }
}
