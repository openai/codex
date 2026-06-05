use anyhow::Context;
use anyhow::Result;
use app_test_support::TestAppServer;
use app_test_support::create_final_assistant_message_sse_response;
use app_test_support::create_mock_responses_server_sequence_unchecked;
use app_test_support::to_response;
use app_test_support::write_mock_responses_config_toml;
use app_test_support::write_models_cache;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCNotification;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::SandboxPolicy;
use codex_app_server_protocol::ThreadReadParams;
use codex_app_server_protocol::ThreadReadResponse;
use codex_app_server_protocol::ThreadSettingsUpdateParams;
use codex_app_server_protocol::ThreadSettingsUpdateResponse;
use codex_app_server_protocol::ThreadSettingsUpdatedNotification;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::UserInput as V2UserInput;
use codex_core::test_support::all_model_presets;
use codex_protocol::config_types::SERVICE_TIER_DEFAULT_REQUEST_VALUE;
use core_test_support::responses;
use pretty_assertions::assert_eq;
use serde_json::Value;
use std::collections::BTreeMap;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

#[tokio::test]
async fn thread_settings_update_emits_notification_and_updates_future_turns() -> Result<()> {
    let server = create_mock_responses_server_sequence_unchecked(vec![
        create_final_assistant_message_sse_response("done")?,
    ])
    .await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;
    write_models_cache(codex_home.path())?;
    let (model_id, service_tier_id) = service_tier_model_and_tier_id()?;

    let mut mcp = TestAppServer::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;
    let thread = start_thread(&mut mcp).await?.thread;

    send_thread_settings_update(
        &mut mcp,
        ThreadSettingsUpdateParams {
            thread_id: thread.id.clone(),
            model: Some(model_id.clone()),
            service_tier: Some(Some(service_tier_id.clone())),
            ..Default::default()
        },
    )
    .await?;
    assert!(
        received_response_bodies(&server).await?.is_empty(),
        "settings-only update should not start a model request"
    );

    start_text_turn(&mut mcp, thread.id.clone()).await?;

    let updated = read_thread_settings_updated(&mut mcp).await?;
    assert_eq!(updated.thread_id, thread.id);
    assert_eq!(updated.thread_settings.model, model_id);
    assert_eq!(
        updated.thread_settings.service_tier.as_deref(),
        Some(service_tier_id.as_str())
    );

    timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    let read = read_thread_with_turns(&mut mcp, &thread.id).await?;
    assert_eq!(read.thread.turns.len(), 1);

    let request_bodies = received_response_bodies(&server).await?;
    assert!(
        request_bodies.iter().any(|body| {
            body.get("model").and_then(Value::as_str) == Some(model_id.as_str())
                && body.get("service_tier").and_then(Value::as_str)
                    == Some(service_tier_id.as_str())
        }),
        "future turn did not use updated model/service tier: {request_bodies:#?}"
    );
    Ok(())
}

#[tokio::test]
async fn thread_settings_update_while_turn_is_active_emits_notification() -> Result<()> {
    let server = responses::start_mock_server().await;
    let first_response =
        responses::sse_response(create_final_assistant_message_sse_response("first done")?)
            .set_delay(Duration::from_secs(2));
    let _requests = responses::mount_response_sequence(&server, vec![first_response]).await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;

    let mut mcp = TestAppServer::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;
    let thread = start_thread(&mut mcp).await?.thread;
    start_text_turn(&mut mcp, thread.id.clone()).await?;
    timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/started"),
    )
    .await??;

    send_thread_settings_update(
        &mut mcp,
        ThreadSettingsUpdateParams {
            thread_id: thread.id.clone(),
            model: Some("mock-model-4".to_string()),
            ..Default::default()
        },
    )
    .await?;

    let updated = read_thread_settings_updated(&mut mcp).await?;
    assert_eq!(updated.thread_id, thread.id);
    assert_eq!(updated.thread_settings.model, "mock-model-4");

    timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;
    Ok(())
}

#[tokio::test]
async fn thread_settings_update_null_service_tier_uses_default() -> Result<()> {
    let server = create_mock_responses_server_sequence_unchecked(vec![
        create_final_assistant_message_sse_response("done")?,
    ])
    .await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;
    write_models_cache(codex_home.path())?;
    let (model_id, service_tier_id) = service_tier_model_and_tier_id()?;

    let mut mcp = TestAppServer::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;
    let thread = start_thread(&mut mcp).await?.thread;

    send_thread_settings_update(
        &mut mcp,
        ThreadSettingsUpdateParams {
            thread_id: thread.id.clone(),
            model: Some(model_id.clone()),
            service_tier: Some(Some(service_tier_id.clone())),
            ..Default::default()
        },
    )
    .await?;

    let set_updated = read_thread_settings_updated(&mut mcp).await?;
    assert_eq!(set_updated.thread_id, thread.id);
    assert_eq!(
        set_updated.thread_settings.service_tier.as_deref(),
        Some(service_tier_id.as_str())
    );

    send_thread_settings_update(
        &mut mcp,
        ThreadSettingsUpdateParams {
            thread_id: thread.id.clone(),
            service_tier: Some(None),
            ..Default::default()
        },
    )
    .await?;

    let clear_updated = read_thread_settings_updated(&mut mcp).await?;
    assert_eq!(clear_updated.thread_id, thread.id);
    assert_eq!(clear_updated.thread_settings.model, model_id);
    assert_eq!(
        clear_updated.thread_settings.service_tier.as_deref(),
        Some(SERVICE_TIER_DEFAULT_REQUEST_VALUE)
    );

    start_text_turn(&mut mcp, thread.id).await?;
    timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    let request_bodies = received_response_bodies(&server).await?;
    assert!(
        request_bodies.iter().any(|body| {
            body.get("model").and_then(Value::as_str) == Some(model_id.as_str())
                && body
                    .as_object()
                    .is_some_and(|object| !object.contains_key("service_tier"))
        }),
        "future turn did not clear service tier: {request_bodies:#?}"
    );
    Ok(())
}

#[tokio::test]
async fn thread_settings_update_cwd_updates_cwd() -> Result<()> {
    let server = create_mock_responses_server_sequence_unchecked(Vec::new()).await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;
    let updated_cwd = codex_home.path().join("workspace");
    std::fs::create_dir_all(&updated_cwd)?;

    let mut mcp = TestAppServer::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;
    let thread = start_thread(&mut mcp).await?.thread;

    send_thread_settings_update(
        &mut mcp,
        ThreadSettingsUpdateParams {
            thread_id: thread.id.clone(),
            cwd: Some(updated_cwd.clone()),
            ..Default::default()
        },
    )
    .await?;

    let updated = read_thread_settings_updated(&mut mcp).await?;
    assert_eq!(updated.thread_id, thread.id.clone());
    assert_eq!(updated.thread_settings.cwd.as_path(), updated_cwd.as_path());

    let read = read_thread(&mut mcp, &thread.id).await?;
    assert_eq!(read.thread.cwd, updated.thread_settings.cwd);
    Ok(())
}

#[tokio::test]
async fn thread_settings_update_rejects_sandbox_policy_with_permissions() -> Result<()> {
    let server = create_mock_responses_server_sequence_unchecked(Vec::new()).await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;

    let mut mcp = TestAppServer::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;
    let thread = start_thread(&mut mcp).await?.thread;

    let request_id = mcp
        .send_thread_settings_update_request(ThreadSettingsUpdateParams {
            thread_id: thread.id,
            sandbox_policy: Some(SandboxPolicy::DangerFullAccess),
            permissions: Some(":workspace".to_string()),
            ..Default::default()
        })
        .await?;
    let error: JSONRPCError = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;

    assert_eq!(
        error.error.message,
        "`permissions` cannot be combined with `sandboxPolicy`"
    );
    Ok(())
}

#[tokio::test]
async fn turn_start_settings_override_emits_thread_settings_updated() -> Result<()> {
    let server = create_mock_responses_server_sequence_unchecked(vec![
        create_final_assistant_message_sse_response("done")?,
    ])
    .await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;

    let mut mcp = TestAppServer::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;
    let thread = start_thread(&mut mcp).await?.thread;
    timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_notification_message("thread/started"),
    )
    .await??;

    let turn_request_id = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            client_user_message_id: None,
            input: vec![V2UserInput::Text {
                text: "hello".to_string(),
                text_elements: Vec::new(),
            }],
            model: Some("mock-model-3".to_string()),
            ..Default::default()
        })
        .await?;
    let turn_response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_request_id)),
    )
    .await??;
    let TurnStartResponse { turn } = to_response(turn_response)?;
    assert!(!turn.id.is_empty());

    let updated = read_thread_settings_updated(&mut mcp).await?;
    assert_eq!(updated.thread_id, thread.id);
    assert_eq!(updated.thread_settings.model, "mock-model-3");

    timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;
    Ok(())
}

async fn send_thread_settings_update(
    mcp: &mut TestAppServer,
    params: ThreadSettingsUpdateParams,
) -> Result<()> {
    let request_id = mcp.send_thread_settings_update_request(params).await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let _: ThreadSettingsUpdateResponse = to_response(response)?;
    Ok(())
}

async fn start_text_turn(mcp: &mut TestAppServer, thread_id: String) -> Result<()> {
    let turn_request_id = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id,
            input: vec![V2UserInput::Text {
                text: "hello".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    let turn_response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_request_id)),
    )
    .await??;
    let TurnStartResponse { turn } = to_response(turn_response)?;
    assert!(!turn.id.is_empty());
    Ok(())
}

async fn start_thread(mcp: &mut TestAppServer) -> Result<ThreadStartResponse> {
    let request_id = mcp
        .send_thread_start_request(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    to_response(response)
}

async fn read_thread_with_turns(
    mcp: &mut TestAppServer,
    thread_id: &str,
) -> Result<ThreadReadResponse> {
    let request_id = mcp
        .send_thread_read_request(ThreadReadParams {
            thread_id: thread_id.to_string(),
            include_turns: true,
        })
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    to_response(response)
}

async fn read_thread(mcp: &mut TestAppServer, thread_id: &str) -> Result<ThreadReadResponse> {
    let request_id = mcp
        .send_thread_read_request(ThreadReadParams {
            thread_id: thread_id.to_string(),
            include_turns: false,
        })
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    to_response(response)
}

async fn read_thread_settings_updated(
    mcp: &mut TestAppServer,
) -> Result<ThreadSettingsUpdatedNotification> {
    let notification: JSONRPCNotification = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_notification_message("thread/settings/updated"),
    )
    .await??;
    let params = notification
        .params
        .context("thread/settings/updated should include params")?;
    Ok(serde_json::from_value(params)?)
}

async fn received_response_bodies(server: &wiremock::MockServer) -> Result<Vec<Value>> {
    let requests = server
        .received_requests()
        .await
        .context("failed to fetch received requests")?;
    let mut bodies = Vec::new();
    for request in requests {
        if request.url.path().ends_with("/responses") {
            bodies.push(request.body_json::<Value>()?);
        }
    }
    Ok(bodies)
}

fn service_tier_model_and_tier_id() -> Result<(String, String)> {
    let model = all_model_presets()
        .iter()
        .find(|preset| preset.show_in_picker && !preset.service_tiers.is_empty())
        .context("bundled model catalog should include a picker model with service tiers")?;
    Ok((model.id.clone(), model.service_tiers[0].id.clone()))
}

fn create_config_toml(codex_home: &std::path::Path, server_uri: &str) -> std::io::Result<()> {
    write_mock_responses_config_toml(
        codex_home,
        server_uri,
        &BTreeMap::default(),
        /*auto_compact_limit*/ 200_000,
        /*requires_openai_auth*/ None,
        "mock_provider",
        "compact",
    )
}
