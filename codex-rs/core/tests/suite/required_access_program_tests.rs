//! Selected threads require an allowed program on every HTTP and WebSocket model request.

use anyhow::Result;
use codex_core::RequiredCyberAccessPrograms;
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
use test_case::test_case;

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

/// A Blue/Red startup requirement rejects Standard, omission, and unsupported providers.
#[test_case(None, true, None; "omitted")]
#[test_case(Some(CyberAccessProgram::Standard), true, None; "standard_rejected")]
#[test_case(Some(CyberAccessProgram::DaybreakBlue), true, Some("daybreak_blue"); "blue")]
#[test_case(Some(CyberAccessProgram::DaybreakRed), true, Some("daybreak_red"); "red")]
#[test_case(Some(CyberAccessProgram::DaybreakBlue), false, None; "unsupported_provider")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn required_actor_program_http(
    program: Option<CyberAccessProgram>,
    provider_enabled: bool,
    expected_wire_program: Option<&str>,
) -> Result<()> {
    core_test_support::skip_if_no_network!(Ok(()));
    let server = responses::start_mock_server().await;
    let requests = responses::mount_sse_once(&server, responses::sse_completed("resp-1")).await;
    let test = test_codex()
        .with_auth(actor_auth())
        .with_config(move |config| {
            config.model_provider_id = "local".to_owned();
            config
                .model_provider
                .capabilities
                .get_or_insert_default()
                .cyber_access_programs = Some(provider_enabled);
        })
        .build_with_auto_env(&server)
        .await?;
    let mut options = test.start_thread_options().await;
    options
        .thread_extension_init
        .insert(RequiredCyberAccessPrograms {
            allowed: vec![
                CyberAccessProgram::DaybreakBlue,
                CyberAccessProgram::DaybreakRed,
            ],
        });
    let thread = test.thread_manager.start_thread(options).await?.thread;
    thread
        .thread_extension_data()
        .remove::<RequiredCyberAccessPrograms>();
    thread
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "hello".into(),
                text_elements: Vec::new(),
            }])
            .on_start(TurnStartOptions {
                cyber_access_program: program,
                ..Default::default()
            }),
        )
        .await?;
    let event = wait_for_event(&thread, |event| {
        matches!(event, EventMsg::TurnComplete(_) | EventMsg::Error(_))
    })
    .await;
    if let Some(expected_wire_program) = expected_wire_program {
        assert!(matches!(event, EventMsg::TurnComplete(_)));
        assert_eq!(
            requests
                .single_request()
                .body_json()
                .get("access_programs")
                .cloned(),
            Some(json!({"cyber": expected_wire_program}))
        );
    } else {
        let EventMsg::Error(error) = event else {
            panic!("host requirement must refuse request")
        };
        assert!(
            error
                .message
                .contains("this thread requires an allowed explicit access program")
        );
        assert!(requests.requests().is_empty());
    }
    thread.shutdown_and_wait().await?;
    test.codex.shutdown_and_wait().await?;
    Ok(())
}

/// A Blue/Red requirement survives WebSocket reuse and rejects Standard or omission.
#[test_case(CyberAccessProgram::DaybreakBlue, "daybreak_blue", Some(CyberAccessProgram::Standard); "blue_then_standard")]
#[test_case(CyberAccessProgram::DaybreakRed, "daybreak_red", Some(CyberAccessProgram::Standard); "red_then_standard")]
#[test_case(CyberAccessProgram::DaybreakBlue, "daybreak_blue", None; "blue_then_omitted")]
#[test_case(CyberAccessProgram::DaybreakRed, "daybreak_red", None; "red_then_omitted")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn required_actor_program_websocket(
    program: CyberAccessProgram,
    expected_wire_program: &str,
    rejected_program: Option<CyberAccessProgram>,
) -> Result<()> {
    core_test_support::skip_if_no_network!(Ok(()));
    let server = responses::start_websocket_server(vec![
        vec![vec![responses::ev_completed("resp-warmup")]],
        vec![vec![responses::ev_completed("resp-allowed")]],
    ])
    .await;
    let base_url = format!("{}/v1", server.uri().replacen("ws://", "http://", 1));
    let mut test = test_codex()
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
    server
        .wait_for_request(/*connection_index*/ 0, /*request_index*/ 0)
        .await;
    let mut options = test.start_thread_options().await;
    options
        .thread_extension_init
        .insert(RequiredCyberAccessPrograms {
            allowed: vec![
                CyberAccessProgram::DaybreakBlue,
                CyberAccessProgram::DaybreakRed,
            ],
        });
    test.codex.shutdown_and_wait().await?;
    test.codex = test.thread_manager.start_thread(options).await?.thread;
    submit(&test, Some(program)).await?;
    let error = submit(&test, rejected_program)
        .await
        .expect_err("Standard or omission must not satisfy a Blue/Red requirement");
    assert!(
        error
            .to_string()
            .contains("this thread requires an allowed explicit access program")
    );
    assert_eq!(
        server
            .connections()
            .into_iter()
            .flatten()
            .map(|request| request.body_json().get("access_programs").cloned())
            .collect::<Vec<_>>(),
        vec![None, Some(json!({"cyber": expected_wire_program}))]
    );
    test.codex.shutdown_and_wait().await?;
    server.shutdown().await;
    Ok(())
}

/// A Standard-only host requirement accepts explicit Standard, never omission.
#[test_case(Some(CyberAccessProgram::Standard), Some("standard"); "standard")]
#[test_case(None, None; "omitted")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn required_actor_program_http_standard_only(
    program: Option<CyberAccessProgram>,
    expected_wire_program: Option<&str>,
) -> Result<()> {
    core_test_support::skip_if_no_network!(Ok(()));
    let server = responses::start_mock_server().await;
    let requests = responses::mount_sse_once(&server, responses::sse_completed("resp-1")).await;
    let mut test = test_codex()
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
    let mut options = test.start_thread_options().await;
    options
        .thread_extension_init
        .insert(RequiredCyberAccessPrograms {
            allowed: vec![CyberAccessProgram::Standard],
        });
    test.codex.shutdown_and_wait().await?;
    test.codex = test.thread_manager.start_thread(options).await?.thread;
    let result = submit(&test, program).await;
    if let Some(expected_wire_program) = expected_wire_program {
        result?;
        assert_eq!(
            requests
                .single_request()
                .body_json()
                .get("access_programs")
                .cloned(),
            Some(json!({"cyber": expected_wire_program}))
        );
    } else {
        let error = result.expect_err("omission must not silently select Standard");
        assert!(
            error
                .to_string()
                .contains("this thread requires an allowed explicit access program")
        );
        assert!(requests.requests().is_empty());
    }
    test.codex.shutdown_and_wait().await?;
    Ok(())
}

/// A Standard-only WebSocket requirement also distinguishes Standard from omission.
#[test_case(Some(CyberAccessProgram::Standard), Some("standard"); "standard")]
#[test_case(None, None; "omitted")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn required_actor_program_websocket_standard_only(
    program: Option<CyberAccessProgram>,
    expected_wire_program: Option<&str>,
) -> Result<()> {
    core_test_support::skip_if_no_network!(Ok(()));
    let server = responses::start_websocket_server(vec![
        vec![vec![responses::ev_completed("resp-warmup")]],
        vec![vec![responses::ev_completed("resp-allowed")]],
    ])
    .await;
    let base_url = format!("{}/v1", server.uri().replacen("ws://", "http://", 1));
    let mut test = test_codex()
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
    server
        .wait_for_request(/*connection_index*/ 0, /*request_index*/ 0)
        .await;
    let mut options = test.start_thread_options().await;
    options
        .thread_extension_init
        .insert(RequiredCyberAccessPrograms {
            allowed: vec![CyberAccessProgram::Standard],
        });
    test.codex.shutdown_and_wait().await?;
    test.codex = test.thread_manager.start_thread(options).await?.thread;
    let result = submit(&test, program).await;
    let mut expected_programs = vec![None];
    if let Some(expected_wire_program) = expected_wire_program {
        result?;
        expected_programs.push(Some(json!({"cyber": expected_wire_program})));
    } else {
        let error = result.expect_err("omission must not silently select Standard");
        assert!(
            error
                .to_string()
                .contains("this thread requires an allowed explicit access program")
        );
    }
    assert_eq!(
        server
            .connections()
            .into_iter()
            .flatten()
            .map(|request| request.body_json().get("access_programs").cloned())
            .collect::<Vec<_>>(),
        expected_programs
    );
    test.codex.shutdown_and_wait().await?;
    server.shutdown().await;
    Ok(())
}
