//! Verify TUI reasoning defaults and server-owned settings on outbound model requests.

use super::*;
use crate::legacy_core::config::ConfigBuilder;
use codex_app_server_client::AppServerEvent;
use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::ServerNotification;
use codex_protocol::config_types::ReasoningSummary;
use core_test_support::responses;
use futures::SinkExt;
use futures::StreamExt;
use pretty_assertions::assert_eq;
use serde_json::json;
use tokio_tungstenite::tungstenite::Message;

#[tokio::test]
async fn reasoning_defaults_reach_responses() -> Result<()> {
    let concurrent = json!({"reasoning_summary_delivery": "sequential_cutoff"});
    for (settings, summary, stream_options, mode) in [
        (
            "",
            json!("auto"),
            concurrent.clone(),
            ThreadParamsMode::Embedded,
        ),
        (
            "[features]\nconcurrent_reasoning_summaries = false",
            json!("auto"),
            json!(null),
            ThreadParamsMode::Embedded,
        ),
        (
            "model_reasoning_summary = 'detailed'",
            json!("detailed"),
            concurrent.clone(),
            ThreadParamsMode::Embedded,
        ),
        (
            "model_reasoning_summary = 'none'",
            json!(null),
            json!(null),
            ThreadParamsMode::Embedded,
        ),
        (
            "model_reasoning_summary = 'detailed'\nmodel_verbosity = 'high'\nweb_search = 'live'",
            json!("detailed"),
            concurrent,
            ThreadParamsMode::Remote,
        ),
    ] {
        let server = responses::start_mock_server().await;
        let home = tempfile::tempdir()?;
        let base_url = server.uri();
        std::fs::write(
            home.path().join("config.toml"),
            format!(
                r#"
model = "gpt-5.5"
model_provider = "reasoning-test"
{settings}
[model_providers.reasoning-test]
name = "OpenAI"
base_url = "{base_url}/v1"
wire_api = "responses"
request_max_retries = 0
stream_max_retries = 0
"#
            ),
        )?;
        crate::legacy_core::config::set_project_trust_level(
            home.path(),
            &std::env::current_dir()?,
            codex_protocol::config_types::TrustLevel::Trusted,
        )
        .map_err(|error| color_eyre::eyre::eyre!(error.to_string()))?;
        let mut server_config = ConfigBuilder::default()
            .codex_home(home.path().to_path_buf())
            .build()
            .await?;
        let mut catalog = codex_models_manager::bundled_models_response()?;
        catalog
            .models
            .iter_mut()
            .find(|model| model.slug == "gpt-5.5")
            .expect("bundled model")
            .default_reasoning_summary = codex_protocol::config_types::ReasoningSummary::Concise;
        server_config.model_catalog = Some(catalog);
        let client_home = tempfile::tempdir()?;
        let config = if mode == ThreadParamsMode::Remote {
            std::fs::write(
                client_home.path().join("config.toml"),
                "model_reasoning_summary = 'concise'\nmodel_verbosity = 'low'\nweb_search = 'disabled'",
            )?;
            ConfigBuilder::default()
                .codex_home(client_home.path().to_path_buf())
                .build()
                .await?
        } else {
            server_config.clone()
        };
        let mut app_server = crate::start_embedded_app_server_for_picker(&server_config).await?;
        app_server.thread_params_mode = mode;
        let started = app_server.start_thread(&config).await?;
        for resume in [false, true] {
            if resume {
                if mode != ThreadParamsMode::Remote {
                    break;
                }
                app_server.shutdown().await?;
                app_server = crate::start_embedded_app_server_for_picker(&server_config).await?;
                app_server.thread_params_mode = mode;
                app_server
                    .resume_thread(
                        &crate::local_settings::LocalSettings::from(&config),
                        config.clone(),
                        started.session.thread_id,
                        ResumeModelSettings::RestoreFromThread,
                    )
                    .await?;
            }
            let response = responses::mount_sse_once(
                &server,
                responses::sse(vec![
                    responses::ev_response_created("response"),
                    responses::ev_completed("response"),
                ]),
            )
            .await;
            let turn = app_server
                .turn_start(
                    started.session.thread_id,
                    "user",
                    "tui-user-message".to_string(),
                    vec![UserInput::Text {
                        text: "hello".to_string(),
                        text_elements: Vec::new(),
                    }],
                    config.cwd.to_path_buf(),
                    /*approval_policy*/ None,
                    /*approvals_reviewer*/ None,
                    TurnPermissionsOverride::Preserve,
                    &config.workspace_roots,
                    started.session.model.clone(),
                    /*effort*/ None,
                    Some(
                        server_config
                            .model_reasoning_summary
                            .unwrap_or(ReasoningSummary::Auto),
                    ),
                    /*service_tier*/ None,
                    /*collaboration_mode*/ None,
                    /*output_schema*/ None,
                    /*cyber_access_program*/ None,
                    /*cloud_selection*/ None,
                )
                .await?;
            tokio::time::timeout(std::time::Duration::from_secs(/*secs*/ 30), async {
                while let Some(event) = app_server.next_event().await {
                    if let AppServerEvent::ServerNotification(notification) = event.event
                        && let ServerNotification::TurnCompleted(completed) = *notification
                    {
                        assert_eq!(
                            completed.turn.status,
                            codex_app_server_protocol::TurnStatus::Completed
                        );
                        return;
                    }
                }
                panic!("app-server disconnected before completing the turn");
            })
            .await?;
            let body = response.single_request().body_json();
            assert_eq!(
                json!({"summary": body["reasoning"]["summary"], "stream_options": body["stream_options"]}),
                json!({"summary": summary, "stream_options": stream_options}),
                "settings: {settings}"
            );
            if mode == ThreadParamsMode::Remote {
                assert_eq!(body["text"]["verbosity"], json!("high"));
                let web_search = body["tools"]
                    .as_array()
                    .expect("tools")
                    .iter()
                    .find(|tool| tool["type"] == "web_search")
                    .expect("server's web search tool");
                assert_eq!(web_search["external_web_access"], json!(true));
            }
            let metadata: serde_json::Value = serde_json::from_str(
                body["client_metadata"]["x-codex-turn-metadata"]
                    .as_str()
                    .expect("canonical turn metadata"),
            )?;
            assert_eq!(
                (
                    &metadata["thread_id"],
                    &metadata["turn_id"],
                    &metadata["turn_trigger"],
                ),
                (
                    &json!(started.session.thread_id),
                    &json!(turn.turn.id),
                    &json!("user"),
                )
            );
        }
        app_server.shutdown().await?;
    }
    Ok(())
}

#[tokio::test]
async fn reasoning_summary_default_preserves_websocket_prewarm() -> Result<()> {
    let websocket = responses::start_websocket_server(vec![vec![
        vec![
            responses::ev_response_created("warmup"),
            responses::ev_completed("warmup"),
        ],
        vec![
            responses::ev_response_created("first"),
            responses::ev_completed("first"),
        ],
    ]])
    .await;
    let home = tempfile::tempdir()?;
    let base_url = websocket.uri();
    std::fs::write(
        home.path().join("config.toml"),
        format!(
            r#"
model = "gpt-5.5"
model_provider = "reasoning-test"
[model_providers.reasoning-test]
name = "OpenAI"
base_url = "{base_url}/v1"
wire_api = "responses"
supports_websockets = true
"#
        ),
    )?;
    let config = ConfigBuilder::default()
        .codex_home(home.path().to_path_buf())
        .build()
        .await?;
    let template = thread_start_params_from_config(
        &config,
        ThreadParamsMode::Embedded,
        /*remote_cwd_override*/ None,
        /*session_start_source*/ None,
    );
    assert_eq!(
        template
            .config
            .as_ref()
            .and_then(|config| config.get("model_reasoning_summary")),
        None,
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let endpoint = crate::resolve_remote_addr(&format!("ws://{}", listener.local_addr()?))?;
    let daemon = tokio::spawn(async move {
        let (stream, _) = listener.accept().await?;
        let mut socket = tokio_tungstenite::accept_async(stream).await?;
        while let Some(Ok(Message::Text(text))) = socket.next().await {
            let JSONRPCMessage::Request(request) = serde_json::from_str(&text)? else {
                continue;
            };
            let mut reply = match request.method.as_str() {
                "initialize" => json!({"result": {"userAgent": "daemon-test/1.0"}}),
                "thread/start" => json!({"error": {"code": -32603, "message": "recorded"}}),
                method => panic!("unexpected request: {method}"),
            };
            reply["id"] = json!(request.id);
            socket.send(Message::Text(reply.to_string().into())).await?;
            if request.method == "thread/start" {
                return Ok::<_, color_eyre::Report>(request.params);
            }
        }
        color_eyre::eyre::bail!("missing thread/start");
    });
    let mut daemon_session = AppServerSession::new(
        crate::backend_id::BackendId::new(),
        crate::connect_remote_app_server(endpoint).await?,
        ThreadParamsMode::Embedded,
    );
    assert!(daemon_session.start_thread(&config).await.is_err());
    let daemon_params = daemon.await??.expect("thread/start params");
    assert_eq!(
        daemon_params["config"]["model_reasoning_summary"],
        json!(null)
    );
    daemon_session.shutdown().await?;
    let mut app_server = crate::start_embedded_app_server_for_picker(&config).await?;
    let started = app_server.start_thread(&config).await?;
    let warmup = tokio::time::timeout(
        std::time::Duration::from_secs(/*secs*/ 30),
        websocket.wait_for_request(/*connection_index*/ 0, /*request_index*/ 0),
    )
    .await?;
    assert_eq!(warmup.body_json()["reasoning"]["summary"], "auto");
    app_server
        .turn_start(
            started.session.thread_id,
            "user",
            "tui-user-message".to_string(),
            vec![UserInput::Text {
                text: "hello".to_string(),
                text_elements: Vec::new(),
            }],
            config.cwd.to_path_buf(),
            /*approval_policy*/ None,
            /*approvals_reviewer*/ None,
            TurnPermissionsOverride::Preserve,
            &config.workspace_roots,
            started.session.model,
            /*effort*/ None,
            reasoning_summary_from_config(&config),
            /*service_tier*/ None,
            /*collaboration_mode*/ None,
            /*output_schema*/ None,
            /*cyber_access_program*/ None,
            /*cloud_selection*/ None,
        )
        .await?;
    let first = tokio::time::timeout(
        std::time::Duration::from_secs(/*secs*/ 30),
        websocket.wait_for_request(/*connection_index*/ 0, /*request_index*/ 1),
    )
    .await?;
    let first = first.body_json();
    assert_eq!(
        (
            first["reasoning"]["summary"].clone(),
            first["previous_response_id"].clone()
        ),
        (json!("auto"), json!("warmup")),
    );
    app_server.shutdown().await?;
    websocket.shutdown().await;
    Ok(())
}
