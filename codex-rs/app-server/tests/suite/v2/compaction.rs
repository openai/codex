use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::to_response;
use codex_app_server_protocol::ClientInfo;
use codex_app_server_protocol::CompactionEndedNotification;
use codex_app_server_protocol::ItemCompletedNotification;
use codex_app_server_protocol::ItemStartedNotification;
use codex_app_server_protocol::JSONRPCNotification;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::UserInput as V2UserInput;
use codex_core::features::FEATURES;
use codex_core::features::Feature;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use std::path::Path;
use tempfile::TempDir;
use tokio::time::timeout;
use wiremock::MockServer;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
const TEST_ORIGINATOR: &str = "codex_vscode";

#[tokio::test]
async fn compaction_emits_item_lifecycle_and_legacy_completion_notification() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    mount_compaction_flow(&server).await;

    let codex_home = TempDir::new()?;
    create_compaction_config_toml(codex_home.path(), &server.uri())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.initialize_with_client_info(ClientInfo {
            name: TEST_ORIGINATOR.to_string(),
            title: Some("Codex VS Code Extension".to_string()),
            version: "0.1.0".to_string(),
        }),
    )
    .await??;

    let thread_req = mcp
        .send_thread_start_request(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;
    let thread_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(thread_req)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response::<ThreadStartResponse>(thread_resp)?;

    let turn_req = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![V2UserInput::Text {
                text: "trigger compaction".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    let turn_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_req)),
    )
    .await??;
    let TurnStartResponse { turn, .. } = to_response::<TurnStartResponse>(turn_resp)?;
    let turn_id = turn.id;

    let compaction_started =
        timeout(DEFAULT_READ_TIMEOUT, read_compaction_item_started(&mut mcp)).await??;
    let compaction_completed = timeout(
        DEFAULT_READ_TIMEOUT,
        read_compaction_item_completed(&mut mcp),
    )
    .await??;
    let legacy_completed = timeout(
        DEFAULT_READ_TIMEOUT,
        read_legacy_compacted_notification(&mut mcp),
    )
    .await??;

    let compaction_id = format!("{turn_id}-compaction");
    let expected_started = ItemStartedNotification {
        thread_id: thread.id.clone(),
        turn_id: turn_id.clone(),
        item: ThreadItem::Compaction {
            id: compaction_id.clone(),
        },
    };
    let expected_completed = ItemCompletedNotification {
        thread_id: thread.id.clone(),
        turn_id: turn_id.clone(),
        item: ThreadItem::Compaction { id: compaction_id },
    };
    let expected_legacy = CompactionEndedNotification {
        thread_id: thread.id.clone(),
        turn_id,
    };

    assert_eq!(compaction_started, expected_started);
    assert_eq!(compaction_completed, expected_completed);
    assert_eq!(legacy_completed, expected_legacy);

    Ok(())
}

async fn read_compaction_item_started(mcp: &mut McpProcess) -> Result<ItemStartedNotification> {
    loop {
        let notification = mcp
            .read_stream_until_notification_message("item/started")
            .await?;
        let params = notification.params.expect("item/started params");
        let started: ItemStartedNotification = serde_json::from_value(params)?;
        if matches!(started.item, ThreadItem::Compaction { .. }) {
            return Ok(started);
        }
    }
}

async fn read_compaction_item_completed(mcp: &mut McpProcess) -> Result<ItemCompletedNotification> {
    loop {
        let notification = mcp
            .read_stream_until_notification_message("item/completed")
            .await?;
        let params = notification.params.expect("item/completed params");
        let completed: ItemCompletedNotification = serde_json::from_value(params)?;
        if matches!(completed.item, ThreadItem::Compaction { .. }) {
            return Ok(completed);
        }
    }
}

async fn read_legacy_compacted_notification(
    mcp: &mut McpProcess,
) -> Result<CompactionEndedNotification> {
    let notification: JSONRPCNotification = mcp
        .read_stream_until_notification_message("thread/compacted")
        .await?;
    let params = notification.params.expect("thread/compacted params");
    let legacy: CompactionEndedNotification = serde_json::from_value(params)?;
    Ok(legacy)
}

async fn mount_compaction_flow(server: &MockServer) {
    responses::mount_sse_once(
        server,
        responses::sse(vec![
            responses::ev_response_created("resp-1"),
            responses::ev_shell_command_call("call-1", "echo hi"),
            responses::ev_completed_with_tokens("resp-1", 1_000_000),
        ]),
    )
    .await;

    responses::mount_sse_once(
        server,
        responses::sse(vec![
            responses::ev_response_created("resp-2"),
            responses::ev_assistant_message("msg-2", "post-compact"),
            responses::ev_completed("resp-2"),
        ]),
    )
    .await;

    let compacted_history = serde_json::json!([
        {
            "type": "message",
            "role": "user",
            "content": [
                {
                    "type": "input_text",
                    "text": "COMPACTED_SUMMARY"
                }
            ]
        },
        {
            "type": "compaction",
            "encrypted_content": "ENCRYPTED_COMPACTION_SUMMARY"
        }
    ]);
    responses::mount_compact_json_once(server, serde_json::json!({ "output": compacted_history }))
        .await;
}

fn create_compaction_config_toml(codex_home: &Path, server_uri: &str) -> std::io::Result<()> {
    let feature_flags = BTreeMap::from([(Feature::RemoteCompaction, true)]);
    let mut features = BTreeMap::from([(Feature::RemoteModels, false)]);
    for (feature, enabled) in feature_flags {
        features.insert(feature, enabled);
    }

    let feature_entries = features
        .into_iter()
        .map(|(feature, enabled)| {
            let key = FEATURES
                .iter()
                .find(|spec| spec.id == feature)
                .map(|spec| spec.key)
                .unwrap_or_else(|| panic!("missing feature key for {feature:?}"));
            format!("{key} = {enabled}")
        })
        .collect::<Vec<_>>()
        .join("\n");

    let config_toml = codex_home.join("config.toml");
    std::fs::write(
        config_toml,
        format!(
            r#"
model = "mock-model"
approval_policy = "never"
sandbox_mode = "read-only"
model_provider = "mock_provider"
model_auto_compact_token_limit = 1
compact_prompt = "Summarize the conversation."

[features]
{feature_entries}

[model_providers.mock_provider]
name = "Mock provider for test"
base_url = "{server_uri}/v1"
wire_api = "responses"
request_max_retries = 0
stream_max_retries = 0
"#
        ),
    )
}
