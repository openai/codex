//! Header-authenticated requests forward explicit programs and preserve backend denials.

use anyhow::Result;
use codex_core::TurnInputRequest;
use codex_core::TurnStartOptions;
use codex_login::AuthHeaders;
use codex_login::CodexAuth;
use codex_protocol::protocol::EventMsg;
use codex_protocol::turn_input::CyberAccessProgram;
use codex_protocol::user_input::UserInput;
use core_test_support::responses;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use http::HeaderMap;
use http::HeaderValue;
use pretty_assertions::assert_eq;
use serde_json::json;

const ACTOR: &str = "x-openai-actor-authorization";
const ACCOUNT: &str = "chatgpt-account-id";

fn actor_headers() -> HeaderMap {
    HeaderMap::from_iter([
        (
            http::HeaderName::from_static(ACTOR),
            HeaderValue::from_static("verified-actor"),
        ),
        (
            http::HeaderName::from_static(ACCOUNT),
            HeaderValue::from_static("workspace-1"),
        ),
    ])
}

fn actor_auth() -> CodexAuth {
    CodexAuth::Headers(AuthHeaders::new(actor_headers()))
}

async fn submit(test: &TestCodex, program: Option<CyberAccessProgram>) -> Result<()> {
    test.codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "hello".to_owned(),
                text_elements: Vec::new(),
            }])
            .on_start(TurnStartOptions {
                cyber_access_program: program,
                ..Default::default()
            }),
        )
        .await?;
    let event = wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_) | EventMsg::Error(_))
    })
    .await;
    if let EventMsg::Error(error) = event {
        anyhow::bail!("actor program request failed: {error:?}");
    }
    Ok(())
}

/// Each explicit choice reaches the receiver alongside its actor and account.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn actor_program_http_preserves_explicit_choices_and_omission() -> Result<()> {
    core_test_support::skip_if_no_network!(Ok(()));
    let server = responses::start_mock_server().await;
    let requests = responses::mount_sse_sequence(
        &server,
        (0..4)
            .map(|i| responses::sse_completed(&format!("resp-{i}")))
            .collect(),
    )
    .await;
    let test = test_codex()
        .with_auth(actor_auth())
        .with_config(|config| {
            config.model_provider_id = "local".to_owned();
            config
                .model_provider
                .capabilities
                .get_or_insert_default()
                .cyber_access_programs = Some(true);
        })
        .build_with_auto_env(&server)
        .await?;
    for program in [
        Some(CyberAccessProgram::DaybreakBlue),
        Some(CyberAccessProgram::DaybreakRed),
        Some(CyberAccessProgram::Standard),
        None,
    ] {
        submit(&test, program).await?;
    }
    assert_eq!(
        requests
            .requests()
            .iter()
            .map(|request| (
                request.body_json().get("access_programs").cloned(),
                request.header(ACTOR),
                request.header(ACCOUNT),
            ))
            .collect::<Vec<_>>(),
        [
            Some(json!({"cyber": "daybreak_blue"})),
            Some(json!({"cyber": "daybreak_red"})),
            Some(json!({"cyber": "standard"})),
            None
        ]
        .into_iter()
        .map(|program| (
            program,
            Some("verified-actor".to_owned()),
            Some("workspace-1".to_owned())
        ))
        .collect::<Vec<_>>()
    );
    test.codex.shutdown_and_wait().await?;
    Ok(())
}

/// Backend denial remains visible and every retry retains the explicit program.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn actor_program_backend_denial_remains_authoritative() -> Result<()> {
    core_test_support::skip_if_no_network!(Ok(()));
    let server = responses::start_mock_server().await;
    let request = responses::mount_response_handler(&server, |_| {
        wiremock::ResponseTemplate::new(403).set_body_json(json!({
            "error": {"message": "program entitlement denied", "type": "invalid_request_error", "code": "access_program_not_enabled"}
        }))
    }).await;
    let test = test_codex()
        .with_auth(actor_auth())
        .with_config(|config| {
            config.model_provider_id = "local".to_owned();
            config
                .model_provider
                .capabilities
                .get_or_insert_default()
                .cyber_access_programs = Some(true);
        })
        .build_with_auto_env(&server)
        .await?;
    test.codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "hello".into(),
                text_elements: Vec::new(),
            }])
            .on_start(TurnStartOptions {
                cyber_access_program: Some(CyberAccessProgram::DaybreakRed),
                ..Default::default()
            }),
        )
        .await?;
    let EventMsg::Error(error) = wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::Error(_) | EventMsg::TurnComplete(_))
    })
    .await
    else {
        panic!("backend denial must fail the turn")
    };
    assert_eq!(
        error.codex_error_info,
        Some(
            codex_protocol::protocol::CodexErrorInfo::HttpConnectionFailed {
                http_status_code: Some(403),
            }
        )
    );
    let requests = request.requests();
    assert!(!requests.is_empty());
    assert_eq!(
        requests
            .iter()
            .map(|request| request.body_json().get("access_programs").cloned())
            .collect::<Vec<_>>(),
        vec![Some(json!({"cyber": "daybreak_red"})); requests.len()]
    );
    test.codex.shutdown_and_wait().await?;
    Ok(())
}

/// Reusing a WebSocket preserves each request's program and the actor headers.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn actor_program_websocket_reuse_preserves_choices_and_actor() -> Result<()> {
    core_test_support::skip_if_no_network!(Ok(()));
    let server = responses::start_websocket_server(vec![
        (0..5)
            .map(|i| vec![responses::ev_completed(&format!("resp-{i}"))])
            .collect(),
    ])
    .await;
    let base_url = format!("{}/v1", server.uri().replacen("ws://", "http://", 1));
    let test = test_codex()
        .with_auth(actor_auth())
        .with_config(move |config| {
            config.model_provider_id = "local".to_owned();
            config
                .model_provider
                .capabilities
                .get_or_insert_default()
                .cyber_access_programs = Some(true);
            config.model_provider.base_url = Some(base_url);
        })
        .build_with_websocket_server(&server)
        .await?;
    for program in [
        Some(CyberAccessProgram::DaybreakBlue),
        Some(CyberAccessProgram::DaybreakRed),
        Some(CyberAccessProgram::Standard),
        None,
    ] {
        submit(&test, program).await?;
    }
    assert_eq!(
        server
            .single_connection()
            .iter()
            .map(|request| request.body_json().get("access_programs").cloned())
            .collect::<Vec<_>>(),
        vec![
            None,
            Some(json!({"cyber": "daybreak_blue"})),
            Some(json!({"cyber": "daybreak_red"})),
            Some(json!({"cyber": "standard"})),
            None
        ]
    );
    let handshakes = server.handshakes();
    assert_eq!(
        handshakes
            .iter()
            .map(|handshake| (handshake.header(ACTOR), handshake.header(ACCOUNT)))
            .collect::<Vec<_>>(),
        vec![(
            Some("verified-actor".to_owned()),
            Some("workspace-1".to_owned())
        )]
    );
    test.codex.shutdown_and_wait().await?;
    server.shutdown().await;
    Ok(())
}
