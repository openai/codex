//! Exercise runtime assertions and turn/socket lifetimes through real model transports.

use super::*;
use base64::Engine;
use codex_features::RuntimeAuthMode;
use codex_login::ExternalAuth;
use codex_login::ExternalAuthFuture;
use codex_login::ExternalAuthRefreshContext;
use codex_model_provider_info::GatewayOAuthConfig;
use codex_model_provider_info::GatewayOAuthDelivery;
use codex_runtime_agent_instance::AgentHostAuthorization;
use codex_runtime_agent_instance::RuntimeAuthManager;
use codex_runtime_agent_instance::RuntimeRegistrationMetadata;
use codex_runtime_agent_instance::RuntimeRole;
use core_test_support::responses;
use pretty_assertions::assert_eq;
use serde_json::Value;
use std::sync::RwLock;
use test_case::test_case;
use uuid::Uuid;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::Request;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

struct Credentials(RwLock<CodexAuth>);

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
) -> (WebsocketTestHarness, Arc<Credentials>) {
    provider.requires_openai_auth = true;
    let initial = CodexAuth::from_api_key("key-a");
    let mut harness = websocket_harness_with_provider_options_and_auth(
        provider,
        /*runtime_metrics_enabled*/ false,
        /*concurrent_reasoning_summaries_enabled*/ false,
        &[],
        Some(initial.clone()),
    )
    .await;
    let credentials = Arc::new(Credentials(RwLock::new(initial)));
    harness
        .auth_manager
        .set_external_auth(credentials.clone())
        .await
        .unwrap();
    let runtime = Arc::new(RuntimeAuthManager::new(
        harness.auth_manager.clone(),
        AgentHostAuthorization::InstallationId {
            installation_id: Uuid::nil(),
        },
        RuntimeRegistrationMetadata {
            harness: Some("codex".into()),
            harness_version: None,
            role: RuntimeRole::OrchestratorAndExecutor,
        },
        format!("{}/register", registration.uri()).parse().unwrap(),
    ));
    harness.client = harness.client.with_runtime_auth(runtime, mode).unwrap();
    (harness, credentials)
}

fn gateway_config(server: &MockServer, delivery: GatewayOAuthDelivery) -> GatewayOAuthConfig {
    GatewayOAuthConfig {
        authorization_url: format!("{}/authorize", server.uri()),
        token_url: format!("{}/token", server.uri()),
        client_id: "client".into(),
        resource: None,
        scopes: vec![],
        redirect_port: None,
        delivery,
    }
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
    let (harness, credentials) = runtime_harness(&server, provider, mode).await;
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
        // Reconnection must follow runtime replacement, not a peer-initiated close.
        vec![response.clone(), response.clone(), response.clone()],
        vec![response.clone(), response.clone()],
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
        ("authorization".into(), "Bearer stale-credential".into()),
        ("x-openai-agent-assertion".into(), "stale-assertion".into()),
        ("x-openai-fedramp".into(), "true".into()),
        ("x-test-provider-header".into(), "preserved".into()),
    ]));
    let (harness, credentials) = runtime_harness(&registration, provider, mode).await;
    let mut session = harness.client.new_session();
    let mut input = vec![message_item("first")];
    stream_until_complete(&mut session, &harness, &prompt_with_input(input.clone())).await;
    input.push(message_item("second"));
    stream_until_complete(&mut session, &harness, &prompt_with_input(input.clone())).await;
    assert_eq!(server.handshakes().len(), 1);

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
    input.push(message_item("third"));
    stream_until_complete(&mut session, &harness, &prompt_with_input(input.clone())).await;
    assert_eq!(server.handshakes().len(), 2);

    *credentials.0.write().unwrap() = CodexAuth::from_api_key("key-other-user");
    input.push(message_item("fourth"));
    stream_until_complete(&mut session, &harness, &prompt_with_input(input)).await;
    let handshakes = server.handshakes();
    assert_eq!(handshakes.len(), 3);
    let runtime_ids: Vec<_> = handshakes
        .iter()
        .zip(["key-a", "key-b", "key-other-user"])
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
    assert_ne!(runtime_ids[0], runtime_ids[1]);
    assert_ne!(runtime_ids[1], runtime_ids[2]);
    assert_eq!(
        server
            .connections()
            .iter()
            .map(Vec::len)
            .collect::<Vec<_>>(),
        vec![2, 1, 1]
    );
    let connections = server.connections();
    assert_eq!(
        connections[0][1].body_json()["previous_response_id"],
        "resp-1"
    );
    assert_eq!(
        connections[0][1].body_json()["client_metadata"]["x-codex-turn-state"],
        "runtime-state"
    );
    for (index, requests) in connections.iter().enumerate().skip(1) {
        let body = requests[0].body_json();
        assert_eq!(body["previous_response_id"], Value::Null);
        assert_eq!(body["client_metadata"]["x-codex-turn-state"], Value::Null);
        assert_eq!(body["input"].as_array().unwrap().len(), index + 2);
    }
    server.shutdown().await;
}

#[tokio::test]
async fn mismatched_resource_selector_never_sends_model_request() {
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
        Some([(String::from("OpenAI-Project"), "another-project".into())].into());
    let (harness, _) = runtime_harness(&server, provider, RuntimeAuthMode::DualHeader).await;
    let error = complete(&mut harness.client.new_session(), &harness)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("openai-project does not match"));
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
    let (harness, _) = runtime_harness(&server, provider.clone(), mode).await;
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
