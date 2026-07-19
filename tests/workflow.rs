mod common;

use common::{write, Fixture, KOTLIN_SOURCE};

const TICKET: &str = "# ACME-77 Paginated feed

Users should scroll the feed infinitely.
FeedViewModel must page through FeedRepository results.
Also ship an iOS widget and update the marketing site.
";

fn prepared(label: &str) -> Fixture {
    let fixture = Fixture::new(label);
    write(
        &fixture.project,
        "app/src/main/kotlin/feed/FeedViewModel.kt",
        KOTLIN_SOURCE,
    );
    fixture.run(&["kb", "sync"]);
    fixture.run(&["index"]);
    fixture
}

fn fetched(label: &str) -> Fixture {
    let fixture = prepared(label);
    fixture.json_stdin(&["fetch", "ACME-77", "--write", "-"], TICKET);
    fixture
}

#[test]
fn fetch_stores_content_written_by_the_agent() {
    let fixture = prepared("work-fetch");

    let envelope = fixture.json_stdin(&["fetch", "ACME-77", "--write", "-"], TICKET);

    assert_eq!(envelope["ok"], true, "{envelope}");
    assert_eq!(envelope["data"]["mode"], "write");
    assert!(fixture
        .project
        .join(".repo-task/work/ACME-77/source.md")
        .is_file());
}

#[test]
fn fetch_rejects_empty_stdin() {
    let fixture = prepared("work-empty");

    let envelope = fixture.json_stdin(&["fetch", "ACME-77", "--write", "-"], "  \n");

    assert_eq!(envelope["ok"], false);
    assert!(envelope["error"]["message"]
        .as_str()
        .unwrap()
        .contains("stdin"));
}

#[test]
fn connectors_must_be_configured_before_fetching() {
    let fixture = prepared("work-no-connector");

    let envelope = fixture.json(&["fetch", "ACME-77"]);

    assert_eq!(envelope["ok"], false);
    assert!(envelope["error"]["message"]
        .as_str()
        .unwrap()
        .contains("No connectors"));
}

#[test]
fn summarize_returns_the_source_and_a_stack_filtered_contract() {
    let fixture = fetched("work-summarize");

    let data = fixture.json(&["summarize", "ACME-77"])["data"].clone();

    assert_eq!(data["mode"], "prepare");
    assert!(data["source"].as_str().unwrap().contains("Paginated feed"));
    assert!(data["contract"]
        .as_str()
        .unwrap()
        .contains("android, kotlin"));
    let ids: Vec<&str> = data["context"]["documents"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|item| item["id"].as_str())
        .collect();
    assert!(!ids.contains(&"ios-architecture"));
}

#[test]
fn summarize_without_a_source_says_what_to_run() {
    let fixture = prepared("work-no-source");

    let envelope = fixture.json(&["summarize", "ACME-77"]);

    assert_eq!(envelope["ok"], false);
    assert!(envelope["error"]["message"]
        .as_str()
        .unwrap()
        .contains("repo-task fetch"));
}

#[test]
fn summarize_stores_what_the_agent_writes_back() {
    let fixture = fetched("work-summarize-write");

    let envelope = fixture.json_stdin(
        &["summarize", "ACME-77", "--write", "-"],
        "## Scope\nFeed only.\n",
    );

    assert_eq!(envelope["ok"], true, "{envelope}");
    assert!(fixture
        .project
        .join(".repo-task/work/ACME-77/summary.md")
        .is_file());
}

#[test]
fn analyze_resolves_ticket_vocabulary_to_indexed_code() {
    let fixture = fetched("work-analyze");

    let data = fixture.json(&["analyze", "ACME-77"])["data"].clone();

    let paths: Vec<&str> = data["impact"]["files"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|item| item["path"].as_str())
        .collect();
    assert!(paths.iter().any(|path| path.contains("FeedViewModel.kt")));
    assert!(data["rubric"].as_str().unwrap().contains("XL"));
}

#[test]
fn analyze_prefers_the_summary_over_the_raw_source() {
    let fixture = fetched("work-basedon");
    fixture.json_stdin(
        &["summarize", "ACME-77", "--write", "-"],
        "## Scope\nFeedViewModel only.\n",
    );

    let envelope = fixture.json(&["analyze", "ACME-77"]);

    assert_eq!(envelope["data"]["basedOn"], "summary.md");
}

#[test]
fn analyze_without_an_index_says_what_to_run() {
    let fixture = Fixture::new("work-analyze-no-index");
    fixture.run(&["kb", "sync"]);
    fixture.json_stdin(&["fetch", "ACME-77", "--write", "-"], TICKET);

    let envelope = fixture.json(&["analyze", "ACME-77"]);

    assert_eq!(envelope["ok"], false);
    assert!(envelope["error"]["message"]
        .as_str()
        .unwrap()
        .contains("repo-task index"));
}

#[test]
fn split_requires_an_analysis_first() {
    let fixture = fetched("work-split-order");

    let envelope = fixture.json(&["split", "ACME-77"]);

    assert_eq!(envelope["ok"], false);
    assert!(envelope["error"]["message"]
        .as_str()
        .unwrap()
        .contains("repo-task analyze"));
}

#[test]
fn split_produces_numbered_increments() {
    let fixture = fetched("work-split");
    fixture.run(&["analyze", "ACME-77"]);

    let data = fixture.json(&["split", "ACME-77"])["data"].clone();

    let increments = data["increments"].as_array().unwrap();
    assert!(!increments.is_empty());
    assert_eq!(increments[0]["step"], 1);
    assert!(data["contract"].as_str().unwrap().contains("revertible"));
}

#[test]
fn bug_dedupe_clusters_tickets_that_share_code() {
    let fixture = prepared("work-dedupe");
    let tickets = r#"[
        {"id": "BUG-1", "title": "Feed stops", "body": "FeedViewModel never emits"},
        {"id": "BUG-2", "title": "Feed spinner", "body": "FeedViewModel stays loading"},
        {"id": "BUG-3", "title": "Login typo", "body": "Copy on the login button is wrong"}
    ]"#;

    let data = fixture.json_stdin(&["bug", "dedupe", "--tickets", "-"], tickets)["data"].clone();

    assert_eq!(data["clusters"].as_array().unwrap().len(), 1);
    let ids: Vec<&str> = data["clusters"][0]["tickets"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|item| item["id"].as_str())
        .collect();
    assert_eq!(ids, vec!["BUG-1", "BUG-2"]);
    assert_eq!(data["unmatched"], serde_json::json!(["BUG-3"]));
}

#[test]
fn bug_dedupe_honours_a_stricter_threshold() {
    let fixture = prepared("work-dedupe-threshold");
    // Partial overlap only: identical impact sets score 1.0 and would pass any threshold.
    let tickets = r#"[
        {"id": "BUG-1", "title": "", "body": "FeedViewModel and FeedRepository both stall"},
        {"id": "BUG-2", "title": "", "body": "FeedViewModel stays loading"}
    ]"#;

    let data = fixture.json_stdin(
        &["bug", "dedupe", "--threshold", "0.99", "--tickets", "-"],
        tickets,
    )["data"]
        .clone();

    assert!(data["clusters"].as_array().unwrap().is_empty());
}

#[test]
fn bug_dedupe_rejects_tickets_without_an_id() {
    let fixture = prepared("work-dedupe-invalid");

    let envelope = fixture.json_stdin(
        &["bug", "dedupe", "--tickets", "-"],
        r#"[{"title": "no id"}]"#,
    );

    assert_eq!(envelope["ok"], false);
    assert!(envelope["error"]["message"]
        .as_str()
        .unwrap()
        .contains("'id'"));
}

#[test]
fn bug_fetch_triages_a_stored_report_against_the_index() {
    let fixture = prepared("work-bug-fetch");
    fixture.json_stdin(
        &["fetch", "BUG-9", "--write", "-"],
        "# Feed hangs\n\nFeedViewModel stops emitting after the second page.\n",
    );

    let data = fixture.json(&["bug", "fetch", "BUG-9"])["data"].clone();

    let paths: Vec<&str> = data["impact"]["files"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|item| item["path"].as_str())
        .collect();
    assert!(paths.iter().any(|path| path.contains("FeedViewModel.kt")));
    assert!(data["contract"].as_str().unwrap().contains("root cause"));
}
