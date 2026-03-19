use super::AuthRequestTelemetryContext;
use super::ModelClient;
use super::PendingUnauthorizedRetry;
use super::UnauthorizedRecoveryExecution;
use crate::client_common::Prompt;
use crate::delegated_model_transport::DelegatedModelCompactRequest;
use crate::delegated_model_transport::DelegatedModelEvent;
use crate::delegated_model_transport::DelegatedModelRequest;
use crate::delegated_model_transport::DelegatedModelTransport;
use async_trait::async_trait;
use codex_api::ResponseEvent;
use codex_app_server_protocol::NetworkDelegationConfig;
use codex_app_server_protocol::NetworkDelegationMode;
use codex_otel::SessionTelemetry;
use codex_protocol::ThreadId;
use codex_protocol::config_types::ReasoningSummary as ReasoningSummaryConfig;
use codex_protocol::models::BaseInstructions;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::protocol::CreditsSnapshot;
use codex_protocol::protocol::RateLimitSnapshot;
use codex_protocol::protocol::RateLimitWindow;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;
use tokio::sync::mpsc;

fn test_model_client(session_source: SessionSource) -> ModelClient {
    let provider = crate::model_provider_info::create_oss_provider_with_base_url(
        "https://example.com/v1",
        crate::model_provider_info::WireApi::Responses,
    );
    ModelClient::new(
        None,
        ThreadId::new(),
        provider,
        session_source,
        None,
        false,
        false,
        None,
    )
}

fn test_model_info() -> ModelInfo {
    serde_json::from_value(json!({
        "slug": "gpt-test",
        "display_name": "gpt-test",
        "description": "desc",
        "default_reasoning_level": "medium",
        "supported_reasoning_levels": [
            {"effort": "medium", "description": "medium"}
        ],
        "shell_type": "shell_command",
        "visibility": "list",
        "supported_in_api": true,
        "priority": 1,
        "upgrade": null,
        "base_instructions": "base instructions",
        "model_messages": null,
        "supports_reasoning_summaries": false,
        "support_verbosity": false,
        "default_verbosity": null,
        "apply_patch_tool_type": null,
        "truncation_policy": {"mode": "bytes", "limit": 10000},
        "supports_parallel_tool_calls": false,
        "supports_image_detail_original": false,
        "context_window": 272000,
        "auto_compact_token_limit": null,
        "experimental_supported_tools": []
    }))
    .expect("deserialize test model info")
}

fn test_session_telemetry() -> SessionTelemetry {
    SessionTelemetry::new(
        ThreadId::new(),
        "gpt-test",
        "gpt-test",
        None,
        None,
        None,
        "test-originator".to_string(),
        false,
        "test-terminal".to_string(),
        SessionSource::Cli,
    )
}

#[derive(Default)]
struct RecordingDelegatedTransport {
    compact_requests: Mutex<Vec<DelegatedModelCompactRequest>>,
    stream_requests: Mutex<Vec<DelegatedModelRequest>>,
}

#[async_trait]
impl DelegatedModelTransport for RecordingDelegatedTransport {
    async fn start_model_request(
        &self,
        request: DelegatedModelRequest,
    ) -> crate::error::Result<mpsc::Receiver<DelegatedModelEvent>> {
        self.stream_requests
            .lock()
            .expect("stream request recording mutex")
            .push(request);
        let (_tx, rx) = mpsc::channel(1);
        Ok(rx)
    }

    async fn run_model_compact_request(
        &self,
        request: DelegatedModelCompactRequest,
    ) -> crate::error::Result<Vec<ResponseItem>> {
        self.compact_requests
            .lock()
            .expect("compact request recording mutex")
            .push(request);
        Ok(vec![ResponseItem::Message {
            id: Some("msg_compact".to_string()),
            role: "assistant".to_string(),
            content: vec![ContentItem::OutputText {
                text: "compacted output".to_string(),
            }],
            end_turn: None,
            phase: None,
        }])
    }
}

#[test]
fn build_subagent_headers_sets_other_subagent_label() {
    let client = test_model_client(SessionSource::SubAgent(SubAgentSource::Other(
        "memory_consolidation".to_string(),
    )));
    let headers = client.build_subagent_headers();
    let value = headers
        .get("x-openai-subagent")
        .and_then(|value| value.to_str().ok());
    assert_eq!(value, Some("memory_consolidation"));
}

#[tokio::test]
async fn summarize_memories_returns_empty_for_empty_input() {
    let client = test_model_client(SessionSource::Cli);
    let model_info = test_model_info();
    let session_telemetry = test_session_telemetry();

    let output = client
        .summarize_memories(Vec::new(), &model_info, None, &session_telemetry)
        .await
        .expect("empty summarize request should succeed");
    assert_eq!(output.len(), 0);
}

#[test]
fn auth_request_telemetry_context_tracks_attached_auth_and_retry_phase() {
    let auth_context = AuthRequestTelemetryContext::new(
        Some(crate::auth::AuthMode::Chatgpt),
        &crate::api_bridge::CoreAuthProvider::for_test(Some("access-token"), Some("workspace-123")),
        PendingUnauthorizedRetry::from_recovery(UnauthorizedRecoveryExecution {
            mode: "managed",
            phase: "refresh_token",
        }),
    );

    assert_eq!(auth_context.auth_mode, Some("Chatgpt"));
    assert!(auth_context.auth_header_attached);
    assert_eq!(auth_context.auth_header_name, Some("authorization"));
    assert!(auth_context.retry_after_unauthorized);
    assert_eq!(auth_context.recovery_mode, Some("managed"));
    assert_eq!(auth_context.recovery_phase, Some("refresh_token"));
}

#[test]
fn metadata_events_sets_turn_state_and_emits_header_derived_events() {
    let turn_state = Arc::new(OnceLock::new());
    let headers = HashMap::from([
        ("x-codex-turn-state".to_string(), "sticky-route".to_string()),
        ("openai-model".to_string(), "gpt-delegated".to_string()),
        (
            "x-codex-primary-used-percent".to_string(),
            "42.0".to_string(),
        ),
        (
            "x-codex-primary-window-minutes".to_string(),
            "15".to_string(),
        ),
        ("x-codex-primary-reset-at".to_string(), "1700".to_string()),
        ("x-codex-limit-name".to_string(), "Codex".to_string()),
        (
            "x-codex-credits-has-credits".to_string(),
            "true".to_string(),
        ),
        ("x-codex-credits-unlimited".to_string(), "false".to_string()),
        ("x-codex-credits-balance".to_string(), "10.50".to_string()),
        ("X-Models-Etag".to_string(), "etag-123".to_string()),
        ("x-reasoning-included".to_string(), "true".to_string()),
    ]);

    let events = super::metadata_events(&headers, &turn_state);

    assert_eq!(turn_state.get().map(String::as_str), Some("sticky-route"));
    assert_eq!(events.len(), 4);
    match &events[0] {
        ResponseEvent::ServerModel(model) => assert_eq!(model, "gpt-delegated"),
        other => panic!("expected server model event, got {other:?}"),
    }
    match &events[1] {
        ResponseEvent::RateLimits(snapshot) => assert_eq!(
            snapshot,
            &RateLimitSnapshot {
                limit_id: Some("codex".to_string()),
                limit_name: Some("Codex".to_string()),
                primary: Some(RateLimitWindow {
                    used_percent: 42.0,
                    window_minutes: Some(15),
                    resets_at: Some(1700),
                }),
                secondary: None,
                credits: Some(CreditsSnapshot {
                    has_credits: true,
                    unlimited: false,
                    balance: Some("10.50".to_string()),
                }),
                plan_type: None,
            }
        ),
        other => panic!("expected rate limits event, got {other:?}"),
    }
    match &events[2] {
        ResponseEvent::ModelsEtag(etag) => assert_eq!(etag, "etag-123"),
        other => panic!("expected models etag event, got {other:?}"),
    }
    match &events[3] {
        ResponseEvent::ServerReasoningIncluded(included) => assert_eq!(included, &true),
        other => panic!("expected reasoning included event, got {other:?}"),
    }
}

#[tokio::test]
async fn delegated_compact_conversation_history_routes_through_transport() {
    let client = test_model_client(SessionSource::SubAgent(SubAgentSource::Compact));
    let transport = Arc::new(RecordingDelegatedTransport::default());
    client.set_delegated_model_transport(Some(transport.clone()));
    client.set_network_delegation_config(Some(NetworkDelegationConfig {
        mode: NetworkDelegationMode::Enabled,
        stream_idle_timeout_ms: None,
    }));

    let prompt = Prompt {
        input: vec![ResponseItem::Message {
            id: Some("msg_user".to_string()),
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "please compact".to_string(),
            }],
            end_turn: None,
            phase: None,
        }],
        tools: Vec::new(),
        parallel_tool_calls: false,
        base_instructions: BaseInstructions {
            text: "compact instructions".to_string(),
        },
        personality: None,
        output_schema: None,
    };

    let output = client
        .compact_conversation_history(
            "turn-compact",
            &prompt,
            &test_model_info(),
            None,
            ReasoningSummaryConfig::None,
            &test_session_telemetry(),
        )
        .await
        .expect("delegated compact request should succeed");

    assert_eq!(
        output,
        vec![ResponseItem::Message {
            id: Some("msg_compact".to_string()),
            role: "assistant".to_string(),
            content: vec![ContentItem::OutputText {
                text: "compacted output".to_string(),
            }],
            end_turn: None,
            phase: None,
        }]
    );

    let recorded = transport
        .compact_requests
        .lock()
        .expect("compact request recording mutex")
        .clone();
    assert_eq!(recorded.len(), 1);
    let request = &recorded[0];
    assert_eq!(request.thread_id, client.state.conversation_id);
    assert_eq!(request.turn_id, "turn-compact");
    assert!(request.request_id.starts_with("turn-compact-compact-"));
    assert_eq!(request.request.model, "gpt-test");
    assert_eq!(request.request.instructions, "compact instructions");
    assert_eq!(request.request.input, prompt.input);
    assert_eq!(request.request.tools, Vec::<serde_json::Value>::new());
    assert!(!request.request.parallel_tool_calls);
    assert_eq!(request.request.reasoning, None);
    assert_eq!(request.request.text, None);
    assert_eq!(
        request.request.request_headers,
        Some(HashMap::from([
            (
                "session_id".to_string(),
                client.state.conversation_id.to_string(),
            ),
            ("x-openai-subagent".to_string(), "compact".to_string()),
        ]))
    );
}

#[tokio::test]
async fn delegated_stream_request_uses_live_turn_id_and_http_headers() {
    let client = test_model_client(SessionSource::SubAgent(SubAgentSource::Other(
        "delegate".to_string(),
    )));
    let transport = Arc::new(RecordingDelegatedTransport::default());
    client.set_delegated_model_transport(Some(transport.clone()));
    client.set_network_delegation_config(Some(NetworkDelegationConfig {
        mode: NetworkDelegationMode::Enabled,
        stream_idle_timeout_ms: None,
    }));

    let prompt = Prompt {
        input: vec![ResponseItem::Message {
            id: Some("msg_user".to_string()),
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "hello".to_string(),
            }],
            end_turn: None,
            phase: None,
        }],
        tools: Vec::new(),
        parallel_tool_calls: false,
        base_instructions: BaseInstructions {
            text: "stream instructions".to_string(),
        },
        personality: None,
        output_schema: None,
    };

    let mut session = client.new_session_with_turn_id("turn-live".to_string());
    let _stream = session
        .stream(
            &prompt,
            &test_model_info(),
            &test_session_telemetry(),
            None,
            ReasoningSummaryConfig::None,
            None,
            None,
        )
        .await
        .expect("delegated stream request should be accepted");

    let recorded = transport
        .stream_requests
        .lock()
        .expect("stream request recording mutex")
        .clone();
    assert_eq!(recorded.len(), 1);
    let request = &recorded[0];
    assert_eq!(request.thread_id, client.state.conversation_id);
    assert_eq!(request.turn_id, "turn-live");
    assert_eq!(request.request_id, "turn-live-0");
    let headers = request
        .request
        .request_headers
        .as_ref()
        .expect("delegated stream request should include headers");
    assert_eq!(
        headers.get("x-client-request-id"),
        Some(&client.state.conversation_id.to_string())
    );
    assert_eq!(
        headers.get("session_id"),
        Some(&client.state.conversation_id.to_string())
    );
    assert_eq!(
        headers.get("x-openai-subagent"),
        Some(&"delegate".to_string())
    );
    assert_eq!(headers.get(super::OPENAI_BETA_HEADER), None);
    assert_eq!(
        headers.get(super::X_RESPONSESAPI_INCLUDE_TIMING_METRICS_HEADER),
        None
    );
}
