//! Shared fixtures: a real git repository holding a minimal knowledge base, and a
//! project wired to it. Using real repositories keeps the remote-source code path
//! (clone, pin, fetch) under test without touching the network.

#![allow(dead_code)]

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub const CONVENTION_ANDROID: &str = r#"---
id: android-architecture
title: Android Architecture
stacks: [android, kotlin]
tags: [mvvm, hilt, module]
---
Feature modules use MVVM with Hilt. UI state is exposed as StateFlow.
"#;

pub const CONVENTION_IOS: &str = r#"---
id: ios-architecture
title: iOS Architecture
stacks: [ios, swift]
tags: [mvvm, swiftui]
---
SwiftUI views bind to ObservableObject view models.
"#;

pub const RECIPE_PAGINATION: &str = r#"---
id: pagination
title: Implement Pagination
stacks: [android]
tags: [paging, list]
---
Use Paging 3 with a RemoteMediator backed by Room.
"#;

pub const KOTLIN_SOURCE: &str = r#"package com.acme.feed

class FeedViewModel(private val repo: FeedRepository) {
    fun load() {}
}

class FeedRepository {
    suspend fun fetch(): List<String> = emptyList()
}
"#;

pub fn git(root: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .expect("git should be installed");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

pub fn init_repo(root: &Path) {
    std::fs::create_dir_all(root).expect("create repo dir");
    git(root, &["init", "-b", "main"]);
    git(root, &["config", "user.email", "tests@example.com"]);
    git(root, &["config", "user.name", "RepoTask Tests"]);
}

pub fn commit_all(root: &Path, message: &str) {
    git(root, &["add", "-A"]);
    git(root, &["commit", "-m", message]);
}

pub fn write(root: &Path, relative: &str, content: &str) -> PathBuf {
    let path = root.join(relative);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create parent dir");
    }
    std::fs::write(&path, content).expect("write file");
    path
}

/// A temporary directory that cleans itself up, without pulling in a crate for it.
pub struct TempDir {
    pub path: PathBuf,
}

impl TempDir {
    pub fn new(label: &str) -> Self {
        let unique = format!(
            "{}-{}-{:?}",
            label,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|value| value.as_nanos())
                .unwrap_or_default()
        );
        let path = std::env::temp_dir().join(format!("repotask-test-{unique}"));
        std::fs::create_dir_all(&path).expect("create temp dir");
        Self { path }
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

pub struct Fixture {
    pub temp: TempDir,
    pub kb: PathBuf,
    pub project: PathBuf,
    pub cache: PathBuf,
}

impl Fixture {
    /// A knowledge-base repository plus an Android project pointed at it.
    pub fn new(label: &str) -> Self {
        let temp = TempDir::new(label);
        let kb = temp.path.join("kb");
        let project = temp.path.join("app");
        let cache = temp.path.join("cache");

        init_repo(&kb);
        write(
            &kb,
            "kb.yaml",
            r#"schema_version: 1
name: test-kb
default_budget: 500
fact_families:
  - name: viewmodels
    description: Android ViewModels
    stacks: [android]
    languages: [kotlin]
    kinds: [class]
    name_pattern: ".*ViewModel$"
"#,
        );
        write(
            &kb,
            "conventions/android-architecture.md",
            CONVENTION_ANDROID,
        );
        write(&kb, "conventions/ios-architecture.md", CONVENTION_IOS);
        write(&kb, "recipes/pagination.md", RECIPE_PAGINATION);
        write(
            &kb,
            "slices/android.yaml",
            r#"stack: android
conventions: [android-architecture]
recipes: [pagination]
facts: [viewmodels]
intents:
  feature: [android-architecture, pagination]
"#,
        );
        commit_all(&kb, "initial");

        init_repo(&project);
        write(&project, "build.gradle.kts", "// app\n");
        write(
            &project,
            ".repo-task/config.yaml",
            &format!(
                r#"schema_version: 2
project:
  name: app
  stacks: [android, kotlin]
  base_branch: main
knowledge:
  remote: "file://{}"
  ref: main
"#,
                kb.display()
            ),
        );
        commit_all(&project, "initial");

        Self {
            temp,
            kb,
            project,
            cache,
        }
    }

    fn command(&self, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_repo-task"));
        command
            .args(args)
            .current_dir(&self.project)
            .env("REPOTASK_CACHE_DIR", &self.cache)
            // Point HOME at the fixture so no real secrets file is ever read.
            .env("HOME", &self.temp.path);
        command
    }

    /// Run the CLI in the fixture project with an isolated knowledge-base cache.
    pub fn run(&self, args: &[&str]) -> std::process::Output {
        self.command(args).output().expect("run repo-task")
    }

    /// Run the CLI and parse the JSON envelope.
    pub fn json(&self, args: &[&str]) -> serde_json::Value {
        let mut full = vec!["--json"];
        full.extend_from_slice(args);
        let output = self.run(&full);
        parse(args, &output.stdout)
    }

    /// Pipe `input` into the CLI on stdin, which is how an agent writes results back.
    pub fn json_stdin(&self, args: &[&str], input: &str) -> serde_json::Value {
        let mut full = vec!["--json"];
        full.extend_from_slice(args);
        let mut child = self
            .command(&full)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn repo-task");
        child
            .stdin
            .as_mut()
            .expect("stdin is piped")
            .write_all(input.as_bytes())
            .unwrap();
        let output = child.wait_with_output().expect("wait for repo-task");
        parse(args, &output.stdout)
    }
}

fn parse(args: &[&str], stdout: &[u8]) -> serde_json::Value {
    let text = String::from_utf8_lossy(stdout);
    serde_json::from_str(&text)
        .unwrap_or_else(|error| panic!("invalid JSON from {args:?}: {error}\n{text}"))
}
