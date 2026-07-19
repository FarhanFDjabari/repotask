mod common;

use std::process::Command;

use common::{commit_all, init_repo, write, Fixture, TempDir};

#[test]
fn sync_writes_skills_and_the_agents_block() {
    let fixture = Fixture::new("skills-write");
    fixture.run(&["kb", "sync"]);

    let envelope = fixture.json(&["skills", "sync"]);

    assert_eq!(envelope["ok"], true, "{envelope}");
    for name in [
        "repotask-search",
        "repotask-feature",
        "repotask-bugfix",
        "repotask-review",
    ] {
        let path = fixture
            .project
            .join(format!(".claude/skills/{name}/SKILL.md"));
        assert!(path.is_file(), "{name} was not written");
    }
    let agents = std::fs::read_to_string(fixture.project.join("AGENTS.md")).unwrap();
    assert!(agents.contains("<!-- repo-task:begin -->"));
    assert!(
        agents.contains("android, kotlin"),
        "the block states the project stacks"
    );
}

#[test]
fn sync_is_idempotent() {
    let fixture = Fixture::new("skills-idempotent");
    fixture.run(&["kb", "sync"]);
    fixture.run(&["skills", "sync"]);

    let envelope = fixture.json(&["skills", "sync"]);

    let actions: Vec<&str> = envelope["data"]["files"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|item| item["action"].as_str())
        .collect();
    assert!(
        actions.iter().all(|action| *action == "unchanged"),
        "{actions:?}"
    );
}

#[test]
fn sync_preserves_hand_written_agents_content() {
    let fixture = Fixture::new("skills-preserve");
    fixture.run(&["kb", "sync"]);
    write(&fixture.project, "AGENTS.md", "# House rules\n\nBe kind.\n");

    fixture.run(&["skills", "sync"]);

    let content = std::fs::read_to_string(fixture.project.join("AGENTS.md")).unwrap();
    assert!(content.starts_with("# House rules"));
    assert!(content.contains("<!-- repo-task:begin -->"));
}

#[test]
fn dry_run_writes_nothing() {
    let fixture = Fixture::new("skills-dry-run");
    fixture.run(&["kb", "sync"]);

    let envelope = fixture.json(&["skills", "sync", "--dry-run"]);

    assert_eq!(envelope["ok"], true);
    assert!(!fixture.project.join(".claude/skills").exists());
}

#[test]
fn other_agent_instruction_files_are_reported() {
    let fixture = Fixture::new("skills-others");
    fixture.run(&["kb", "sync"]);
    write(&fixture.project, "CLAUDE.md", "# legacy\n");

    let envelope = fixture.json(&["skills", "sync"]);

    assert_eq!(
        envelope["data"]["otherAgentFiles"],
        serde_json::json!(["CLAUDE.md"])
    );
}

/// A bare project with no knowledge base, exercising `init` and `kb init` end to end.
struct LocalProject {
    temp: TempDir,
    path: std::path::PathBuf,
}

impl LocalProject {
    fn new(label: &str) -> Self {
        let temp = TempDir::new(label);
        let path = temp.path.join("app");
        init_repo(&path);
        write(&path, "build.gradle.kts", "// app\n");
        commit_all(&path, "initial");
        Self { temp, path }
    }

    fn json(&self, args: &[&str]) -> serde_json::Value {
        let mut full = vec!["--json"];
        full.extend_from_slice(args);
        let output = Command::new(env!("CARGO_BIN_EXE_repo-task"))
            .args(&full)
            .current_dir(&self.path)
            .env("HOME", &self.temp.path)
            .output()
            .expect("run repo-task");
        let text = String::from_utf8_lossy(&output.stdout);
        serde_json::from_str(&text).unwrap_or_else(|error| panic!("bad JSON: {error}\n{text}"))
    }
}

#[test]
fn init_detects_stacks_and_writes_a_config() {
    let project = LocalProject::new("setup-init");

    let envelope = project.json(&["init"]);

    assert_eq!(envelope["ok"], true, "{envelope}");
    assert_eq!(
        envelope["data"]["config"]["project"]["stacks"],
        serde_json::json!(["android", "kotlin"])
    );
    assert!(project.path.join(".repo-task/config.yaml").is_file());
    let gitignore = std::fs::read_to_string(project.path.join(".gitignore")).unwrap();
    assert!(gitignore.contains(".repo-task/work/"));
}

#[test]
fn init_refuses_to_overwrite_without_force() {
    let project = LocalProject::new("setup-init-force");
    project.json(&["init"]);

    let envelope = project.json(&["init"]);

    assert_eq!(envelope["ok"], false);
    assert!(envelope["error"]["message"]
        .as_str()
        .unwrap()
        .contains("--force"));
    assert_eq!(project.json(&["init", "--force"])["ok"], true);
}

#[test]
fn kb_init_scaffolds_only_the_relevant_stacks() {
    let project = LocalProject::new("seed-stacks");
    project.json(&["init"]);

    let envelope = project.json(&["kb", "init"]);

    assert_eq!(envelope["ok"], true, "{envelope}");
    let knowledge = project.path.join(".repo-task/knowledge");
    assert!(knowledge.join("conventions/android.md").is_file());
    assert!(
        !knowledge.join("conventions/ios.md").exists(),
        "other stacks must not be copied"
    );
    assert!(!knowledge.join("conventions/rust.md").exists());
    assert!(knowledge.join("facts/README.md").is_file());
}

#[test]
fn the_scaffold_is_immediately_usable() {
    let project = LocalProject::new("seed-usable");
    project.json(&["init"]);
    project.json(&["kb", "init"]);

    assert_eq!(project.json(&["doctor"])["data"]["ok"], true);
    assert_eq!(project.json(&["index"])["ok"], true);
    let brief = project.json(&["brief", "add a screen"]);
    assert_eq!(brief["ok"], true, "{brief}");
    assert!(!brief["data"]["documents"].as_array().unwrap().is_empty());
}

#[test]
fn kb_init_refuses_to_clobber_without_force() {
    let project = LocalProject::new("seed-clobber");
    project.json(&["init"]);
    project.json(&["kb", "init"]);

    let envelope = project.json(&["kb", "init"]);

    assert_eq!(envelope["ok"], false);
    assert!(envelope["error"]["message"]
        .as_str()
        .unwrap()
        .contains("--force"));
    assert_eq!(project.json(&["kb", "init", "--force"])["ok"], true);
}
