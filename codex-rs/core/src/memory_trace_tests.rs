use super::*;
use pretty_assertions::assert_eq;
use tempfile::tempdir;

#[test]
fn normalize_trace_items_handles_payload_wrapper_and_message_role_filtering() {
    let items = vec![
        serde_json::json!({
            "type": "response_item",
            "payload": {"type": "message", "role": "assistant", "content": []}
        }),
        serde_json::json!({
            "type": "response_item",
            "payload": [
                {"type": "message", "role": "user", "content": []},
                {"type": "message", "role": "tool", "content": []},
                {"type": "function_call", "name": "shell", "arguments": "{}", "call_id": "c1"}
            ]
        }),
        serde_json::json!({
            "type": "not_response_item",
            "payload": {"type": "message", "role": "assistant", "content": []}
        }),
        serde_json::json!({
            "type": "message",
            "role": "developer",
            "content": []
        }),
    ];

    let normalized = normalize_trace_items(items, Path::new("trace.json")).expect("normalize");
    let expected = vec![
        serde_json::json!({"type": "message", "role": "assistant", "content": []}),
        serde_json::json!({"type": "message", "role": "user", "content": []}),
        serde_json::json!({"type": "function_call", "name": "shell", "arguments": "{}", "call_id": "c1"}),
        serde_json::json!({"type": "message", "role": "developer", "content": []}),
    ];
    assert_eq!(normalized, expected);
}

#[test]
fn load_trace_items_supports_jsonl_arrays_and_objects() {
    let text = r#"
{"type":"response_item","payload":{"type":"message","role":"assistant","content":[]}}
[{"type":"message","role":"user","content":[]},{"type":"message","role":"tool","content":[]}]
"#;
    let loaded = load_trace_items(Path::new("trace.jsonl"), text).expect("load");
    let expected = vec![
        serde_json::json!({"type":"message","role":"assistant","content":[]}),
        serde_json::json!({"type":"message","role":"user","content":[]}),
    ];
    assert_eq!(loaded, expected);
}

#[tokio::test]
async fn load_trace_text_decodes_utf8_sig() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("trace.json");
    tokio::fs::write(
        &path,
        [
            0xEF, 0xBB, 0xBF, b'[', b'{', b'"', b't', b'y', b'p', b'e', b'"', b':', b'"', b'm',
            b'e', b's', b's', b'a', b'g', b'e', b'"', b',', b'"', b'r', b'o', b'l', b'e', b'"',
            b':', b'"', b'u', b's', b'e', b'r', b'"', b',', b'"', b'c', b'o', b'n', b't', b'e',
            b'n', b't', b'"', b':', b'[', b']', b'}', b']',
        ],
    )
    .await
    .expect("write");

    let text = load_trace_text(&path).await.expect("decode");
    assert!(text.starts_with('['));
}
