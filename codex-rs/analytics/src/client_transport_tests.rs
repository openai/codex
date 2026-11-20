#![cfg(test)]

//! Protect mixed-batch HTTP compatibility and the boundaries of host transport opt-in.

use super::AnalyticsEventsClient;
use super::AnalyticsEventsDestination;
use super::sample_regular_track_event;
use super::sample_thread_start_response;
use super::send_track_events;
use crate::AnalyticsEventDelivery;
use crate::AnalyticsEventTransport;
use crate::AnalyticsTransportEvent;
use crate::events::AppServerRpcTransport;
use crate::events::CodexAppServerClientMetadata;
use crate::events::CodexCompactionEventRequest;
use crate::events::CodexRuntimeMetadata;
use crate::events::TrackEventRequest;
use crate::events::codex_compaction_event_params;
use crate::facts::CodexCompactionEvent;
use crate::facts::CompactionImplementation;
use crate::facts::CompactionPhase;
use crate::facts::CompactionReason;
use crate::facts::CompactionStatus;
use crate::facts::CompactionStrategy;
use crate::facts::CompactionTrigger;
use crate::product_attribution::ThreadProducts;
use codex_app_server_protocol::ClientInfo;
use codex_app_server_protocol::ClientResponsePayload;
use codex_app_server_protocol::InitializeParams;
use codex_app_server_protocol::RequestId;
use codex_login::AuthManager;
use codex_login::CodexAuth;
use pretty_assertions::assert_eq;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::path;

struct RecordingTransport {
    selected_type: &'static str,
    delivery: AnalyticsEventDelivery,
    events: Arc<Mutex<Vec<AnalyticsTransportEvent>>>,
}

impl AnalyticsEventTransport for RecordingTransport {
    fn selects(&self, event_type: &str) -> bool {
        self.selected_type == event_type
    }

    fn publish(
        &self,
        event: AnalyticsTransportEvent,
    ) -> Pin<Box<dyn Future<Output = AnalyticsEventDelivery> + Send + '_>> {
        Box::pin(async move {
            self.events.lock().unwrap().push(event);
            self.delivery
        })
    }
}

fn compaction_fact() -> CodexCompactionEvent {
    CodexCompactionEvent {
        thread_id: "thread-1".to_string(),
        turn_id: "turn-1".to_string(),
        trigger: CompactionTrigger::Auto,
        reason: CompactionReason::ContextLimit,
        implementation: CompactionImplementation::ResponsesCompactionV2,
        phase: CompactionPhase::MidTurn,
        strategy: CompactionStrategy::Memento,
        status: CompactionStatus::Completed,
        codex_error_kind: None,
        codex_error_http_status_code: None,
        usage_limit_window_minutes: None,
        active_context_tokens_before: 120_000,
        active_context_tokens_after: 18_000,
        retained_image_count: None,
        compaction_summary_tokens: None,
        cached_input_tokens: None,
        cache_write_input_tokens: None,
        started_at: 100,
        completed_at: 106,
        duration_ms: Some(6543),
    }
}

fn compaction() -> TrackEventRequest {
    TrackEventRequest::Compaction(Box::new(CodexCompactionEventRequest {
        event_type: "codex_compaction_event",
        event_params: codex_compaction_event_params(
            compaction_fact(),
            "session-1".to_string(),
            CodexAppServerClientMetadata {
                product_client_id: "codex-cloud-agent".to_string(),
                client_name: Some("codex-cloud-agent".to_string()),
                client_version: Some("1.0.0".to_string()),
                rpc_transport: AppServerRpcTransport::InProcess,
                experimental_api_enabled: Some(true),
            },
            CodexRuntimeMetadata {
                codex_rs_version: "0.1.0".to_string(),
                runtime_os: "linux".to_string(),
                runtime_os_version: "6".to_string(),
                runtime_arch: "x86_64".to_string(),
            },
            /*thread_source*/ None,
            /*subagent_source*/ None,
            /*parent_thread_id*/ None,
        ),
    }))
}

/// The host can select either wire type without changing the HTTP payload of its neighbor.
#[tokio::test]
async fn host_delivery_preserves_http_payload_and_product_for_unhandled_events() {
    for (selected_type, delivery) in [
        ("codex_compaction_event", None),
        ("codex_compaction_event", Some(AnalyticsEventDelivery::Http)),
        (
            "codex_compaction_event",
            Some(AnalyticsEventDelivery::Handled),
        ),
        ("skill_invocation", Some(AnalyticsEventDelivery::Http)),
        ("skill_invocation", Some(AnalyticsEventDelivery::Handled)),
    ] {
        let server = MockServer::start().await;
        Mock::given(path("/codex/analytics-events/events"))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&server)
            .await;
        let recorded = Arc::new(Mutex::new(Vec::new()));
        let transport: Option<Arc<dyn AnalyticsEventTransport>> = delivery.map(|delivery| {
            Arc::new(RecordingTransport {
                selected_type,
                delivery,
                events: Arc::clone(&recorded),
            }) as Arc<dyn AnalyticsEventTransport>
        });
        let events = vec![compaction(), sample_regular_track_event("thread-1")];
        let payloads = serde_json::to_value(&events).unwrap();
        let payloads = payloads.as_array().unwrap();
        let selected = payloads
            .iter()
            .find(|event| event["event_type"] == selected_type)
            .unwrap();
        let expected_http: Vec<_> = payloads
            .iter()
            .filter(|event| {
                delivery != Some(AnalyticsEventDelivery::Handled)
                    || event["event_type"] != selected_type
            })
            .collect();
        let mut products = ThreadProducts::default();
        products.register("thread-1".to_string(), Some("tpp".to_string()));
        send_track_events(
            &AuthManager::from_auth_for_testing(CodexAuth::create_dummy_chatgpt_auth_for_testing()),
            &AnalyticsEventsDestination::Http {
                base_url: server.uri().into(),
                api_key_headers: Default::default(),
            },
            events,
            &products,
            /*runtime_auth*/ None,
            transport.as_ref(),
        )
        .await;
        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(
            requests[0].body_json::<serde_json::Value>().unwrap(),
            serde_json::json!({ "events": expected_http })
        );
        assert_eq!(requests[0].headers["x-openai-product-sku"], "tpp");
        assert_eq!(
            *recorded.lock().unwrap(),
            if delivery.is_some() {
                vec![AnalyticsTransportEvent {
                    event_type: selected_type,
                    payload: selected.clone(),
                    product_sku: Some("tpp".to_string()),
                }]
            } else {
                vec![]
            }
        );
    }
}

/// Installing a host transport must not bypass the existing event eligibility filter.
#[tokio::test]
async fn api_key_auth_does_not_send_ineligible_compaction_to_host() {
    let server = MockServer::start().await;
    let recorded = Arc::new(Mutex::new(Vec::new()));
    let transport: Arc<dyn AnalyticsEventTransport> = Arc::new(RecordingTransport {
        selected_type: "codex_compaction_event",
        delivery: AnalyticsEventDelivery::Handled,
        events: Arc::clone(&recorded),
    });
    send_track_events(
        &AuthManager::from_auth_for_testing(CodexAuth::from_api_key("test-key")),
        &AnalyticsEventsDestination::Http {
            base_url: server.uri().into(),
            api_key_headers: Default::default(),
        },
        vec![compaction()],
        &ThreadProducts::default(),
        /*runtime_auth*/ None,
        Some(&transport),
    )
    .await;
    assert!(recorded.lock().unwrap().is_empty());
    assert!(server.received_requests().await.unwrap().is_empty());
}

/// Local capture keeps source payloads and must not trigger an external host publisher.
#[cfg(debug_assertions)]
#[tokio::test]
async fn capture_file_never_invokes_host_transport() {
    let capture = std::env::temp_dir().join(format!(
        "analytics-host-capture-{}-{}.jsonl",
        std::process::id(),
        crate::now_unix_millis()
    ));
    let recorded = Arc::new(Mutex::new(Vec::new()));
    let transport: Arc<dyn AnalyticsEventTransport> = Arc::new(RecordingTransport {
        selected_type: "codex_compaction_event",
        delivery: AnalyticsEventDelivery::Handled,
        events: Arc::clone(&recorded),
    });
    send_track_events(
        &AuthManager::from_auth_for_testing(CodexAuth::create_dummy_chatgpt_auth_for_testing()),
        &AnalyticsEventsDestination::CaptureFile {
            path: capture.clone(),
        },
        vec![compaction()],
        &ThreadProducts::default(),
        /*runtime_auth*/ None,
        Some(&transport),
    )
    .await;
    let payload: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&capture).unwrap()).unwrap();
    std::fs::remove_file(capture).unwrap();
    assert_eq!(payload, serde_json::json!({"events": [compaction()]}));
    assert!(recorded.lock().unwrap().is_empty());
}

/// The worker routes reduced compaction only for an enabled, explicitly opted-in actor.
/// Flushing waits for delivery; rebinding must not reuse the previous actor's transport.
#[tokio::test]
async fn worker_preserves_opt_out_and_actor_rebinding() {
    enum Mode {
        Host,
        Disabled,
        Rebound,
    }
    for mode in [Mode::Host, Mode::Disabled, Mode::Rebound] {
        let server = MockServer::start().await;
        Mock::given(path("/codex/analytics-events/events"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;
        let recorded = Arc::new(Mutex::new(Vec::new()));
        let transport = Arc::new(RecordingTransport {
            selected_type: "codex_compaction_event",
            delivery: AnalyticsEventDelivery::Handled,
            events: Arc::clone(&recorded),
        });
        let client = AnalyticsEventsClient::new_with_event_transport(
            AuthManager::from_auth_for_testing(CodexAuth::create_dummy_chatgpt_auth_for_testing()),
            server.uri(),
            Some(!matches!(mode, Mode::Disabled)),
            transport,
        );
        let client = if matches!(mode, Mode::Rebound) {
            client.for_auth_context(
                AuthManager::from_auth_for_testing(
                    CodexAuth::create_dummy_chatgpt_auth_for_testing(),
                ),
                server.uri(),
                /*runtime_auth*/ None,
                /*api_key_headers*/ Default::default(),
            )
        } else {
            client
        };
        client.track_initialize(
            /*connection_id*/ 1,
            InitializeParams {
                client_info: ClientInfo {
                    name: "test".to_string(),
                    title: None,
                    version: "1".to_string(),
                },
                capabilities: None,
            },
            "test_client".to_string(),
            AppServerRpcTransport::InProcess,
        );
        let response = sample_thread_start_response();
        let ClientResponsePayload::ThreadStart(started) = &response else {
            unreachable!()
        };
        let mut fact = compaction_fact();
        fact.thread_id = started.thread.id.clone();
        client.track_response(/*connection_id*/ 1, RequestId::Integer(1), &response);
        client.track_compaction(fact);
        client.flush().await;

        let http_events = server
            .received_requests()
            .await
            .unwrap()
            .iter()
            .flat_map(|request| {
                request.body_json::<serde_json::Value>().unwrap()["events"]
                    .as_array()
                    .unwrap()
                    .clone()
            })
            .collect::<Vec<_>>();
        let http_compactions = http_events
            .iter()
            .filter(|event| event["event_type"] == "codex_compaction_event")
            .count();
        let delivered = recorded.lock().unwrap();
        match mode {
            Mode::Host => {
                assert_eq!(delivered.len(), 1);
                assert_eq!(
                    delivered[0].payload["event_params"]["thread_id"],
                    started.thread.id
                );
                assert_eq!(http_compactions, 0);
                assert!(
                    !http_events.is_empty(),
                    "unmigrated thread events retain HTTP delivery"
                );
            }
            Mode::Disabled => {
                assert!(delivered.is_empty());
                assert!(http_events.is_empty());
            }
            Mode::Rebound => {
                assert!(delivered.is_empty());
                assert_eq!(http_compactions, 1);
            }
        }
    }
}
