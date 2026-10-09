//! Verifies that Guardian model and Cyber selections follow the current parent turn.

use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::Ordering;

use anyhow::Result;
use app_test_support::ChatGptAuthFixture;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use app_test_support::write_chatgpt_auth;
use app_test_support::write_models_cache_with_models;
use axum::Json;
use axum::Router;
use axum::extract::State;
use axum::extract::ws::WebSocketUpgrade;
use axum::http::HeaderMap;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use codex_app_server_protocol::ApprovalsReviewer;
use codex_app_server_protocol::CyberAccessProgram;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnCompletedNotification;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::TurnStatus;
use codex_app_server_protocol::UserInput;
use codex_features::Feature;
use codex_login::AuthCredentialsStoreMode;
use core_test_support::load_default_config_for_test;
use core_test_support::skip_if_no_network;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use tempfile::TempDir;
use test_case::test_case;
use tokio::net::TcpListener;
use tokio::time::timeout;

use super::MODEL;
use super::MockResponsesState;
use super::TEST_SERVER_NAME;
use super::TEST_TOOL_NAME;
use super::TIMEOUT;
use super::USER_CONTEXT;
use super::luna_websocket;
use super::parent_response;
use super::start_mcp_server;
use super::start_mcp_server_with_tools;
use super::wait_for_guardian_reviews;
use super::wait_for_luna_request;

#[test_case("node_repl"; "browser")]
#[test_case("cua_repl"; "computer use")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn computer_use_scoring_follows_model_review_requirement(
    server_name: &'static str,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    const REVIEWED_MODEL: &str = "reviewed-model";
    let state = Arc::new(MockResponsesState {
        mcp_server_name: Some(server_name),
        mcp_tool_sequence: Some(&["js"]),
        mcp_messages: Mutex::new(vec!["hello"]),
        ..Default::default()
    });
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let responses_url = format!("http://{}", listener.local_addr()?);
    let router = Router::new()
        .route("/v1/responses", get(luna_websocket).post(parent_response))
        .with_state(Arc::clone(&state));
    let responses_server = tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });
    let (mcp_url, mcp_server) =
        start_mcp_server_with_tools(&["js"], /*sensitive_action*/ None).await?;
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&responses_url)
        .with_model(MODEL)
        .with_provider_config("supports_websockets = false")
        .with_approval_policy("on-request")
        .with_root_config("approvals_reviewer = \"auto_review\"")
        .with_extra_config(&format!(
            "[mcp_servers.{server_name}]\nurl = \"{mcp_url}/mcp\"\ndefault_tools_approval_mode = \"auto\"\n\n[features.guardianv2]\nenabled = true"
        ))
        .enable_feature(Feature::GuardianApproval)
        .write(codex_home.path())?;
    let config = load_default_config_for_test(&codex_home).await;
    let ordinary_model = codex_core::test_support::construct_model_info_offline(MODEL, &config);
    let mut reviewed_model =
        codex_core::test_support::construct_model_info_offline(REVIEWED_MODEL, &config);
    reviewed_model.node_repl_auto_review_required = true;
    write_models_cache_with_models(codex_home.path(), vec![ordinary_model, reviewed_model]).await?;
    let mut app_server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized_with_timeout(TIMEOUT)
        .await?;
    let request_id = app_server
        .send_thread_start_request_with_auto_env(ThreadStartParams {
            approvals_reviewer: Some(ApprovalsReviewer::AutoReview),
            ..Default::default()
        })
        .await?;
    let thread: ThreadStartResponse =
        timeout(TIMEOUT, app_server.read_response(request_id)).await??;

    state.allow_guardian_review.notify_one();
    for (model, expected_samples) in [
        (MODEL, 0),
        (REVIEWED_MODEL, 1),
        (MODEL, 1),
        (REVIEWED_MODEL, 2),
    ] {
        state.parent_requests.store(0, Ordering::SeqCst);
        app_server.clear_message_buffer();
        state.allow_luna.notify_one();
        let request_id = app_server
            .send_turn_start_request(TurnStartParams {
                thread_id: thread.thread.id.clone(),
                model: Some(model.to_owned()),
                input: vec![UserInput::Text {
                    text: USER_CONTEXT.to_owned(),
                    text_elements: Vec::new(),
                }],
                ..Default::default()
            })
            .await?;
        let _: TurnStartResponse = timeout(TIMEOUT, app_server.read_response(request_id)).await??;
        let completed: TurnCompletedNotification =
            timeout(TIMEOUT, app_server.read_notification("turn/completed")).await??;
        assert_eq!(completed.turn.status, TurnStatus::Completed);
        if model == REVIEWED_MODEL {
            wait_for_luna_request(&state, expected_samples - 1).await?;
        }
        assert_eq!(
            state
                .luna_requests
                .lock()
                .expect("luna requests lock")
                .len(),
            expected_samples,
            "only models requiring REPL review should send classifier requests"
        );
    }

    app_server.shutdown_gracefully().await?;
    mcp_server.abort();
    responses_server.abort();
    Ok(())
}

#[test_case("snapshot", 0.0; "snapshot low risk")]
#[test_case("snapshot", 1.0; "snapshot high risk")]
#[test_case("conversation", 0.0; "conversation low risk")]
#[test_case("conversation", 1.0; "conversation high risk")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn guardian_programs_follow_each_turn_without_changing_parent_selection(
    classifier_mode: &str,
    luna_score: f64,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let state = Arc::new(MockResponsesState {
        mcp_tool_sequence: Some(&[TEST_TOOL_NAME]),
        gate_each_guardian_review: true,
        luna_score,
        ..Default::default()
    });
    let parent_requests = Arc::new(Mutex::new(Vec::<Value>::new()));
    let router = Router::new()
        .route(
            "/backend-api/codex/responses",
            get(
                |state: State<Arc<MockResponsesState>>,
                 headers: HeaderMap,
                 websocket: WebSocketUpgrade| async move {
                    if headers
                        .get("x-codex-guardian")
                        .and_then(|value| value.to_str().ok())
                        == Some("classifier")
                    {
                        luna_websocket(state, websocket).await.into_response()
                    } else {
                        StatusCode::UPGRADE_REQUIRED.into_response()
                    }
                },
            )
            .post({
                let parent_requests = Arc::clone(&parent_requests);
                move |State(state): State<Arc<MockResponsesState>>, Json(request): Json<Value>| {
                    let parent_requests = Arc::clone(&parent_requests);
                    async move {
                        if request["model"] == MODEL {
                            parent_requests
                                .lock()
                                .expect("parent request lock")
                                .push(request.clone());
                        }
                        parent_response(State(state), Json(request)).await
                    }
                }
            }),
        )
        .with_state(Arc::clone(&state));
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let responses_url = format!("http://{}", listener.local_addr()?);
    let responses_server = tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });
    let (mcp_url, mcp_server) = start_mcp_server(/*sensitive_action*/ None).await?;
    let codex_home = TempDir::new()?;
    std::fs::write(
        codex_home.path().join("config.toml"),
        format!(
            r#"
model = "{MODEL}"
approval_policy = "on-request"
approvals_reviewer = "auto_review"
sandbox_mode = "read-only"
cli_auth_credentials_store = "file"
openai_base_url = "{responses_url}/backend-api/codex"
chatgpt_base_url = "{responses_url}/backend-api"

[features]
guardian_approval = true
enable_request_compression = false

[mcp_servers.{TEST_SERVER_NAME}]
url = "{mcp_url}/mcp"
default_tools_approval_mode = "prompt"

[features.guardianv2]
enabled = true
async_classifier_mode = "{classifier_mode}"

[features.guardianv2.review_scope]
computer_use_only = false
"#
        ),
    )?;
    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("access-chatgpt").plan_type("pro"),
        AuthCredentialsStoreMode::File,
    )?;
    let config = load_default_config_for_test(&codex_home).await;
    let models = [MODEL, "gpt-5.6-luna", "gpt-6.1-sol"]
        .into_iter()
        .map(|slug| {
            let mut info = codex_core::test_support::construct_model_info_offline(slug, &config);
            if slug == MODEL {
                info.auto_review_model_override = Some("gpt-6.1-sol".to_owned());
            }
            info
        })
        .collect();
    write_models_cache_with_models(codex_home.path(), models).await?;
    let mut app_server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .with_env_overrides(&[
            ("OPENAI_API_KEY", None),
            ("CODEX_API_KEY", None),
            ("CODEX_ACCESS_TOKEN", None),
            ("OPENAI_FEDERATION_RULE_ID", None),
            ("OPENAI_IDENTITY_TOKEN_FILE", None),
            ("OPENAI_WORKLOAD_IDENTITY_CONTEXT", None),
        ])
        .build_initialized_with_timeout(TIMEOUT)
        .await?;
    let thread = app_server
        .start_thread(ThreadStartParams::default())
        .await?
        .thread;
    let cases = [
        (
            Some(CyberAccessProgram::DaybreakBlue),
            Some("daybreak_blue"),
            Some("daybreak_blue"),
        ),
        (
            Some(CyberAccessProgram::DaybreakRed),
            Some("daybreak_red"),
            Some("daybreak_blue"),
        ),
        (
            Some(CyberAccessProgram::Standard),
            Some("standard"),
            Some("standard"),
        ),
        (None, None, None),
    ];
    let mut classifier_requests = Vec::new();
    for (index, (program, _, _)) in cases.iter().enumerate() {
        state.parent_requests.store(0, Ordering::SeqCst);
        app_server.clear_message_buffer();
        let request_id = app_server
            .send_turn_start_request(TurnStartParams {
                thread_id: thread.id.clone(),
                cyber_access_program: *program,
                input: vec![UserInput::Text {
                    text: format!("{USER_CONTEXT} This is program turn {index}."),
                    text_elements: Vec::new(),
                }],
                ..Default::default()
            })
            .await?;
        let _: TurnStartResponse = timeout(TIMEOUT, app_server.read_response(request_id)).await??;
        let classifier_request = wait_for_luna_request(&state, index).await?;
        assert!(
            classifier_request
                .to_string()
                .contains(&format!("program turn {index}"))
        );
        assert!(
            classifier_request.to_string().contains("program turn 0"),
            "prior turn evidence remains available after changing programs"
        );
        classifier_requests.push(classifier_request);
        // Wait for the reviewer to start before releasing a low classifier score.
        wait_for_guardian_reviews(&state, index + 1).await?;
        state.allow_luna.notify_one();
        state.allow_guardian_review.notify_one();
        let completed: TurnCompletedNotification =
            timeout(TIMEOUT, app_server.read_notification("turn/completed")).await??;
        assert_eq!(completed.turn.status, TurnStatus::Completed);
    }

    let expected_guardian_programs = cases
        .iter()
        .map(|(_, _, name)| name.map(|name| json!({"cyber": name})))
        .collect::<Vec<_>>();
    assert_eq!(
        classifier_requests
            .iter()
            .map(|request| (
                request["model"].clone(),
                request.get("access_programs").cloned()
            ))
            .collect::<Vec<_>>(),
        expected_guardian_programs
            .iter()
            .map(|program| (json!("gpt-5.6-luna"), program.clone()))
            .collect::<Vec<_>>()
    );
    let guardian_requests = state
        .guardian_requests
        .lock()
        .expect("Guardian request lock")
        .clone();
    assert_eq!(
        guardian_requests
            .iter()
            .map(|request| (
                request["model"].clone(),
                request.get("access_programs").cloned()
            ))
            .collect::<Vec<_>>(),
        expected_guardian_programs
            .iter()
            .map(|program| (json!("gpt-6.1-sol"), program.clone()))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        parent_requests
            .lock()
            .expect("parent request lock")
            .iter()
            .map(|request| request.get("access_programs").cloned())
            .collect::<Vec<_>>(),
        cases
            .iter()
            .flat_map(|(_, name, _)| std::iter::repeat_n(
                name.map(|name| json!({"cyber": name})),
                2
            ))
            .collect::<Vec<_>>()
    );
    let reviewer_thread_ids = guardian_requests
        .iter()
        .map(|request| request["client_metadata"]["thread_id"].clone())
        .collect::<Vec<_>>();
    assert!(reviewer_thread_ids[0].is_string());
    assert_eq!(
        reviewer_thread_ids,
        vec![reviewer_thread_ids[0].clone(); cases.len()],
        "program changes should reuse the reviewer session"
    );

    app_server.shutdown_gracefully().await?;
    mcp_server.abort();
    responses_server.abort();
    Ok(())
}
