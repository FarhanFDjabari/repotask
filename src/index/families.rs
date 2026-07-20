//! Project facts: curated views over the symbol index, declared in kb.yaml.
//!
//! A fact family is a filter, not a parser. The knowledge base decides that
//! "viewmodels" means Kotlin classes named `*ViewModel`; the CLI just applies it.

use std::collections::BTreeMap;

use anyhow::{bail, Context, Result};
use globset::Glob;
use regex::Regex;
use serde_json::{to_value, Value};

use crate::index::runner::{IndexResult, Symbol};
use crate::kb::schema::FactFamily;

pub const SYMBOLS_FAMILY: &str = "symbols";
pub const SYMBOLS_DESCRIPTION: &str = "Every indexed declaration (searchable with `symbol`)";

fn matches(family: &FactFamily, symbol: &Symbol, pattern: Option<&Regex>) -> bool {
    if !family.languages.is_empty() && !family.languages.contains(&symbol.language) {
        return false;
    }
    if !family.kinds.is_empty() && !family.kinds.contains(&symbol.kind) {
        return false;
    }
    if !family.path_pattern.is_empty() {
        let hit = Glob::new(&family.path_pattern)
            .map(|glob| glob.compile_matcher().is_match(&symbol.path))
            .unwrap_or(false);
        if !hit {
            return false;
        }
    }
    if let Some(pattern) = pattern {
        if !pattern.is_match(&symbol.name) {
            return false;
        }
    }
    true
}

pub fn collect(family: &FactFamily, index: &IndexResult) -> Result<Vec<Value>> {
    let pattern = compile(family)?;
    index
        .symbols
        .iter()
        .filter(|symbol| matches(family, symbol, pattern.as_ref()))
        .map(|symbol| to_value(symbol).context("Could not serialize symbol"))
        .collect()
}

/// Whether a family applies to a project declaring `stacks`.
///
/// An empty `stacks` means the family is stack-agnostic and always applies.
fn applies_to(family: &FactFamily, stacks: &[String]) -> bool {
    family.stacks.is_empty() || family.stacks.iter().any(|stack| stacks.contains(stack))
}

/// Every declared family that applies to `stacks`, plus the raw `symbols` family
/// the index always provides.
pub fn build_all(
    families: &[FactFamily],
    stacks: &[String],
    index: &IndexResult,
) -> Result<BTreeMap<String, Vec<Value>>> {
    let mut result: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    let symbols: Result<Vec<Value>> = index
        .symbols
        .iter()
        .map(|symbol| to_value(symbol).context("Could not serialize symbol"))
        .collect();
    result.insert(SYMBOLS_FAMILY.to_string(), symbols?);

    for family in families {
        if family.name == SYMBOLS_FAMILY {
            bail!("'{SYMBOLS_FAMILY}' is reserved and cannot be redeclared.");
        }
        if !applies_to(family, stacks) {
            continue;
        }
        result.insert(family.name.clone(), collect(family, index)?);
    }
    Ok(result)
}

fn compile(family: &FactFamily) -> Result<Option<Regex>> {
    if family.name_pattern.is_empty() {
        return Ok(None);
    }
    Regex::new(&family.name_pattern)
        .map(Some)
        .with_context(|| format!("Fact family '{}' has an invalid name_pattern", family.name))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn family(name: &str, stacks: &[&str]) -> FactFamily {
        FactFamily {
            name: name.into(),
            stacks: stacks.iter().map(|stack| stack.to_string()).collect(),
            ..FactFamily::default()
        }
    }

    fn stacks(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn a_family_without_stacks_applies_everywhere() {
        assert!(applies_to(
            &family("screens", &[]),
            &stacks(["go"].as_slice())
        ));
    }

    #[test]
    fn a_family_applies_when_one_stack_overlaps() {
        let components = family("components", &["web", "flutter"]);

        assert!(applies_to(&components, &stacks(&["dart", "flutter"])));
    }

    #[test]
    fn a_family_is_skipped_when_no_stack_overlaps() {
        let components = family("components", &["web", "flutter"]);

        assert!(!applies_to(&components, &stacks(&["go", "rust"])));
    }

    #[test]
    fn build_all_omits_families_the_project_does_not_use() {
        let index = IndexResult::default();
        let families = vec![family("components", &["web"]), family("usecases", &[])];

        let built = build_all(&families, &stacks(&["rust"]), &index).unwrap();

        assert!(built.contains_key(SYMBOLS_FAMILY));
        assert!(built.contains_key("usecases"));
        assert!(
            !built.contains_key("components"),
            "a web-only family must not be built for a Rust project"
        );
    }
}
