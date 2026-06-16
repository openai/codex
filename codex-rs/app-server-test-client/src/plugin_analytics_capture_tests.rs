use super::PluginEventIdentity;
use super::read_events_for_remote_plugin;
use super::validate_mutation_events;
use serde_json::Value;
use serde_json::json;
use std::fs;
use std::path::PathBuf;
use std::process;
use std::time::SystemTime;

const REMOTE_PLUGIN_ID: &str = "plugins~Plugin_test";

#[test]
fn reads_and_validates_remote_plugin_mutation_events() {
    let path = unique_capture_path("valid");
    let installed = mutation_event("codex_plugin_installed");
    let unrelated = json!({
        "event_type": "codex_plugin_installed",
        "event_params": {
            "plugin_id": "plugins~Plugin_other"
        }
    });
    let contents = [
        json!({"events": [unrelated]}),
        json!({"events": [installed]}),
    ]
    .into_iter()
    .map(|payload| serde_json::to_string(&payload).expect("serialize capture payload"))
    .collect::<Vec<_>>()
    .join("\n");
    fs::write(&path, contents).expect("write capture file");

    let events = read_events_for_remote_plugin(&path, REMOTE_PLUGIN_ID)
        .expect("read matching plugin events");
    let validated =
        validate_mutation_events(events, expected_identity()).expect("validate mutation events");

    assert_eq!(validated, vec![installed]);
    fs::remove_file(path).expect("remove capture file");
}

#[test]
fn rejects_duplicate_mutation_events() {
    let installed = mutation_event("codex_plugin_installed");
    let error = validate_mutation_events(vec![installed.clone(), installed], expected_identity())
        .expect_err("duplicate install events should fail validation");

    assert!(error.to_string().contains("found 2"));
}

#[test]
fn rejects_missing_capability_metadata() {
    let mut installed = mutation_event("codex_plugin_installed");
    installed["event_params"]["has_skills"] = Value::Null;
    let error = validate_mutation_events(vec![installed], expected_identity())
        .expect_err("missing capability metadata should fail validation");

    assert!(error.to_string().contains("has_skills"));
}

fn mutation_event(event_type: &str) -> Value {
    json!({
        "event_type": event_type,
        "event_params": {
            "plugin_id": REMOTE_PLUGIN_ID,
            "plugin_name": "sample",
            "marketplace_name": "openai-curated-remote",
            "has_skills": true,
            "mcp_server_count": 0,
            "connector_ids": [],
            "product_client_id": "test-client"
        }
    })
}

fn expected_identity() -> PluginEventIdentity<'static> {
    PluginEventIdentity {
        plugin_id: REMOTE_PLUGIN_ID,
        plugin_name: "sample",
        marketplace_name: "openai-curated-remote",
    }
}

fn unique_capture_path(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("system clock should be after Unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "codex-plugin-analytics-capture-{name}-{}-{nonce}.jsonl",
        process::id()
    ))
}
