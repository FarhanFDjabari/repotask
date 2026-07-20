//! Knowledge base manifest, document frontmatter, and slice definitions.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

pub const KB_SCHEMA_VERSION: u32 = 1;
pub const MANIFEST_NAME: &str = "kb.yaml";
const FRONTMATTER_FENCE: &str = "---";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Layer {
    Convention,
    Recipe,
}

impl Layer {
    pub fn as_str(self) -> &'static str {
        match self {
            Layer::Convention => "convention",
            Layer::Recipe => "recipe",
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FactFamily {
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// Project stacks this family applies to. Empty means every stack. A family
    /// that matches none of `project.stacks` is not built at all.
    #[serde(default)]
    pub stacks: Vec<String>,
    /// Languages a symbol may be written in. Empty means every language — prefer
    /// `path_pattern` over listing languages to keep a family off other stacks.
    #[serde(default)]
    pub languages: Vec<String>,
    #[serde(default)]
    pub kinds: Vec<String>,
    #[serde(default)]
    pub name_pattern: String,
    #[serde(default)]
    pub path_pattern: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema_version: u32,
    #[serde(default = "default_kb_name")]
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_kb_budget")]
    pub default_budget: usize,
    #[serde(default)]
    pub fact_families: Vec<FactFamily>,
}

fn default_kb_name() -> String {
    "knowledge-base".into()
}

fn default_kb_budget() -> usize {
    6000
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Slice {
    #[serde(default)]
    pub stack: String,
    #[serde(default)]
    pub extends: Vec<String>,
    #[serde(default)]
    pub conventions: Vec<String>,
    #[serde(default)]
    pub recipes: Vec<String>,
    #[serde(default)]
    pub facts: Vec<String>,
    #[serde(default)]
    pub intents: BTreeMap<String, Vec<String>>,
}

/// Frontmatter as written on disk. `layer` may restate the directory, never contradict it.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Frontmatter {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    layer: Option<Layer>,
    #[serde(default)]
    stacks: Vec<String>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    applies_to: Vec<String>,
    #[serde(default)]
    budget_hint: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct Document {
    pub id: String,
    pub title: String,
    pub layer: Layer,
    pub stacks: Vec<String>,
    pub tags: Vec<String>,
    pub applies_to: Vec<String>,
    pub budget_hint: usize,
    pub body: String,
    pub path: String,
}

/// Split `---` delimited YAML frontmatter from the markdown body.
pub fn parse_frontmatter(text: &str) -> Result<(String, String)> {
    let lines: Vec<&str> = text.lines().collect();
    if lines.first().map(|line| line.trim()) != Some(FRONTMATTER_FENCE) {
        return Ok((String::new(), text.to_string()));
    }
    for (index, line) in lines.iter().enumerate().skip(1) {
        if line.trim() == FRONTMATTER_FENCE {
            let front = lines[1..index].join("\n");
            let body = lines[index + 1..].join("\n").trim().to_string();
            return Ok((front, body));
        }
    }
    bail!("Document frontmatter is missing its closing `---`.");
}

pub fn load_document(path: &Path, layer: Layer, kb_root: &Path) -> Result<Document> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("Could not read {}", path.display()))?;
    let (front_text, body) = parse_frontmatter(&text)?;
    let front: Frontmatter = if front_text.trim().is_empty() {
        Frontmatter::default()
    } else {
        serde_norway::from_str(&front_text)
            .with_context(|| format!("Invalid frontmatter in {}", path.display()))?
    };

    if let Some(declared) = front.layer {
        if declared != layer {
            bail!(
                "{} declares layer '{}' but sits in the {} directory.",
                path.display(),
                declared.as_str(),
                layer.as_str()
            );
        }
    }

    let stem = path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let relative = path
        .strip_prefix(kb_root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/");
    Ok(Document {
        id: front.id.unwrap_or_else(|| stem.clone()),
        title: front.title.unwrap_or_else(|| title_case(&stem)),
        layer,
        stacks: front.stacks,
        tags: front.tags,
        applies_to: front.applies_to,
        budget_hint: front.budget_hint,
        body,
        path: relative,
    })
}

fn title_case(stem: &str) -> String {
    stem.split('-')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn load_manifest(path: &Path) -> Result<Manifest> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("Could not read {}", path.display()))?;
    let manifest: Manifest =
        serde_norway::from_str(&text).with_context(|| format!("Invalid {MANIFEST_NAME}"))?;
    if manifest.schema_version != KB_SCHEMA_VERSION {
        bail!(
            "Knowledge base schema_version {} is not supported; this build reads version {}.",
            manifest.schema_version,
            KB_SCHEMA_VERSION
        );
    }
    Ok(manifest)
}

pub fn load_slice(path: &Path) -> Result<Slice> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("Could not read {}", path.display()))?;
    let mut slice: Slice = serde_norway::from_str(&text)
        .with_context(|| format!("Invalid slice {}", path.display()))?;
    if slice.stack.is_empty() {
        slice.stack = path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
    }
    Ok(slice)
}
