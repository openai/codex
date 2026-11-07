//! Exercise runtime assertions and turn/socket lifetimes through real model transports.

use super::*;
use base64::Engine;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::ModelRequestInput;
use codex_extension_api::ModelResponseInterceptor;
use codex_features::RuntimeAuthMode;
use codex_login::ExternalAuth;
use codex_login::ExternalAuthFuture;
use codex_login::ExternalAuthRefreshContext;
use codex_model_provider_info::GatewayOAuthConfig;
use codex_model_provider_info::GatewayOAuthDelivery;
use codex_model_provider_info::GatewayOAuthSettings;
use codex_runtime_agent_instance::AgentHostAuthorization;
use codex_runtime_agent_instance::RuntimeAuthManager;
use codex_runtime_agent_instance::RuntimeRegistrationMetadata;
use codex_runtime_agent_instance::RuntimeRole;
use core_test_support::responses;
use pretty_assertions::assert_eq;
use serde_json::Value;
use std::collections::VecDeque;
use std::sync::Mutex;
use std::sync::RwLock;
use test_case::test_case;
use tokio::sync::Barrier;
use uuid::Uuid;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::Request;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

struct Credentials(RwLock<CodexAuth>);

#[derive(Debug, Default)]
struct BudgetChecks(Mutex<VecDeque<Option<Arc<Barrier>>>>);

impl ModelRequestContributor for BudgetChecks {
    fn budget_exhausted<'a>(&'a self, _thread_id: &'a str) -> ExtensionFuture<'a, bool> {
        let gate = self.0.lock().unwrap().pop_front().flatten();
        Box::pin(async move {
            if let Some(gate) = gate {
                gate.wait().await; // Signal that Core reached this check.
                gate.wait().await; // Wait for the account switch.
            }
            false
        })
    }

    fn request(&self, _: ModelRequestInput<'_>) -> Option<Box<dyn ModelResponseInterceptor>> {
        None
    }
}

impl ExternalAuth for Credentials {
    fn resolve(&self) -> ExternalAuthFuture<'_, CodexAuth> {
        Box::pin(async { Ok(self.0.read().unwrap().clone()) })
    }

    fn refresh(&self, _: ExternalAuthRefreshContext) -> ExternalAuthFuture<'_, CodexAuth> {
        self.resolve()
    }
}

fn registered(request: &Request) -> ResponseTemplate {
    let body: Value = request.body_json().unwrap();
    let user = if body["actor_auth"]["credential"] == "key-other-user" {
        "other-user"
    } else {
        "user-a"
    };
    ResponseTemplate::new(200).set_body_json(json!({
        "runtime_agent_instance_id": request.headers["idempotency-key"].to_str().unwrap(),
        "agent_host_id": "host-a",
        "actor": {"kind": "platform", "user_id": user, "organization_id": "org-a", "project_id": "project-a"},
    }))
}

async fn runtime_harness(
    registration: &MockServer,
    mut provider: ModelProviderInfo,
    mode: RuntimeAuthMode,
    request_contributors: Vec<Arc<dyn ModelRequestContributor>>,
) -> (WebsocketTestHarness, Arc<Credentials>) {
    provider.requires_openai_auth = true;
    let initial = CodexAuth::from_api_key("key-a");
    let registration_url = format!("{}/register", registration.uri()).parse().unwrap();
    let harness = websocket_harness_with_provider_factory(
        provider,
        /*runtime_metrics_enabled*/ false,
        /*concurrent_reasoning_summaries_enabled*/ false,
        &[],
        Some(initial.clone()),
        request_contributors,
        move |provider, credentials| {
            let credentials = credentials.expect("runtime fixture requires credentials");
            let runtime = Arc::new(RuntimeAuthManager::new(
                Arc::clone(&credentials),
                AgentHostAuthorization::InstallationId {
                    installation_id: Uuid::nil(),
                },
                RuntimeRegistrationMetadata {
                    harness: Some("codex".into()),
                    harness_version: None,
                    role: RuntimeRole::OrchestratorAndExecutor,
                },
                registration_url,
            ));
            codex_model_provider::test_support::runtime_model_provider(
                provider,
                credentials,
                (runtime, mode),
            )
        },
    )
    .await;
    let credentials = Arc::new(Credentials(RwLock::new(initial)));
    harness
        .auth_manager
        .set_external_auth(credentials.clone())
        .await
        .unwrap();
    (harness, credentials)
}

fn gateway_config(server: &MockServer, delivery: GatewayOAuthDelivery) -> GatewayOAuthConfig {
    GatewayOAuthConfig::OAuth(GatewayOAuthSettings {
        authorization_url: format!("{}/authorize", server.uri()),
        device_authorization_url: None,
        token_url: format!("{}/token", server.uri()),
        client_id: "client".into(),
        resource: None,
        scopes: vec![],
        redirect_port: None,
        credentials_store: Default::default(),
        delivery,
    })
}

fn assertion_payload(
    authorization: &str,
    assertion: Option<String>,
    mode: RuntimeAuthMode,
    credential: &str,
) -> Value {
    let assertion = match mode {
        RuntimeAuthMode::DualHeader => {
            assert_eq!(authorization, format!("Bearer {credential}"));
            assertion.expect("dual-header assertion")
        }
        RuntimeAuthMode::AssertionOnly => {
            assert!(assertion.is_none());
            assert!(!authorization.contains(credential));
            authorization
                .strip_prefix("AgentAssertion ")
                .unwrap()
                .to_string()
        }
    };
    let parts: Vec<_> = assertion.split('.').collect();
    assert_eq!(parts.len(), 4);
    assert_eq!(parts[0], "1");
    serde_json::from_slice(
        &base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(parts[2])
            .unwrap(),
    )
    .unwrap()
}

async fn complete(
    session: &mut ModelClientSession,
    harness: &WebsocketTestHarness,
) -> codex_protocol::error::Result<()> {
    let prompt = prompt_with_input(vec![message_item("hello")]);
    let mut stream = session
        .stream(
            &prompt,
            &harness.model_info,
            &harness.session_telemetry,
            harness.effort.clone(),
            harness.summary,
            /*service_tier*/ None,
            &turn_metadata(harness, /*turn_id*/ None),
            &InferenceTraceContext::disabled(),
        )
        .await?;
    while let Some(event) = stream.next().await {
        if matches!(event?, ResponseEvent::Completed { .. }) {
            return Ok(());
        }
    }
    panic!("model stream ended before completion");
}

#[test_case(RuntimeAuthMode::DualHeader, false; "dual_header")]
#[test_case(RuntimeAuthMode::AssertionOnly, false; "assertion_only")]
#[test_case(RuntimeAuthMode::DualHeader, true; "dual_header_with_gateway")]
#[test_case(RuntimeAuthMode::AssertionOnly, true; "assertion_only_with_gateway")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn http_requests_keep_matching_runtime_and_credentials_across_retries_and_account_changes(
    mode: RuntimeAuthMode,
    gateway: bool,
) {
    skip_if_no_network!();
    let server = MockServer::start().await;
    Mock::given(path("/register"))
        .respond_with(registered)
        .expect(2)
        .mount(&server)
        .await;
    Mock::given(path("/v1/responses"))
        .respond_with(ResponseTemplate::new(503))
        .up_to_n_times(1)
        .with_priority(1)
        .mount(&server)
        .await;
    Mock::given(path("/v1/responses"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_string(responses::sse(vec![
                    ev_response_created("resp-1"),
                    ev_completed("resp-1"),
                ])),
        )
        .mount(&server)
        .await;
    let mut provider =
        ModelProviderInfo::create_openai_provider(Some(format!("{}/v1", server.uri())));
    provider.supports_websockets = false;
    provider.http_headers = Some(std::collections::HashMap::from([
        ("openai-organization".into(), "org-a".into()),
        ("openai-project".into(), "project-a".into()),
    ]));
    provider.request_max_retries = Some(1);
    if gateway {
        provider.gateway_oauth = Some(gateway_config(
            &server,
            GatewayOAuthDelivery::Header {
                name: "x-gateway-auth".into(),
                scheme: "Bearer".into(),
            },
        ));
    }
    let (harness, credentials) = runtime_harness(&server, provider, mode, Vec::new()).await;
    let mut session = harness.client.new_session();
    complete(&mut session, &harness).await.unwrap();
    *credentials.0.write().unwrap() = CodexAuth::from_api_key("key-other-user");
    complete(&mut session, &harness).await.unwrap();
    let requests = server.received_requests().await.unwrap();
    let bootstraps: Vec<_> = requests
        .iter()
        .filter(|r| r.url.path() == "/register")
        .collect();
    assert_eq!(bootstraps.len(), 2);
    for registration in &bootstraps {
        let body: Value = registration.body_json().unwrap();
        assert_eq!(body["actor_auth"]["organization_id"], "org-a");
        assert_eq!(body["actor_auth"]["project_id"], "project-a");
    }
    assert!(
        bootstraps
            .iter()
            .all(|r| !r.headers.contains_key("authorization"))
    );
    let attempts: Vec<_> = requests
        .iter()
        .filter(|r| r.url.path() == "/v1/responses")
        .collect();
    assert_eq!(attempts.len(), 3);
    let proofs: Vec<_> = attempts
        .iter()
        .zip(["key-a", "key-a", "key-other-user"])
        .map(|(r, credential)| {
            assert_eq!(r.headers["openai-organization"], "org-a");
            assert_eq!(r.headers["openai-project"], "project-a");
            assert_eq!(
                r.headers
                    .get("x-gateway-auth")
                    .map(|value| value.to_str().unwrap()),
                gateway.then_some("Bearer gateway-token"),
            );
            assertion_payload(
                r.headers["authorization"].to_str().unwrap(),
                r.headers
                    .get("x-openai-agent-assertion")
                    .map(|v| v.to_str().unwrap().to_string()),
                mode,
                credential,
            )
        })
        .collect();
    assert_eq!(
        proofs[0]["runtime_agent_instance"],
        bootstraps[0].headers["idempotency-key"].to_str().unwrap()
    );
    assert_eq!(
        proofs[0]["runtime_agent_instance"],
        proofs[1]["runtime_agent_instance"]
    );
    assert_ne!(proofs[0]["id"], proofs[1]["id"]);
    assert_eq!(
        proofs[2]["runtime_agent_instance"],
        bootstraps[1].headers["idempotency-key"].to_str().unwrap()
    );
    assert_ne!(
        proofs[0]["runtime_agent_instance"],
        proofs[2]["runtime_agent_instance"]
    );
}

#[test_case(RuntimeAuthMode::DualHeader, false; "dual_header")]
#[test_case(RuntimeAuthMode::AssertionOnly, false; "assertion_only")]
#[test_case(RuntimeAuthMode::DualHeader, true; "dual_header_with_gateway")]
#[test_case(RuntimeAuthMode::AssertionOnly, true; "assertion_only_with_gateway")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn websocket_reconnects_for_replacement_and_account_switch_within_a_turn(
    mode: RuntimeAuthMode,
    gateway: bool,
) {
    skip_if_no_network!();
    let registration = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(registered)
        .expect(3)
        .mount(&registration)
        .await;
    let response = vec![
        ev_response_created("resp-1"),
        json!({"type": "response.metadata", "headers": {"x-codex-turn-state": "runtime-state"}}),
        ev_completed("resp-1"),
    ];
    let server = start_websocket_server(vec![
        // Leave one unread response on each old connection so the server does not close it.
        // Runtime admission must happen again even when the peer keeps the socket open.
        vec![response.clone(), response.clone()],
        vec![response.clone(), response.clone()],
        vec![response.clone(), response.clone()],
        // The account switches after this handshake, before its request is sent.
        vec![response.clone()],
        vec![response],
    ])
    .await;
    let mut provider = websocket_provider(&server);
    if gateway {
        provider.base_url = Some(format!(
            "{}/v1",
            server.uri().replacen("ws://", "http://", 1)
        ));
        provider.gateway_oauth = Some(gateway_config(
            &registration,
            GatewayOAuthDelivery::Cookie {
                name: "gateway-session".into(),
            },
        ));
    }
    provider.http_headers = Some(std::collections::HashMap::from([
        ("openai-organization".into(), "org-a".into()),
        ("openai-project".into(), "project-a".into()),
        ("authorization".into(), "Bearer stale-credential".into()),
        ("x-openai-agent-assertion".into(), "stale-assertion".into()),
        ("x-openai-fedramp".into(), "true".into()),
        ("x-test-provider-header".into(), "preserved".into()),
    ]));
    let budget = Arc::new(BudgetChecks::default());
    let (harness, credentials) =
        runtime_harness(&registration, provider, mode, vec![budget.clone()]).await;
    let mut session = harness.client.new_session();
    let mut input = vec![message_item("first")];
    stream_until_complete(&mut session, &harness, &prompt_with_input(input.clone())).await;
    input.push(message_item("second"));
    stream_until_complete(&mut session, &harness, &prompt_with_input(input.clone())).await;
    assert_eq!(server.handshakes().len(), 2);

    *credentials.0.write().unwrap() = CodexAuth::from_api_key("key-b");
    session
        .preconnect_websocket(
            &harness.model_info,
            /*service_tier*/ None,
            &harness.session_telemetry,
            &websocket_connection_metadata(&harness),
        )
        .await
        .unwrap();
    assert_eq!(
        server.handshakes().len(),
        2,
        "preconnect must not admit work"
    );
    input.push(message_item("third"));
    stream_until_complete(&mut session, &harness, &prompt_with_input(input.clone())).await;
    assert_eq!(server.handshakes().len(), 3);

    input.push(message_item("fourth"));
    let prompt = prompt_with_input(input);
    let gate = Arc::new(Barrier::new(2));
    // Allow the pre-connect check, then pause after the old account's socket opens.
    budget.0.lock().unwrap().extend([None, Some(gate.clone())]);
    let generation = stream_until_complete(&mut session, &harness, &prompt);
    tokio::pin!(generation);
    tokio::select! {
        () = &mut generation => panic!("generation finished before the post-connect budget check"),
        _ = gate.wait() => {}
    }
    let previous_owner = harness.auth_manager.auth_owner();
    *credentials.0.write().unwrap() = CodexAuth::from_api_key("key-other-user");
    harness.auth_manager.reload().await;
    assert!(!previous_owner.is_current());
    tokio::join!(generation, gate.wait());
    let handshakes = server.handshakes();
    assert_eq!(handshakes.len(), 5);
    let runtime_ids: Vec<_> = handshakes
        .iter()
        .zip(["key-a", "key-a", "key-b", "key-b", "key-other-user"])
        .map(|(h, credential)| {
            assert_eq!(
                h.header("cookie").as_deref(),
                gateway.then_some("gateway-session=gateway-token"),
            );
            assert!(h.header("x-openai-fedramp").is_none());
            assert_eq!(
                h.header("x-test-provider-header").as_deref(),
                Some("preserved")
            );
            assertion_payload(
                &h.header("authorization").unwrap(),
                h.header("x-openai-agent-assertion"),
                mode,
                credential,
            )["runtime_agent_instance"]
                .clone()
        })
        .collect();
    assert_eq!(runtime_ids[0], runtime_ids[1]);
    assert_ne!(runtime_ids[1], runtime_ids[2]);
    assert_eq!(runtime_ids[2], runtime_ids[3]);
    assert_ne!(runtime_ids[3], runtime_ids[4]);
    assert_eq!(
        server
            .connections()
            .iter()
            .map(Vec::len)
            .collect::<Vec<_>>(),
        vec![1, 1, 1, 0, 1]
    );
    for (index, request) in server.connections().iter().flatten().enumerate() {
        let body = request.body_json();
        assert_eq!(body["previous_response_id"], Value::Null);
        // Reconnecting with the same actor preserves turn routing.
        let expected_turn_state = if index == 1 {
            json!("runtime-state")
        } else {
            Value::Null
        };
        assert_eq!(
            body["client_metadata"]["x-codex-turn-state"],
            expected_turn_state
        );
        assert_eq!(body["input"].as_array().unwrap().len(), index + 1);
    }
    for request in registration.received_requests().await.unwrap() {
        let body: Value = request.body_json().unwrap();
        assert_eq!(body["actor_auth"]["organization_id"], "org-a");
        assert_eq!(body["actor_auth"]["project_id"], "project-a");
    }
    server.shutdown().await;
}

#[test_case(RuntimeAuthMode::DualHeader; "dual_header")]
#[test_case(RuntimeAuthMode::AssertionOnly; "assertion_only")]
#[tokio::test]
async fn mismatched_organization_never_sends_model_request(mode: RuntimeAuthMode) {
    skip_if_no_network!();
    let server = MockServer::start().await;
    Mock::given(path("/register"))
        .respond_with(registered)
        .mount(&server)
        .await;
    let mut provider =
        ModelProviderInfo::create_openai_provider(Some(format!("{}/v1", server.uri())));
    provider.supports_websockets = false;
    provider.http_headers =
        Some([(String::from("OpenAI-Organization"), "another-org".into())].into());
    let (harness, _) = runtime_harness(&server, provider, mode, Vec::new()).await;
    let error = complete(&mut harness.client.new_session(), &harness)
        .await
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("openai-organization does not match")
    );
    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].url.path(), "/register");
}

#[test_case(RuntimeAuthMode::DualHeader; "dual_header")]
#[test_case(RuntimeAuthMode::AssertionOnly; "assertion_only")]
#[tokio::test]
async fn gateway_refresh_failure_blocks_runtime_inference(mode: RuntimeAuthMode) {
    skip_if_no_network!();
    let server = MockServer::start().await;
    Mock::given(path("/register"))
        .respond_with(registered)
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(path("/token"))
        .respond_with(ResponseTemplate::new(500).set_body_json(json!({"error": "server_error"})))
        .expect(1..)
        .mount(&server)
        .await;
    let mut provider =
        ModelProviderInfo::create_openai_provider(Some(format!("{}/v1", server.uri())));
    provider.supports_websockets = false;
    provider.gateway_oauth = Some(gateway_config(
        &server,
        GatewayOAuthDelivery::Header {
            name: "x-gateway-auth".into(),
            scheme: "Bearer".into(),
        },
    ));
    let (harness, _) = runtime_harness(&server, provider.clone(), mode, Vec::new()).await;
    let _gateway = codex_model_provider::test_support::seed_gateway_auth(
        &provider,
        &harness.auth_manager,
        json!({
            "access_token": "expired-gateway-token",
            "refresh_token": "refresh-token",
            "expires_at": 0,
        }),
    );
    let error = complete(&mut harness.client.new_session(), &harness)
        .await
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("Gateway OAuth authentication failed")
    );
    let requests = server.received_requests().await.unwrap();
    assert!(
        requests
            .iter()
            .any(|request| request.url.path() == "/token")
    );
    assert!(
        requests
            .iter()
            .all(|request| matches!(request.url.path(), "/register" | "/token"))
    );
}

#[test_case(RuntimeAuthMode::DualHeader; "dual_header")]
#[test_case(RuntimeAuthMode::AssertionOnly; "assertion_only")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn model_runtime_auth_strips_credentials_on_cross_origin_redirects(mode: RuntimeAuthMode) {
    skip_if_no_network!();
    let server = MockServer::start().await;
    let destination = MockServer::start().await;
    Mock::given(path("/register"))
        .respond_with(registered)
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(path("/v1/responses"))
        .respond_with(ResponseTemplate::new(307).insert_header("location", destination.uri()))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_string(responses::sse(vec![
                    ev_response_created("resp-redirected"),
                    ev_completed("resp-redirected"),
                ])),
        )
        .expect(1)
        .mount(&destination)
        .await;
    let mut provider =
        ModelProviderInfo::create_openai_provider(Some(format!("{}/v1", server.uri())));
    provider.supports_websockets = false;
    let (harness, _) = runtime_harness(&server, provider, mode, Vec::new()).await;
    complete(&mut harness.client.new_session(), &harness)
        .await
        .unwrap();
    let requests = server.received_requests().await.unwrap();
    let request = requests
        .iter()
        .find(|r| r.url.path() == "/v1/responses")
        .unwrap();
    assertion_payload(
        request.headers["authorization"].to_str().unwrap(),
        request
            .headers
            .get("x-openai-agent-assertion")
            .map(|v| v.to_str().unwrap().to_string()),
        mode,
        "key-a",
    );
    let redirected = destination.received_requests().await.unwrap();
    assert_eq!(redirected.len(), 1);
    assert_eq!(redirected[0].body, request.body);
    assert!(!redirected[0].headers.contains_key("authorization"));
    assert!(
        !redirected[0]
            .headers
            .contains_key("x-openai-agent-assertion")
    );
}

#[test_case(RuntimeAuthMode::DualHeader; "dual_header")]
#[test_case(RuntimeAuthMode::AssertionOnly; "assertion_only")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn paused_runtime_cannot_continue_on_an_admitted_websocket(mode: RuntimeAuthMode) {
    use futures::SinkExt;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;
    use tokio_tungstenite::accept_hdr_async_with_config;
    use tokio_tungstenite::tungstenite::Message;
    use tokio_tungstenite::tungstenite::extensions::ExtensionsConfig;
    use tokio_tungstenite::tungstenite::extensions::compression::deflate::DeflateConfig;
    use tokio_tungstenite::tungstenite::handshake::server::Request as HandshakeRequest;
    use tokio_tungstenite::tungstenite::handshake::server::Response as HandshakeResponse;
    use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;

    skip_if_no_network!();
    let registration = MockServer::start().await;
    Mock::given(path("/register"))
        .respond_with(registered)
        .expect(1)
        .mount(&registration)
        .await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mut provider = ModelProviderInfo::create_openai_provider(Some(format!(
        "ws://{}/v1",
        listener.local_addr().unwrap()
    )));
    provider.request_max_retries = Some(0);
    let (harness, _) = runtime_harness(&registration, provider, mode, Vec::new()).await;
    let requests = Arc::new(AtomicUsize::new(0));
    let proofs = Arc::new(std::sync::Mutex::new(Vec::new()));
    let server_requests = Arc::clone(&requests);
    let server_proofs = Arc::clone(&proofs);
    let server = tokio::spawn(async move {
        let mut connections = tokio::task::JoinSet::new();
        for paused in [false, true, true, false] {
            let (stream, _) = listener.accept().await.unwrap();
            let proofs = Arc::clone(&server_proofs);
            let requests = Arc::clone(&server_requests);
            connections.spawn(async move {
                let handshake = move |request: &HandshakeRequest, response: HandshakeResponse| {
                    proofs.lock().unwrap().push(assertion_payload(
                        request.headers()["authorization"].to_str().unwrap(),
                        request
                            .headers()
                            .get("x-openai-agent-assertion")
                            .map(|value| value.to_str().unwrap().to_string()),
                        mode,
                        "key-a",
                    ));
                    if paused {
                        let body = json!({"error": {
                            "message": "The runtime agent instance is paused.",
                            "code": "runtime_instance_paused", "type": "runtime_agent_error"
                        }})
                        .to_string();
                        return Err(HandshakeResponse::builder()
                            .status(403)
                            .header("content-type", "application/json")
                            .header("content-length", body.len())
                            .body(Some(body))
                            .unwrap());
                    }
                    Ok(response)
                };
                let mut extensions = ExtensionsConfig::default();
                extensions.permessage_deflate = Some(DeflateConfig::default());
                let mut config = WebSocketConfig::default();
                config.extensions = extensions;
                let Ok(mut socket) =
                    accept_hdr_async_with_config(stream, handshake, Some(config)).await
                else {
                    return;
                };
                // Keep admitted sockets usable: only another handshake observes the pause.
                while let Some(Ok(message)) = socket.next().await {
                    if message.is_text() {
                        requests.fetch_add(/*val*/ 1, Ordering::SeqCst);
                        for event in [ev_response_created("resp-1"), ev_completed("resp-1")] {
                            socket
                                .send(Message::Text(event.to_string().into()))
                                .await
                                .unwrap();
                        }
                    }
                }
            });
        }
        while connections.join_next().await.is_some() {}
    });

    let assert_paused = |error: &codex_protocol::error::CodexErr| {
        let codex_protocol::error::CodexErrorDetails::UnexpectedStatus(response) = error.details()
        else {
            panic!("expected a paused-runtime HTTP response: {error:?}");
        };
        assert_eq!(response.status.as_u16(), 403);
        let body: Value = serde_json::from_str(&response.body).unwrap();
        assert_eq!(body["error"]["code"], "runtime_instance_paused");
    };
    let mut session = harness.client.new_session();
    complete(&mut session, &harness).await.unwrap();
    let error = complete(&mut session, &harness).await.unwrap_err();
    assert_paused(&error);
    drop(session);
    let mut next_turn = harness.client.new_session();
    let error = complete(&mut next_turn, &harness).await.unwrap_err();
    assert_paused(&error);
    // An authorized unpause permits the original registration to run again.
    complete(&mut next_turn, &harness).await.unwrap();
    assert_eq!(requests.load(Ordering::SeqCst), 2);
    let proofs = proofs.lock().unwrap();
    assert_eq!(proofs.len(), 4);
    for proof in proofs.iter().skip(1) {
        assert_eq!(
            proof["runtime_agent_instance"],
            proofs[0]["runtime_agent_instance"]
        );
    }
    let distinct_proofs = proofs
        .iter()
        .map(|proof| proof["id"].as_str().expect("assertion proof ID"))
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(distinct_proofs.len(), proofs.len());
    server.abort();
}
