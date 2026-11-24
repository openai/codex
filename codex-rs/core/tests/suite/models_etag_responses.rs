#![cfg(not(target_os = "windows"))]

use codex_core::TurnInputRequest;
use codex_core::build_startup_models;
use core_test_support::test_codex::local_requests;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use codex_features::Feature;
use codex_login::AuthManager;
use codex_login::CodexAuth;
use codex_model_provider_info::ModelProviderInfo;
use codex_models_manager::manager::ModelCatalogSettings;
use codex_models_manager::model_info::model_info_from_slug;
use codex_protocol::config_types::CollaborationMode;
use codex_protocol::config_types::ModeKind;
use codex_protocol::config_types::Settings;
use codex_protocol::models::PermissionProfile;
use codex_protocol::openai_models::ModelsResponse;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::user_input::UserInput;
use core_test_support::TempDirExt;
use core_test_support::load_default_config_for_test;
use core_test_support::responses;
use core_test_support::responses::WebSocketConnectionConfig;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_exec_command_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::sse;
use core_test_support::responses::sse_response;
use core_test_support::responses::start_websocket_server_with_headers;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::test_codex::turn_permission_fields;
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_with_timeout;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use wiremock::MockServer;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn handshake_models_etag_refreshes_catalog_after_warmup_without_response_metadata()
-> Result<()> {
    skip_if_no_network!(Ok(()));

    let models_server = MockServer::start().await;
    let mut model = model_info_from_slug("gpt-5.2");
    model.description = Some("original catalog".to_string());
    let original_catalog = ModelsResponse {
        models: vec![model.clone()],
    };
    let initial_fetch = responses::mount_models_once_with_etag(
        &models_server,
        original_catalog.clone(),
        "original-etag",
    )
    .await;

    // Use a real HTTP catalog client with a separate WebSocket inference fixture.
    let catalog_home = TempDir::new()?;
    let mut catalog_config = load_default_config_for_test(&catalog_home).await;
    catalog_config.model_provider =
        ModelProviderInfo::create_openai_provider(Some(format!("{}/v1", models_server.uri())));
    catalog_config.model_provider.request_max_retries = Some(0);
    catalog_config.model_catalog = None;
    let auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();
    let manager = build_startup_models(
        &catalog_config,
        AuthManager::from_auth_for_testing(auth.clone()),
        "handshake-etag-test".to_string(),
        Arc::new(ModelCatalogSettings::default()),
    )
    .models_manager();

    let websocket_server = start_websocket_server_with_headers(vec![WebSocketConnectionConfig {
        requests: vec![
            vec![ev_response_created("warmup"), ev_completed("warmup")],
            vec![
                ev_response_created("turn-1"),
                ev_assistant_message("msg-1", "done"),
                ev_completed("turn-1"),
            ],
            vec![
                ev_response_created("turn-2"),
                ev_assistant_message("msg-2", "done"),
                ev_completed("turn-2"),
            ],
        ],
        response_headers: vec![("x-models-etag".into(), "updated-etag".into())],
        accept_delay: None,
        close_after_requests: false,
    }])
    .await;
    let websocket_base_url = format!("{}/v1", websocket_server.uri());
    let test = test_codex()
        .with_auth(auth)
        .with_model("gpt-5.2")
        .with_models_manager(Arc::clone(&manager))
        .with_config(move |config| {
            config.model_provider.base_url = Some(websocket_base_url);
            config.model_provider.supports_websockets = true;
            config.model_provider.request_max_retries = Some(0);
            config.model_provider.stream_max_retries = Some(0);
        })
        .build_with_auto_env(&models_server)
        .await?;
    let warmup = tokio::time::timeout(
        Duration::from_secs(10),
        websocket_server.wait_for_request(/*connection_index*/ 0, /*request_index*/ 0),
    )
    .await?;
    assert_eq!(warmup.body_json()["generate"], false);
    assert_eq!(initial_fetch.requests().len(), 1);
    assert_eq!(
        manager
            .get_remote_models()
            .await
            .into_iter()
            .filter(|model| model.slug == "gpt-5.2")
            .collect::<Vec<_>>(),
        original_catalog.models,
    );

    model.description = Some("updated catalog".to_string());
    let updated_catalog = ModelsResponse {
        models: vec![model],
    };
    let refresh_fetch = responses::mount_models_once_with_etag(
        &models_server,
        updated_catalog.clone(),
        "updated-etag",
    )
    .await;
    for _ in 0..2 {
        test.codex
            .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
                text: "hello".into(),
                text_elements: Vec::new(),
            }]))
            .await?;
        wait_for_event(&test.codex, |event| {
            matches!(event, EventMsg::TurnComplete(_))
        })
        .await;
        assert_eq!(
            manager
                .get_remote_models()
                .await
                .into_iter()
                .filter(|model| model.slug == "gpt-5.2")
                .collect::<Vec<_>>(),
            updated_catalog.models,
        );
        assert_eq!(refresh_fetch.requests().len(), 1);
    }
    assert_eq!(websocket_server.handshakes().len(), 1);
    let requests = websocket_server.single_connection();
    assert_eq!(requests.len(), 3);
    websocket_server.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn refresh_models_on_models_etag_mismatch_and_avoid_duplicate_models_fetch() -> Result<()> {
    skip_if_no_network!(Ok(()));

    const ETAG_1: &str = "\"models-etag-1\"";
    const ETAG_2: &str = "\"models-etag-2\"";
    const CALL_ID: &str = "exec-command-call-1";

    let server = MockServer::start().await;

    // 1) On spawn, Codex fetches /models and stores the ETag.
    let spawn_models_mock = responses::mount_models_once_with_etag(
        &server,
        ModelsResponse { models: Vec::new() },
        ETAG_1,
    )
    .await;

    let auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();
    let mut builder = test_codex()
        .with_auth(auth)
        .with_model("gpt-5.2")
        .with_config(|config| {
            // Keep this test deterministic: no request retries, and a small stream retry budget.
            config.model_provider.request_max_retries = Some(0);
            config.model_provider.stream_max_retries = Some(1);
            config
                .features
                .disable(Feature::Apps)
                .expect("test config should allow feature update");
        });

    let test = builder.build(&server).await?;
    let codex = Arc::clone(&test.codex);
    let cwd = Arc::clone(&test.cwd);
    let session_model = test.session_configured.model.clone();
    let cwd_path = cwd.abs();
    let (sandbox_policy, permission_profile) =
        turn_permission_fields(PermissionProfile::Disabled, cwd_path.as_path());

    assert_eq!(spawn_models_mock.requests().len(), 1);
    assert_eq!(spawn_models_mock.single_request_path(), "/v1/models");

    // 2) If the server sends a different X-Models-Etag on /responses, Codex refreshes /models.
    let refresh_models_mock = responses::mount_models_once_with_etag(
        &server,
        ModelsResponse { models: Vec::new() },
        ETAG_2,
    )
    .await;

    // First /responses request (user message) succeeds and returns a tool call.
    // It also includes a mismatched X-Models-Etag, which should trigger a /models refresh.
    let first_response_body = sse(vec![
        ev_response_created("resp-1"),
        ev_exec_command_call(CALL_ID, "/bin/echo 'etag ok'"),
        ev_completed("resp-1"),
    ]);
    responses::mount_response_once(
        &server,
        sse_response(first_response_body).insert_header("X-Models-Etag", ETAG_2),
    )
    .await;

    // Second /responses request (tool output) includes the same X-Models-Etag; Codex should not
    // refetch /models again after it has already refreshed the catalog.
    let completion_response_body = sse(vec![
        ev_response_created("resp-2"),
        ev_assistant_message("msg-1", "done"),
        ev_completed("resp-2"),
    ]);
    let tool_output_mock = responses::mount_response_once(
        &server,
        sse_response(completion_response_body).insert_header("X-Models-Etag", ETAG_2),
    )
    .await;

    codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "please run a tool".into(),
                text_elements: Vec::new(),
            }])
            .with_thread_settings(ThreadSettingsOverrides {
                environments: Some(local_requests(cwd_path)),
                approval_policy: Some(AskForApproval::Never),
                sandbox_policy: Some(sandbox_policy),
                permission_profile,
                collaboration_mode: Some(CollaborationMode {
                    mode: ModeKind::Default,
                    settings: Settings {
                        model: session_model,
                        reasoning_effort: None,
                        developer_instructions: None,
                    },
                }),
                ..Default::default()
            }),
        )
        .await?;

    let _ = wait_for_event_with_timeout(
        &codex,
        |ev| matches!(ev, EventMsg::TurnComplete(_)),
        Duration::from_secs(30),
    )
    .await;

    // Assert /models was refreshed exactly once after the X-Models-Etag mismatch.
    assert_eq!(refresh_models_mock.requests().len(), 1);
    assert_eq!(refresh_models_mock.single_request_path(), "/v1/models");
    let refresh_req = refresh_models_mock
        .requests()
        .into_iter()
        .next()
        .expect("one request");
    // Ensure Codex includes client_version on refresh. (This is a stable signal that we're using the /models client.)
    assert!(
        refresh_req
            .url
            .query_pairs()
            .any(|(k, _)| k == "client_version"),
        "expected /models refresh to include client_version query param"
    );

    // Assert the tool output /responses request succeeded and did not trigger another /models fetch.
    let tool_req = tool_output_mock.single_request();
    let _ = tool_req.function_call_output(CALL_ID);
    assert_eq!(refresh_models_mock.requests().len(), 1);

    Ok(())
}
