//! Checks client attribution on exported storage metrics through the public RPC API.

use super::connection_handling_websocket::connect_websocket;
use super::connection_handling_websocket::read_response_for_id;
use super::connection_handling_websocket::send_initialize_request;
use super::connection_handling_websocket::send_request;
use super::connection_handling_websocket::spawn_websocket_server;
use anyhow::Context;
use anyhow::Result;
use app_test_support::ChatGptAuthFixture;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use app_test_support::write_chatgpt_auth;
use app_test_support::write_models_cache;
use codex_app_server_protocol::ClientInfo;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnCompletedNotification;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::TurnStatus;
use codex_app_server_protocol::UserInput;
use codex_login::AuthCredentialsStoreMode;
use codex_login::REFRESH_TOKEN_URL_OVERRIDE_ENV_VAR;
use core_test_support::responses;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;
use std::time::Duration;
use tempfile::TempDir;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

pub(super) async fn configure_collector(codex_home: &Path) -> Result<MockServer> {
    let collector = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/metrics"))
        .respond_with(ResponseTemplate::new(/*s*/ 200))
        .mount(&collector)
        .await;
    let endpoint = format!("{}/v1/metrics", collector.uri());
    let mut config = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(codex_home.join("config.toml"))?;
    writeln!(
        config,
        "\n[analytics]\nenabled = true\n[otel.metrics_exporter.otlp-http]\nendpoint = {endpoint:?}\nprotocol = \"json\""
    )?;
    Ok(collector)
}

pub(super) async fn assert_saved_originators(
    collector: &MockServer,
    expected: &[&str],
) -> Result<()> {
    let expected: BTreeMap<_, _> = expected
        .iter()
        .map(|value| (value.to_string(), 1_u64))
        .collect();
    let actual = tokio::time::timeout(Duration::from_secs(/*secs*/ 60), async {
        loop {
            let mut actual = BTreeMap::<String, u64>::new();
            for request in collector.received_requests().await.unwrap_or_default() {
                let body: Value = serde_json::from_slice(&request.body)?;
                for resource in body["resourceMetrics"].as_array().into_iter().flatten() {
                    for scope in resource["scopeMetrics"].as_array().into_iter().flatten() {
                        for metric in scope["metrics"].as_array().into_iter().flatten() {
                            if metric["name"] != "codex.auth_storage.operation" {
                                continue;
                            }
                            for point in
                                metric["sum"]["dataPoints"].as_array().into_iter().flatten()
                            {
                                let tags: BTreeMap<_, _> = point["attributes"]
                                    .as_array()
                                    .into_iter()
                                    .flatten()
                                    .filter_map(|tag| {
                                        Some((
                                            tag["key"].as_str()?,
                                            tag["value"]["stringValue"].as_str()?,
                                        ))
                                    })
                                    .collect();
                                if tags.get("credential_kind") == Some(&"codex")
                                    && tags.get("operation") == Some(&"save")
                                {
                                    let count = point["asInt"]
                                        .as_u64()
                                        .or_else(|| point["asInt"].as_str()?.parse().ok())
                                        .context("storage count must be an integer")?;
                                    *actual
                                        .entry(
                                            tags.get("originator")
                                                .unwrap_or(&"missing")
                                                .to_string(),
                                        )
                                        .or_default() += count;
                                }
                            }
                        }
                    }
                }
            }
            if actual.values().sum::<u64>() >= expected.len() as u64 {
                break Ok::<_, anyhow::Error>(actual);
            }
            tokio::time::sleep(Duration::from_millis(/*millis*/ 25)).await;
        }
    })
    .await??;
    assert_eq!(actual, expected);
    Ok(())
}

#[tokio::test]
async fn concurrent_connections_export_their_own_storage_originator() -> Result<()> {
    let codex_home = TempDir::new()?;
    std::fs::write(
        codex_home.path().join("config.toml"),
        "cli_auth_credentials_store = \"file\"\n",
    )?;
    let collector = configure_collector(codex_home.path()).await?;
    let (_process, address) = spawn_websocket_server(codex_home.path()).await?;
    let mut clients = Vec::new();
    for name in ["Codex Desktop", "codex_vscode", "private-client-name"] {
        let mut client = connect_websocket(address).await?;
        send_initialize_request(&mut client, /*id*/ 1, name).await?;
        read_response_for_id(&mut client, /*id*/ 1).await?;
        clients.push(client);
    }
    // All clients initialize before any storage request; a process-global identity fails this test.
    for client in &mut clients {
        send_request(
            client,
            "account/login/start",
            /*id*/ 2,
            Some(json!({"type": "apiKey", "apiKey": "sk-test-key"})),
        )
        .await?;
    }
    for client in &mut clients {
        let response = read_response_for_id(client, /*id*/ 2).await?;
        assert_eq!(response.result, json!({"type": "apiKey"}));
    }
    assert_saved_originators(&collector, &["codex_desktop", "codex_vscode", "other"]).await
}

#[tokio::test]
async fn turn_token_refresh_exports_thread_storage_originator() -> Result<()> {
    let codex_home = TempDir::new()?;
    let backend = MockServer::start().await;
    MockResponsesConfig::new(&backend.uri())
        .with_root_config("cli_auth_credentials_store = \"file\"")
        .with_provider_config("requires_openai_auth = true")
        .write(codex_home.path())?;
    write_models_cache(codex_home.path()).await?;
    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("initial-access-token")
            .account_id("account-123")
            .chatgpt_user_id("user-123"),
        AuthCredentialsStoreMode::File,
    )?;
    let collector = configure_collector(codex_home.path()).await?;

    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(ResponseTemplate::new(/*s*/ 200).set_body_json(json!({
            "access_token": "refreshed-access-token",
            "refresh_token": "refreshed-refresh-token",
        })))
        .expect(/*r*/ 1)
        .mount(&backend)
        .await;
    // Fresh credentials prevent startup refresh. The turn first reloads storage on 401,
    // then refreshes and persists the tokens when the retry is also unauthorized.
    let responses_mock = responses::mount_response_sequence(
        &backend,
        vec![
            ResponseTemplate::new(/*s*/ 401),
            ResponseTemplate::new(/*s*/ 401),
            responses::sse_response(responses::sse(vec![
                responses::ev_response_created("resp-turn"),
                responses::ev_completed("resp-turn"),
            ])),
        ],
    )
    .await;
    let refresh_url = format!("{}/oauth/token", backend.uri());
    let mut server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .with_env_overrides(&[
            ("OPENAI_API_KEY", None),
            (
                REFRESH_TOKEN_URL_OVERRIDE_ENV_VAR,
                Some(refresh_url.as_str()),
            ),
        ])
        .build()
        .await?;
    server
        .initialize_with_client_info(ClientInfo {
            name: "codex_vscode".to_string(),
            title: None,
            version: "0.1.0".to_string(),
        })
        .await?;
    let request = server
        .send_thread_start_request_with_auto_env(ThreadStartParams::default())
        .await?;
    let thread: ThreadStartResponse = server.read_response(request).await?;
    let request = server
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.thread.id,
            input: vec![UserInput::Text {
                text: "Hello".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    let _: TurnStartResponse = server.read_response(request).await?;
    let notification = server
        .read_stream_until_notification_message("turn/completed")
        .await?;
    let completed: TurnCompletedNotification =
        serde_json::from_value(notification.params.context("turn/completed params")?)?;
    assert_eq!(completed.turn.status, TurnStatus::Completed);
    assert_eq!(
        responses_mock
            .requests()
            .iter()
            .map(|request| request.header("authorization"))
            .collect::<Vec<_>>(),
        vec![
            Some("Bearer initial-access-token".to_string()),
            Some("Bearer initial-access-token".to_string()),
            Some("Bearer refreshed-access-token".to_string()),
        ]
    );
    assert_saved_originators(&collector, &["codex_vscode"]).await?;
    backend.verify().await;
    Ok(())
}
