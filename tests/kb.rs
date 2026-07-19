mod common;

use common::{commit_all, write, Fixture};

#[test]
fn remote_source_clones_and_pins_a_revision() {
    let fixture = Fixture::new("kb-clone");

    let envelope = fixture.json(&["kb", "sync"]);
    let source = &envelope["data"]["source"];

    assert_eq!(envelope["ok"], true, "{envelope}");
    assert_eq!(source["kind"], "remote");
    assert_eq!(source["revision"].as_str().unwrap().len(), 40);
    assert!(!source["syncedAt"].as_str().unwrap().is_empty());
}

#[test]
fn sync_pulls_new_knowledge_documents() {
    let fixture = Fixture::new("kb-sync");
    fixture.run(&["kb", "sync"]);

    write(
        &fixture.kb,
        "recipes/caching.md",
        "---\nid: caching\ntitle: Caching\nstacks: [android]\n---\nUse a repository cache.\n",
    );
    commit_all(&fixture.kb, "add caching recipe");

    let envelope = fixture.json(&["kb", "sync"]);

    assert_eq!(envelope["data"]["counts"]["recipes"], 2);
}

#[test]
fn status_without_a_prior_sync_explains_what_to_run() {
    let fixture = Fixture::new("kb-unsynced");

    let envelope = fixture.json(&["kb", "status"]);

    assert_eq!(envelope["ok"], false);
    assert!(envelope["error"]["message"]
        .as_str()
        .unwrap()
        .contains("never been synced"));
}

#[test]
fn layers_load_with_ids_and_counts() {
    let fixture = Fixture::new("kb-layers");

    let counts = fixture.json(&["kb", "sync"])["data"]["counts"].clone();

    assert_eq!(counts["conventions"], 2);
    assert_eq!(counts["recipes"], 1);
    assert_eq!(counts["slices"], 1);
}

#[test]
fn slice_resolution_selects_only_the_matching_stack() {
    let fixture = Fixture::new("kb-slice");

    let slice = fixture.json(&["kb", "sync"])["data"]["resolvedSlice"].clone();

    assert_eq!(
        slice["conventions"],
        serde_json::json!(["android-architecture"])
    );
    assert_eq!(slice["facts"], serde_json::json!(["viewmodels"]));
}

#[test]
fn search_ranks_id_matches_first() {
    let fixture = Fixture::new("kb-search");
    fixture.run(&["kb", "sync"]);

    let envelope = fixture.json(&["search", "pagination"]);

    assert_eq!(envelope["data"]["hits"][0]["id"], "pagination");
}

#[test]
fn search_can_restrict_to_a_layer() {
    let fixture = Fixture::new("kb-search-layer");
    fixture.run(&["kb", "sync"]);

    let envelope = fixture.json(&["search", "mvvm", "--layer", "recipe"]);

    assert!(envelope["data"]["hits"].as_array().unwrap().is_empty());
}

#[test]
fn search_rejects_an_unknown_layer() {
    let fixture = Fixture::new("kb-bad-layer");
    fixture.run(&["kb", "sync"]);

    let envelope = fixture.json(&["search", "mvvm", "--layer", "nonsense"]);

    assert_eq!(envelope["ok"], false);
    assert!(envelope["error"]["message"]
        .as_str()
        .unwrap()
        .contains("--layer"));
}

#[test]
fn convention_falls_back_to_search_and_stays_on_stack() {
    let fixture = Fixture::new("kb-convention-fallback");
    fixture.run(&["kb", "sync"]);

    let envelope = fixture.json(&["convention", "mvvm"]);

    let ids: Vec<&str> = envelope["data"]["documents"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|item| item["id"].as_str())
        .collect();
    assert!(ids.contains(&"android-architecture"));
    assert!(
        !ids.contains(&"ios-architecture"),
        "other stacks must not leak in"
    );
}

#[test]
fn convention_with_no_match_points_at_search() {
    let fixture = Fixture::new("kb-convention-miss");
    fixture.run(&["kb", "sync"]);

    let envelope = fixture.json(&["convention", "nonexistent-topic"]);

    assert_eq!(envelope["ok"], false);
    assert!(envelope["error"]["message"]
        .as_str()
        .unwrap()
        .contains("repo-task search"));
}

#[test]
fn recipe_returns_the_document_body() {
    let fixture = Fixture::new("kb-recipe");
    fixture.run(&["kb", "sync"]);

    let envelope = fixture.json(&["recipe", "pagination"]);

    assert!(envelope["data"]["documents"][0]["body"]
        .as_str()
        .unwrap()
        .contains("Paging 3"));
}

#[test]
fn missing_manifest_is_a_clear_error() {
    let fixture = Fixture::new("kb-no-manifest");
    fixture.run(&["kb", "sync"]);
    std::fs::remove_file(fixture.kb.join("kb.yaml")).unwrap();
    commit_all(&fixture.kb, "remove manifest");

    let envelope = fixture.json(&["kb", "sync"]);

    assert_eq!(envelope["ok"], false);
    assert!(envelope["error"]["message"]
        .as_str()
        .unwrap()
        .contains("kb.yaml not found"));
}

#[test]
fn a_document_may_not_contradict_its_directory() {
    let fixture = Fixture::new("kb-layer-mismatch");
    write(
        &fixture.kb,
        "conventions/mislabelled.md",
        "---\nid: mislabelled\nlayer: recipe\n---\nBody.\n",
    );
    commit_all(&fixture.kb, "add mislabelled document");

    let envelope = fixture.json(&["kb", "sync"]);

    assert_eq!(envelope["ok"], false);
    assert!(envelope["error"]["message"]
        .as_str()
        .unwrap()
        .contains("declares layer 'recipe'"));
}

#[test]
fn duplicate_document_ids_are_rejected() {
    let fixture = Fixture::new("kb-duplicate");
    write(
        &fixture.kb,
        "conventions/copy.md",
        "---\nid: android-architecture\ntitle: Copy\n---\nDuplicate.\n",
    );
    commit_all(&fixture.kb, "add duplicate id");

    let envelope = fixture.json(&["kb", "sync"]);

    assert_eq!(envelope["ok"], false);
    assert!(envelope["error"]["message"]
        .as_str()
        .unwrap()
        .contains("Duplicate"));
}

#[test]
fn doctor_reports_a_healthy_project() {
    let fixture = Fixture::new("kb-doctor");
    fixture.run(&["kb", "sync"]);

    let envelope = fixture.json(&["doctor"]);

    assert_eq!(envelope["data"]["ok"], true, "{envelope}");
}

#[test]
fn doctor_fails_when_the_knowledge_base_is_unreachable() {
    let fixture = Fixture::new("kb-doctor-fail");

    let output = fixture.run(&["--json", "doctor"]);

    assert!(
        !output.status.success(),
        "doctor must exit non-zero when a check fails"
    );
}
