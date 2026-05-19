use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::create_final_assistant_message_sse_response;
use app_test_support::create_mock_responses_server_repeating_assistant;
use app_test_support::create_mock_responses_server_sequence_unchecked;
use app_test_support::to_response;
use app_test_support::write_mock_responses_config_toml;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::SandboxPolicy;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ThreadSettingsUpdateParams;
use codex_app_server_protocol::ThreadSettingsUpdateResponse;
use codex_app_server_protocol::ThreadSettingsUpdatedNotification;
use codex_app_server_protocol::ThreadSource;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::UserInput as V2UserInput;
use codex_features::Feature;
use codex_protocol::openai_models::ReasoningEffort;
use codex_utils_absolute_path::AbsolutePathBuf;
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
    read_response(mcp, request_id).await
}

async fn send_thread_settings_update(
    mcp: &mut McpProcess,
    params: ThreadSettingsUpdateParams,
) -> Result<ThreadSettingsUpdateResponse> {
    let request_id = mcp.send_thread_settings_update_request(params).await?;
    read_response(mcp, request_id).await
}

async fn read_response<T: serde::de::DeserializeOwned>(
    mcp: &mut McpProcess,
    request_id: i64,
) -> Result<T> {
    let response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    to_response(response)
}

fn text_input(text: &str) -> V2UserInput {
    V2UserInput::Text {
        text: text.to_string(),
        text_elements: Vec::new(),
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

async fn read_thread_settings_updated(
    mcp: &mut McpProcess,
) -> Result<ThreadSettingsUpdatedNotification> {
    let notification = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("thread/settings/updated"),
    )
    .await??;
    let notification: ServerNotification = notification.try_into()?;
    let ServerNotification::ThreadSettingsUpdated(notification) = notification else {
        anyhow::bail!("expected thread/settings/updated notification");
    };
    Ok(notification)
}

#[tokio::test]
async fn thread_settings_update_applies_partial_patch_and_emits_full_state() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    write_config(&codex_home, &server.uri())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;
    let ThreadStartResponse { thread, .. } = start_thread(&mut mcp).await?;

    let response = send_thread_settings_update(
        &mut mcp,
        ThreadSettingsUpdateParams {
            thread_id: thread.id.clone(),
            model: Some("gpt-5.2".to_string()),
            effort: Some(Some(ReasoningEffort::High)),
            ..Default::default()
        },
    )
    .await?;

    assert_eq!(response.thread_settings.model, "gpt-5.2");
    assert_eq!(
        response.thread_settings.service_tier.as_deref(),
        Some("flex")
    );
    assert_eq!(response.thread_settings.effort, Some(ReasoningEffort::High));
    assert_eq!(response.thread_settings.cwd, thread.cwd);

    let notification = read_thread_settings_updated(&mut mcp).await?;
    assert_eq!(notification.thread_id, thread.id);
    assert_eq!(notification.thread_settings, response.thread_settings);

    mcp.clear_message_buffer();
    let no_op_response = send_thread_settings_update(
        &mut mcp,
        ThreadSettingsUpdateParams {
            thread_id: thread.id,
            model: Some("gpt-5.2".to_string()),
            effort: Some(Some(ReasoningEffort::High)),
            ..Default::default()
        },
    )
    .await?;
    assert_eq!(no_op_response.thread_settings, response.thread_settings);
    assert!(
        !mcp.pending_notification_methods()
            .iter()
            .any(|method| method == "thread/settings/updated")
    );

    Ok(())
}

#[tokio::test]
async fn thread_settings_update_absolutizes_relative_cwd_before_permissions() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    write_config(&codex_home, &server.uri())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;
    let ThreadStartResponse { thread, .. } = start_thread(&mut mcp).await?;
    let next_cwd = std::path::PathBuf::from("next-cwd");
    let next_cwd_abs = thread.cwd.join(&next_cwd);
    std::fs::create_dir_all(next_cwd_abs.as_path())?;

    let response = send_thread_settings_update(
        &mut mcp,
        ThreadSettingsUpdateParams {
            thread_id: thread.id.clone(),
            cwd: Some(next_cwd),
            permissions: Some(":workspace".to_string()),
            ..Default::default()
        },
    )
    .await?;

    assert_eq!(response.thread_settings.cwd, next_cwd_abs);
    assert_eq!(
        response
            .thread_settings
            .active_permission_profile
            .map(|profile| profile.id),
        Some(":workspace".to_string())
    );

    Ok(())
}

#[tokio::test]
async fn thread_settings_update_clears_service_tier_with_explicit_null() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    write_config(&codex_home, &server.uri())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;
    let ThreadStartResponse { thread, .. } = start_thread(&mut mcp).await?;

    let response = send_thread_settings_update(
        &mut mcp,
        ThreadSettingsUpdateParams {
            thread_id: thread.id,
            service_tier: Some(None),
            ..Default::default()
        },
    )
    .await?;

    assert_eq!(response.thread_settings.service_tier, None);
    let notification = read_thread_settings_updated(&mut mcp).await?;
    assert_eq!(notification.thread_settings.service_tier, None);

    Ok(())
}

#[tokio::test]
async fn thread_settings_update_rejects_sandbox_policy_with_permissions() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    write_config(&codex_home, &server.uri())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;
    let ThreadStartResponse { thread, .. } = start_thread(&mut mcp).await?;

    let request_id = mcp
        .send_thread_settings_update_request(ThreadSettingsUpdateParams {
            thread_id: thread.id,
            sandbox_policy: Some(SandboxPolicy::DangerFullAccess),
            permissions: Some(":read-only".to_string()),
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
async fn thread_settings_update_waits_for_pending_cwd_before_permissions() -> Result<()> {
    let server = create_mock_responses_server_sequence_unchecked(vec![
        create_final_assistant_message_sse_response("Done")?,
    ])
    .await;
    let codex_home = TempDir::new()?;
    write_config(&codex_home, &server.uri())?;
    let next_cwd = TempDir::new()?;
    let next_cwd_abs = AbsolutePathBuf::try_from(next_cwd.path())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;
    let ThreadStartResponse { thread, .. } = start_thread(&mut mcp).await?;

    let turn_request_id = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![text_input("Hello")],
            cwd: Some(next_cwd.path().to_path_buf()),
            ..Default::default()
        })
        .await?;
    let update_request_id = mcp
        .send_thread_settings_update_request(ThreadSettingsUpdateParams {
            thread_id: thread.id.clone(),
            permissions: Some(":workspace".to_string()),
            ..Default::default()
        })
        .await?;

    let _: TurnStartResponse = read_response(&mut mcp, turn_request_id).await?;
    let update_response =
        read_response::<ThreadSettingsUpdateResponse>(&mut mcp, update_request_id).await?;

    assert_eq!(update_response.thread_settings.cwd, next_cwd_abs);
    assert_eq!(
        update_response
            .thread_settings
            .active_permission_profile
            .map(|profile| profile.id),
        Some(":workspace".to_string())
    );

    wait_for_turn_completed(&mut mcp).await?;

    Ok(())
}

#[tokio::test]
async fn turn_start_emits_thread_settings_updated_when_overrides_change_defaults() -> Result<()> {
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
            input: vec![text_input("Hello")],
            model: Some("gpt-5.2".to_string()),
            effort: Some(ReasoningEffort::Low),
            ..Default::default()
        })
        .await?;
    let _: TurnStartResponse = read_response(&mut mcp, request_id).await?;

    let notification = read_thread_settings_updated(&mut mcp).await?;
    assert_eq!(notification.thread_id, thread.id);
    assert_eq!(notification.thread_settings.model, "gpt-5.2");
    assert_eq!(
        notification.thread_settings.effort,
        Some(ReasoningEffort::Low)
    );
    assert_eq!(
        notification.thread_settings.service_tier.as_deref(),
        Some("flex")
    );

    wait_for_turn_completed(&mut mcp).await?;

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
            input: vec![text_input("Hello")],
            ..turn_start_overrides
        })
        .await?;
    let update_request_id = mcp
        .send_thread_settings_update_request(ThreadSettingsUpdateParams {
            thread_id: thread.id.clone(),
            model: Some("gpt-5.4".to_string()),
            effort: Some(Some(ReasoningEffort::High)),
            ..Default::default()
        })
        .await?;

    let _: TurnStartResponse = read_response(&mut mcp, turn_request_id).await?;
    let update_response =
        read_response::<ThreadSettingsUpdateResponse>(&mut mcp, update_request_id).await?;
    assert_eq!(update_response.thread_settings.model, "gpt-5.4");
    assert_eq!(
        update_response.thread_settings.effort,
        Some(ReasoningEffort::High)
    );

    wait_for_turn_completed(&mut mcp).await?;

    mcp.clear_message_buffer();
    let read_current_response = send_thread_settings_update(
        &mut mcp,
        ThreadSettingsUpdateParams {
            thread_id: thread.id,
            ..Default::default()
        },
    )
    .await?;
    assert_eq!(
        read_current_response.thread_settings,
        update_response.thread_settings
    );

    Ok(())
}

#[tokio::test]
async fn thread_settings_update_after_turn_start_preserves_newer_update() -> Result<()> {
    assert_newer_update_survives_turn_start(TurnStartParams {
        model: Some("gpt-5.2".to_string()),
        effort: Some(ReasoningEffort::Low),
        ..Default::default()
    })
    .await
}

#[tokio::test]
async fn queued_updates_keep_each_thread_settings_notification_snapshot() -> Result<()> {
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
            input: vec![text_input("Hello")],
            model: Some("gpt-5.2".to_string()),
            effort: Some(ReasoningEffort::Low),
            ..Default::default()
        })
        .await?;
    let update_request_id = mcp
        .send_thread_settings_update_request(ThreadSettingsUpdateParams {
            thread_id: thread.id,
            model: Some("gpt-5.4".to_string()),
            effort: Some(Some(ReasoningEffort::High)),
            ..Default::default()
        })
        .await?;

    let _: TurnStartResponse = read_response(&mut mcp, turn_request_id).await?;
    let _: ThreadSettingsUpdateResponse = read_response(&mut mcp, update_request_id).await?;

    let notifications = [
        read_thread_settings_updated(&mut mcp).await?,
        read_thread_settings_updated(&mut mcp).await?,
    ];
    assert!(notifications.iter().any(|notification| {
        notification.thread_settings.model == "gpt-5.2"
            && notification.thread_settings.effort == Some(ReasoningEffort::Low)
    }));
    assert!(notifications.iter().any(|notification| {
        notification.thread_settings.model == "gpt-5.4"
            && notification.thread_settings.effort == Some(ReasoningEffort::High)
    }));

    wait_for_turn_completed(&mut mcp).await?;

    Ok(())
}

#[tokio::test]
async fn thread_settings_update_after_no_op_turn_start_override_preserves_newer_update()
-> Result<()> {
    assert_newer_update_survives_turn_start(TurnStartParams {
        model: Some("mock-model".to_string()),
        ..Default::default()
    })
    .await
}
