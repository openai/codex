use anyhow::Result;
use codex_core::TurnInputRequest;
use codex_model_provider_info::WireApi;
use codex_models_manager::bundled_models_response;
use codex_protocol::config_types::CollaborationMode;
use codex_protocol::config_types::ModeKind;
use codex_protocol::config_types::Settings;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::user_input::UserInput;
use core_test_support::TempDirExt;
use core_test_support::responses;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::local_requests;
use core_test_support::test_codex::test_codex;
use core_test_support::test_codex::turn_permission_fields;
use core_test_support::tracing::NamedTraceEvents;
use pretty_assertions::assert_eq;
use std::net::SocketAddr;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::sync::oneshot;
use tokio::task::JoinSet;
use tokio::time::Duration;
use tokio::time::timeout;
use wiremock::Mock;
use wiremock::ResponseTemplate;
use wiremock::http::Method;
use wiremock::matchers::method;
use wiremock::matchers::path_regex;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn websocket_fallback_switches_to_http_on_upgrade_required_connect() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    Mock::given(method("GET"))
        .and(path_regex(".*/responses$"))
        .respond_with(ResponseTemplate::new(426))
        .mount(&server)
        .await;

    let response_mock = mount_sse_sequence(
        &server,
        vec![
            responses::sse_completed("first"),
            responses::sse_completed("second"),
        ],
    )
    .await;

    let mut builder = test_codex().with_config({
        let base_url = format!("{}/v1", server.uri());
        move |config| {
            config.model_provider.base_url = Some(base_url);
            config.model_provider.wire_api = WireApi::Responses;
            config.model_provider.supports_websockets = true;
            // If we don't treat 426 specially, the sampling loop would retry the WebSocket
            // handshake before switching to the HTTP transport.
            config.model_provider.stream_max_retries = Some(2);
            config.model_provider.request_max_retries = Some(0);
        }
    });
    let test = builder.build(&server).await?;

    test.submit_turn("first").await?;
    test.submit_turn("second").await?;

    let requests = server.received_requests().await.unwrap_or_default();
    let websocket_attempts = requests
        .iter()
        .filter(|req| req.method == Method::GET && req.url.path().ends_with("/responses"))
        .count();
    let http_attempts = requests
        .iter()
        .filter(|req| req.method == Method::POST && req.url.path().ends_with("/responses"))
        .count();

    // The startup prewarm request sees 426 and immediately switches the session to HTTP fallback,
    // so both turns use HTTP without scheduling another upgrade.
    assert_eq!(websocket_attempts, 1);
    assert_eq!(http_attempts, 2);
    assert_eq!(response_mock.requests().len(), 2);

    Ok(())
}

#[tokio::test]
async fn websocket_fallback_switches_to_http_after_retries_exhausted() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let mut probe = NamedTraceEvents::subscribe("codex.websocket_probe_finished");
    let server = responses::start_mock_server().await;
    let response_mock = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;

    let mut builder = test_codex().with_config({
        let base_url = format!("{}/v1", server.uri());
        move |config| {
            config.model_provider.base_url = Some(base_url);
            config.model_provider.wire_api = WireApi::Responses;
            config.model_provider.supports_websockets = true;
            config.model_provider.stream_max_retries = Some(2);
            config.model_provider.request_max_retries = Some(0);
        }
    });
    let test = builder.build(&server).await?;

    test.submit_turn("hello").await?;
    probe.next().await?;

    let requests = server.received_requests().await.unwrap_or_default();
    let websocket_attempts = requests
        .iter()
        .filter(|req| req.method == Method::GET && req.url.path().ends_with("/responses"))
        .count();
    let http_attempts = requests
        .iter()
        .filter(|req| req.method == Method::POST && req.url.path().ends_with("/responses"))
        .count();

    // One startup prewarm, three foreground attempts and one background probe.
    assert_eq!(websocket_attempts, 5);
    assert_eq!(http_attempts, 1);
    assert_eq!(response_mock.requests().len(), 1);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn websocket_fallback_hides_first_websocket_retry_stream_error() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let response_mock = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;

    let mut builder = test_codex().with_config({
        let base_url = format!("{}/v1", server.uri());
        move |config| {
            config.model_provider.base_url = Some(base_url);
            config.model_provider.wire_api = WireApi::Responses;
            config.model_provider.supports_websockets = true;
            config.model_provider.stream_max_retries = Some(2);
            config.model_provider.request_max_retries = Some(0);
        }
    });
    let TestCodex {
        codex,
        session_configured,
        cwd,
        ..
    } = builder.build(&server).await?;
    let (sandbox_policy, permission_profile) =
        turn_permission_fields(PermissionProfile::Disabled, cwd.path());

    codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "hello".into(),
                text_elements: Vec::new(),
            }])
            .with_thread_settings(ThreadSettingsOverrides {
                environments: Some(local_requests(cwd.abs())),
                approval_policy: Some(AskForApproval::Never),
                sandbox_policy: Some(sandbox_policy),
                permission_profile,
                collaboration_mode: Some(CollaborationMode {
                    mode: ModeKind::Default,
                    settings: Settings {
                        model: session_configured.model.clone(),
                        reasoning_effort: None,
                        developer_instructions: None,
                    },
                }),
                ..Default::default()
            }),
        )
        .await?;

    let mut stream_error_messages = Vec::new();
    loop {
        let event = timeout(Duration::from_secs(10), codex.next_event())
            .await
            .expect("timeout waiting for event")
            .expect("event stream ended unexpectedly")
            .msg;
        match event {
            EventMsg::StreamError(e) => stream_error_messages.push(e.message),
            EventMsg::TurnComplete(_) => break,
            _ => {}
        }
    }

    let expected_stream_errors = if cfg!(debug_assertions) {
        vec!["Reconnecting... 1/2", "Reconnecting... 2/2"]
    } else {
        vec!["Reconnecting... 2/2"]
    };
    assert_eq!(stream_error_messages, expected_stream_errors);
    assert_eq!(response_mock.requests().len(), 1);

    Ok(())
}

#[derive(Clone, Copy)]
enum RecoveredRequest {
    Completes,
    ClosesBeforeOutput,
    ClosesAfterOutput,
}

#[test_case::test_case(RecoveredRequest::Completes; "recovered_request_completes")]
#[test_case::test_case(RecoveredRequest::ClosesBeforeOutput; "recovered_request_closes_before_output")]
#[test_case::test_case(RecoveredRequest::ClosesAfterOutput; "recovered_request_closes_after_output")]
#[tokio::test]
async fn http_retries_finish_before_same_turn_websocket_recovery(
    outcome: RecoveredRequest,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let mut probe = NamedTraceEvents::subscribe("codex.websocket_probe_finished");
    let http = responses::start_mock_server().await;
    let mut http_responses = vec![
        sse(vec![ev_response_created("interrupted")]),
        sse(vec![
            ev_response_created("replay"),
            responses::ev_function_call("http-call", "update_plan", r#"{"plan":[]}"#),
            ev_completed("replay"),
        ]),
    ];
    let recovered = match outcome {
        RecoveredRequest::Completes => vec![
            vec![
                ev_response_created("recovered"),
                responses::ev_function_call("ws-call", "update_plan", r#"{"plan":[]}"#),
                ev_completed("recovered"),
            ],
            vec![ev_response_created("final"), ev_completed("final")],
        ],
        RecoveredRequest::ClosesBeforeOutput => {
            // The rejected socket must leave the full HTTP retry budget available.
            http_responses.push(sse(vec![ev_response_created("interrupted-replay")]));
            http_responses.push(responses::sse_completed("final"));
            vec![vec![ev_response_created("rejected")]]
        }
        RecoveredRequest::ClosesAfterOutput => {
            http_responses.push(sse(vec![ev_response_created("interrupted-replay")]));
            vec![vec![
                ev_response_created("partial"),
                responses::ev_message_item_added("message", ""),
                responses::ev_output_text_delta("Partial answer"),
            ]]
        }
    };
    let http_requests = mount_sse_sequence(&http, http_responses).await;
    // Close the first two foreground streams before response.completed. The third
    // socket is the probe and must stay idle until the HTTP tool result is ready.
    let websocket = responses::start_websocket_server(vec![
        vec![
            vec![ev_response_created("warmup"), ev_completed("warmup")],
            vec![ev_response_created("failed")],
        ],
        vec![vec![ev_response_created("failed-again")]],
        recovered,
        // A client that retries the rejected request over WebSocket will be caught below.
        vec![vec![
            ev_response_created("unexpected"),
            ev_completed("unexpected"),
        ]],
    ])
    .await;
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let base_url = format!("http://{}/v1", listener.local_addr()?);
    let (release_http, first_replay) = oneshot::channel();
    let ws = websocket.uri().trim_start_matches("ws://").parse()?;
    let mut tasks = JoinSet::new();
    tasks.spawn(serve_recovering_endpoint(
        listener,
        *http.address(),
        ws,
        first_replay,
    ));
    let test = test_codex()
        .with_config(move |config| {
            config.update_plan_enabled = true;
            config.model_catalog = Some(bundled_models_response().unwrap());
            config.model_provider.base_url = Some(base_url);
            config.model_provider.supports_websockets = true;
            config.model_provider.stream_max_retries = Some(1);
            config.model_provider.request_max_retries = Some(0);
        })
        .build_with_auto_env(&http)
        .await?;

    tokio::try_join!(test.submit_turn("Make a plan"), async {
        probe.next().await?;
        release_http
            .send(())
            .map_err(|()| anyhow::anyhow!("HTTP replay closed"))
    })?;

    let connections = websocket.connections();
    let [_, _, recovered] = connections.as_slice() else {
        anyhow::bail!("expected two failed sockets and one recovered socket");
    };
    let (full, rest) = recovered
        .split_first()
        .expect("first request on probe socket");
    let full = full.body_json();
    assert!(full.get("previous_response_id").is_none());
    let input = full["input"].as_array().expect("full request input");
    assert!(
        input
            .iter()
            .any(|item| item["content"][0]["text"] == "Make a plan")
    );
    assert!(
        input
            .iter()
            .any(|item| item["type"] == "function_call_output" && item["call_id"] == "http-call")
    );

    if matches!(outcome, RecoveredRequest::ClosesAfterOutput) {
        assert!(rest.is_empty());
        // A disconnect after visible output spends the one ordinary retry. Count even
        // unmatched requests so a second HTTP replay cannot escape the assertion.
        let posts = http
            .received_requests()
            .await
            .expect("mock records requests")
            .iter()
            .filter(|req| req.method == Method::POST && req.url.path().ends_with("/responses"))
            .count();
        assert_eq!(posts, 3);
        return Ok(());
    }
    let http = http_requests.requests();
    if matches!(outcome, RecoveredRequest::ClosesBeforeOutput) {
        assert!(rest.is_empty());
        let [_, _, replay, retry] = http.as_slice() else {
            anyhow::bail!("expected the rejected request and its retry over HTTP");
        };
        for request in [replay, retry] {
            assert!(
                request
                    .inputs_of_type("function_call_output")
                    .iter()
                    .any(|item| item["call_id"] == "http-call")
            );
        }
        return Ok(());
    }

    assert_eq!(http.len(), 2);
    let [delta] = rest else {
        anyhow::bail!("expected one incremental request after recovery");
    };
    let delta = delta.body_json();
    assert_eq!(delta["previous_response_id"], "recovered");
    let output_ids = delta["input"]
        .as_array()
        .expect("incremental input")
        .iter()
        .filter(|item| item["type"] == "function_call_output")
        .map(|item| item["call_id"].as_str())
        .collect::<Vec<_>>();
    assert_eq!(output_ids, [Some("ws-call")]);
    Ok(())
}

// Both fixtures share one origin. Release the interrupted HTTP replay only after
// the client has published the probe result, so its retry must pass over a ready socket.
async fn serve_recovering_endpoint(
    listener: TcpListener,
    http: SocketAddr,
    websocket: SocketAddr,
    first_replay: oneshot::Receiver<()>,
) -> Result<()> {
    let mut first_replay = Some(first_replay);
    let mut connections = JoinSet::new();
    loop {
        let (stream, _) = listener.accept().await?;
        let mut incoming = BufReader::new(stream);
        let mut line = String::new();
        incoming.read_line(&mut line).await?;
        let (upstream, gate) = if line.starts_with("GET /v1/responses ") {
            (websocket, None)
        } else if line.starts_with("POST /v1/responses ") {
            (http, first_replay.take())
        } else {
            incoming
                .get_mut()
                .write_all(
                    b"HTTP/1.1 404 Not Found\r\nConnection: close\r\nContent-Length: 0\r\n\r\n",
                )
                .await?;
            continue;
        };
        connections.spawn(async move {
            if let Some(gate) = gate {
                let _ = gate.await;
            }
            let mut upstream = TcpStream::connect(upstream).await?;
            upstream.write_all(line.as_bytes()).await?;
            tokio::io::copy_bidirectional(&mut incoming, &mut upstream).await
        });
    }
}
