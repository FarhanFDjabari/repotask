//! Deterministic impact analysis and increment splitting.
//!
//! Neither step estimates or decides anything: they turn ticket text into evidence
//! (which symbols and files the words point at, grouped into shippable slices) and
//! hand that to the agent with the rubric it should apply.

use std::collections::BTreeMap;

use regex::Regex;
use serde::Serialize;
use serde_json::{json, Value};

const STOP_WORDS: &[&str] = &[
    "the", "and", "for", "with", "that", "this", "from", "into", "when", "then", "than", "should",
    "would", "could", "must", "will", "shall", "user", "users", "app", "page", "screen", "feature",
    "bug", "issue", "ticket", "task", "add", "remove", "update", "change", "fix", "make", "need",
    "needs", "want", "have", "has", "not", "but", "all", "any", "our", "their", "them", "they",
    "you", "your", "are", "was", "were", "been",
];

/// Layer order for incremental delivery: data lands before the UI that consumes it.
const LAYER_ORDER: &[(&str, &[&str])] = &[
    (
        "data",
        &[
            "data",
            "repository",
            "repo",
            "network",
            "api",
            "db",
            "database",
            "dao",
        ],
    ),
    (
        "domain",
        &[
            "domain",
            "usecase",
            "use_case",
            "interactor",
            "model",
            "entity",
        ],
    ),
    (
        "presentation",
        &[
            "ui",
            "view",
            "screen",
            "compose",
            "widget",
            "viewmodel",
            "presenter",
        ],
    ),
];
const DEFAULT_LAYER: &str = "other";

const SOURCE_ROOT_DIRS: &[&str] = &[
    "src", "main", "lib", "app", "java", "kotlin", "swift", "dart", "sources",
];

#[derive(Debug, Default, Clone, Serialize)]
pub struct Impact {
    pub terms: Vec<String>,
    pub files: Vec<Value>,
    pub symbols: Vec<Value>,
    pub modules: BTreeMap<String, usize>,
}

/// Pull likely code-bearing terms out of prose: identifiers first, then salient words.
pub fn keywords(text: &str, limit: usize) -> Vec<String> {
    let camel = Regex::new(r"\b[A-Z][a-zA-Z0-9]*(?:[A-Z][a-zA-Z0-9]*)+\b").expect("valid regex");
    let backticked = Regex::new(r"`([^`]+)`").expect("valid regex");
    let words = Regex::new(r"\b[a-zA-Z][a-zA-Z0-9_]{3,}\b").expect("valid regex");

    let mut candidates: Vec<String> = Vec::new();
    candidates.extend(camel.find_iter(text).map(|item| item.as_str().to_string()));
    candidates.extend(
        backticked
            .captures_iter(text)
            .filter_map(|caps| caps.get(1).map(|item| item.as_str().to_string())),
    );
    candidates.extend(
        words
            .find_iter(text)
            .map(|item| item.as_str().to_string())
            .filter(|word| !STOP_WORDS.contains(&word.to_lowercase().as_str())),
    );

    let mut ordered: Vec<String> = Vec::new();
    for term in candidates {
        let cleaned = term.trim().to_string();
        if cleaned.is_empty() {
            continue;
        }
        let lowered = cleaned.to_lowercase();
        if !ordered.iter().any(|item| item.to_lowercase() == lowered) {
            ordered.push(cleaned);
        }
    }
    ordered.truncate(limit);
    ordered
}

/// Match ticket vocabulary against the symbol index.
pub fn impact_set(text: &str, symbols: &[Value], term_limit: usize) -> Impact {
    let terms = keywords(text, term_limit);
    if terms.is_empty() {
        return Impact::default();
    }
    let lowered_terms: Vec<String> = terms.iter().map(|term| term.to_lowercase()).collect();

    let mut file_hits: BTreeMap<String, usize> = BTreeMap::new();
    let mut file_terms: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut file_order: Vec<String> = Vec::new();
    let mut matched: Vec<Value> = Vec::new();

    for entry in symbols {
        let name = entry["name"].as_str().unwrap_or_default().to_lowercase();
        let hits: Vec<String> = terms
            .iter()
            .zip(&lowered_terms)
            .filter(|(_, lowered)| name.contains(lowered.as_str()))
            .map(|(term, _)| term.clone())
            .collect();
        if hits.is_empty() {
            continue;
        }
        let path = entry["path"].as_str().unwrap_or_default().to_string();
        if !file_hits.contains_key(&path) {
            file_order.push(path.clone());
        }
        *file_hits.entry(path.clone()).or_insert(0) += hits.len();
        let bucket = file_terms.entry(path).or_default();
        for hit in &hits {
            if !bucket.contains(hit) {
                bucket.push(hit.clone());
            }
        }
        let mut item = entry.clone();
        item["matchedTerms"] = json!(hits);
        matched.push(item);
    }

    // Most-hit files first; ties keep discovery order, matching Counter.most_common.
    let mut ordered_paths = file_order;
    ordered_paths.sort_by_key(|path| std::cmp::Reverse(file_hits[path]));

    let mut modules: BTreeMap<String, usize> = BTreeMap::new();
    let files: Vec<Value> = ordered_paths
        .iter()
        .map(|path| {
            let module = module_of(path);
            *modules.entry(module.clone()).or_insert(0) += 1;
            let mut terms = file_terms[path].clone();
            terms.sort();
            json!({
                "path": path,
                "score": file_hits[path],
                "terms": terms,
                "module": module,
                "layer": layer_of(path),
            })
        })
        .collect();

    Impact {
        terms,
        files,
        symbols: matched,
        modules,
    }
}

/// The Gradle/SPM-style module a file belongs to.
///
/// `app/src/main/kotlin/...` is the `app` module: the segment before `src` wins.
/// Layouts without a `src` fall back to the first segment that is not a language
/// or source-root directory.
pub fn module_of(path: &str) -> String {
    let mut parts: Vec<&str> = path.split('/').filter(|part| !part.is_empty()).collect();
    if parts.len() < 2 {
        return ".".into();
    }
    if let Some(index) = parts.iter().position(|part| *part == "src") {
        if index > 0 {
            return parts[index - 1].to_string();
        }
        parts = parts[index + 1..].to_vec();
    }
    for part in &parts[..parts.len().saturating_sub(1)] {
        if !SOURCE_ROOT_DIRS.contains(part) {
            return (*part).to_string();
        }
    }
    parts
        .first()
        .map(|part| (*part).to_string())
        .unwrap_or_else(|| ".".into())
}

/// Longest matching marker wins, so `viewmodel` beats `model`.
pub fn layer_of(path: &str) -> String {
    let lowered = path.to_lowercase();
    let mut best_layer = DEFAULT_LAYER;
    let mut best_length = 0;
    for (layer, markers) in LAYER_ORDER {
        for marker in *markers {
            if lowered.contains(marker) && marker.len() > best_length {
                best_layer = layer;
                best_length = marker.len();
            }
        }
    }
    best_layer.to_string()
}

/// Group impacted files into reviewable steps: by module, then by layer, data first.
pub fn split_increments(impact: &Impact, max_files: usize) -> Vec<Value> {
    let max_files = max_files.max(1);
    let order = |layer: &str| {
        LAYER_ORDER
            .iter()
            .position(|(name, _)| *name == layer)
            .unwrap_or(LAYER_ORDER.len())
    };

    let mut grouped: Vec<((String, String), Vec<Value>)> = Vec::new();
    for item in &impact.files {
        let key = (
            item["module"].as_str().unwrap_or_default().to_string(),
            item["layer"].as_str().unwrap_or_default().to_string(),
        );
        match grouped.iter_mut().find(|(existing, _)| *existing == key) {
            Some((_, items)) => items.push(item.clone()),
            None => grouped.push((key, vec![item.clone()])),
        }
    }
    grouped.sort_by(|left, right| {
        order(&left.0 .1)
            .cmp(&order(&right.0 .1))
            .then_with(|| left.0 .0.cmp(&right.0 .0))
    });

    let mut increments: Vec<Value> = Vec::new();
    for ((module, layer), items) in grouped {
        for (chunk_index, chunk) in items.chunks(max_files).enumerate() {
            let suffix = if items.len() <= max_files {
                String::new()
            } else {
                format!(" (part {})", chunk_index + 1)
            };
            let mut terms: Vec<String> = chunk
                .iter()
                .flat_map(|item| {
                    item["terms"]
                        .as_array()
                        .map(|values| {
                            values
                                .iter()
                                .filter_map(|value| value.as_str().map(String::from))
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default()
                })
                .collect();
            terms.sort();
            terms.dedup();
            increments.push(json!({
                "step": increments.len() + 1,
                "title": format!("{module}: {layer} changes{suffix}"),
                "module": module,
                "layer": layer,
                "paths": chunk.iter().map(|item| item["path"].clone()).collect::<Vec<_>>(),
                "terms": terms,
            }));
        }
    }
    increments
}

#[cfg(test)]
mod tests {
    use super::*;

    fn symbols() -> Vec<Value> {
        vec![
            json!({"name": "FeedViewModel", "kind": "class",
                   "path": "app/src/main/kotlin/feed/FeedViewModel.kt"}),
            json!({"name": "FeedRepository", "kind": "class",
                   "path": "data/src/main/kotlin/feed/FeedRepo.kt"}),
            json!({"name": "LoginScreen", "kind": "class",
                   "path": "app/src/main/kotlin/login/LoginScreen.kt"}),
        ]
    }

    #[test]
    fn keywords_prefer_identifiers_over_prose() {
        let found = keywords(
            "The FeedViewModel should page through results using `PagingSource`.",
            25,
        );

        assert!(found.contains(&"FeedViewModel".to_string()));
        assert!(found.contains(&"PagingSource".to_string()));
        assert!(!found.contains(&"should".to_string()));
    }

    #[test]
    fn keywords_are_deduplicated_case_insensitively() {
        let found = keywords("Feed feed FEED FeedViewModel", 25);

        assert_eq!(
            found
                .iter()
                .filter(|term| term.to_lowercase() == "feed")
                .count(),
            1
        );
    }

    #[test]
    fn keywords_respect_the_limit() {
        let text = (0..50)
            .map(|i| format!("Word{i}Camel "))
            .collect::<String>();

        assert_eq!(keywords(&text, 10).len(), 10);
    }

    #[test]
    fn module_uses_the_segment_before_src() {
        assert_eq!(module_of("app/src/main/kotlin/com/acme/Feed.kt"), "app");
        assert_eq!(module_of("core/data/src/main/kotlin/Repo.kt"), "data");
    }

    #[test]
    fn module_falls_back_past_source_roots() {
        assert_eq!(module_of("lib/features/feed/view.dart"), "features");
        assert_eq!(module_of("main.py"), ".");
    }

    #[test]
    fn layer_prefers_the_most_specific_marker() {
        assert_eq!(layer_of("app/ui/FeedViewModel.kt"), "presentation");
        assert_eq!(layer_of("app/domain/FeedModel.kt"), "domain");
        assert_eq!(layer_of("app/data/FeedDao.kt"), "data");
        assert_eq!(layer_of("tools/script.sh"), "other");
    }

    #[test]
    fn impact_set_matches_symbols_and_groups_by_module() {
        let impact = impact_set(
            "FeedViewModel stops paging after the second FeedRepository call",
            &symbols(),
            25,
        );

        let paths: Vec<&str> = impact
            .files
            .iter()
            .filter_map(|file| file["path"].as_str())
            .collect();
        assert!(paths.contains(&"app/src/main/kotlin/feed/FeedViewModel.kt"));
        assert!(paths.contains(&"data/src/main/kotlin/feed/FeedRepo.kt"));
        assert!(!paths.contains(&"app/src/main/kotlin/login/LoginScreen.kt"));
        assert_eq!(impact.modules.len(), 2);
    }

    #[test]
    fn impact_set_is_empty_without_usable_terms() {
        let impact = impact_set("it is not ok", &symbols(), 25);

        assert!(impact.files.is_empty());
        assert!(impact.terms.is_empty());
    }

    #[test]
    fn split_orders_data_before_presentation() {
        let impact = impact_set("FeedViewModel and FeedRepository", &symbols(), 25);

        let increments = split_increments(&impact, 8);

        let layers: Vec<&str> = increments
            .iter()
            .filter_map(|item| item["layer"].as_str())
            .collect();
        assert_eq!(layers, vec!["data", "presentation"]);
        assert_eq!(increments[0]["step"], 1);
    }

    #[test]
    fn split_chunks_large_groups() {
        let symbols: Vec<Value> = (0..10)
            .map(|index| {
                json!({
                    "name": format!("Feed{index}"),
                    "kind": "class",
                    "path": format!("app/src/main/kotlin/ui/Feed{index}.kt"),
                })
            })
            .collect();
        let text: String = (0..10).map(|index| format!("Feed{index} ")).collect();
        let impact = impact_set(&text, &symbols, 25);

        let increments = split_increments(&impact, 4);

        assert_eq!(increments.len(), 3);
        assert!(increments
            .iter()
            .all(|item| item["paths"].as_array().unwrap().len() <= 4));
        assert!(increments[0]["title"].as_str().unwrap().contains("part 1"));
    }
}
