use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use anyhow::Context;
use anyhow::Result;
use app_test_support::ChatGptAuthFixture;
use app_test_support::TestAppServer;
use app_test_support::create_final_assistant_message_sse_response;
use app_test_support::to_response;
use app_test_support::write_chatgpt_auth;
use app_test_support::write_mock_responses_config_toml_with_chatgpt_base_url;
use codex_app_server_protocol::LoginAccountResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadResumeParams;
use codex_app_server_protocol::ThreadResumeResponse;
use codex_app_server_protocol::ThreadRollbackParams;
use codex_app_server_protocol::ThreadRollbackResponse;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::UserInput;
use codex_config::types::AuthCredentialsStoreMode;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::RolloutLine;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use pretty_assertions::assert_eq;
use serde::de::DeserializeOwned;
use serde_json::json;
use tempfile::TempDir;
use tokio::time::Duration;
use tokio::time::timeout;
use wiremock::Mock;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

// macOS and Windows Bazel CI can spend tens of seconds starting app-server
// subprocesses or processing test RPCs under load.
#[cfg(any(target_os = "macos", windows))]
const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(60);
#[cfg(not(any(target_os = "macos", windows)))]
const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(10);
const COMMIT_ATTRIBUTION: &str = "Co-authored-by: Codex <noreply@openai.com>";
const PR_ATTRIBUTION: &str = "Generated with Codex.";
const ATTRIBUTION_DISABLED: &str = "attribution is disabled for the current workspace";
const LEGACY_COMMIT_ATTRIBUTION_INSTRUCTIONS: &str = "\
When you write or edit a git commit message, ensure the message ends with this trailer exactly once:
Co-authored-by: Codex <noreply@openai.com>

Rules:
- Keep existing trailers and append this trailer at the end if missing.
- Do not duplicate this trailer if it already exists.
- Keep one blank line between the commit body and trailer block.";

#[tokio::test]
async fn git_attribution_follows_authenticated_workspace_policy() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let settings_server = responses::start_mock_server().await;
    let response_mock = responses::mount_sse_sequence(
        &server,
        [
            "Unavailable",
            "Recovered",
            "Cached",
            "After switch",
            "After rollback",
        ]
        .into_iter()
        .map(create_final_assistant_message_sse_response)
        .collect::<Result<Vec<_>>>()?,
    )
    .await;
    Mock::given(method("GET"))
        .and(path("/backend-api/wham/config/bundle"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
        .mount(&server)
        .await;
    let enabled_settings_requests = Arc::new(AtomicUsize::new(0));
    Mock::given(method("GET"))
        .and(path("/backend-api/wham/settings/user"))
        .and(header("chatgpt-account-id", "workspace-enabled"))
        .respond_with({
            let enabled_settings_requests = enabled_settings_requests.clone();
            move |_request: &wiremock::Request| {
                if enabled_settings_requests.fetch_add(1, Ordering::SeqCst) == 0 {
                    ResponseTemplate::new(503)
                } else {
                    ResponseTemplate::new(200).set_body_json(json!({
                        "commit_attribution_enabled": true,
                    }))
                }
            }
        })
        .expect(3)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/backend-api/wham/settings/user"))
        .and(header("chatgpt-account-id", "workspace-disabled"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "commit_attribution_enabled": false,
        })))
        .expect(1)
        .mount(&server)
        .await;

    let codex_home = TempDir::new()?;
    write_mock_responses_config_toml_with_chatgpt_base_url(
        codex_home.path(),
        &server.uri(),
        &format!("{}/backend-api", server.uri()),
    )?;
    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("chatgpt-token")
            .account_id("workspace-enabled")
            .plan_type("enterprise"),
        AuthCredentialsStoreMode::File,
    )?;

    let mut app_server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .with_env_overrides(&[("OPENAI_API_KEY", None), ("CODEX_ACCESS_TOKEN", None)])
        .build()
        .await?;
    timeout(DEFAULT_READ_TIMEOUT, app_server.initialize()).await??;

    let request_id = app_server
        .send_thread_start_request_with_auto_env(ThreadStartParams {
            config: Some(HashMap::from([(
                "chatgpt_base_url".to_string(),
                json!(format!("{}/backend-api", settings_server.uri())),
            )])),
            ..Default::default()
        })
        .await?;
    let ThreadStartResponse { thread, .. } = read_response(&mut app_server, request_id).await?;
    run_turn(&mut app_server, &thread.id, "First turn").await?;
    run_turn(&mut app_server, &thread.id, "Second turn").await?;
    run_turn(&mut app_server, &thread.id, "Third turn").await?;

    let request_id = app_server
        .send_chatgpt_auth_tokens_login_request(
            "e30.e30.c2ln".to_string(),
            "workspace-disabled".to_string(),
            Some("enterprise".to_string()),
        )
        .await?;
    let _: LoginAccountResponse = read_response(&mut app_server, request_id).await?;
    run_turn(&mut app_server, &thread.id, "Turn after workspace switch").await?;

    let request_id = app_server
        .send_thread_rollback_request(ThreadRollbackParams {
            thread_id: thread.id.clone(),
            num_turns: 1,
        })
        .await?;
    let _: ThreadRollbackResponse = read_response(&mut app_server, request_id).await?;

    let request_id = app_server
        .send_chatgpt_auth_tokens_login_request(
            "e30.e30.c2ln".to_string(),
            "workspace-enabled".to_string(),
            Some("enterprise".to_string()),
        )
        .await?;
    let _: LoginAccountResponse = read_response(&mut app_server, request_id).await?;
    run_turn(&mut app_server, &thread.id, "Turn after rollback").await?;

    let requests = response_mock.requests();
    assert_eq!(requests.len(), 5);
    for (request, expected) in requests
        .into_iter()
        .zip([(0, 0), (1, 0), (1, 0), (1, 1), (1, 0)])
    {
        let developer_text = request.message_input_texts("developer").join("\n");
        assert_eq!(
            (
                developer_text.matches(COMMIT_ATTRIBUTION).count(),
                developer_text.matches(PR_ATTRIBUTION).count(),
                developer_text.matches(ATTRIBUTION_DISABLED).count(),
            ),
            (expected.0, expected.0, expected.1)
        );
    }
    server.verify().await;
    assert!(
        settings_server
            .received_requests()
            .await
            .context("failed to fetch thread-override requests")?
            .iter()
            .all(|request| request.url.path() != "/backend-api/wham/settings/user"),
        "attribution settings must use the process-level base URL"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cold_resume_replaces_legacy_attribution_without_duplication() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let response_mock = responses::mount_sse_sequence(
        &server,
        ["Initial", "Resumed", "Resumed again"]
            .into_iter()
            .map(create_final_assistant_message_sse_response)
            .collect::<Result<Vec<_>>>()?,
    )
    .await;
    Mock::given(method("GET"))
        .and(path("/backend-api/wham/config/bundle"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
        .mount(&server)
        .await;
    let settings_requests = Arc::new(AtomicUsize::new(0));
    Mock::given(method("GET"))
        .and(path("/backend-api/wham/settings/user"))
        .and(header("chatgpt-account-id", "workspace-resume"))
        .respond_with({
            let settings_requests = Arc::clone(&settings_requests);
            move |_request: &wiremock::Request| {
                ResponseTemplate::new(200).set_body_json(json!({
                    "commit_attribution_enabled": settings_requests
                        .fetch_add(1, Ordering::SeqCst)
                        == 0,
                }))
            }
        })
        .expect(2)
        .mount(&server)
        .await;

    let codex_home = TempDir::new()?;
    write_mock_responses_config_toml_with_chatgpt_base_url(
        codex_home.path(),
        &server.uri(),
        &format!("{}/backend-api", server.uri()),
    )?;
    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("chatgpt-token")
            .account_id("workspace-resume")
            .plan_type("enterprise"),
        AuthCredentialsStoreMode::File,
    )?;

    let mut app_server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .with_env_overrides(&[("OPENAI_API_KEY", None), ("CODEX_ACCESS_TOKEN", None)])
        .without_auto_env()
        .build()
        .await?;
    timeout(DEFAULT_READ_TIMEOUT, app_server.initialize()).await??;
    let request_id = app_server
        .send_thread_start_request(ThreadStartParams::default())
        .await?;
    let ThreadStartResponse { thread, .. } = read_response(&mut app_server, request_id).await?;
    run_turn(&mut app_server, &thread.id, "persist enabled attribution").await?;
    let rollout_path = thread
        .path
        .context("initial thread should have a rollout path")?;
    let status = timeout(DEFAULT_READ_TIMEOUT, app_server.shutdown_gracefully()).await??;
    anyhow::ensure!(
        status.success(),
        "initial app-server did not exit successfully"
    );
    replace_attribution_fragment_with_legacy(&rollout_path)?;

    let mut app_server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .with_env_overrides(&[("OPENAI_API_KEY", None), ("CODEX_ACCESS_TOKEN", None)])
        .without_auto_env()
        .build()
        .await?;
    timeout(DEFAULT_READ_TIMEOUT, app_server.initialize()).await??;
    let request_id = app_server
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: thread.id,
            ..Default::default()
        })
        .await?;
    let ThreadResumeResponse { thread, .. } = read_response(&mut app_server, request_id).await?;
    run_turn(&mut app_server, &thread.id, "resume disabled attribution").await?;
    run_turn(&mut app_server, &thread.id, "continue disabled attribution").await?;

    let requests = response_mock.requests();
    assert_eq!(requests.len(), 3);
    let initial_text = requests[0].message_input_texts("developer").join("\n");
    assert_eq!(initial_text.matches(COMMIT_ATTRIBUTION).count(), 1);
    assert_eq!(initial_text.matches(PR_ATTRIBUTION).count(), 1);
    assert_eq!(initial_text.matches(ATTRIBUTION_DISABLED).count(), 0);
    for request in &requests[1..] {
        let developer_text = request.message_input_texts("developer").join("\n");
        assert_eq!(developer_text.matches(COMMIT_ATTRIBUTION).count(), 1);
        assert_eq!(developer_text.matches(PR_ATTRIBUTION).count(), 0);
        assert_eq!(developer_text.matches(ATTRIBUTION_DISABLED).count(), 1);
    }
    server.verify().await;
    Ok(())
}

fn replace_attribution_fragment_with_legacy(rollout_path: &Path) -> Result<()> {
    let rollout = std::fs::read_to_string(rollout_path)?;
    let mut replaced = false;
    let mut removed_saved_attribution = false;
    let lines = rollout
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let mut line = serde_json::from_str::<RolloutLine>(line)?;
            if let RolloutItem::ResponseItem(ResponseItem::Message { role, content, .. }) =
                &mut line.item
                && role == "developer"
            {
                for item in content {
                    if let ContentItem::InputText { text } = item
                        && text.contains("<git_attribution>")
                    {
                        *text = LEGACY_COMMIT_ATTRIBUTION_INSTRUCTIONS.to_string();
                        replaced = true;
                    }
                }
            }
            if let RolloutItem::WorldState(world_state) = &mut line.item
                && let Some(state) = world_state.state.as_object_mut()
                && state.remove("git_attribution").is_some()
            {
                removed_saved_attribution = true;
            }
            serde_json::to_string(&line)
        })
        .collect::<std::result::Result<Vec<_>, _>>()?;
    anyhow::ensure!(replaced, "rollout did not contain git attribution context");
    anyhow::ensure!(
        removed_saved_attribution,
        "rollout did not contain saved git attribution state"
    );
    std::fs::write(rollout_path, format!("{}\n", lines.join("\n")))?;
    Ok(())
}

async fn read_response<T: DeserializeOwned>(
    app_server: &mut TestAppServer,
    request_id: i64,
) -> Result<T> {
    let response = timeout(
        DEFAULT_READ_TIMEOUT,
        app_server.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    to_response(response)
}

async fn run_turn(app_server: &mut TestAppServer, thread_id: &str, text: &str) -> Result<()> {
    timeout(
        DEFAULT_READ_TIMEOUT,
        app_server.start_turn_and_wait_for_completion(TurnStartParams {
            thread_id: thread_id.to_string(),
            input: vec![UserInput::Text {
                text: text.to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        }),
    )
    .await??;
    Ok(())
}
