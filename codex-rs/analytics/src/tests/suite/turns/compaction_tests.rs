//! Compaction fact, wire payload, and implementation tests.

use crate::events::AppServerRpcTransport;
use crate::events::CodexCompactionEventRequest;
use crate::events::TrackEventRequest;
use crate::facts::AnalyticsFact;
use crate::facts::CodexCompactionEvent;
use crate::facts::CodexErrKind;
use crate::facts::CompactionImplementation;
use crate::facts::CompactionPhase;
use crate::facts::CompactionReason;
use crate::facts::CompactionStatus;
use crate::facts::CompactionStrategy;
use crate::facts::CompactionTrigger;
use crate::facts::CustomAnalyticsFact;
use crate::reducer::AnalyticsReducer;
use crate::tests::support::sample_app_server_client_metadata;
use crate::tests::support::sample_runtime_metadata;
use crate::tests::support::sample_thread_resume_response_with_source;
use codex_app_server_protocol::ClientInfo;
use codex_app_server_protocol::InitializeCapabilities;
use codex_app_server_protocol::InitializeParams;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::SessionSource as AppServerSessionSource;
use codex_app_server_protocol::ThreadSource as AppServerThreadSource;
use codex_login::default_client::DEFAULT_ORIGINATOR;
use codex_protocol::protocol::SubAgentSource;
use codex_protocol::protocol::ThreadSource;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn compaction_event_serializes_expected_shape() {
    let event = TrackEventRequest::Compaction(Box::new(CodexCompactionEventRequest {
        event_type: "codex_compaction_event",
        event_params: crate::events::codex_compaction_event_params(
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
                active_context_tokens_before: 120_000,
                active_context_tokens_after: 18_000,
                retained_image_count: None,
                compaction_summary_tokens: None,
                cached_input_tokens: None,
                cache_write_input_tokens: Some(456),
                started_at: 100,
                completed_at: 106,
                duration_ms: Some(6543),
            },
            "session-thread-1".to_string(),
            sample_app_server_client_metadata(),
            sample_runtime_metadata(),
            Some(ThreadSource::User),
            /*subagent_source*/ None,
            /*parent_thread_id*/ None,
        ),
    }));

    let payload = serde_json::to_value(&event).expect("serialize compaction event");

    assert_eq!(
        payload,
        json!({
            "event_type": "codex_compaction_event",
            "event_params": {
                "thread_id": "thread-1",
                "session_id": "session-thread-1",
                "turn_id": "turn-1",
                "app_server_client": {
                    "product_client_id": DEFAULT_ORIGINATOR,
                    "client_name": "codex-tui",
                    "client_version": "1.0.0",
                    "rpc_transport": "stdio",
                    "experimental_api_enabled": true
                },
                "runtime": {
                    "codex_rs_version": "0.1.0",
                    "runtime_os": "macos",
                    "runtime_os_version": "15.3.1",
                    "runtime_arch": "aarch64"
                },
                "thread_source": "user",
                "subagent_source": null,
                "parent_thread_id": null,
                "trigger": "auto",
                "reason": "context_limit",
                "implementation": "responses_compaction_v2",
                "phase": "mid_turn",
                "strategy": "memento",
                "status": "completed",
                "codex_error_kind": null,
                "codex_error_http_status_code": null,
                "active_context_tokens_before": 120000,
                "active_context_tokens_after": 18000,
                "retained_image_count": null,
                "compaction_summary_tokens": null,
                "cached_input_tokens": null,
                "cache_write_input_tokens": 456,
                "started_at": 100,
                "completed_at": 106,
                "duration_ms": 6543
            }
        })
    );
}

#[test]
fn compaction_implementation_serializes_remote_v2() {
    let payload = serde_json::to_value(CompactionImplementation::ResponsesCompactionV2)
        .expect("serialize compaction implementation");

    assert_eq!(payload, json!("responses_compaction_v2"));
}

#[tokio::test]
async fn compaction_event_ingests_custom_fact() {
    let mut reducer = AnalyticsReducer::default();
    let mut events = Vec::new();
    let parent_thread_id =
        codex_protocol::ThreadId::from_string("22222222-2222-2222-2222-222222222222")
            .expect("valid parent thread id");

    reducer
        .ingest(
            AnalyticsFact::Initialize {
                connection_id: 7,
                params: InitializeParams {
                    client_info: ClientInfo {
                        name: "codex-tui".to_string(),
                        title: None,
                        version: "1.0.0".to_string(),
                    },
                    capabilities: Some(InitializeCapabilities {
                        explicit_gateway_oauth: false,
                        experimental_api: false,
                        request_attestation: false,
                        opt_out_notification_methods: None,
                        mcp_server_openai_form_elicitation: false,
                        extensions: None,
                    }),
                },
                product_client_id: DEFAULT_ORIGINATOR.to_string(),
                runtime: sample_runtime_metadata(),
                rpc_transport: AppServerRpcTransport::Websocket,
            },
            &mut events,
        )
        .await;
    reducer
        .ingest(
            AnalyticsFact::ClientResponse {
                connection_id: 7,
                request_id: RequestId::Integer(2),
                response: Box::new(sample_thread_resume_response_with_source(
                    "thread-1",
                    /*ephemeral*/ false,
                    "gpt-5",
                    AppServerSessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                        parent_thread_id,
                        depth: 1,
                        agent_path: None,
                        agent_nickname: None,
                        agent_role: None,
                    }),
                    Some(AppServerThreadSource::Subagent),
                    Some(parent_thread_id.to_string()),
                )),
                thread_originator: None,
            },
            &mut events,
        )
        .await;
    events.clear();

    reducer
        .ingest(
            AnalyticsFact::Custom(CustomAnalyticsFact::Compaction(Box::new(
                CodexCompactionEvent {
                    thread_id: "thread-1".to_string(),
                    turn_id: "turn-compact".to_string(),
                    trigger: CompactionTrigger::Manual,
                    reason: CompactionReason::UserRequested,
                    implementation: CompactionImplementation::Responses,
                    phase: CompactionPhase::StandaloneTurn,
                    strategy: CompactionStrategy::Memento,
                    status: CompactionStatus::Failed,
                    codex_error_kind: Some(CodexErrKind::ContextWindowExceeded),
                    codex_error_http_status_code: None,
                    active_context_tokens_before: 131_000,
                    active_context_tokens_after: 131_000,
                    retained_image_count: None,
                    compaction_summary_tokens: None,
                    cached_input_tokens: None,
                    cache_write_input_tokens: None,
                    started_at: 100,
                    completed_at: 101,
                    duration_ms: Some(1200),
                },
            ))),
            &mut events,
        )
        .await;

    let payload = serde_json::to_value(&events).expect("serialize events");
    assert_eq!(payload.as_array().expect("events array").len(), 1);
    assert_eq!(payload[0]["event_type"], "codex_compaction_event");
    assert_eq!(payload[0]["event_params"]["session_id"], "session-thread-1");
    assert_eq!(payload[0]["event_params"]["thread_id"], "thread-1");
    assert_eq!(payload[0]["event_params"]["turn_id"], "turn-compact");
    assert_eq!(
        payload[0]["event_params"]["codex_error_kind"],
        json!("context_window_exceeded")
    );
    assert_eq!(
        payload[0]["event_params"]["codex_error_http_status_code"],
        json!(null)
    );
    assert_eq!(
        payload[0]["event_params"]["app_server_client"]["product_client_id"],
        DEFAULT_ORIGINATOR
    );
    assert_eq!(
        payload[0]["event_params"]["app_server_client"]["client_name"],
        "codex-tui"
    );
    assert_eq!(
        payload[0]["event_params"]["app_server_client"]["rpc_transport"],
        "websocket"
    );
    assert_eq!(
        payload[0]["event_params"]["runtime"]["codex_rs_version"],
        "0.1.0"
    );
    assert_eq!(payload[0]["event_params"]["thread_source"], "subagent");
    assert_eq!(
        payload[0]["event_params"]["subagent_source"],
        "thread_spawn"
    );
    assert_eq!(
        payload[0]["event_params"]["parent_thread_id"],
        "22222222-2222-2222-2222-222222222222"
    );
    assert_eq!(payload[0]["event_params"]["trigger"], "manual");
    assert_eq!(payload[0]["event_params"]["reason"], "user_requested");
    assert_eq!(payload[0]["event_params"]["implementation"], "responses");
    assert_eq!(payload[0]["event_params"]["phase"], "standalone_turn");
    assert_eq!(payload[0]["event_params"]["strategy"], "memento");
    assert_eq!(payload[0]["event_params"]["status"], "failed");
}
