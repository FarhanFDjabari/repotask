//! Load and query the knowledge base layers.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use serde_json::Value;

use crate::kb::schema::{
    load_document, load_manifest, load_slice, Document, Layer, Manifest, Slice, MANIFEST_NAME,
};
use crate::kb::source::KnowledgeSource;

pub const CONVENTIONS_DIR: &str = "conventions";
pub const RECIPES_DIR: &str = "recipes";
pub const FACTS_DIR: &str = "facts";
pub const SLICES_DIR: &str = "slices";

pub struct SearchHit {
    pub document: Document,
    pub score: u32,
    pub excerpt: String,
}

pub struct KnowledgeBase {
    pub source: KnowledgeSource,
    pub manifest: Manifest,
    pub conventions: BTreeMap<String, Document>,
    pub recipes: BTreeMap<String, Document>,
    pub slices: BTreeMap<String, Slice>,
}

impl KnowledgeBase {
    pub fn documents(&self) -> Vec<&Document> {
        self.conventions
            .values()
            .chain(self.recipes.values())
            .collect()
    }

    pub fn facts_path(&self, family: &str) -> PathBuf {
        self.source
            .path
            .join(FACTS_DIR)
            .join(format!("{family}.json"))
    }

    /// Read a generated fact family. A missing family is empty, not an error — the
    /// project may simply not have been indexed yet.
    pub fn facts(&self, family: &str) -> Vec<Value> {
        let path = self.facts_path(family);
        let Ok(text) = std::fs::read_to_string(&path) else {
            return Vec::new();
        };
        let Ok(value) = serde_json::from_str::<Value>(&text) else {
            return Vec::new();
        };
        let entries = match &value {
            Value::Object(map) => map.get("entries").cloned().unwrap_or(value.clone()),
            other => other.clone(),
        };
        match entries {
            Value::Array(items) => items,
            _ => Vec::new(),
        }
    }

    pub fn fact_families(&self) -> Vec<String> {
        let directory = self.source.path.join(FACTS_DIR);
        let Ok(entries) = std::fs::read_dir(&directory) else {
            return Vec::new();
        };
        let mut names: Vec<String> = entries
            .flatten()
            .filter_map(|entry| {
                let path = entry.path();
                if path.extension().and_then(|value| value.to_str()) == Some("json") {
                    path.file_stem()
                        .map(|stem| stem.to_string_lossy().to_string())
                } else {
                    None
                }
            })
            .collect();
        names.sort();
        names
    }

    /// Merge every slice matching the project stacks, following `extends` chains.
    pub fn slice_for(&self, stacks: &[String]) -> Slice {
        let mut seen: Vec<String> = Vec::new();
        let mut merged = Slice {
            stack: stacks.join("+"),
            ..Slice::default()
        };
        for stack in stacks {
            self.visit_slice(stack, &mut seen, &mut merged);
        }
        merged
    }

    fn visit_slice(&self, name: &str, seen: &mut Vec<String>, merged: &mut Slice) {
        if seen.iter().any(|item| item == name) {
            return;
        }
        seen.push(name.to_string());
        let Some(current) = self.slices.get(name) else {
            return;
        };
        for parent in &current.extends {
            self.visit_slice(parent, seen, merged);
        }
        extend(&mut merged.conventions, &current.conventions);
        extend(&mut merged.recipes, &current.recipes);
        extend(&mut merged.facts, &current.facts);
        for (intent, ids) in &current.intents {
            extend(merged.intents.entry(intent.clone()).or_default(), ids);
        }
    }

    pub fn search(&self, query: &str, layer: Option<Layer>, limit: usize) -> Vec<SearchHit> {
        let terms: Vec<String> = split_words(query);
        if terms.is_empty() {
            return Vec::new();
        }
        let mut hits: Vec<SearchHit> = self
            .documents()
            .into_iter()
            .filter(|document| layer.is_none_or(|value| document.layer == value))
            .filter_map(|document| {
                let score = score(document, &terms);
                (score > 0).then(|| SearchHit {
                    document: document.clone(),
                    score,
                    excerpt: excerpt(&document.body, &terms),
                })
            })
            .collect();
        hits.sort_by(|left, right| {
            right
                .score
                .cmp(&left.score)
                .then_with(|| left.document.id.cmp(&right.document.id))
        });
        hits.truncate(limit);
        hits
    }
}

pub fn load(source: KnowledgeSource) -> Result<KnowledgeBase> {
    let manifest_path = source.path.join(MANIFEST_NAME);
    if !manifest_path.is_file() {
        bail!(
            "{MANIFEST_NAME} not found in {}. This does not look like a RepoTask knowledge base.",
            source.path.display()
        );
    }
    let manifest = load_manifest(&manifest_path)?;
    let conventions = load_layer(&source.path, CONVENTIONS_DIR, Layer::Convention)?;
    let recipes = load_layer(&source.path, RECIPES_DIR, Layer::Recipe)?;

    let mut slices = BTreeMap::new();
    let slices_dir = source.path.join(SLICES_DIR);
    if slices_dir.is_dir() {
        let mut paths: Vec<PathBuf> = std::fs::read_dir(&slices_dir)?
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("yaml"))
            .collect();
        paths.sort();
        for path in paths {
            let slice = load_slice(&path)?;
            slices.insert(slice.stack.clone(), slice);
        }
    }

    Ok(KnowledgeBase {
        source,
        manifest,
        conventions,
        recipes,
        slices,
    })
}

fn load_layer(root: &Path, directory: &str, layer: Layer) -> Result<BTreeMap<String, Document>> {
    let path = root.join(directory);
    if !path.is_dir() {
        return Ok(BTreeMap::new());
    }
    let mut files: Vec<PathBuf> = walkdir::WalkDir::new(&path)
        .into_iter()
        .flatten()
        .map(|entry| entry.into_path())
        .filter(|item| item.extension().and_then(|value| value.to_str()) == Some("md"))
        .collect();
    files.sort();

    let mut documents: BTreeMap<String, Document> = BTreeMap::new();
    for file in files {
        let document = load_document(&file, layer, root)?;
        if let Some(existing) = documents.get(&document.id) {
            bail!(
                "Duplicate knowledge document id '{}' in {directory}: {} and {}",
                document.id,
                existing.path,
                document.path
            );
        }
        documents.insert(document.id.clone(), document);
    }
    Ok(documents)
}

fn extend(target: &mut Vec<String>, values: &[String]) {
    for value in values {
        if !target.iter().any(|item| item == value) {
            target.push(value.clone());
        }
    }
}

pub fn split_words(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(|word| word.to_lowercase())
        .collect()
}

/// Weight matches by where they land: id and tags beat prose.
fn score(document: &Document, terms: &[String]) -> u32 {
    let tags = document.tags.join(" ").to_lowercase();
    let haystacks = [
        (document.id.to_lowercase(), 8u32),
        (document.title.to_lowercase(), 5),
        (tags, 5),
        (document.body.to_lowercase(), 1),
    ];
    let mut total = 0;
    for term in terms {
        for (text, weight) in &haystacks {
            if text.contains(term) {
                total += weight;
            }
        }
    }
    total
}

fn excerpt(body: &str, terms: &[String]) -> String {
    const WIDTH: usize = 200;
    let lowered = body.to_lowercase();
    for term in terms {
        if let Some(position) = lowered.find(term.as_str()) {
            let start = floor_boundary(body, position.saturating_sub(WIDTH / 2));
            let end = ceil_boundary(body, (start + WIDTH).min(body.len()));
            return body[start..end].trim().replace('\n', " ");
        }
    }
    let end = ceil_boundary(body, WIDTH.min(body.len()));
    body[..end].trim().replace('\n', " ")
}

fn floor_boundary(text: &str, mut index: usize) -> usize {
    while index > 0 && !text.is_char_boundary(index) {
        index -= 1;
    }
    index
}

fn ceil_boundary(text: &str, mut index: usize) -> usize {
    while index < text.len() && !text.is_char_boundary(index) {
        index += 1;
    }
    index
}
