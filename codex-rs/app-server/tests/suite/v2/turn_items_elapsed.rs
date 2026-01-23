use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::create_apply_patch_sse_response;
use app_test_support::create_final_assistant_message_sse_response;
use app_test_support::create_mock_responses_server_sequence;
use app_test_support::create_shell_command_sse_response;
use app_test_support::to_response;
use codex_app_server_protocol::ItemCompletedNotification;
use codex_app_server_protocol::JSONRPCNotification;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::UserInput as V2UserInput;
use core_test_support::responses;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::collections::HashSet;
use std::path::Path;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[tokio::test]
async fn turn_items_emit_elapsed_ms_on_completion() -> Result<()> {
    let tmp = TempDir::new()?;
    let codex_home = tmp.path().join("codex_home");
    std::fs::create_dir(&codex_home)?;
    let workspace = tmp.path().join("workspace");
    std::fs::create_dir(&workspace)?;

    let image_path = workspace.join("image.png");
    std::fs::write(&image_path, b"not an image")?;
    let image_path_str = image_path.to_string_lossy().into_owned();

    let mcp_tool_name = "list_mcp_resources";

    let patch = r#"*** Begin Patch
*** Add File: README.md
+new line
*** End Patch
"#;

    let responses = vec![
        responses::sse(vec![
            responses::ev_response_created("resp-1"),
            responses::ev_reasoning_item("reason-1", &["summary"], &["details"]),
            responses::ev_web_search_call_done("search-1", "completed", "query"),
            responses::ev_assistant_message("msg-1", "hello"),
            responses::ev_completed("resp-1"),
        ]),
        create_shell_command_sse_response(
            vec!["echo".to_string(), "hi".to_string()],
            None,
            None,
            "shell-1",
        )?,
        create_final_assistant_message_sse_response("shell done")?,
        create_apply_patch_sse_response(patch, "patch-1")?,
        create_final_assistant_message_sse_response("patch done")?,
        create_function_call_sse_response(
            "view-1",
            "view_image",
            json!({ "path": image_path_str }),
        )?,
        create_final_assistant_message_sse_response("view done")?,
        create_function_call_sse_response(
            "collab-1",
            "close_agent",
            json!({ "id": "00000000-0000-0000-0000-000000000001" }),
        )?,
        create_final_assistant_message_sse_response("collab done")?,
        create_function_call_sse_response("mcp-1", mcp_tool_name, json!({}))?,
        create_final_assistant_message_sse_response("mcp done")?,
    ];

    let server = create_mock_responses_server_sequence(responses).await;
    create_config_toml(&codex_home, &server.uri())?;

    let mut mcp = McpProcess::new(&codex_home).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let thread_id = start_thread(&mut mcp, &workspace).await?;

    start_turn(&mut mcp, &thread_id, &workspace, "first turn").await?;
    let mut pending = HashSet::from(["user", "agent", "reasoning", "web_search"]);
    for _ in 0..12 {
        let item = timeout(DEFAULT_READ_TIMEOUT, next_item_completed(&mut mcp)).await??;
        if let Some(kind) = item_kind(&item) {
            if pending.remove(kind) {
                assert_elapsed_ms(&item);
            }
        }
        if pending.is_empty() {
            break;
        }
    }
    assert_eq!(pending.is_empty(), true);
    wait_for_turn_completed(&mut mcp).await?;

    start_turn(&mut mcp, &thread_id, &workspace, "shell turn").await?;
    let command_exec_item = timeout(
        DEFAULT_READ_TIMEOUT,
        wait_for_completed_item(&mut mcp, |item| {
            matches!(item, ThreadItem::CommandExecution { .. })
        }),
    )
    .await??;
    assert_elapsed_ms(&command_exec_item);
    wait_for_turn_completed(&mut mcp).await?;

    start_turn(&mut mcp, &thread_id, &workspace, "patch turn").await?;
    let file_change_item = timeout(
        DEFAULT_READ_TIMEOUT,
        wait_for_completed_item(&mut mcp, |item| {
            matches!(item, ThreadItem::FileChange { .. })
        }),
    )
    .await??;
    assert_elapsed_ms(&file_change_item);
    wait_for_turn_completed(&mut mcp).await?;

    start_turn(&mut mcp, &thread_id, &workspace, "image turn").await?;
    let image_item = timeout(
        DEFAULT_READ_TIMEOUT,
        wait_for_completed_item(&mut mcp, |item| {
            matches!(item, ThreadItem::ImageView { .. })
        }),
    )
    .await??;
    assert_elapsed_ms(&image_item);
    wait_for_turn_completed(&mut mcp).await?;

    start_turn(&mut mcp, &thread_id, &workspace, "collab turn").await?;
    let collab_item = timeout(
        DEFAULT_READ_TIMEOUT,
        wait_for_completed_item(&mut mcp, |item| {
            matches!(item, ThreadItem::CollabAgentToolCall { .. })
        }),
    )
    .await??;
    assert_elapsed_ms(&collab_item);
    wait_for_turn_completed(&mut mcp).await?;

    start_turn(&mut mcp, &thread_id, &workspace, "mcp turn").await?;
    let mcp_item = timeout(
        DEFAULT_READ_TIMEOUT,
        wait_for_completed_item(&mut mcp, |item| {
            matches!(item, ThreadItem::McpToolCall { .. })
        }),
    )
    .await??;
    assert_elapsed_ms(&mcp_item);
    wait_for_turn_completed(&mut mcp).await?;

    Ok(())
}

async fn start_thread(mcp: &mut McpProcess, cwd: &Path) -> Result<String> {
    let start_req = mcp
        .send_thread_start_request(ThreadStartParams {
            model: Some("mock-model".to_string()),
            cwd: Some(cwd.to_string_lossy().into_owned()),
            ..Default::default()
        })
        .await?;
    let start_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(start_req)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response::<ThreadStartResponse>(start_resp)?;
    Ok(thread.id)
}

async fn start_turn(
    mcp: &mut McpProcess,
    thread_id: &str,
    cwd: &Path,
    text: &str,
) -> Result<String> {
    let turn_req = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread_id.to_string(),
            input: vec![V2UserInput::Text {
                text: text.to_string(),
                text_elements: Vec::new(),
            }],
            cwd: Some(cwd.to_path_buf()),
            ..Default::default()
        })
        .await?;
    let turn_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_req)),
    )
    .await??;
    let TurnStartResponse { turn } = to_response::<TurnStartResponse>(turn_resp)?;
    Ok(turn.id)
}

async fn next_item_completed(mcp: &mut McpProcess) -> Result<ThreadItem> {
    let notification: JSONRPCNotification = mcp
        .read_stream_until_notification_message("item/completed")
        .await?;
    let completed: ItemCompletedNotification =
        serde_json::from_value(notification.params.expect("item/completed params"))?;
    Ok(completed.item)
}

async fn wait_for_completed_item<F>(mcp: &mut McpProcess, predicate: F) -> Result<ThreadItem>
where
    F: Fn(&ThreadItem) -> bool,
{
    loop {
        let item = next_item_completed(mcp).await?;
        if predicate(&item) {
            return Ok(item);
        }
    }
}

async fn wait_for_turn_completed(mcp: &mut McpProcess) -> Result<()> {
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;
    Ok(())
}

fn item_kind(item: &ThreadItem) -> Option<&'static str> {
    match item {
        ThreadItem::UserMessage { .. } => Some("user"),
        ThreadItem::AgentMessage { .. } => Some("agent"),
        ThreadItem::Reasoning { .. } => Some("reasoning"),
        ThreadItem::WebSearch { .. } => Some("web_search"),
        _ => None,
    }
}

fn assert_elapsed_ms(item: &ThreadItem) {
    let elapsed_ms = match item {
        ThreadItem::UserMessage { elapsed_ms, .. } => *elapsed_ms,
        ThreadItem::AgentMessage { elapsed_ms, .. } => *elapsed_ms,
        ThreadItem::Reasoning { elapsed_ms, .. } => *elapsed_ms,
        ThreadItem::CommandExecution { elapsed_ms, .. } => *elapsed_ms,
        ThreadItem::FileChange { elapsed_ms, .. } => *elapsed_ms,
        ThreadItem::McpToolCall { elapsed_ms, .. } => *elapsed_ms,
        ThreadItem::CollabAgentToolCall { elapsed_ms, .. } => *elapsed_ms,
        ThreadItem::WebSearch { elapsed_ms, .. } => *elapsed_ms,
        ThreadItem::ImageView { elapsed_ms, .. } => *elapsed_ms,
        ThreadItem::EnteredReviewMode { elapsed_ms, .. } => *elapsed_ms,
        ThreadItem::ExitedReviewMode { elapsed_ms, .. } => *elapsed_ms,
    };
    assert_eq!(elapsed_ms.is_some(), true);
    if let Some(value) = elapsed_ms {
        assert_eq!(value >= 0, true);
    }
}

fn create_function_call_sse_response(
    call_id: &str,
    name: &str,
    args: serde_json::Value,
) -> Result<String> {
    let arguments = serde_json::to_string(&args)?;
    Ok(responses::sse(vec![
        responses::ev_response_created("resp-1"),
        responses::ev_function_call(call_id, name, &arguments),
        responses::ev_completed("resp-1"),
    ]))
}

fn create_config_toml(codex_home: &Path, server_uri: &str) -> std::io::Result<()> {
    let config_toml = codex_home.join("config.toml");
    std::fs::write(
        config_toml,
        format!(
            r#"
model = "mock-model"
approval_policy = "never"
sandbox_mode = "workspace-write"

model_provider = "mock_provider"

[model_providers.mock_provider]
name = "Mock provider for test"
base_url = "{server_uri}/v1"
wire_api = "responses"
request_max_retries = 0
stream_max_retries = 0

[features]
collab = true
"#
        ),
    )
}
