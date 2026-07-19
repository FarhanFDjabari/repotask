mod common;

use common::{write, Fixture, KOTLIN_SOURCE};

/// Point a fixture project at a config-declared system.
fn with_connector(fixture: &Fixture, yaml: &str) {
    let path = fixture.project.join(".repo-task/config.yaml");
    let existing = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, format!("{existing}\nconnectors:\n{yaml}")).unwrap();
}

#[test]
fn connect_lists_declared_verbs() {
    let fixture = Fixture::new("connect-list");
    with_connector(
        &fixture,
        "  acme:\n    mode: auto\n    base_url: http://127.0.0.1:1\n    verbs:\n      \
         task:\n        path: /task/{id}\n        mcp_tool: acme_get_task\n",
    );

    let envelope = fixture.json(&["connect", "acme"]);

    assert_eq!(envelope["ok"], true, "{envelope}");
    assert_eq!(envelope["data"]["verbs"][0]["verb"], "task");
}

#[test]
fn an_unknown_verb_names_the_declared_ones() {
    let fixture = Fixture::new("connect-unknown-verb");
    with_connector(
        &fixture,
        "  acme:\n    mode: auto\n    base_url: http://127.0.0.1:1\n    verbs:\n      \
         task:\n        path: /task/{id}\n",
    );

    let envelope = fixture.json(&["connect", "acme", "nope"]);

    assert_eq!(envelope["ok"], false);
    assert!(envelope["error"]["message"]
        .as_str()
        .unwrap()
        .contains("task"));
}

#[test]
fn an_unreachable_rest_call_falls_back_to_the_declared_mcp_tool() {
    let fixture = Fixture::new("connect-fallback");
    with_connector(
        &fixture,
        "  acme:\n    mode: auto\n    base_url: http://127.0.0.1:1\n    verbs:\n      \
         task:\n        path: /task/{id}\n        mcp_tool: acme_get_task\n",
    );

    let envelope = fixture.json(&["connect", "acme", "task", "--arg", "id=T-1"]);

    assert_eq!(envelope["ok"], true, "{envelope}");
    assert_eq!(envelope["data"]["mode"], "mcp");
    assert_eq!(envelope["data"]["request"]["tool"], "acme_get_task");
    assert_eq!(envelope["data"]["request"]["arguments"]["id"], "T-1");
    assert!(
        !envelope["warnings"].as_array().unwrap().is_empty(),
        "the costlier path must be announced"
    );
}

#[test]
fn rest_mode_surfaces_the_failure_rather_than_falling_back() {
    let fixture = Fixture::new("connect-rest-only");
    with_connector(
        &fixture,
        "  acme:\n    mode: rest\n    base_url: http://127.0.0.1:1\n    verbs:\n      \
         task:\n        path: /task/{id}\n        mcp_tool: acme_get_task\n",
    );

    let envelope = fixture.json(&["connect", "acme", "task", "--arg", "id=T-1"]);

    assert_eq!(
        envelope["ok"], false,
        "an explicit rest mode must not silently fall back"
    );
}

#[test]
fn a_missing_argument_is_named() {
    let fixture = Fixture::new("connect-missing-arg");
    with_connector(
        &fixture,
        "  acme:\n    mode: auto\n    base_url: http://127.0.0.1:1\n    verbs:\n      \
         task:\n        path: /task/{id}\n        mcp_tool: acme_get_task\n",
    );

    let envelope = fixture.json(&["connect", "acme", "task"]);

    assert_eq!(envelope["ok"], false);
    assert!(envelope["error"]["message"]
        .as_str()
        .unwrap()
        .contains("--arg id"));
}

#[test]
fn a_disabled_connector_is_refused() {
    let fixture = Fixture::new("connect-off");
    with_connector(&fixture, "  acme:\n    mode: off\n");

    let envelope = fixture.json(&["connect", "acme"]);

    assert_eq!(envelope["ok"], false);
    assert!(envelope["error"]["message"]
        .as_str()
        .unwrap()
        .contains("disabled"));
}

#[test]
fn design_falls_back_to_figma_mcp_without_a_token() {
    let fixture = Fixture::new("design-fallback");
    with_connector(
        &fixture,
        "  figma:\n    mode: auto\n    project: \"filekey\"\n",
    );

    let envelope = fixture.json(&["design", "file"]);

    assert_eq!(envelope["ok"], true, "{envelope}");
    assert_eq!(envelope["data"]["mode"], "mcp");
    assert_eq!(envelope["data"]["request"]["server"], "figma");
    assert_eq!(envelope["data"]["request"]["tool"], "get_design_context");
}

#[test]
fn design_image_asks_for_a_screenshot_because_the_cli_cannot_look() {
    let fixture = Fixture::new("design-image");
    with_connector(
        &fixture,
        "  figma:\n    mode: auto\n    project: \"filekey\"\n",
    );

    let envelope = fixture.json(&["design", "image", "1:2"]);

    assert_eq!(envelope["data"]["request"]["tool"], "get_screenshot");
}

#[test]
fn design_map_lines_component_names_up_against_indexed_code() {
    let fixture = Fixture::new("design-map");
    write(&fixture.project, "app/feed/FeedViewModel.kt", KOTLIN_SOURCE);
    write(
        &fixture.project,
        "ui/Components.kt",
        "package com.acme.ui\n\nclass PrimaryButton\nclass FeedCard\n",
    );
    fixture.run(&["kb", "sync"]);
    fixture.run(&["index"]);

    let data = fixture.json(&[
        "design",
        "map",
        "Button/Primary",
        "FeedCard",
        "Checkout/Sheet",
    ])["data"]
        .clone();

    let matched: Vec<&str> = data["existingCode"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|item| item["name"].as_str())
        .collect();
    assert!(matched.contains(&"PrimaryButton"), "{matched:?}");
    assert!(matched.contains(&"FeedCard"), "{matched:?}");
    assert_eq!(
        data["unmatched"],
        serde_json::json!(["Checkout/Sheet"]),
        "a design component with no code must be reported, not silently dropped"
    );
    assert!(data["contract"]
        .as_str()
        .unwrap()
        .contains("near-duplicates"));
}

#[test]
fn design_map_requires_an_index() {
    let fixture = Fixture::new("design-map-no-index");
    fixture.run(&["kb", "sync"]);

    let envelope = fixture.json(&["design", "map", "Button/Primary"]);

    assert_eq!(envelope["ok"], false);
    assert!(envelope["error"]["message"]
        .as_str()
        .unwrap()
        .contains("repo-task index"));
}
