//! Select and rank the knowledge that applies to one piece of work.
//!
//! Ranking inputs, highest weight first:
//!   * the document is named by the stack slice for this intent
//!   * the document is named by the stack slice at all
//!   * the document declares one of the project stacks
//!   * intent words hit the document's tags, title, or id
//!   * the document's `applies_to` globs hit the changed paths
//!
//! Documents that match nothing are dropped, not merely ranked low: an Android task
//! should never pay for the iOS conventions.

use globset::Glob;
use serde::Serialize;
use serde_json::{json, Map, Value};

use crate::kb::budget::{estimate, fit};
use crate::kb::schema::Document;
use crate::kb::store::{split_words, KnowledgeBase};

const WEIGHT_INTENT_SLICE: u32 = 100;
const WEIGHT_SLICE: u32 = 50;
const WEIGHT_STACK: u32 = 20;
const WEIGHT_TAG: u32 = 10;
const WEIGHT_TITLE: u32 = 5;
const WEIGHT_PATH: u32 = 15;

const FACT_ENTRY_LIMIT: usize = 200;
const FACT_BUDGET_SHARE: f64 = 0.4;

pub struct RankedDocument {
    pub document: Document,
    pub score: u32,
    pub reasons: Vec<String>,
}

#[derive(Serialize)]
pub struct ContextPack {
    pub intent: String,
    pub budget: usize,
    pub used: usize,
    pub documents: Vec<Value>,
    pub facts: Map<String, Value>,
    pub omitted: Vec<String>,
}

pub fn rank(
    kb: &KnowledgeBase,
    stacks: &[String],
    intent: &str,
    changed_paths: &[String],
) -> Vec<RankedDocument> {
    let slice = kb.slice_for(stacks);
    let intent_ids = slice.intents.get(intent).cloned().unwrap_or_default();
    let mut slice_ids = slice.conventions.clone();
    slice_ids.extend(slice.recipes.clone());
    let words = split_words(intent);
    let words: Vec<&String> = words.iter().filter(|word| word.len() > 2).collect();

    let mut ranked: Vec<RankedDocument> = Vec::new();
    for document in kb.documents() {
        let mut score = 0;
        let mut reasons: Vec<String> = Vec::new();

        if intent_ids.contains(&document.id) {
            score += WEIGHT_INTENT_SLICE;
            reasons.push(format!("slice:{intent}"));
        }
        if slice_ids.contains(&document.id) {
            score += WEIGHT_SLICE;
            reasons.push("slice".into());
        }
        if !document.stacks.is_empty() && document.stacks.iter().any(|stack| stacks.contains(stack))
        {
            score += WEIGHT_STACK;
            reasons.push("stack".into());
        }

        let mut tag_hits: Vec<String> = document
            .tags
            .iter()
            .map(|tag| tag.to_lowercase())
            .filter(|tag| words.contains(&tag))
            .collect();
        tag_hits.sort();
        tag_hits.dedup();
        if !tag_hits.is_empty() {
            score += WEIGHT_TAG * tag_hits.len() as u32;
            reasons.push(format!("tags:{}", tag_hits.join(",")));
        }

        let title_words = split_words(&format!("{} {}", document.id, document.title));
        if title_words
            .iter()
            .any(|word| word.len() > 2 && words.contains(&word))
        {
            score += WEIGHT_TITLE;
            reasons.push("title".into());
        }

        let path_hits = path_hits(document, changed_paths);
        if !path_hits.is_empty() {
            score += WEIGHT_PATH;
            reasons.push(format!(
                "paths:{}",
                path_hits
                    .iter()
                    .take(3)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(",")
            ));
        }

        if score > 0 {
            ranked.push(RankedDocument {
                document: document.clone(),
                score,
                reasons,
            });
        }
    }

    ranked.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| {
                left.document
                    .layer
                    .as_str()
                    .cmp(right.document.layer.as_str())
            })
            .then_with(|| left.document.id.cmp(&right.document.id))
    });
    ranked
}

/// Assemble a budgeted context pack: ranked documents first, then project facts.
pub fn build_pack(
    kb: &KnowledgeBase,
    stacks: &[String],
    intent: &str,
    budget: usize,
    changed_paths: &[String],
) -> ContextPack {
    let ranked = rank(kb, stacks, intent, changed_paths);

    // Slice order is priority order: the knowledge base lists the families that matter
    // most first, and a tight budget must drop from the tail, not alphabetically.
    let families = kb.slice_for(stacks).facts;
    let mut available: Vec<(String, Vec<Value>)> = Vec::new();
    for name in families {
        let entries: Vec<Value> = kb.facts(&name).into_iter().take(FACT_ENTRY_LIMIT).collect();
        if !entries.is_empty() {
            available.push((name, entries));
        }
    }
    let (facts, facts_cost, dropped) = fit_facts(available, budget);

    let mut pack = ContextPack {
        intent: intent.to_string(),
        budget,
        used: facts_cost,
        documents: Vec::new(),
        facts,
        omitted: dropped.iter().map(|name| format!("facts:{name}")).collect(),
    };

    let mut remaining = budget.saturating_sub(facts_cost);
    for item in ranked {
        let document = item.document;
        if remaining == 0 {
            pack.omitted.push(document.id);
            continue;
        }
        let (body, truncated) = fit(
            &document.body,
            remaining,
            document.layer.as_str(),
            &document.id,
        );
        if body.is_empty() {
            pack.omitted.push(document.id);
            continue;
        }
        let used = estimate(&body);
        remaining = remaining.saturating_sub(used);
        pack.used += used;
        pack.documents.push(json!({
            "id": document.id,
            "title": document.title,
            "layer": document.layer.as_str(),
            "path": document.path,
            "score": item.score,
            "reasons": item.reasons,
            "truncated": truncated,
            "body": body,
        }));
    }

    pack
}

/// Cap facts at a share of the budget so documents always keep room.
fn fit_facts(
    families: Vec<(String, Vec<Value>)>,
    budget: usize,
) -> (Map<String, Value>, usize, Vec<String>) {
    let allowance = (budget as f64 * FACT_BUDGET_SHARE) as usize;
    let mut kept: Map<String, Value> = Map::new();
    let mut dropped: Vec<String> = Vec::new();
    let mut used = 0usize;

    for (name, entries) in families {
        let mut taken: Vec<Value> = Vec::new();
        for entry in &entries {
            let cost = estimate(&python_repr(entry));
            if used + cost > allowance {
                break;
            }
            taken.push(entry.clone());
            used += cost;
        }
        if taken.len() < entries.len() {
            dropped.push(name.clone());
        }
        if !taken.is_empty() {
            kept.insert(name, Value::Array(taken));
        }
    }
    (kept, used, dropped)
}

/// Fact cost is measured the way the Python implementation measured it, so budgets
/// stay comparable across the two: a dict rendered as `{'key': 'value'}`.
fn python_repr(value: &Value) -> String {
    match value {
        Value::Object(map) => {
            let inner: Vec<String> = map
                .iter()
                .map(|(key, item)| format!("'{key}': {}", python_repr(item)))
                .collect();
            format!("{{{}}}", inner.join(", "))
        }
        Value::Array(items) => {
            let inner: Vec<String> = items.iter().map(python_repr).collect();
            format!("[{}]", inner.join(", "))
        }
        Value::String(text) => format!("'{text}'"),
        Value::Null => "None".into(),
        Value::Bool(true) => "True".into(),
        Value::Bool(false) => "False".into(),
        other => other.to_string(),
    }
}

fn path_hits(document: &Document, paths: &[String]) -> Vec<String> {
    document
        .applies_to
        .iter()
        .filter(|pattern| {
            Glob::new(pattern)
                .map(|glob| {
                    let matcher = glob.compile_matcher();
                    paths.iter().any(|path| matcher.is_match(path))
                })
                .unwrap_or(false)
        })
        .cloned()
        .collect()
}
