use anyhow::Result;
use app_test_support::TestAppServer;
use app_test_support::create_mock_responses_server_repeating_assistant;
use app_test_support::to_response;
use app_test_support::write_mock_responses_config_toml;
use codex_app_server_protocol::Automation;
use codex_app_server_protocol::AutomationCreateParams;
use codex_app_server_protocol::AutomationCreateResponse;
use codex_app_server_protocol::AutomationDeleteResponse;
use codex_app_server_protocol::AutomationListParams;
use codex_app_server_protocol::AutomationListResponse;
use codex_app_server_protocol::AutomationReadResponse;
use codex_app_server_protocol::AutomationStatus;
use codex_app_server_protocol::AutomationTarget;
use codex_app_server_protocol::AutomationUpdateResponse;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_features::Feature;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use std::path::Path;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[tokio::test]
async fn automation_api_rejects_disabled_feature() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    write_config(
        codex_home.path(),
        &server.uri(),
        /*automations_enabled*/ false,
    )?;

    let mut app_server = init_app_server(codex_home.path()).await?;
    let request_id = app_server
        .send_raw_request(
            "automation/list",
            Some(serde_json::to_value(AutomationListParams::default())?),
        )
        .await?;

    let err: JSONRPCError = timeout(
        DEFAULT_READ_TIMEOUT,
        app_server.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;
    assert_eq!(err.error.code, -32600);
    assert_eq!(err.error.message, "automations feature is disabled");

    Ok(())
}

#[tokio::test]
async fn automation_crud_is_scoped_to_subscribed_threads() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    write_config(
        codex_home.path(),
        &server.uri(),
        /*automations_enabled*/ true,
    )?;
    let cwd = codex_home.path().join("workspace");
    std::fs::create_dir(&cwd)?;

    let mut owner = init_app_server(codex_home.path()).await?;
    start_thread(&mut owner, &cwd).await?;

    let created = create_cron_automation(&mut owner, &cwd).await?;
    assert_eq!(created.name, "daily tidy");
    assert_eq!(created.prompt, "summarize the repo");
    assert_eq!(created.status, AutomationStatus::Active);
    assert_eq!(created.model.as_deref(), Some("mock-model"));
    assert_eq!(
        created.target,
        AutomationTarget::Cron {
            cwds: vec![cwd.clone()],
        }
    );

    let listed = list_automations(&mut owner).await?;
    assert_eq!(listed.data, vec![created.clone()]);
    assert_eq!(listed.next_cursor, None);

    let updated = update_automation_name(&mut owner, created.id.as_str()).await?;
    assert_eq!(updated.name, "daily polish");
    assert_eq!(updated.status, AutomationStatus::Paused);

    let read_back = read_automation(&mut owner, created.id.as_str()).await?;
    assert_eq!(read_back, Some(updated));

    let mut outsider = init_app_server(codex_home.path()).await?;
    let outsider_listed = list_automations(&mut outsider).await?;
    assert_eq!(outsider_listed.data, Vec::<Automation>::new());
    assert!(!delete_automation(&mut outsider, created.id.as_str()).await?);

    assert!(delete_automation(&mut owner, created.id.as_str()).await?);
    assert_eq!(
        read_automation(&mut owner, created.id.as_str()).await?,
        None
    );

    Ok(())
}

async fn init_app_server(codex_home: &Path) -> Result<TestAppServer> {
    let mut app_server = TestAppServer::new(codex_home).await?;
    timeout(DEFAULT_READ_TIMEOUT, app_server.initialize()).await??;
    Ok(app_server)
}

fn write_config(codex_home: &Path, server_uri: &str, automations_enabled: bool) -> Result<()> {
    write_mock_responses_config_toml(
        codex_home,
        server_uri,
        &BTreeMap::from([(Feature::Automations, automations_enabled)]),
        200_000,
        /*requires_openai_auth*/ None,
        "mock_provider",
        "compact",
    )?;
    Ok(())
}

async fn start_thread(app_server: &mut TestAppServer, cwd: &Path) -> Result<ThreadStartResponse> {
    let request_id = app_server
        .send_thread_start_request(ThreadStartParams {
            cwd: Some(cwd.display().to_string()),
            ..Default::default()
        })
        .await?;
    let resp = response_for(app_server, request_id).await?;
    to_response::<ThreadStartResponse>(resp)
}

async fn create_cron_automation(app_server: &mut TestAppServer, cwd: &Path) -> Result<Automation> {
    let request_id = app_server
        .send_raw_request(
            "automation/create",
            Some(serde_json::to_value(AutomationCreateParams {
                name: "daily tidy".to_string(),
                prompt: "summarize the repo".to_string(),
                rrule: Some("FREQ=DAILY;INTERVAL=1;BYHOUR=9;BYMINUTE=30".to_string()),
                model: Some("mock-model".to_string()),
                reasoning_effort: None,
                status: Some(AutomationStatus::Active),
                target: AutomationTarget::Cron {
                    cwds: vec![cwd.to_path_buf()],
                },
            })?),
        )
        .await?;
    let resp = response_for(app_server, request_id).await?;
    Ok(to_response::<AutomationCreateResponse>(resp)?.automation)
}

async fn list_automations(app_server: &mut TestAppServer) -> Result<AutomationListResponse> {
    let request_id = app_server
        .send_raw_request(
            "automation/list",
            Some(serde_json::to_value(AutomationListParams {
                cursor: None,
                limit: Some(10),
            })?),
        )
        .await?;
    let resp = response_for(app_server, request_id).await?;
    to_response::<AutomationListResponse>(resp)
}

async fn read_automation(
    app_server: &mut TestAppServer,
    automation_id: &str,
) -> Result<Option<Automation>> {
    let request_id = app_server
        .send_raw_request(
            "automation/read",
            Some(serde_json::json!({ "automationId": automation_id })),
        )
        .await?;
    let resp = response_for(app_server, request_id).await?;
    Ok(to_response::<AutomationReadResponse>(resp)?.automation)
}

async fn update_automation_name(
    app_server: &mut TestAppServer,
    automation_id: &str,
) -> Result<Automation> {
    let request_id = app_server
        .send_raw_request(
            "automation/update",
            Some(serde_json::json!({
                "automationId": automation_id,
                "name": "daily polish",
                "status": "PAUSED"
            })),
        )
        .await?;
    let resp = response_for(app_server, request_id).await?;
    to_response::<AutomationUpdateResponse>(resp)?
        .automation
        .ok_or_else(|| anyhow::anyhow!("expected updated automation"))
}

async fn delete_automation(app_server: &mut TestAppServer, automation_id: &str) -> Result<bool> {
    let request_id = app_server
        .send_raw_request(
            "automation/delete",
            Some(serde_json::json!({ "automationId": automation_id })),
        )
        .await?;
    let resp = response_for(app_server, request_id).await?;
    Ok(to_response::<AutomationDeleteResponse>(resp)?.deleted)
}

async fn response_for(app_server: &mut TestAppServer, request_id: i64) -> Result<JSONRPCResponse> {
    timeout(
        DEFAULT_READ_TIMEOUT,
        app_server.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await?
}
