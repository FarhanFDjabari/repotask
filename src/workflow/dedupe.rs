//! Cluster bug tickets that point at the same code.
//!
//! Two tickets that describe different symptoms but resolve to the same files and
//! symbols usually share a root cause. The CLI surfaces that overlap as evidence;
//! the decision to merge stays with a human.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{json, Value};

use crate::workflow::analyze::{impact_set, Impact};

pub const MIN_SIMILARITY: f64 = 0.34;

pub struct TicketImpact {
    pub ticket_id: String,
    pub title: String,
    pub impact: Impact,
}

impl TicketImpact {
    pub fn paths(&self) -> BTreeSet<String> {
        self.impact
            .files
            .iter()
            .filter_map(|file| file["path"].as_str().map(String::from))
            .collect()
    }

    pub fn symbols(&self) -> BTreeSet<String> {
        self.impact
            .symbols
            .iter()
            .map(|symbol| {
                format!(
                    "{}:{}",
                    symbol["path"].as_str().unwrap_or_default(),
                    symbol["name"].as_str().unwrap_or_default()
                )
            })
            .collect()
    }
}

pub fn jaccard(left: &BTreeSet<String>, right: &BTreeSet<String>) -> f64 {
    if left.is_empty() || right.is_empty() {
        return 0.0;
    }
    left.intersection(right).count() as f64 / left.union(right).count() as f64
}

fn round3(value: f64) -> f64 {
    (value * 1000.0).round() / 1000.0
}

/// Blend file and symbol overlap; symbol agreement is the stronger signal.
pub fn similarity(left: &TicketImpact, right: &TicketImpact) -> (f64, Value) {
    let left_paths = left.paths();
    let right_paths = right.paths();
    let left_symbols = left.symbols();
    let right_symbols = right.symbols();

    let path_score = jaccard(&left_paths, &right_paths);
    let symbol_score = jaccard(&left_symbols, &right_symbols);
    let score = round3(0.4 * path_score + 0.6 * symbol_score);

    let shared_symbols: BTreeSet<String> = left_symbols
        .intersection(&right_symbols)
        .filter_map(|item| item.split_once(':').map(|(_, name)| name.to_string()))
        .collect();
    let evidence = json!({
        "sharedPaths": left_paths.intersection(&right_paths).cloned().collect::<Vec<_>>(),
        "sharedSymbols": shared_symbols.into_iter().collect::<Vec<_>>(),
        "pathSimilarity": round3(path_score),
        "symbolSimilarity": round3(symbol_score),
    });
    (score, evidence)
}

pub fn build_impacts(tickets: &[Value], symbols: &[Value]) -> Vec<TicketImpact> {
    tickets
        .iter()
        .map(|ticket| {
            let title = ticket["title"].as_str().unwrap_or_default().to_string();
            let body = ticket["body"].as_str().unwrap_or_default();
            TicketImpact {
                ticket_id: ticket["id"].as_str().unwrap_or_default().to_string(),
                impact: impact_set(&format!("{title}\n{body}"), symbols, 25),
                title,
            }
        })
        .collect()
}

fn find(parent: &mut BTreeMap<String, String>, node: &str) -> String {
    let mut current = node.to_string();
    while parent[&current] != current {
        let grandparent = parent[&parent[&current]].clone();
        parent.insert(current.clone(), grandparent.clone());
        current = grandparent;
    }
    current
}

/// Single-linkage clustering over the similarity graph.
pub fn cluster(impacts: &[TicketImpact], threshold: f64) -> Vec<Value> {
    let mut pairs: Vec<(String, String, f64, Value)> = Vec::new();
    for (index, left) in impacts.iter().enumerate() {
        for right in impacts.iter().skip(index + 1) {
            let (score, evidence) = similarity(left, right);
            if score >= threshold {
                pairs.push((
                    left.ticket_id.clone(),
                    right.ticket_id.clone(),
                    score,
                    evidence,
                ));
            }
        }
    }

    let mut parent: BTreeMap<String, String> = impacts
        .iter()
        .map(|item| (item.ticket_id.clone(), item.ticket_id.clone()))
        .collect();
    for (left_id, right_id, _, _) in &pairs {
        let left_root = find(&mut parent, left_id);
        let right_root = find(&mut parent, right_id);
        if left_root != right_root {
            parent.insert(right_root, left_root);
        }
    }

    let by_id: BTreeMap<&str, &TicketImpact> = impacts
        .iter()
        .map(|item| (item.ticket_id.as_str(), item))
        .collect();
    let ids: Vec<String> = parent.keys().cloned().collect();
    let mut groups: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for ticket_id in ids {
        let root = find(&mut parent, &ticket_id);
        groups.entry(root).or_default().push(ticket_id);
    }

    let mut clusters: Vec<Value> = Vec::new();
    for (_, mut members) in groups {
        if members.len() < 2 {
            continue;
        }
        members.sort();

        let mut member_pairs: Vec<Value> = pairs
            .iter()
            .filter(|(left, right, _, _)| members.contains(left) && members.contains(right))
            .map(|(left, right, score, evidence)| {
                json!({"tickets": [left, right], "score": score, "evidence": evidence})
            })
            .collect();
        member_pairs.sort_by(|left, right| {
            right["score"]
                .as_f64()
                .unwrap_or(0.0)
                .partial_cmp(&left["score"].as_f64().unwrap_or(0.0))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let top_score = member_pairs
            .iter()
            .filter_map(|pair| pair["score"].as_f64())
            .fold(0.0, f64::max);

        let mut shared: Option<BTreeSet<String>> = None;
        for member in &members {
            let paths = by_id[member.as_str()].paths();
            shared = Some(match shared {
                Some(current) => current.intersection(&paths).cloned().collect(),
                None => paths,
            });
        }

        clusters.push(json!({
            "tickets": members
                .iter()
                .map(|id| json!({"id": id, "title": by_id[id.as_str()].title}))
                .collect::<Vec<_>>(),
            "sharedPaths": shared.unwrap_or_default().into_iter().collect::<Vec<_>>(),
            "pairs": member_pairs,
            "topScore": top_score,
        }));
    }

    clusters.sort_by(|left, right| {
        right["topScore"]
            .as_f64()
            .unwrap_or(0.0)
            .partial_cmp(&left["topScore"].as_f64().unwrap_or(0.0))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    clusters
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
    fn jaccard_handles_empty_sets() {
        let empty = BTreeSet::new();
        let one: BTreeSet<String> = ["a".to_string()].into_iter().collect();

        assert_eq!(jaccard(&empty, &one), 0.0);
        assert_eq!(jaccard(&one, &one), 1.0);
    }

    #[test]
    fn groups_tickets_that_share_code() {
        let tickets = vec![
            json!({"id": "BUG-1", "title": "Feed stops", "body": "FeedViewModel never emits"}),
            json!({"id": "BUG-2", "title": "Feed spinner", "body": "FeedViewModel stays loading"}),
            json!({"id": "BUG-3", "title": "Login typo", "body": "LoginScreen copy is wrong"}),
        ];

        let clusters = cluster(&build_impacts(&tickets, &symbols()), MIN_SIMILARITY);

        assert_eq!(clusters.len(), 1);
        let ids: Vec<&str> = clusters[0]["tickets"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|item| item["id"].as_str())
            .collect();
        assert_eq!(ids, vec!["BUG-1", "BUG-2"]);
    }

    #[test]
    fn reports_shared_evidence() {
        let tickets = vec![
            json!({"id": "A", "title": "", "body": "FeedViewModel breaks"}),
            json!({"id": "B", "title": "", "body": "FeedViewModel is stuck"}),
        ];

        let clusters = cluster(&build_impacts(&tickets, &symbols()), MIN_SIMILARITY);

        assert_eq!(
            clusters[0]["sharedPaths"],
            json!(["app/src/main/kotlin/feed/FeedViewModel.kt"])
        );
        assert_eq!(
            clusters[0]["pairs"][0]["evidence"]["sharedSymbols"],
            json!(["FeedViewModel"])
        );
    }

    #[test]
    fn respects_the_threshold() {
        let tickets = vec![
            json!({"id": "A", "title": "", "body": "FeedViewModel and FeedRepository"}),
            json!({"id": "B", "title": "", "body": "FeedViewModel only"}),
        ];

        assert!(cluster(&build_impacts(&tickets, &symbols()), 0.99).is_empty());
    }

    #[test]
    fn unrelated_tickets_form_no_cluster() {
        let tickets = vec![
            json!({"id": "A", "title": "", "body": "FeedViewModel breaks"}),
            json!({"id": "B", "title": "", "body": "LoginScreen copy is wrong"}),
        ];

        assert!(cluster(&build_impacts(&tickets, &symbols()), MIN_SIMILARITY).is_empty());
    }

    #[test]
    fn single_linkage_joins_transitive_matches() {
        let tickets = vec![
            json!({"id": "A", "title": "", "body": "FeedViewModel"}),
            json!({"id": "B", "title": "", "body": "FeedViewModel"}),
            json!({"id": "C", "title": "", "body": "FeedViewModel"}),
        ];

        let clusters = cluster(&build_impacts(&tickets, &symbols()), MIN_SIMILARITY);

        assert_eq!(clusters.len(), 1);
        assert_eq!(clusters[0]["tickets"].as_array().unwrap().len(), 3);
    }
}
