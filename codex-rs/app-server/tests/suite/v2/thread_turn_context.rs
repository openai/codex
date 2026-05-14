use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::create_final_assistant_message_sse_response;
use app_test_support::create_mock_responses_server_repeating_assistant;
use app_test_support::create_mock_responses_server_sequence_unchecked;
use app_test_support::to_response;
use app_test_support::write_mock_responses_config_toml;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::PermissionProfileSelectionParams;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::SandboxPolicy;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ThreadSource;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::ThreadTurnContextUpdateParams;
use codex_app_server_protocol::ThreadTurnContextUpdateResponse;
use codex_app_server_protocol::ThreadTurnContextUpdatedNotification;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::UserInput as V2UserInput;
use codex_features::Feature;
use codex_protocol::openai_models::ReasoningEffort;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
const INVALID_REQUEST_ERROR_CODE: i64 = -32600;

fn write_config(codex_home: &TempDir, server_uri: &str) -> Result<()> {
    write_mock_responses_config_toml(
        codex_home.path(),
        server_uri,
        &BTreeMap::<Feature, bool>::new(),
        /*auto_compact_limit*/ 1_000_000,
        /*requires_openai_auth*/ None,
        "mock_provider",
        "compact",
    )?;
    Ok(())
}

async fn start_thread(mcp: &mut McpProcess) -> Result<ThreadStartResponse> {
    let request_id = mcp
        .send_thread_start_request(ThreadStartParams {
            model: Some("mock-model".to_string()),
            service_tier: Some(Some("flex".to_string())),
            thread_source: Some(ThreadSource::User),
            ..Default::default()
        })
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    to_response::<ThreadStartResponse>(response)
}

async fn read_turn_context_updated(
    mcp: &mut McpProcess,
) -> Result<ThreadTurnContextUpdatedNotification> {
    let notification = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("thread/turnContext/updated"),
    )
    .await??;
    let notification: ServerNotification = notification.try_into()?;
    let ServerNotification::ThreadTurnContextUpdated(notification) = notification else {
        anyhow::bail!("expected thread/turnContext/updated notification");
    };
    Ok(notification)
}

#[tokio::test]
async fn thread_turn_context_update_applies_partial_patch_and_emits_full_state() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    write_config(&codex_home, &server.uri())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;
    let ThreadStartResponse { thread, .. } = start_thread(&mut mcp).await?;

    let request_id = mcp
        .send_thread_turn_context_update_request(ThreadTurnContextUpdateParams {
            thread_id: thread.id.clone(),
            model: Some("gpt-5.2".to_string()),
            effort: Some(Some(ReasoningEffort::High)),
            ..Default::default()
        })
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let response = to_response::<ThreadTurnContextUpdateResponse>(response)?;

    assert_eq!(response.turn_context.model, "gpt-5.2");
    assert_eq!(response.turn_context.service_tier.as_deref(), Some("flex"));
    assert_eq!(response.turn_context.effort, Some(ReasoningEffort::High));
    assert_eq!(response.turn_context.cwd, thread.cwd);

    let notification = read_turn_context_updated(&mut mcp).await?;
    assert_eq!(notification.thread_id, thread.id);
    assert_eq!(notification.turn_context, response.turn_context);

    mcp.clear_message_buffer();
    let no_op_request = mcp
        .send_thread_turn_context_update_request(ThreadTurnContextUpdateParams {
            thread_id: thread.id,
            model: Some("gpt-5.2".to_string()),
            effort: Some(Some(ReasoningEffort::High)),
            ..Default::default()
        })
        .await?;
    let no_op_response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(no_op_request)),
    )
    .await??;
    let no_op_response = to_response::<ThreadTurnContextUpdateResponse>(no_op_response)?;
    assert_eq!(no_op_response.turn_context, response.turn_context);
    assert!(
        !mcp.pending_notification_methods()
            .iter()
            .any(|method| method == "thread/turnContext/updated")
    );

    Ok(())
}

#[tokio::test]
async fn thread_turn_context_update_clears_service_tier_with_explicit_null() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    write_config(&codex_home, &server.uri())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;
    let ThreadStartResponse { thread, .. } = start_thread(&mut mcp).await?;

    let request_id = mcp
        .send_thread_turn_context_update_request(ThreadTurnContextUpdateParams {
            thread_id: thread.id,
            service_tier: Some(None),
            ..Default::default()
        })
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let response = to_response::<ThreadTurnContextUpdateResponse>(response)?;

    assert_eq!(response.turn_context.service_tier, None);
    let notification = read_turn_context_updated(&mut mcp).await?;
    assert_eq!(notification.turn_context.service_tier, None);

    Ok(())
}

#[tokio::test]
async fn thread_turn_context_update_rejects_sandbox_policy_with_permissions() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    write_config(&codex_home, &server.uri())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;
    let ThreadStartResponse { thread, .. } = start_thread(&mut mcp).await?;

    let request_id = mcp
        .send_thread_turn_context_update_request(ThreadTurnContextUpdateParams {
            thread_id: thread.id,
            sandbox_policy: Some(SandboxPolicy::DangerFullAccess),
            permissions: Some(PermissionProfileSelectionParams::Profile {
                id: ":read-only".to_string(),
                modifications: None,
            }),
            ..Default::default()
        })
        .await?;
    let err: JSONRPCError = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;

    assert_eq!(err.error.code, INVALID_REQUEST_ERROR_CODE);
    assert!(
        err.error
            .message
            .contains("`permissions` cannot be combined with `sandboxPolicy`"),
        "unexpected error message: {}",
        err.error.message
    );

    Ok(())
}

#[tokio::test]
async fn turn_start_emits_turn_context_updated_when_overrides_change_defaults() -> Result<()> {
    let server = create_mock_responses_server_sequence_unchecked(vec![
        create_final_assistant_message_sse_response("Done")?,
    ])
    .await;
    let codex_home = TempDir::new()?;
    write_config(&codex_home, &server.uri())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;
    let ThreadStartResponse { thread, .. } = start_thread(&mut mcp).await?;

    let request_id = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![V2UserInput::Text {
                text: "Hello".to_string(),
                text_elements: Vec::new(),
            }],
            model: Some("gpt-5.2".to_string()),
            effort: Some(ReasoningEffort::Low),
            ..Default::default()
        })
        .await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;

    let notification = read_turn_context_updated(&mut mcp).await?;
    assert_eq!(notification.thread_id, thread.id);
    assert_eq!(notification.turn_context.model, "gpt-5.2");
    assert_eq!(notification.turn_context.effort, Some(ReasoningEffort::Low));
    assert_eq!(
        notification.turn_context.service_tier.as_deref(),
        Some("flex")
    );

    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    Ok(())
}

async fn assert_newer_update_survives_turn_start(
    turn_start_overrides: TurnStartParams,
) -> Result<()> {
    let server = create_mock_responses_server_sequence_unchecked(vec![
        create_final_assistant_message_sse_response("Done")?,
    ])
    .await;
    let codex_home = TempDir::new()?;
    write_config(&codex_home, &server.uri())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;
    let ThreadStartResponse { thread, .. } = start_thread(&mut mcp).await?;

    let turn_request_id = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![V2UserInput::Text {
                text: "Hello".to_string(),
                text_elements: Vec::new(),
            }],
            ..turn_start_overrides
        })
        .await?;
    let update_request_id = mcp
        .send_thread_turn_context_update_request(ThreadTurnContextUpdateParams {
            thread_id: thread.id.clone(),
            model: Some("gpt-5.4".to_string()),
            effort: Some(Some(ReasoningEffort::High)),
            ..Default::default()
        })
        .await?;

    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_request_id)),
    )
    .await??;
    let update_response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(update_request_id)),
    )
    .await??;
    let update_response = to_response::<ThreadTurnContextUpdateResponse>(update_response)?;
    assert_eq!(update_response.turn_context.model, "gpt-5.4");
    assert_eq!(
        update_response.turn_context.effort,
        Some(ReasoningEffort::High)
    );

    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    mcp.clear_message_buffer();
    let read_current_request_id = mcp
        .send_thread_turn_context_update_request(ThreadTurnContextUpdateParams {
            thread_id: thread.id,
            ..Default::default()
        })
        .await?;
    let read_current_response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(read_current_request_id)),
    )
    .await??;
    let read_current_response =
        to_response::<ThreadTurnContextUpdateResponse>(read_current_response)?;
    assert_eq!(
        read_current_response.turn_context,
        update_response.turn_context
    );

    Ok(())
}

#[tokio::test]
async fn thread_turn_context_update_after_turn_start_preserves_newer_update() -> Result<()> {
    assert_newer_update_survives_turn_start(TurnStartParams {
        model: Some("gpt-5.2".to_string()),
        effort: Some(ReasoningEffort::Low),
        ..Default::default()
    })
    .await
}

#[tokio::test]
async fn thread_turn_context_update_after_no_op_turn_start_override_preserves_newer_update()
-> Result<()> {
    assert_newer_update_survives_turn_start(TurnStartParams {
        model: Some("mock-model".to_string()),
        ..Default::default()
    })
    .await
}
