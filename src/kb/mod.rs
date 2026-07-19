//! Three-layer knowledge base: conventions, project facts, recipes.

pub mod budget;
pub mod schema;
pub mod seed;
pub mod slicing;
pub mod source;
pub mod store;

use anyhow::Result;

use crate::config::RepoTaskConfig;
use store::KnowledgeBase;

/// Load project config and the knowledge base it points at.
pub fn open(config: &RepoTaskConfig, sync: Option<bool>) -> Result<KnowledgeBase> {
    store::load(source::resolve(config, sync)?)
}
