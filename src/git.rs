//! Git operations.
//!
//! Shelling out to `git` rather than linking libgit2: it keeps the binary small,
//! honours the user's own git configuration, credentials, and SSH agent, and
//! matches the behaviour the Python implementation established.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};

pub fn run(args: &[&str], cwd: Option<&Path>) -> Result<String> {
    let mut command = Command::new("git");
    command.args(args);
    if let Some(directory) = cwd {
        command.current_dir(directory);
    }
    let output = command
        .output()
        .context("git is not installed or not on PATH.")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let detail = if stderr.is_empty() { stdout } else { stderr };
        if detail.is_empty() {
            bail!("git {} failed", args.join(" "));
        }
        bail!("{detail}");
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

pub fn run_ok(args: &[&str], cwd: Option<&Path>) -> bool {
    run(args, cwd).is_ok()
}

pub fn resolve_root(start: Option<&Path>) -> Result<PathBuf> {
    let location = match start {
        Some(path) => path.to_path_buf(),
        None => std::env::current_dir()?,
    };
    let output = run(&["rev-parse", "--show-toplevel"], Some(&location))
        .with_context(|| format!("{} is not inside a Git repository.", location.display()))?;
    Ok(PathBuf::from(output.trim()))
}

pub fn changed_paths(root: &Path, base_branch: &str, include_worktree: bool) -> Vec<String> {
    let range = format!("{base_branch}...HEAD");
    let mut commands: Vec<Vec<&str>> = vec![vec!["diff", "--name-only", &range, "--"]];
    if include_worktree {
        commands.push(vec!["diff", "--cached", "--name-only", "--"]);
        commands.push(vec!["diff", "--name-only", "--"]);
    }
    let mut paths: Vec<String> = Vec::new();
    for command in commands {
        // A missing base branch is not fatal here: ranking simply loses a signal.
        if let Ok(output) = run(&command, Some(root)) {
            for line in output.lines() {
                let line = line.trim();
                if !line.is_empty() && !paths.iter().any(|item| item == line) {
                    paths.push(line.to_string());
                }
            }
        }
    }
    paths.sort();
    paths
}
