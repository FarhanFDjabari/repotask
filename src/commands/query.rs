//! Direct knowledge lookups: `convention`, `recipe`, `search`.

use anyhow::{bail, Result};
use serde_json::{json, Value};

use crate::config;
use crate::kb;
use crate::kb::schema::{Document, Layer};
use crate::kb::store::KnowledgeBase;
use crate::output;

fn document_json(document: &Document) -> Value {
    json!({
        "id": document.id,
        "title": document.title,
        "layer": document.layer.as_str(),
        "stacks": document.stacks,
        "tags": document.tags,
        "path": document.path,
        "body": document.body,
    })
}

/// Exact id first; otherwise fall back to a stack-filtered search.
fn lookup(kb: &KnowledgeBase, layer: Layer, topic: &str, stacks: &[String]) -> Vec<Document> {
    let pool = match layer {
        Layer::Convention => &kb.conventions,
        Layer::Recipe => &kb.recipes,
    };
    if let Some(exact) = pool.get(topic) {
        return vec![exact.clone()];
    }
    let hits = kb.search(topic, Some(layer), 20);
    let scoped: Vec<Document> = hits
        .iter()
        .filter(|hit| {
            hit.document.stacks.is_empty()
                || hit
                    .document
                    .stacks
                    .iter()
                    .any(|stack| stacks.contains(stack))
        })
        .map(|hit| hit.document.clone())
        .collect();
    if scoped.is_empty() {
        hits.into_iter().map(|hit| hit.document).collect()
    } else {
        scoped
    }
}

fn render_documents(value: &Value) -> String {
    let Some(documents) = value["documents"].as_array() else {
        return "No matching documents.".into();
    };
    documents
        .iter()
        .map(|document| {
            format!(
                "{}\n{}\n\n{}",
                output::style::heading(&format!(
                    "# {}",
                    document["title"].as_str().unwrap_or("")
                )),
                output::style::dim(document["path"].as_str().unwrap_or("")),
                document["body"].as_str().unwrap_or(""),
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

pub fn convention(topic: &str, limit: usize) -> Result<()> {
    let config = config::load()?;
    let kb = kb::open(&config, None)?;
    let documents: Vec<Document> = lookup(&kb, Layer::Convention, topic, &config.project.stacks)
        .into_iter()
        .take(limit)
        .collect();
    if documents.is_empty() {
        bail!("No convention matched '{topic}'. Try `repo-task search {topic}`.");
    }
    let data = json!({
        "topic": topic,
        "documents": documents.iter().map(document_json).collect::<Vec<_>>(),
    });
    output::emit("convention", &data, render_documents);
    Ok(())
}

pub fn recipe(task: &str, limit: usize) -> Result<()> {
    let config = config::load()?;
    let kb = kb::open(&config, None)?;
    let documents: Vec<Document> = lookup(&kb, Layer::Recipe, task, &config.project.stacks)
        .into_iter()
        .take(limit)
        .collect();
    if documents.is_empty() {
        bail!("No recipe matched '{task}'. Try `repo-task search {task}`.");
    }
    let data = json!({
        "task": task,
        "documents": documents.iter().map(document_json).collect::<Vec<_>>(),
    });
    output::emit("recipe", &data, render_documents);
    Ok(())
}

pub fn search(query: &str, layer: &str, limit: usize) -> Result<()> {
    let layer = match layer {
        "" => None,
        "convention" => Some(Layer::Convention),
        "recipe" => Some(Layer::Recipe),
        _ => bail!("--layer must be `convention` or `recipe`."),
    };
    let config = config::load()?;
    let kb = kb::open(&config, None)?;
    let hits = kb.search(query, layer, limit);
    let data = json!({
        "query": query,
        "hits": hits.iter().map(|hit| json!({
            "id": hit.document.id,
            "title": hit.document.title,
            "layer": hit.document.layer.as_str(),
            "stacks": hit.document.stacks,
            "tags": hit.document.tags,
            "score": hit.score,
            "excerpt": hit.excerpt,
        })).collect::<Vec<_>>(),
    });
    output::emit("search", &data, |value| {
        let Some(hits) = value["hits"].as_array() else {
            return String::new();
        };
        if hits.is_empty() {
            return format!(
                "No knowledge matched '{}'.",
                value["query"].as_str().unwrap_or("")
            );
        }
        hits.iter()
            .map(|hit| {
                format!(
                    "{} {:<11} {}\n    {}",
                    output::style::id(&format!("{:<26}", hit["id"].as_str().unwrap_or(""))),
                    hit["layer"].as_str().unwrap_or(""),
                    hit["title"].as_str().unwrap_or(""),
                    output::style::dim(hit["excerpt"].as_str().unwrap_or("")),
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    });
    Ok(())
}
