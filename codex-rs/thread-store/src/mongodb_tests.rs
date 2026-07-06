use std::path::PathBuf;

use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::tempdir;

use super::DEFAULT_MONGODB_URI;
use super::MongoThreadStoreConfig;
use super::reconstruct_rollout_line;
use crate::mongodb_blob::EXTERNAL_OUTPUT_THRESHOLD_BYTES;
use crate::mongodb_blob::externalize_rollout_item;
use crate::mongodb_blob::hydrate_rollout_item;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::RolloutItem;

#[test]
fn missing_uri_env_uses_local_default() {
    let config = MongoThreadStoreConfig {
        codex_home: PathBuf::from("/tmp/codex-mongo-test"),
        database: "codex".to_string(),
        uri_env: "CODEX_MONGODB_URI_TEST_UNSET".to_string(),
    };
    assert_eq!(
        config.resolved_uri().expect("default URI"),
        DEFAULT_MONGODB_URI
    );
}

#[test]
fn reconstructs_line_with_missing_timestamp() {
    let line = json!({
        "type": "event_msg",
        "payload": {
            "type": "turn_aborted",
            "reason": "interrupted"
        }
    })
    .to_string();
    let reconstructed = reconstruct_rollout_line(&line).expect("reconstructed rollout line");
    assert_eq!(reconstructed.timestamp, "1970-01-01T00:00:00.000Z");
}

#[test]
fn reconstructs_legacy_absolute_cwd_as_path_uri() {
    let line = json!({
        "timestamp": "2026-04-29T15:29:36.000Z",
        "type": "event_msg",
        "payload": {
            "type": "exec_command_end",
            "call_id": "call-1",
            "turn_id": "turn-1",
            "command": ["pwd"],
            "cwd": "/home/dev-user/code/codex",
            "parsed_cmd": [],
            "stdout": "",
            "stderr": "",
            "aggregated_output": "{",
            "exit_code": 0,
            "duration": {
                "secs": 0,
                "nanos": 0
            },
            "formatted_output": "",
            "status": "completed"
        }
    })
    .to_string();

    reconstruct_rollout_line(&line).expect("legacy cwd should reconstruct");
}

#[test]
fn reconstructs_first_json_object_when_line_has_trailing_content() {
    let line = format!(
        "{} trailing rollout noise",
        json!({
            "type": "event_msg",
            "payload": {
                "type": "turn_aborted",
                "reason": "interrupted"
            }
        })
    );

    reconstruct_rollout_line(&line).expect("rollout envelope should reconstruct");
}

#[test]
fn rejects_line_missing_core_rollout_fields() {
    let error = match reconstruct_rollout_line(r#"{"timestamp":"2026-01-01T00:00:00.000Z"}"#) {
        Ok(_) => panic!("missing core fields should not reconstruct"),
        Err(error) => error,
    };
    assert_eq!(error, "missing required type or payload fields");
}

#[test]
fn externalizes_and_hydrates_large_text_tool_outputs() {
    let codex_home = tempdir().expect("temporary codex home");
    let output = "x".repeat(EXTERNAL_OUTPUT_THRESHOLD_BYTES + 1);
    let mut item = RolloutItem::ResponseItem(ResponseItem::CustomToolCallOutput {
        id: None,
        call_id: "call-1".to_string(),
        name: None,
        output: FunctionCallOutputPayload::from_text(output.clone()),
        internal_chat_message_metadata_passthrough: None,
    });

    let fields =
        externalize_rollout_item(codex_home.path(), &mut item).expect("externalized output");
    assert_eq!(fields.len(), 1);
    assert!(!fields[0].file_name.contains('/'));
    let RolloutItem::ResponseItem(ResponseItem::CustomToolCallOutput {
        output: externalized_output,
        ..
    }) = &item
    else {
        panic!("expected custom tool output");
    };
    assert_eq!(externalized_output.text_content(), Some(""));

    hydrate_rollout_item(codex_home.path(), &mut item, &fields).expect("hydrated output");
    let RolloutItem::ResponseItem(ResponseItem::CustomToolCallOutput {
        output: hydrated_output,
        ..
    }) = &item
    else {
        panic!("expected custom tool output");
    };
    assert_eq!(hydrated_output.text_content(), Some(output.as_str()));
}
