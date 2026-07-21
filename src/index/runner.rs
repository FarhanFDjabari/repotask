//! Walk the project and extract declarations into a symbol index.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::Result;
use globset::{Glob, GlobSet, GlobSetBuilder};
use serde::Serialize;
use tree_sitter::{Node, Parser};

use crate::config::RepoTaskConfig;
use crate::index::languages::{
    declarations, grammar, is_known_suffix, language_for, IDENTIFIER_NODES,
};
use crate::output;

#[derive(Debug, Clone, Serialize)]
pub struct Symbol {
    pub name: String,
    pub kind: String,
    pub language: String,
    pub path: String,
    pub line: usize,
    pub scope: String,
}

#[derive(Debug, Default)]
pub struct IndexResult {
    pub symbols: Vec<Symbol>,
    pub files_scanned: usize,
    pub files_skipped: usize,
    pub languages: BTreeMap<String, usize>,
}

/// Index the whole project, or only `paths` when given (incremental refresh).
pub fn build_index(config: &RepoTaskConfig, paths: Option<&[String]>) -> Result<IndexResult> {
    let excludes = build_globset(&config.index.exclude);
    let files = candidate_files(config, paths, &excludes);

    let mut result = IndexResult::default();
    let mut missing: Vec<String> = Vec::new();
    let mut parser = Parser::new();

    for file in files {
        let Some(suffix) = file.extension().and_then(|value| value.to_str()) else {
            continue;
        };
        let Some(language) = language_for(suffix) else {
            continue;
        };
        let Ok(metadata) = file.metadata() else {
            result.files_skipped += 1;
            continue;
        };
        if metadata.len() > config.index.max_file_bytes {
            result.files_skipped += 1;
            continue;
        }
        let Some(grammar) = grammar(language) else {
            if !missing.iter().any(|item| item == language) {
                missing.push(language.to_string());
            }
            result.files_skipped += 1;
            continue;
        };
        if parser.set_language(&grammar).is_err() {
            result.files_skipped += 1;
            continue;
        }
        let Ok(source) = std::fs::read(&file) else {
            result.files_skipped += 1;
            continue;
        };
        let relative = file
            .strip_prefix(&config.root)
            .unwrap_or(&file)
            .to_string_lossy()
            .replace('\\', "/");

        result
            .symbols
            .extend(extract(&mut parser, language, &source, &relative));
        result.files_scanned += 1;
        *result.languages.entry(language.to_string()).or_insert(0) += 1;
    }

    missing.sort();
    for language in missing {
        output::warn(format!(
            "This build has no grammar for {language}; those files were skipped."
        ));
    }

    result.symbols.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then_with(|| left.line.cmp(&right.line))
            .then_with(|| left.name.cmp(&right.name))
    });
    Ok(result)
}

fn build_globset(patterns: &[String]) -> GlobSet {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        if let Ok(glob) = Glob::new(pattern) {
            builder.add(glob);
        }
    }
    builder.build().unwrap_or_else(|_| GlobSet::empty())
}

fn candidate_files(
    config: &RepoTaskConfig,
    paths: Option<&[String]>,
    excludes: &GlobSet,
) -> Vec<PathBuf> {
    if let Some(paths) = paths {
        return paths
            .iter()
            .map(|path| config.root.join(path))
            .filter(|path| path.is_file() && !excluded(config, path, excludes))
            .collect();
    }
    let mut files: Vec<PathBuf> = walkdir::WalkDir::new(&config.root)
        .into_iter()
        .filter_entry(|entry| {
            // Never descend into dot directories; .git alone would dominate the walk.
            !entry.file_name().to_string_lossy().starts_with('.') || entry.depth() == 0
        })
        .flatten()
        .map(|entry| entry.into_path())
        .filter(|path| {
            path.is_file()
                && path
                    .extension()
                    .and_then(|value| value.to_str())
                    .is_some_and(is_known_suffix)
                && !excluded(config, path, excludes)
        })
        .collect();
    files.sort();
    files
}

fn excluded(config: &RepoTaskConfig, path: &Path, excludes: &GlobSet) -> bool {
    let Ok(relative) = path.strip_prefix(&config.root) else {
        return true;
    };
    let mut components: Vec<_> = relative.components().collect();
    components.pop();
    if components
        .iter()
        .any(|component| component.as_os_str().to_string_lossy().starts_with('.'))
    {
        return true;
    }
    excludes.is_match(relative.to_string_lossy().replace('\\', "/").as_str())
}

fn extract(parser: &mut Parser, language: &str, source: &[u8], relative: &str) -> Vec<Symbol> {
    let mapping = declarations(language);
    if mapping.is_empty() {
        return Vec::new();
    }
    let Some(tree) = parser.parse(source, None) else {
        return Vec::new();
    };

    let mut symbols: Vec<Symbol> = Vec::new();
    let mut stack: Vec<(Node, String)> = vec![(tree.root_node(), String::new())];
    while let Some((node, scope)) = stack.pop() {
        let mut child_scope = scope.clone();
        if let Some((_, kind)) = mapping
            .iter()
            .find(|(node_type, _)| *node_type == node.kind())
        {
            if let Some(name) = name_of(&node, source) {
                let kind = refine_kind(language, &node, kind);
                symbols.push(Symbol {
                    name: name.clone(),
                    kind: kind.to_string(),
                    language: language.to_string(),
                    path: relative.to_string(),
                    line: node.start_position().row + 1,
                    scope: scope.clone(),
                });
                child_scope = if scope.is_empty() {
                    name
                } else {
                    format!("{scope}.{name}")
                };
            }
        }
        let mut cursor = node.walk();
        let children: Vec<Node> = node.children(&mut cursor).collect();
        for child in children.into_iter().rev() {
            stack.push((child, child_scope.clone()));
        }
    }
    symbols
}

/// Swift folds `struct`, `class`, `actor`, and `enum` into a single
/// `class_declaration`; the leading keyword is what distinguishes them. Split
/// out `struct` so SwiftUI views and components can be targeted precisely.
/// Other keywords keep the mapped kind.
fn refine_kind<'a>(language: &str, node: &Node, kind: &'a str) -> &'a str {
    if language == "swift" && node.kind() == "class_declaration" {
        let mut cursor = node.walk();
        if node
            .children(&mut cursor)
            .any(|child| child.kind() == "struct")
        {
            return "struct";
        }
    }
    kind
}

fn name_of(node: &Node, source: &[u8]) -> Option<String> {
    let field = match node.child_by_field_name("name") {
        Some(field) => field,
        None => {
            // Collect first: the iterator borrows the cursor, but Node is tied to
            // the tree's lifetime, so the nodes outlive the walk.
            let mut cursor = node.walk();
            let children: Vec<Node> = node.children(&mut cursor).collect();
            children
                .into_iter()
                .find(|child| IDENTIFIER_NODES.contains(&child.kind()))?
        }
    };
    let text = source.get(field.start_byte()..field.end_byte())?;
    let name = String::from_utf8_lossy(text).to_string();
    (!name.is_empty()).then_some(name)
}
