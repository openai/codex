//! Exercise selected transport revocation and Realtime compatibility with runtime auth enabled.
#![cfg(test)]

use super::*;
use crate::client::ClientRouting;
use crate::client::RequestRouteTelemetry;
use codex_client::HttpTransport;
use codex_http_client::DestinationPolicy;
use codex_http_client::NetworkPolicyController;
use codex_http_client::NetworkPolicyDenied;
use codex_login::AuthCredentialsStoreMode;
use codex_login::AuthKeyringBackendKind;
use codex_login::AuthRouteConfig;
use pretty_assertions::assert_eq;

#[test_case::test_case(RuntimeAuthMode::DualHeader; "dual_header")]
#[test_case::test_case(RuntimeAuthMode::AssertionOnly; "assertion_only")]
#[tokio::test]
async fn selected_runtime_transport_stays_revoked(mode: RuntimeAuthMode) {
    let server = MockServer::start().await;
    Mock::given(path("/register"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "runtime_agent_instance_id": "runtime-a", "agent_host_id": "host-a",
            "actor": {"kind": "platform", "user_id": "user-a", "organization_id": "org-a", "project_id": null},
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(path("/v1/responses"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;
    let controller = NetworkPolicyController::default();
    let policy = controller.policy();
    let factory = HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault)
        .with_network_policy(policy.clone());
    let home = TempDir::new().unwrap();
    codex_login::login_with_api_key(
        home.path(),
        "key-a",
        AuthCredentialsStoreMode::Ephemeral,
        AuthKeyringBackendKind::Direct,
    )
    .unwrap();
    let manager = Arc::new(
        AuthManager::new(
            home.path().to_path_buf(),
            /*enable_codex_api_key_env*/ false,
            AuthCredentialsStoreMode::Ephemeral,
            /*forced_chatgpt_workspace_id*/ None,
            /*chatgpt_base_url*/ None,
            AuthKeyringBackendKind::Direct,
            AuthRouteConfig::from_http_client_factory(factory.clone()),
        )
        .await,
    );
    assert!(controller.publish(policy.revision(), DestinationPolicy::Unrestricted));
    let runtime = Arc::new(RuntimeAuthManager::new(
        manager.clone(),
        AgentHostAuthorization::InstallationId {
            installation_id: uuid::Uuid::nil(),
        },
        RuntimeRegistrationMetadata {
            harness: None,
            harness_version: None,
            role: RuntimeRole::OrchestratorAndExecutor,
        },
        format!("{}/register", server.uri()).parse().unwrap(),
    ));
    let mut client = test_model_client(SessionSource::Exec);
    client.http_client_factory = factory;
    Arc::get_mut(&mut client.state).unwrap().provider = create_model_provider(
        ModelProviderInfo::create_openai_provider(Some(format!("{}/v1", server.uri()))),
        Some(manager),
    );
    let client = client.with_runtime_auth(runtime, mode).unwrap();
    let setup = client
        .current_client_setup(ClientRouting::Workspace)
        .await
        .unwrap();
    let transport = setup.build_api_transport("/responses").unwrap();
    let request = setup
        .api_provider
        .build_request(http::Method::POST, "/responses");
    let request = setup.api_auth.apply_auth(request).await.unwrap();
    transport.execute(request.clone()).await.unwrap();

    // A new policy may allow the destination, but cannot authorize the old account's request.
    policy.invalidate();
    assert!(controller.publish(policy.revision(), DestinationPolicy::Unrestricted));
    assert!(matches!(
        transport.execute(request).await,
        Err(TransportError::Policy(NetworkPolicyDenied::Revoked))
    ));
    let auth_context = AuthRequestTelemetryContext::new(
        setup.auth.as_ref().map(CodexAuth::auth_mode),
        setup.api_auth.as_ref(),
        /*agent_identity_telemetry*/ None,
        PendingUnauthorizedRetry::default(),
    )
    .unwrap();
    let metadata = test_responses_metadata_for_client(
        &client,
        /*turn_id*/ None,
        "test-session".into(),
        /*parent_thread_id*/ None,
        TestCodexResponsesRequestKind::WebsocketConnection,
    );
    let result = client
        .connect_websocket(
            &test_session_telemetry(),
            setup.api_provider,
            setup.api_auth,
            &setup.http_client_factory,
            &metadata,
            auth_context,
            RequestRouteTelemetry::for_endpoint("/responses"),
            &http::HeaderMap::new(),
            /*parent_routing_affinity*/ None,
        )
        .await;
    assert!(matches!(
        result,
        Err(ApiError::Transport(TransportError::Policy(
            NetworkPolicyDenied::Revoked
        )))
    ));
    server.verify().await;
}

#[test_case::test_case(RuntimeAuthMode::DualHeader, false; "dual_header")]
#[test_case::test_case(RuntimeAuthMode::AssertionOnly, false; "assertion_only")]
#[test_case::test_case(RuntimeAuthMode::DualHeader, true; "runtime_provider_dual_header")]
#[test_case::test_case(RuntimeAuthMode::AssertionOnly, true; "runtime_provider_assertion_only")]
#[tokio::test]
async fn realtime_keeps_provider_auth_until_runtime_integration(
    mode: RuntimeAuthMode,
    runtime_provider: bool,
) {
    let (mut client, _) = model_client_with_counting_attestation(/*include_attestation*/ true);
    let registration = MockServer::start().await;
    let manager = client.auth_manager().unwrap();
    let expected_authorization = format!(
        "Bearer {}",
        manager.auth().await.unwrap().get_token().unwrap()
    );
    let runtime = Arc::new(RuntimeAuthManager::new(
        manager.clone(),
        AgentHostAuthorization::InstallationId {
            installation_id: uuid::Uuid::nil(),
        },
        RuntimeRegistrationMetadata {
            harness: None,
            harness_version: None,
            role: RuntimeRole::OrchestratorAndExecutor,
        },
        format!("{}/register", registration.uri()).parse().unwrap(),
    ));
    if runtime_provider {
        let info = client.state.provider.info().clone();
        Arc::get_mut(&mut client.state).unwrap().provider =
            codex_model_provider::ProviderAuthContext {
                credentials: manager,
                runtime_auth: Some((Arc::clone(&runtime), mode)),
            }
            .model_provider(info);
    }
    let client = client.with_runtime_auth(runtime, mode).unwrap();
    let sideband = client
        .realtime_sideband_headers(http::HeaderMap::new())
        .await
        .unwrap();
    assert_eq!(
        sideband[http::header::AUTHORIZATION],
        expected_authorization
    );
    assert!(!sideband.contains_key("x-openai-agent-assertion"));

    let server = MockServer::start().await;
    Mock::given(path("/realtime/calls"))
        .respond_with(
            ResponseTemplate::new(201)
                .insert_header("location", "/realtime/calls/rtc_test")
                .set_body_string("answer"),
        )
        .expect(1)
        .mount(&server)
        .await;
    let mut provider = client.state.provider.api_provider().await.unwrap();
    provider.base_url = server.uri();
    let call = client
        .create_realtime_call_with_headers(
            "offer".into(),
            codex_api::RealtimeSessionConfig {
                instructions: "hi".into(),
                initial_items: Vec::new(),
                delegation_ack_filler: None,
                model: Some("gpt-realtime".into()),
                session_id: None,
                event_parser: codex_api::RealtimeEventParser::V1,
                session_mode: codex_api::RealtimeSessionMode::Conversational,
                output_modality: codex_protocol::protocol::RealtimeOutputModality::Audio,
                voice: codex_protocol::protocol::RealtimeVoice::Cove,
            },
            http::HeaderMap::new(),
            Some(provider),
        )
        .await
        .unwrap();
    assert_eq!(
        (call.call_id.as_str(), call.sdp.as_str()),
        ("rtc_test", "answer")
    );
    assert_eq!(
        call.sideband_headers[http::header::AUTHORIZATION],
        expected_authorization
    );
    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].headers["authorization"], expected_authorization);
    assert!(!requests[0].headers.contains_key("x-openai-agent-assertion"));
    assert!(registration.received_requests().await.unwrap().is_empty());
}
