mod common;

use common::{commit_all, write, Fixture, KOTLIN_SOURCE};
use serde_json::Value;

const SWIFT_SOURCE: &str =
    "import Foundation\n\nclass FeedViewModel: ObservableObject {\n    func load() {}\n}\n";
const SWIFTUI_SOURCE: &str = "import SwiftUI\n\nstruct ContentView: View {\n    var body: some View { Text(\"hi\") }\n}\n\nclass FeedViewModel: ObservableObject {}\n";
const PYTHON_SOURCE: &str =
    "class Loader:\n    def load(self) -> None:\n        pass\n\n\ndef helper() -> int:\n    return 1\n";

fn indexed(label: &str) -> Fixture {
    let fixture = Fixture::new(label);
    write(
        &fixture.project,
        "app/src/main/kotlin/com/acme/feed/FeedViewModel.kt",
        KOTLIN_SOURCE,
    );
    fixture.run(&["kb", "sync"]);
    fixture.run(&["index"]);
    fixture
}

fn symbols(fixture: &Fixture) -> Vec<Value> {
    fixture.json(&["fact", "symbols"])["data"]["entries"]
        .as_array()
        .cloned()
        .unwrap_or_default()
}

#[test]
fn extracts_kotlin_classes_and_functions_with_scope() {
    let fixture = indexed("index-kotlin");

    let found: Vec<(String, String, String)> = symbols(&fixture)
        .iter()
        .map(|item| {
            (
                item["name"].as_str().unwrap_or("").into(),
                item["kind"].as_str().unwrap_or("").into(),
                item["scope"].as_str().unwrap_or("").into(),
            )
        })
        .collect();

    assert!(found.contains(&("FeedViewModel".into(), "class".into(), String::new())));
    assert!(found.contains(&("FeedRepository".into(), "class".into(), String::new())));
    assert!(found.contains(&("load".into(), "function".into(), "FeedViewModel".into())));
    assert!(found.contains(&("fetch".into(), "function".into(), "FeedRepository".into())));
}

#[test]
fn swift_structs_are_indexed_distinctly_from_classes() {
    let fixture = Fixture::new("index-swift-struct");
    write(&fixture.project, "ios/ContentView.swift", SWIFTUI_SOURCE);
    fixture.run(&["kb", "sync"]);
    fixture.run(&["index"]);

    let found: Vec<(String, String)> = symbols(&fixture)
        .iter()
        .map(|item| {
            (
                item["name"].as_str().unwrap_or("").into(),
                item["kind"].as_str().unwrap_or("").into(),
            )
        })
        .collect();

    assert!(found.contains(&("ContentView".into(), "struct".into())));
    assert!(found.contains(&("FeedViewModel".into(), "class".into())));
}

#[test]
fn records_line_numbers() {
    let fixture = indexed("index-lines");

    let viewmodel = symbols(&fixture)
        .into_iter()
        .find(|item| item["name"] == "FeedViewModel")
        .expect("FeedViewModel is indexed");

    assert_eq!(viewmodel["line"], 3);
}

#[test]
fn indexes_multiple_languages() {
    let fixture = Fixture::new("index-languages");
    write(&fixture.project, "app/FeedViewModel.kt", KOTLIN_SOURCE);
    write(&fixture.project, "ios/FeedViewModel.swift", SWIFT_SOURCE);
    write(&fixture.project, "tools/loader.py", PYTHON_SOURCE);
    fixture.run(&["kb", "sync"]);

    let languages = fixture.json(&["index"])["data"]["languages"].clone();

    assert!(languages["kotlin"].is_number());
    assert!(languages["swift"].is_number());
    assert!(languages["python"].is_number());
    assert!(symbols(&fixture)
        .iter()
        .any(|item| item["name"] == "Loader"));
}

#[test]
fn excluded_paths_are_not_indexed() {
    let fixture = Fixture::new("index-excluded");
    write(
        &fixture.project,
        "app/build/generated/Junk.kt",
        "class Junk {}\n",
    );
    write(&fixture.project, "app/Real.kt", "class Real {}\n");
    fixture.run(&["kb", "sync"]);
    fixture.run(&["index"]);

    let names: Vec<String> = symbols(&fixture)
        .iter()
        .filter_map(|item| item["name"].as_str().map(String::from))
        .collect();

    assert!(names.contains(&"Real".to_string()));
    assert!(!names.contains(&"Junk".to_string()));
}

#[test]
fn fact_family_filters_symbols_by_pattern_and_language() {
    let fixture = indexed("index-family");

    let envelope = fixture.json(&["fact", "viewmodels"]);
    let names: Vec<&str> = envelope["data"]["entries"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|item| item["name"].as_str())
        .collect();

    assert_eq!(names, vec!["FeedViewModel"]);
}

#[test]
fn fact_without_a_family_lists_what_is_declared() {
    let fixture = indexed("index-family-list");

    let envelope = fixture.json(&["fact"]);

    let families = envelope["data"]["families"].as_array().unwrap();
    assert_eq!(families[0]["name"], "viewmodels");
    assert_eq!(families[0]["indexed"], true);
}

#[test]
fn fact_without_an_index_explains_what_to_run() {
    let fixture = Fixture::new("index-missing");
    fixture.run(&["kb", "sync"]);

    let envelope = fixture.json(&["fact", "viewmodels"]);

    assert_eq!(envelope["ok"], false);
    assert!(envelope["error"]["message"]
        .as_str()
        .unwrap()
        .contains("repo-task index"));
}

#[test]
fn symbol_search_matches_case_insensitively() {
    let fixture = indexed("index-symbol");

    let envelope = fixture.json(&["symbol", "feedrepo"]);

    assert_eq!(envelope["data"]["hits"][0]["name"], "FeedRepository");
}

#[test]
fn symbol_search_can_restrict_to_a_kind() {
    let fixture = indexed("index-symbol-kind");

    let envelope = fixture.json(&["symbol", "Feed", "--kind", "function"]);

    let kinds: Vec<&str> = envelope["data"]["hits"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|item| item["kind"].as_str())
        .collect();
    assert!(kinds.iter().all(|kind| *kind == "function"));
}

#[test]
fn symbol_search_rejects_an_invalid_pattern() {
    let fixture = indexed("index-bad-pattern");

    let envelope = fixture.json(&["symbol", "(unclosed"]);

    assert_eq!(envelope["ok"], false);
    assert!(envelope["error"]["message"]
        .as_str()
        .unwrap()
        .contains("Invalid search pattern"));
}

#[test]
fn brief_excludes_knowledge_for_other_stacks() {
    let fixture = indexed("brief-stacks");

    let envelope = fixture.json(&["brief", "add pagination to the feed"]);
    let ids: Vec<&str> = envelope["data"]["documents"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|item| item["id"].as_str())
        .collect();

    assert!(ids.contains(&"android-architecture"));
    assert!(!ids.contains(&"ios-architecture"));
}

#[test]
fn brief_never_exceeds_its_budget() {
    let fixture = indexed("brief-budget");

    for budget in ["4", "12", "30", "60", "6000"] {
        let data = fixture.json(&["brief", "add pagination", "--budget", budget])["data"].clone();
        let used = data["used"].as_u64().unwrap();
        let limit = data["budget"].as_u64().unwrap();
        assert!(used <= limit, "budget {budget} exceeded: used {used}");
    }
}

#[test]
fn brief_reports_documents_it_had_to_omit() {
    let fixture = indexed("brief-omitted");

    let data = fixture.json(&["brief", "add pagination", "--budget", "12"])["data"].clone();

    assert!(data["documents"].as_array().unwrap().is_empty());
    let omitted: Vec<&str> = data["omitted"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|item| item.as_str())
        .collect();
    assert!(omitted.contains(&"pagination"));
}

#[test]
fn brief_includes_indexed_facts() {
    let fixture = indexed("brief-facts");

    let envelope = fixture.json(&["brief", "add pagination"]);

    let names: Vec<&str> = envelope["data"]["facts"]["viewmodels"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|item| item["name"].as_str())
        .collect();
    assert_eq!(names, vec!["FeedViewModel"]);
}

#[test]
fn brief_ranks_the_intent_slice_highest() {
    let fixture = indexed("brief-intent");

    let envelope = fixture.json(&["brief", "feature"]);

    let top = &envelope["data"]["documents"][0];
    assert!(top["score"].as_u64().unwrap() >= 100, "{top}");
    let reasons: Vec<&str> = top["reasons"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|item| item.as_str())
        .collect();
    assert!(reasons.contains(&"slice:feature"));
}

#[test]
fn changed_paths_lift_documents_that_declare_them() {
    let fixture = Fixture::new("brief-paths");
    write(
        &fixture.kb,
        "conventions/room.md",
        "---\nid: room\ntitle: Room Schemas\nstacks: [android]\napplies_to: [\"**/schemas/*.json\"]\n---\nSchemas are immutable once committed.\n",
    );
    commit_all(&fixture.kb, "add room convention");
    fixture.run(&["kb", "sync"]);

    let score_of = |envelope: &Value| -> u64 {
        envelope["data"]["documents"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["id"] == "room")
            .and_then(|item| item["score"].as_u64())
            .unwrap_or(0)
    };
    let without = fixture.json(&["brief", "bump database version"]);
    let with_paths = fixture.json(&[
        "brief",
        "bump database version",
        "--path",
        "app/schemas/2.json",
    ]);

    assert!(score_of(&with_paths) > score_of(&without));
}
