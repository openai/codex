use super::*;
use codex_backend_client::ApiKeyResponseCost;
use codex_core::config::ConfigBuilder;
use codex_login::AuthCredentialsStoreMode;
use codex_login::AuthKeyringBackendKind;
use codex_login::login_with_api_key;
use codex_otel::TelemetryAuthMode;
use codex_protocol::protocol::SessionSource;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio::time::timeout;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

const TURN_COST_PATH: &str = "/v1/analytics/codex/turn-costs";

#[tokio::test]
async fn worker_waits_for_late_api_key_login() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(TURN_COST_PATH))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "turns": []
        })))
        .expect(1)
        .mount(&server)
        .await;
    let auth_home = TempDir::new().expect("temporary auth home");
    let auth_manager = Arc::new(
        AuthManager::new(
            auth_home.path().to_path_buf(),
            /*enable_codex_api_key_env*/ false,
            AuthCredentialsStoreMode::File,
            /*forced_chatgpt_workspace_id*/ None,
            /*chatgpt_base_url*/ None,
            AuthKeyringBackendKind::default(),
            codex_login::test_support::transport_default_auth_route_config(),
        )
        .await,
    );
    let runtime = test_runtime(&server, Arc::clone(&auth_manager)).await;
    let (_sender, receiver) = mpsc::channel(OBSERVATION_CHANNEL_CAPACITY);
    let shutdown = CancellationToken::new();
    let mut task = tokio::spawn(runtime.run(receiver, shutdown.clone()));

    assert!(
        timeout(Duration::from_millis(/*millis*/ 50), &mut task)
            .await
            .is_err(),
        "worker exited while waiting for login"
    );
    login_with_api_key(
        auth_home.path(),
        "sk-test",
        AuthCredentialsStoreMode::File,
        AuthKeyringBackendKind::default(),
    )
    .expect("write API-key auth");
    assert!(auth_manager.reload().await);
    wait_for_request_count(&server, /*expected*/ 1).await;

    shutdown.cancel();
    task.await.expect("worker task");
    server.verify().await;
}

#[tokio::test]
async fn transient_probe_failure_keeps_worker_alive() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(TURN_COST_PATH))
        .respond_with(ResponseTemplate::new(500).set_body_string("temporary failure"))
        .expect(1)
        .mount(&server)
        .await;
    let auth_manager = AuthManager::from_auth_for_testing(CodexAuth::from_api_key("sk-test"));
    let runtime = test_runtime(&server, Arc::clone(&auth_manager)).await;
    let backend_availability = runtime.probe_backend().await;
    assert_eq!(backend_availability, BackendAvailability::RetryProbe);
    let (_sender, receiver) = mpsc::channel(OBSERVATION_CHANNEL_CAPACITY);
    let shutdown = CancellationToken::new();
    let auth_changes = auth_manager.auth_change_receiver();
    let mut task = tokio::spawn(runtime.run_with_backend_availability(
        receiver,
        shutdown.clone(),
        auth_changes,
        backend_availability,
    ));

    assert!(
        timeout(Duration::from_millis(/*millis*/ 50), &mut task)
            .await
            .is_err(),
        "worker exited after a transient probe failure"
    );

    shutdown.cancel();
    task.await.expect("worker task");
    server.verify().await;
}

#[tokio::test]
async fn priced_cost_uses_telemetry_captured_before_thread_removal() {
    let server = MockServer::start().await;
    let auth_manager = AuthManager::from_auth_for_testing(CodexAuth::from_api_key("sk-test"));
    let mut runtime = test_runtime(&server, auth_manager).await;
    let thread_id = ThreadId::new();
    let turn_id = "turn-1";

    runtime.record_observation(TurnCostObservation {
        thread_id,
        turn_id: turn_id.to_string(),
        kind: TurnCostObservationKind::Started {
            session_telemetry: Box::new(test_session_telemetry(thread_id)),
        },
    });
    runtime.record_observation(TurnCostObservation {
        thread_id,
        turn_id: turn_id.to_string(),
        kind: TurnCostObservationKind::ResponseCompleted,
    });
    runtime.record_observation(TurnCostObservation {
        thread_id,
        turn_id: turn_id.to_string(),
        kind: TurnCostObservationKind::Finished { interrupted: false },
    });

    runtime.process_api_key_cost(
        turn_id,
        &ApiKeyTurnCost {
            turn_id: turn_id.to_string(),
            status: ApiKeyTurnCostStatus::Priced,
            total_usd: Some("1.25".to_string()),
            event_count: Some(1),
            responses: None,
            model: Some("gpt-5.6".to_string()),
            speed: Some("fast".to_string()),
            reasoning_effort: Some("high".to_string()),
        },
    );

    assert_eq!(runtime.turns.len(), 0);
}

#[tokio::test]
async fn priced_cost_waits_for_every_response_when_response_costs_are_available() {
    let server = MockServer::start().await;
    let auth_manager = AuthManager::from_auth_for_testing(CodexAuth::from_api_key("sk-test"));
    let mut runtime = test_runtime(&server, auth_manager).await;
    let thread_id = ThreadId::new();
    let turn_id = "turn-1";

    runtime.turns.insert(
        turn_id.to_string(),
        TurnCostEntry {
            thread_id,
            session_telemetry: test_session_telemetry(thread_id),
            expected_response_count: 2,
            status: TurnCostStatus::Completed,
            next_poll_at: Instant::now(),
            attempt_count: 0,
        },
    );

    let mut cost = ApiKeyTurnCost {
        turn_id: turn_id.to_string(),
        status: ApiKeyTurnCostStatus::Priced,
        total_usd: Some("1.25".to_string()),
        event_count: Some(2),
        responses: Some(vec![ApiKeyResponseCost {
            response_id: "resp-one".to_string(),
            total_usd: "1.25".to_string(),
        }]),
        model: Some("gpt-5.6".to_string()),
        speed: Some("fast".to_string()),
        reasoning_effort: Some("high".to_string()),
    };
    runtime.process_api_key_cost(turn_id, &cost);

    let entry = runtime.turns.get(turn_id).expect("turn remains tracked");
    assert_eq!(entry.attempt_count, 1);

    cost.event_count = None;
    cost.responses
        .as_mut()
        .expect("response costs")
        .push(ApiKeyResponseCost {
            response_id: "resp-two".to_string(),
            total_usd: "0.50".to_string(),
        });
    runtime.process_api_key_cost(turn_id, &cost);

    assert_eq!(runtime.turns.len(), 0);
}

async fn test_runtime(server: &MockServer, auth_manager: Arc<AuthManager>) -> WorkerRuntime {
    let codex_home = TempDir::new().expect("temporary Codex home");
    let mut config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .build()
        .await
        .expect("test config");
    config.chatgpt_base_url = server.uri();
    WorkerRuntime {
        config: Arc::new(config),
        auth_manager,
        turns: HashMap::new(),
    }
}

fn test_session_telemetry(thread_id: ThreadId) -> SessionTelemetry {
    SessionTelemetry::new(
        thread_id,
        "gpt-5.6",
        "gpt-5.6",
        /*account_id*/ None,
        /*account_email*/ None,
        Some(TelemetryAuthMode::ApiKey),
        "test".to_string(),
        /*log_user_prompts*/ false,
        "test".to_string(),
        SessionSource::Cli,
    )
}

async fn wait_for_request_count(server: &MockServer, expected: usize) {
    timeout(Duration::from_secs(/*secs*/ 15), async {
        loop {
            let requests = server.received_requests().await.unwrap_or_default();
            if requests.len() >= expected {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("timed out waiting for turn-cost request");
}
