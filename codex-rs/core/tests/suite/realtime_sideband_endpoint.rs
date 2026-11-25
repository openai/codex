//! Verifies that existing-call sidebands honor trusted per-call endpoint overrides,
//! fall back to the configured endpoint, and keep host header authentication separate
//! from public runtime authentication.

use anyhow::Context;
use anyhow::Result;
use codex_features::Feature;
use codex_features::RuntimeAuthMode;
use codex_login::AuthHeaders;
use codex_login::CodexAuth;
use codex_protocol::protocol::CodexResponseHandoffMode;
use codex_protocol::protocol::ConversationStartParams;
use codex_protocol::protocol::ConversationStartTransport;
use codex_protocol::protocol::ConversationTextParams;
use codex_protocol::protocol::ConversationTextRole;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::RealtimeAttachmentOptions;
use codex_protocol::protocol::RealtimeConversationVersion;
use codex_protocol::protocol::RealtimeEvent;
use codex_protocol::protocol::RealtimeExistingCallAuth;
use codex_protocol::protocol::RealtimeOutputModality;
use core_test_support::responses::start_mock_server;
use core_test_support::responses::start_websocket_server;
use core_test_support::runtime_auth::PROBE_SERVER;
use core_test_support::runtime_auth::run_harness_through_proxy_until;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_match;
use futures::SinkExt;
use futures::StreamExt;
use http::HeaderMap;
use http::HeaderName;
use http::HeaderValue;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::time::Duration;
use test_case::test_case;
use tokio::sync::oneshot;
use tokio::time::timeout;
use tokio_tungstenite::accept_hdr_async;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::handshake::server::Request;
use tokio_tungstenite::tungstenite::handshake::server::Response;
use tokio_util::task::AbortOnDropHandle;
use wiremock::Mock;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

const OBSERVER_URL: &str = "https://observer.internal.api.openai.org/v1";

fn observer_params(sideband_base_url: Option<String>) -> ConversationStartParams {
    ConversationStartParams {
        client_managed_handoffs: false,
        delegation_ack_filler: None,
        flush_transcript_tail_on_session_end: false,
        codex_responses_as_items: false,
        codex_response_item_prefix: None,
        codex_response_handoff_mode: CodexResponseHandoffMode::Thinking,
        backend_reasoning_status: false,
        codex_response_handoff_channel_prefixes: None,
        model: None,
        output_modality: RealtimeOutputModality::Audio,
        include_startup_context: false,
        initial_items: Vec::new(),
        realtime_start_instructions: None,
        realtime_end_instructions: None,
        prompt: None,
        realtime_session_id: Some("observer-session".into()),
        voice_session_id: None,
        transport: Some(ConversationStartTransport::ExistingCall {
            call_id: "rtc_existing".into(),
            sideband_base_url,
        }),
        version: Some(RealtimeConversationVersion::V3),
    }
}

fn observer_auth() -> CodexAuth {
    CodexAuth::Headers(AuthHeaders::new(HeaderMap::from_iter([(
        HeaderName::from_static("x-openai-actor-authorization"),
        HeaderValue::from_static("Bearer synthetic-owner-biscuit"),
    )])))
}

/// Private observers and public Realtime retain distinct auth through events, reconnect, and stop.
#[test_case(true; "private observer")]
#[test_case(false; "public realtime")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn existing_call_runtime_auth_reconnects_and_stops(external_headers: bool) -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    if std::env::var(PROBE_SERVER).is_ok() {
        let test = test_codex()
            .with_auth(if external_headers {
                observer_auth()
            } else {
                CodexAuth::from_api_key("public-key")
            })
            .with_config(move |config| {
                config
                    .features
                    .enable(Feature::RuntimeAgentInstances)
                    .expect("enable runtime authentication");
                config.runtime_auth_mode = RuntimeAuthMode::DualHeader;
                if external_headers {
                    config.model_provider.experimental_bearer_token =
                        Some("inference-only-key".into());
                    config.model_provider.http_headers = Some(
                        [
                            ("authorization".into(), "Bearer configured-model-key".into()),
                            (
                                "x-openai-agent-assertion".into(),
                                "configured-model-assertion".into(),
                            ),
                        ]
                        .into(),
                    );
                }
            })
            .build_with_auto_env(&server)
            .await?;
        let (reply, result) = oneshot::channel();
        test.codex
            .submit(Op::RealtimeConversationAttach {
                params: observer_params(Some(if external_headers {
                    OBSERVER_URL.into()
                } else {
                    "https://chatgpt.com/v1".into()
                })),
                options: RealtimeAttachmentOptions {
                    existing_call_auth: if external_headers {
                        RealtimeExistingCallAuth::ExternalHeaders
                    } else {
                        RealtimeExistingCallAuth::default()
                    },
                    ..Default::default()
                },
                reply,
            })
            .await?;
        timeout(Duration::from_secs(10), result).await???;
        for text in ["first connection", "reconnected"] {
            wait_for_event_match(&test.codex, |event| match event {
                EventMsg::RealtimeConversationRealtime(event) => match &event.payload {
                    RealtimeEvent::OutputTranscriptDelta(delta) if delta.delta == text => {
                        Some(Ok(()))
                    }
                    RealtimeEvent::Error(message) => Some(Err(anyhow::anyhow!("{message}"))),
                    _ => None,
                },
                EventMsg::RealtimeConversationClosed(event) => {
                    Some(Err(anyhow::anyhow!("unexpected close: {event:?}")))
                }
                EventMsg::Error(error) => Some(Err(anyhow::anyhow!("{error:?}"))),
                _ => None,
            })
            .await?;
            test.codex
                .submit(Op::RealtimeConversationText(ConversationTextParams {
                    text: text.into(),
                    role: ConversationTextRole::User,
                }))
                .await?;
        }
        wait_for_event(&test.codex, |event| matches!(event,
            EventMsg::RealtimeConversationRealtime(event)
                if matches!(&event.payload, RealtimeEvent::OutputTranscriptDelta(delta) if delta.delta == "stop ready")
        )).await;
        let (reply, result) = oneshot::channel();
        test.codex
            .submit(Op::RealtimeConversationDetach {
                realtime_session_id: "observer-session".into(),
                reply,
            })
            .await?;
        timeout(Duration::from_secs(10), result).await???;
        test.codex.shutdown_and_wait().await?;
        return Ok(());
    }

    if !external_headers {
        Mock::given(method("POST"))
            .and(path("/agent/register"))
            .respond_with(|request: &wiremock::Request| {
                let body: serde_json::Value = request.body_json().expect("parse registration body");
                assert_eq!(body["actor_auth"]["credential"], "public-key");
                ResponseTemplate::new(200).set_body_json(json!({
                    "runtime_agent_instance_id": request.headers["idempotency-key"].to_str().expect("registration idempotency key is text"),
                    "agent_host_id": "host-a",
                    "actor": {"kind": "platform", "user_id": "user-a", "organization_id": "org-a", "project_id": null},
                }))
            })
            .expect(1)
            .mount(&server)
            .await;
    }
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let observer = AbortOnDropHandle::new(tokio::spawn(async move {
        for (index, text) in ["first connection", "reconnected"].into_iter().enumerate() {
            let (stream, _) = listener.accept().await?;
            let mut websocket =
                accept_hdr_async(stream, |request: &Request, response: Response| {
                    assert_eq!(request.uri().to_string(), "/v1/live/rtc_existing");
                    if external_headers {
                        assert_eq!(
                            request.headers().get("x-openai-actor-authorization"),
                            Some(&HeaderValue::from_static("Bearer synthetic-owner-biscuit"))
                        );
                        assert_eq!(request.headers().get("authorization"), None);
                        assert_eq!(request.headers().get("x-openai-agent-assertion"), None);
                    } else {
                        assert_eq!(request.headers().get("x-openai-actor-authorization"), None);
                        assert_eq!(request.headers()["authorization"], "Bearer public-key");
                        assert!(request.headers().contains_key("x-openai-agent-assertion"));
                    }
                    Ok(response)
                })
                .await?;
            websocket
                .send(Message::Text(
                    json!({
                        "type": "output_transcript.added", "item": { "text": text }
                    })
                    .to_string()
                    .into(),
                ))
                .await?;
            loop {
                let message = websocket
                    .next()
                    .await
                    .context("observer disconnected before text")??;
                if message.is_text() && message.to_text()?.contains(text) {
                    break;
                }
            }
            if index == 1 {
                websocket
                    .send(Message::Text(
                        json!({
                            "type": "output_transcript.added", "item": { "text": "stop ready" }
                        })
                        .to_string()
                        .into(),
                    ))
                    .await?;
                assert!(matches!(
                    websocket.next().await,
                    Some(Ok(Message::Close(_)))
                ));
            }
            // Dropping the first socket without a close frame exercises Core's reconnect path.
        }
        Ok::<_, anyhow::Error>(())
    }));
    let case = if external_headers {
        "private_observer"
    } else {
        "public_realtime"
    };
    run_harness_through_proxy_until(
        &server,
        &format!("suite::realtime_sideband_endpoint::existing_call_runtime_auth_reconnects_and_stops::{case}"),
        Some(address),
        &[],
        async move {
            timeout(Duration::from_secs(10), observer)
                .await
                .context("observer did not receive the WebSocket close frame")???;
            Ok(())
        },
    )
    .await?;
    assert_eq!(
        server
            .received_requests()
            .await
            .expect("request recording is enabled")
            .iter()
            .filter(|request| request.url.path() == "/agent/register")
            .count(),
        usize::from(!external_headers),
    );
    Ok(())
}

#[derive(Clone, Copy)]
enum RejectedObserver {
    ModelProvider,
    ApiKey,
    MissingUrl,
    NetworkPolicy,
}

/// External header admission cannot be inferred from a URL or bypass auth-kind and network policy checks.
#[test_case(RejectedObserver::ModelProvider; "untrusted destination")]
#[test_case(RejectedObserver::ApiKey; "wrong authentication kind")]
#[test_case(RejectedObserver::MissingUrl; "missing explicit route")]
#[test_case(RejectedObserver::NetworkPolicy; "destination policy denial")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn observer_authentication_fails_closed(case: RejectedObserver) -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let observer = start_websocket_server(vec![vec![]]).await;
    let controller = codex_http_client::NetworkPolicyController::default();
    let policy = controller.policy();
    controller.publish(
        policy.revision(),
        codex_http_client::DestinationPolicy::Unrestricted,
    );
    let auth = match case {
        RejectedObserver::ApiKey => CodexAuth::from_api_key("not-host-headers"),
        _ => observer_auth(),
    };
    let test = test_codex()
        .with_auth(auth)
        .with_config(move |config| {
            config
                .features
                .enable(Feature::RuntimeAgentInstances)
                .expect("enable runtime authentication");
            config.runtime_auth_mode = RuntimeAuthMode::DualHeader;
            if matches!(case, RejectedObserver::NetworkPolicy) {
                config.application_network_policy = policy;
            }
        })
        .build_with_auto_env(&server)
        .await?;
    if matches!(case, RejectedObserver::NetworkPolicy) {
        controller.publish(
            controller.policy().revision(),
            codex_http_client::DestinationPolicy::Restricted {
                allowed_hosts: Default::default(),
            },
        );
    }
    let (url, auth, expected) = match case {
        RejectedObserver::ModelProvider => (
            Some(OBSERVER_URL.into()),
            RealtimeExistingCallAuth::ModelProvider,
            "runtime auth does not support this OpenAI endpoint",
        ),
        RejectedObserver::ApiKey => (
            Some(observer.uri().into()),
            RealtimeExistingCallAuth::ExternalHeaders,
            "external header authentication requires header-based authentication",
        ),
        RejectedObserver::MissingUrl => (
            None,
            RealtimeExistingCallAuth::ExternalHeaders,
            "external header authentication requires an existing call with an explicit sideband URL",
        ),
        RejectedObserver::NetworkPolicy => (
            Some(observer.uri().into()),
            RealtimeExistingCallAuth::ExternalHeaders,
            "policy",
        ),
    };
    let (reply, result) = oneshot::channel();
    test.codex
        .submit(Op::RealtimeConversationAttach {
            params: observer_params(url),
            options: RealtimeAttachmentOptions {
                existing_call_auth: auth,
                ..Default::default()
            },
            reply,
        })
        .await?;
    let error = timeout(Duration::from_secs(10), result)
        .await??
        .expect_err("invalid observer authentication must fail")
        .to_string();
    assert!(error.contains(expected), "{error}");
    assert!(observer.handshakes().is_empty());
    test.codex.shutdown_and_wait().await?;
    observer.shutdown().await;
    Ok(())
}

#[derive(Clone, Copy)]
enum EndpointSelection {
    Configured,
    PerCall,
    Revoked,
}

#[test_case(EndpointSelection::Configured; "configured endpoint")]
#[test_case(EndpointSelection::PerCall; "per-call endpoint")]
#[test_case(EndpointSelection::Revoked; "policy revocation")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn existing_call_uses_selected_endpoint_and_runtime_auth(
    selection: EndpointSelection,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let events = vec![vec![vec![json!({
        "type": "session.started",
        "session": { "id": "rtc_existing" }
    })]]];
    let configured_endpoint = start_websocket_server(events.clone()).await;
    let call_endpoint = start_websocket_server(events).await;
    let configured_base_url = configured_endpoint.uri().to_string();
    let controller = codex_http_client::NetworkPolicyController::default();
    let policy = controller.policy();
    controller.publish(
        policy.revision(),
        codex_http_client::DestinationPolicy::Unrestricted,
    );
    let mut headers = HeaderMap::new();
    headers.insert(
        "x-runtime-auth",
        HeaderValue::from_static("runtime-credential"),
    );
    let mut builder = test_codex()
        .with_auth(CodexAuth::Headers(AuthHeaders::new(headers)))
        .with_config(move |config| {
            config.experimental_realtime_ws_base_url = Some(configured_base_url);
            if matches!(selection, EndpointSelection::Revoked) {
                config.application_network_policy = policy;
            }
        });
    let test = builder.build_with_auto_env(&server).await?;

    test.codex
        .submit(Op::RealtimeConversationStart(ConversationStartParams {
            client_managed_handoffs: false,
            delegation_ack_filler: None,
            flush_transcript_tail_on_session_end: false,
            codex_responses_as_items: false,
            codex_response_item_prefix: None,
            codex_response_handoff_mode: CodexResponseHandoffMode::Thinking,
            backend_reasoning_status: false,
            codex_response_handoff_channel_prefixes: None,
            model: None,
            output_modality: RealtimeOutputModality::Audio,
            include_startup_context: false,
            initial_items: Vec::new(),
            realtime_start_instructions: None,
            realtime_end_instructions: None,
            prompt: None,
            realtime_session_id: None,
            voice_session_id: None,
            transport: Some(ConversationStartTransport::ExistingCall {
                call_id: "rtc_existing".to_string(),
                sideband_base_url: match selection {
                    EndpointSelection::Configured | EndpointSelection::Revoked => None,
                    EndpointSelection::PerCall => Some(call_endpoint.uri().to_string()),
                },
            }),
            version: Some(RealtimeConversationVersion::V3),
        }))
        .await?;

    wait_for_event_match(&test.codex, |event| match event {
        EventMsg::RealtimeConversationStarted(_) => Some(Ok(())),
        EventMsg::Error(error) => Some(Err(anyhow::anyhow!("{error:?}"))),
        _ => None,
    })
    .await?;

    let (selected, unused) = match selection {
        EndpointSelection::Configured | EndpointSelection::Revoked => {
            (&configured_endpoint, &call_endpoint)
        }
        EndpointSelection::PerCall => (&call_endpoint, &configured_endpoint),
    };
    let handshake = selected.single_handshake();
    assert_eq!(
        (
            handshake.uri(),
            handshake.header("x-runtime-auth"),
            handshake.header("authorization"),
        ),
        (
            "/v1/live/rtc_existing",
            Some("runtime-credential".to_string()),
            None,
        ),
    );
    assert!(unused.handshakes().is_empty());

    if matches!(selection, EndpointSelection::Revoked) {
        controller.publish(
            controller.policy().revision(),
            codex_http_client::DestinationPolicy::Restricted {
                allowed_hosts: Default::default(),
            },
        );
        let error = wait_for_event_match(&test.codex, |event| match event {
            EventMsg::RealtimeConversationRealtime(event) => match &event.payload {
                codex_protocol::protocol::RealtimeEvent::Error(message) => Some(message.clone()),
                _ => None,
            },
            _ => None,
        })
        .await;
        assert!(error.contains("revoked"), "{error}");
        // Recovery must not revive a sideband whose denial was reported as terminal.
        controller.publish(
            controller.policy().revision(),
            codex_http_client::DestinationPolicy::Unrestricted,
        );
        assert!(
            !selected
                .wait_for_handshakes(
                    /*expected*/ 2,
                    std::time::Duration::from_secs(/*secs*/ 1)
                )
                .await
        );
        assert_eq!(selected.handshakes().len(), 1);
    }

    test.codex.submit(Op::RealtimeConversationClose).await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::RealtimeConversationClosed(_))
    })
    .await;
    test.codex.shutdown_and_wait().await?;
    configured_endpoint.shutdown().await;
    call_endpoint.shutdown().await;
    Ok(())
}
