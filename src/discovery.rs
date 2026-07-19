//! Project discovery: stacks, base branch, project name.

use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::git;

pub struct Discovery {
    pub root: PathBuf,
    pub project_name: String,
    pub stacks: Vec<String>,
    pub base_branch: String,
}

pub fn discover() -> Result<Discovery> {
    let root = git::resolve_root(None)?;
    Ok(Discovery {
        project_name: root
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| "project".into()),
        stacks: detect_stacks(&root),
        base_branch: detect_base_branch(&root),
        root,
    })
}

pub fn detect_stacks(root: &Path) -> Vec<String> {
    let mut stacks: Vec<String> = Vec::new();
    fn add(values: &[&str], stacks: &mut Vec<String>) {
        for value in values {
            if !stacks.iter().any(|item| item == value) {
                stacks.push((*value).to_string());
            }
        }
    }

    let exists = |name: &str| root.join(name).exists();
    let has_extension = |extension: &str| {
        std::fs::read_dir(root)
            .map(|entries| {
                entries.flatten().any(|entry| {
                    entry.path().extension().and_then(|value| value.to_str()) == Some(extension)
                })
            })
            .unwrap_or(false)
    };

    if exists("build.gradle") || exists("build.gradle.kts") || exists("settings.gradle.kts") {
        add(&["android", "kotlin"], &mut stacks);
        if gradle_mentions_compose(root) {
            add(&["jetpack-compose"], &mut stacks);
        }
    }
    if has_extension("xcodeproj") || has_extension("xcworkspace") {
        add(&["ios", "swift"], &mut stacks);
    }
    if exists("Package.swift") {
        add(&["swift"], &mut stacks);
    }
    if exists("pubspec.yaml") {
        add(&["flutter", "dart"], &mut stacks);
    }
    if exists("package.json") {
        if exists("tsconfig.json") {
            add(&["typescript"], &mut stacks);
        }
        let package = std::fs::read_to_string(root.join("package.json")).unwrap_or_default();
        if package.contains("\"react-native\"") {
            add(&["react-native", "typescript"], &mut stacks);
        } else {
            add(&["web"], &mut stacks);
        }
    }
    if exists("pyproject.toml") || exists("requirements.txt") {
        add(&["python"], &mut stacks);
    }
    if exists("go.mod") {
        add(&["go"], &mut stacks);
    }
    if exists("Cargo.toml") {
        add(&["rust"], &mut stacks);
    }

    if stacks.is_empty() {
        stacks.push("generic".into());
    }
    stacks
}

fn gradle_mentions_compose(root: &Path) -> bool {
    walkdir::WalkDir::new(root)
        .max_depth(3)
        .into_iter()
        .filter_entry(|entry| entry.file_name() != ".git")
        .flatten()
        .filter(|entry| {
            let name = entry.path().to_string_lossy().to_string();
            name.ends_with(".gradle.kts") || name.ends_with(".gradle")
        })
        .any(|entry| {
            std::fs::read_to_string(entry.path())
                .map(|text| text.to_lowercase().contains("compose"))
                .unwrap_or(false)
        })
}

pub fn detect_base_branch(root: &Path) -> String {
    let symbolic = git::run(
        &[
            "symbolic-ref",
            "--quiet",
            "--short",
            "refs/remotes/origin/HEAD",
        ],
        Some(root),
    )
    .unwrap_or_default();
    if let Some(branch) = symbolic.trim().strip_prefix("origin/") {
        if !branch.is_empty() {
            return branch.to_string();
        }
    }
    for candidate in ["main", "master"] {
        if git::run_ok(&["rev-parse", "--verify", candidate], Some(root)) {
            return candidate.to_string();
        }
    }
    let current = git::run(&["branch", "--show-current"], Some(root)).unwrap_or_default();
    let current = current.trim();
    if current.is_empty() {
        "main".into()
    } else {
        current.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::{detect_stacks, gradle_mentions_compose};

    fn temp_dir(label: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "repotask-discovery-{label}-{}-{:?}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn detects_android_and_compose() {
        let root = temp_dir("compose");
        std::fs::write(
            root.join("build.gradle.kts"),
            "plugins { id(\"com.android.application\") }\ncomposeOptions {}\n",
        )
        .unwrap();

        let stacks = detect_stacks(&root);

        assert_eq!(stacks, vec!["android", "kotlin", "jetpack-compose"]);
        assert!(gradle_mentions_compose(&root));
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn detects_flutter_and_dart() {
        let root = temp_dir("flutter");
        std::fs::write(root.join("pubspec.yaml"), "name: demo\n").unwrap();

        assert_eq!(detect_stacks(&root), vec!["flutter", "dart"]);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn falls_back_to_generic() {
        let root = temp_dir("generic");

        assert_eq!(detect_stacks(&root), vec!["generic"]);
        std::fs::remove_dir_all(&root).ok();
    }
}
