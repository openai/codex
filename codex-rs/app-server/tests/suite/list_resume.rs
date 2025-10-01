use std::fs;
use std::path::Path;

use app_test_support::McpProcess;
use app_test_support::to_response;
use codex_app_server_protocol::JSONRPCNotification;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::ListConversationsParams;
use codex_app_server_protocol::ListConversationsResponse;
use codex_app_server_protocol::NewConversationParams; // reused for overrides shape
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ResumeConversationParams;
use codex_app_server_protocol::ResumeConversationResponse;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::SessionConfiguredNotification;
use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;
use tokio::time::timeout;
use uuid::Uuid;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_list_and_resume_conversations() {
    // Prepare a temporary CODEX_HOME with a few fake rollout files.
    let codex_home = TempDir::new().expect("create temp dir");
    create_fake_rollout(
        codex_home.path(),
        "2025-01-02T12-00-00",
        "2025-01-02T12:00:00Z",
        "Hello A",
    );
    create_fake_rollout(
        codex_home.path(),
        "2025-01-01T13-00-00",
        "2025-01-01T13:00:00Z",
        "Hello B",
    );
    create_fake_rollout(
        codex_home.path(),
        "2025-01-01T12-00-00",
        "2025-01-01T12:00:00Z",
        "Hello C",
    );

    let mut mcp = McpProcess::new(codex_home.path())
        .await
        .expect("spawn mcp process");
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize())
        .await
        .expect("init timeout")
        .expect("init failed");

    // Request first page with size 2
    let req_id = mcp
        .send_list_conversations_request(ListConversationsParams {
            page_size: Some(2),
            cursor: None,
        })
        .await
        .expect("send listConversations");
    let resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(req_id)),
    )
    .await
    .expect("listConversations timeout")
    .expect("listConversations resp");
    let ListConversationsResponse { items, next_cursor } =
        to_response::<ListConversationsResponse>(resp).expect("deserialize response");

    assert_eq!(items.len(), 2);
    // Newest first; preview text should match
    assert_eq!(items[0].preview, "Hello A");
    assert_eq!(items[1].preview, "Hello B");
    assert!(items[0].path.is_absolute());
    assert!(next_cursor.is_some());

    // Request the next page using the cursor
    let req_id2 = mcp
        .send_list_conversations_request(ListConversationsParams {
            page_size: Some(2),
            cursor: next_cursor,
        })
        .await
        .expect("send listConversations page 2");
    let resp2: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(req_id2)),
    )
    .await
    .expect("listConversations page 2 timeout")
    .expect("listConversations page 2 resp");
    let ListConversationsResponse {
        items: items2,
        next_cursor: next2,
        ..
    } = to_response::<ListConversationsResponse>(resp2).expect("deserialize response");
    assert_eq!(items2.len(), 1);
    assert_eq!(items2[0].preview, "Hello C");
    assert!(next2.is_some());

    // Now resume one of the sessions and expect a SessionConfigured notification and response.
    let resume_req_id = mcp
        .send_resume_conversation_request(ResumeConversationParams {
            path: items[0].path.clone(),
            overrides: Some(NewConversationParams {
                model: Some("o3".to_string()),
                ..Default::default()
            }),
        })
        .await
        .expect("send resumeConversation");

    // Expect a codex/event notification with msg.type == sessionConfigured
    let notification: JSONRPCNotification = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("sessionConfigured"),
    )
    .await
    .expect("sessionConfigured notification timeout")
    .expect("sessionConfigured notification");
    let session_configured: ServerNotification = notification
        .try_into()
        .expect("deserialize sessionConfigured notification");
    // Basic shape assertion: ensure event type is sessionConfigured
    let ServerNotification::SessionConfigured(SessionConfiguredNotification {
        model,
        rollout_path,
        ..
    }) = session_configured
    else {
        unreachable!("expected sessionConfigured notification");
    };
    assert_eq!(model, "o3");
    assert_eq!(items[0].path.clone(), rollout_path);

    // Then the response for resumeConversation
    let resume_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(resume_req_id)),
    )
    .await
    .expect("resumeConversation timeout")
    .expect("resumeConversation resp");
    let ResumeConversationResponse {
        conversation_id, ..
    } = to_response::<ResumeConversationResponse>(resume_resp)
        .expect("deserialize resumeConversation response");
    // conversation id should be a valid UUID
    assert!(!conversation_id.to_string().is_empty());
}

fn create_fake_rollout(codex_home: &Path, filename_ts: &str, meta_rfc3339: &str, preview: &str) {
    let uuid = Uuid::new_v4();
    // sessions/YYYY/MM/DD/ derived from filename_ts (YYYY-MM-DDThh-mm-ss)
    let year = &filename_ts[0..4];
    let month = &filename_ts[5..7];
    let day = &filename_ts[8..10];
    let dir = codex_home.join("sessions").join(year).join(month).join(day);
    fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("create sessions dir: {e}"));

    let file_path = dir.join(format!("rollout-{filename_ts}-{uuid}.jsonl"));
    let mut lines = Vec::new();
    // Meta line with timestamp (flattened meta in payload for new schema)
    lines.push(
        json!({
            "timestamp": meta_rfc3339,
            "type": "session_meta",
            "payload": {
                "id": uuid,
                "timestamp": meta_rfc3339,
                "cwd": "/",
                "originator": "codex",
                "cli_version": "0.0.0",
                "instructions": null
            }
        })
        .to_string(),
    );
    // Minimal user message entry as a persisted response item (with envelope timestamp)
    lines.push(
        json!({
            "timestamp": meta_rfc3339,
            "type":"response_item",
            "payload": {
                "type":"message",
                "role":"user",
                "content":[{"type":"input_text","text": preview}]
            }
        })
        .to_string(),
    );
    // Add a matching user message event line to satisfy filters
    lines.push(
        json!({
            "timestamp": meta_rfc3339,
            "type":"event_msg",
            "payload": {
                "type":"user_message",
                "message": preview,
                "kind": "plain"
            }
        })
        .to_string(),
    );
    fs::write(file_path, lines.join("\n") + "\n")
        .unwrap_or_else(|e| panic!("write rollout file: {e}"));
}
