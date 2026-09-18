//! Turn start, steer, and interrupt request event tests.

use crate::events::AppServerRpcTransport;
use crate::events::TrackEventRequest;
use crate::facts::AnalyticsFact;
use crate::facts::AnalyticsJsonRpcError;
use crate::facts::CustomAnalyticsFact;
use crate::facts::InputError;
use crate::facts::TurnSteerRequestError;
use crate::reducer::AnalyticsReducer;
use crate::tests::support::ingest_initialize;
use crate::tests::support::ingest_turn_prerequisites;
use crate::tests::support::sample_runtime_metadata;
use crate::tests::support::sample_thread_resume_response;
use crate::tests::support::sample_turn_completed_notification;
use crate::tests::support::sample_turn_resolved_config;
use crate::tests::support::sample_turn_start_request;
use crate::tests::support::sample_turn_start_response;
use codex_app_server_protocol::ClientInfo;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::ClientResponsePayload;
use codex_app_server_protocol::CodexErrorInfo;
use codex_app_server_protocol::InitializeParams;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::NonSteerableTurnKind;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::TurnError as AppServerTurnError;
use codex_app_server_protocol::TurnInterruptResponse;
use codex_app_server_protocol::TurnStatus as AppServerTurnStatus;
use codex_app_server_protocol::TurnSteerParams;
use codex_app_server_protocol::TurnSteerResponse;
use codex_app_server_protocol::UserInput;
use pretty_assertions::assert_eq;
use serde_json::json;

fn sample_turn_steer_request(
    thread_id: &str,
    expected_turn_id: &str,
    request_id: i64,
) -> ClientRequest {
    ClientRequest::TurnSteer {
        request_id: RequestId::Integer(request_id),
        params: TurnSteerParams {
            thread_id: thread_id.to_string(),
            expected_turn_id: expected_turn_id.to_string(),
            client_user_message_id: None,
            input: vec![
                UserInput::Text {
                    text: "more".to_string(),
                    text_elements: vec![],
                },
                UserInput::LocalImage {
                    path: "/tmp/a.png".into(),
                    detail: None,
                },
            ],
            responsesapi_client_metadata: None,
            additional_context: None,
        },
    }
}

fn sample_turn_steer_response(turn_id: &str) -> ClientResponsePayload {
    ClientResponsePayload::TurnSteer(TurnSteerResponse {
        turn_id: turn_id.to_string(),
    })
}

fn sample_turn_interrupt_response() -> ClientResponsePayload {
    ClientResponsePayload::TurnInterrupt(TurnInterruptResponse {})
}

fn no_active_turn_steer_error() -> JSONRPCErrorError {
    JSONRPCErrorError {
        code: -32600,
        message: "no active turn to steer".to_string(),
        data: None,
    }
}

fn no_active_turn_steer_error_type() -> AnalyticsJsonRpcError {
    AnalyticsJsonRpcError::TurnSteer(TurnSteerRequestError::NoActiveTurn)
}

fn non_steerable_review_error() -> JSONRPCErrorError {
    JSONRPCErrorError {
        code: -32600,
        message: "cannot steer a review turn".to_string(),
        data: Some(
            serde_json::to_value(AppServerTurnError {
                misalignment: None,
                message: "cannot steer a review turn".to_string(),
                codex_error_info: Some(CodexErrorInfo::ActiveTurnNotSteerable {
                    turn_kind: NonSteerableTurnKind::Review,
                }),
                additional_details: None,
            })
            .expect("serialize turn error"),
        ),
    }
}

fn non_steerable_review_error_type() -> AnalyticsJsonRpcError {
    AnalyticsJsonRpcError::TurnSteer(TurnSteerRequestError::NonSteerableReview)
}

fn input_too_large_steer_error() -> JSONRPCErrorError {
    JSONRPCErrorError {
        code: -32602,
        message: "Input exceeds the maximum length of 1048576 characters.".to_string(),
        data: Some(json!({
            "input_error_code": "input_too_large",
            "actual_chars": 1048577,
            "max_chars": 1048576,
        })),
    }
}

fn input_too_large_error_type() -> AnalyticsJsonRpcError {
    AnalyticsJsonRpcError::Input(InputError::TooLarge)
}

async fn ingest_rejected_turn_steer(
    reducer: &mut AnalyticsReducer,
    out: &mut Vec<TrackEventRequest>,
    error: JSONRPCErrorError,
    error_type: Option<AnalyticsJsonRpcError>,
) -> serde_json::Value {
    ingest_turn_prerequisites(
        reducer, out, /*include_initialize*/ true, /*include_resolved_config*/ false,
        /*include_started*/ false, /*include_token_usage*/ false,
    )
    .await;
    reducer
        .ingest(
            AnalyticsFact::Initialize {
                connection_id: 8,
                params: InitializeParams {
                    client_info: ClientInfo {
                        name: "codex-web".to_string(),
                        title: None,
                        version: "1.0.0".to_string(),
                    },
                    capabilities: None,
                },
                product_client_id: "codex-web".to_string(),
                runtime: sample_runtime_metadata(),
                rpc_transport: AppServerRpcTransport::Stdio,
            },
            out,
        )
        .await;
    reducer
        .ingest(
            AnalyticsFact::ClientResponse {
                connection_id: 8,
                request_id: RequestId::Integer(6),
                response: Box::new(sample_thread_resume_response(
                    "thread-2", /*ephemeral*/ false, "gpt-5",
                )),
                thread_originator: None,
            },
            out,
        )
        .await;
    out.clear();
    reducer
        .ingest(
            AnalyticsFact::ClientRequest {
                connection_id: 7,
                request_id: RequestId::Integer(4),
                request: Box::new(sample_turn_steer_request(
                    "thread-2", "turn-2", /*request_id*/ 4,
                )),
            },
            out,
        )
        .await;
    reducer
        .ingest(
            AnalyticsFact::ErrorResponse {
                connection_id: 7,
                request_id: RequestId::Integer(4),
                error,
                error_type,
            },
            out,
        )
        .await;

    assert_eq!(out.len(), 1);
    serde_json::to_value(&out[0]).expect("serialize turn steer event")
}

#[tokio::test]
async fn accepted_turn_steer_emits_expected_event() {
    let mut reducer = AnalyticsReducer::default();
    let mut out = Vec::new();

    ingest_turn_prerequisites(
        &mut reducer,
        &mut out,
        /*include_initialize*/ true,
        /*include_resolved_config*/ false,
        /*include_started*/ false,
        /*include_token_usage*/ false,
    )
    .await;
    reducer
        .ingest(
            AnalyticsFact::ClientRequest {
                connection_id: 7,
                request_id: RequestId::Integer(4),
                request: Box::new(sample_turn_steer_request(
                    "thread-2", "turn-2", /*request_id*/ 4,
                )),
            },
            &mut out,
        )
        .await;
    reducer
        .ingest(
            AnalyticsFact::ClientResponse {
                connection_id: 7,
                request_id: RequestId::Integer(4),
                response: Box::new(sample_turn_steer_response("turn-2")),
                thread_originator: None,
            },
            &mut out,
        )
        .await;

    assert_eq!(out.len(), 1);
    let payload = serde_json::to_value(&out[0]).expect("serialize turn steer event");
    assert_eq!(payload["event_type"], json!("codex_turn_steer_event"));
    assert_eq!(payload["event_params"]["thread_id"], json!("thread-2"));
    assert_eq!(
        payload["event_params"]["session_id"],
        json!("session-thread-2")
    );
    assert_eq!(payload["event_params"]["expected_turn_id"], json!("turn-2"));
    assert_eq!(payload["event_params"]["accepted_turn_id"], json!("turn-2"));
    assert_eq!(payload["event_params"]["num_input_images"], json!(1));
    assert_eq!(payload["event_params"]["result"], json!("accepted"));
    assert_eq!(payload["event_params"]["rejection_reason"], json!(null));
    assert!(
        payload["event_params"]["created_at"]
            .as_u64()
            .expect("created_at")
            > 0
    );
    assert_eq!(
        payload["event_params"]["app_server_client"]["product_client_id"],
        json!("codex-tui")
    );
    assert_eq!(
        payload["event_params"]["runtime"]["codex_rs_version"],
        json!("0.1.0")
    );
    assert_eq!(payload["event_params"]["thread_source"], json!("user"));
    assert_eq!(payload["event_params"]["subagent_source"], json!(null));
    assert_eq!(payload["event_params"]["parent_thread_id"], json!(null));
    assert!(payload["event_params"].get("product_client_id").is_none());
}

#[tokio::test]
async fn rejected_turn_steer_uses_request_connection_metadata() {
    let mut reducer = AnalyticsReducer::default();
    let mut out = Vec::new();
    let payload = ingest_rejected_turn_steer(
        &mut reducer,
        &mut out,
        no_active_turn_steer_error(),
        Some(no_active_turn_steer_error_type()),
    )
    .await;

    assert_eq!(payload["event_type"], json!("codex_turn_steer_event"));
    assert_eq!(payload["event_params"]["thread_id"], json!("thread-2"));
    assert_eq!(payload["event_params"]["expected_turn_id"], json!("turn-2"));
    assert_eq!(payload["event_params"]["accepted_turn_id"], json!(null));
    assert_eq!(payload["event_params"]["num_input_images"], json!(1));
    assert_eq!(
        payload["event_params"]["app_server_client"]["product_client_id"],
        json!("codex-tui")
    );
    assert_eq!(
        payload["event_params"]["runtime"]["codex_rs_version"],
        json!("0.1.0")
    );
    assert_eq!(payload["event_params"]["thread_source"], json!("user"));
    assert_eq!(payload["event_params"]["subagent_source"], json!(null));
    assert_eq!(payload["event_params"]["parent_thread_id"], json!(null));
    assert_eq!(payload["event_params"]["result"], json!("rejected"));
    assert_eq!(
        payload["event_params"]["rejection_reason"],
        json!("no_active_turn")
    );
    assert!(
        payload["event_params"]["created_at"]
            .as_u64()
            .expect("created_at")
            > 0
    );
}

#[tokio::test]
async fn rejected_turn_steer_maps_active_turn_not_steerable_error_type() {
    let mut reducer = AnalyticsReducer::default();
    let mut out = Vec::new();
    let payload = ingest_rejected_turn_steer(
        &mut reducer,
        &mut out,
        non_steerable_review_error(),
        Some(non_steerable_review_error_type()),
    )
    .await;

    assert_eq!(
        payload["event_params"]["rejection_reason"],
        json!("non_steerable_review")
    );
}

#[tokio::test]
async fn rejected_turn_steer_maps_input_too_large_error_type() {
    let mut reducer = AnalyticsReducer::default();
    let mut out = Vec::new();
    let payload = ingest_rejected_turn_steer(
        &mut reducer,
        &mut out,
        input_too_large_steer_error(),
        Some(input_too_large_error_type()),
    )
    .await;

    assert_eq!(
        payload["event_params"]["rejection_reason"],
        json!("input_too_large")
    );
}

#[tokio::test]
async fn turn_steer_does_not_emit_without_pending_request() {
    let mut reducer = AnalyticsReducer::default();
    let mut out = Vec::new();

    reducer
        .ingest(
            AnalyticsFact::ErrorResponse {
                connection_id: 7,
                request_id: RequestId::Integer(4),
                error: no_active_turn_steer_error(),
                error_type: Some(no_active_turn_steer_error_type()),
            },
            &mut out,
        )
        .await;

    assert!(out.is_empty());
}

#[tokio::test]
async fn turn_start_error_response_discards_pending_start_request() {
    let mut reducer = AnalyticsReducer::default();
    let mut out = Vec::new();

    ingest_initialize(&mut reducer, &mut out).await;
    reducer
        .ingest(
            AnalyticsFact::ClientRequest {
                connection_id: 7,
                request_id: RequestId::Integer(3),
                request: Box::new(sample_turn_start_request("thread-2", /*request_id*/ 3)),
            },
            &mut out,
        )
        .await;
    reducer
        .ingest(
            AnalyticsFact::ErrorResponse {
                connection_id: 7,
                request_id: RequestId::Integer(3),
                error: no_active_turn_steer_error(),
                error_type: None,
            },
            &mut out,
        )
        .await;

    // A late/synthetic response for the same request id must not resurrect the
    // failed turn/start request and attach request-scoped connection metadata.
    reducer
        .ingest(
            AnalyticsFact::ClientResponse {
                connection_id: 7,
                request_id: RequestId::Integer(3),
                response: Box::new(sample_turn_start_response("turn-2")),
                thread_originator: None,
            },
            &mut out,
        )
        .await;
    assert!(out.is_empty());

    reducer
        .ingest(
            AnalyticsFact::Custom(CustomAnalyticsFact::TurnResolvedConfig(Box::new(
                sample_turn_resolved_config("thread-2", "turn-2"),
            ))),
            &mut out,
        )
        .await;
    reducer
        .ingest(
            AnalyticsFact::Notification(Box::new(sample_turn_completed_notification(
                "thread-2",
                "turn-2",
                AppServerTurnStatus::Completed,
                /*codex_error_info*/ None,
            ))),
            &mut out,
        )
        .await;

    assert!(out.is_empty());
}

#[tokio::test]
async fn accepted_steers_increment_turn_steer_count() {
    let mut reducer = AnalyticsReducer::default();
    let mut out = Vec::new();

    ingest_turn_prerequisites(
        &mut reducer,
        &mut out,
        /*include_initialize*/ true,
        /*include_resolved_config*/ true,
        /*include_started*/ true,
        /*include_token_usage*/ false,
    )
    .await;

    reducer
        .ingest(
            AnalyticsFact::ClientRequest {
                connection_id: 7,
                request_id: RequestId::Integer(4),
                request: Box::new(sample_turn_steer_request(
                    "thread-2", "turn-2", /*request_id*/ 4,
                )),
            },
            &mut out,
        )
        .await;
    reducer
        .ingest(
            AnalyticsFact::ClientResponse {
                connection_id: 7,
                request_id: RequestId::Integer(4),
                response: Box::new(sample_turn_steer_response("turn-2")),
                thread_originator: None,
            },
            &mut out,
        )
        .await;

    reducer
        .ingest(
            AnalyticsFact::ClientRequest {
                connection_id: 7,
                request_id: RequestId::Integer(5),
                request: Box::new(sample_turn_steer_request(
                    "thread-2", "turn-2", /*request_id*/ 5,
                )),
            },
            &mut out,
        )
        .await;
    reducer
        .ingest(
            AnalyticsFact::ErrorResponse {
                connection_id: 7,
                request_id: RequestId::Integer(5),
                error: no_active_turn_steer_error(),
                error_type: Some(no_active_turn_steer_error_type()),
            },
            &mut out,
        )
        .await;

    reducer
        .ingest(
            AnalyticsFact::ClientRequest {
                connection_id: 7,
                request_id: RequestId::Integer(6),
                request: Box::new(sample_turn_steer_request(
                    "thread-2", "turn-2", /*request_id*/ 6,
                )),
            },
            &mut out,
        )
        .await;
    reducer
        .ingest(
            AnalyticsFact::ClientResponse {
                connection_id: 7,
                request_id: RequestId::Integer(6),
                response: Box::new(sample_turn_steer_response("turn-2")),
                thread_originator: None,
            },
            &mut out,
        )
        .await;

    reducer
        .ingest(
            AnalyticsFact::Notification(Box::new(sample_turn_completed_notification(
                "thread-2",
                "turn-2",
                AppServerTurnStatus::Completed,
                /*codex_error_info*/ None,
            ))),
            &mut out,
        )
        .await;

    let turn_event = out
        .iter()
        .find(|event| matches!(event, TrackEventRequest::TurnEvent(_)))
        .expect("turn event should be emitted");
    let payload = serde_json::to_value(turn_event).expect("serialize turn event");
    assert_eq!(payload["event_params"]["steer_count"], json!(2));
}

#[tokio::test]
async fn rejected_turn_interrupt_does_not_tag_interrupted_turn_event() {
    let mut reducer = AnalyticsReducer::default();
    let mut out = Vec::new();

    ingest_turn_prerequisites(
        &mut reducer,
        &mut out,
        /*include_initialize*/ true,
        /*include_resolved_config*/ true,
        /*include_started*/ true,
        /*include_token_usage*/ false,
    )
    .await;
    reducer
        .ingest(
            AnalyticsFact::ExplicitClientInterruptRequest {
                connection_id: 7,
                request_id: RequestId::Integer(4),
                turn_id: "turn-2".to_string(),
                requested_at_ms: 1716000000123,
            },
            &mut out,
        )
        .await;
    reducer
        .ingest(
            AnalyticsFact::ErrorResponse {
                connection_id: 7,
                request_id: RequestId::Integer(4),
                error: no_active_turn_steer_error(),
                error_type: None,
            },
            &mut out,
        )
        .await;
    reducer
        .ingest(
            AnalyticsFact::Notification(Box::new(sample_turn_completed_notification(
                "thread-2",
                "turn-2",
                AppServerTurnStatus::Interrupted,
                /*codex_error_info*/ None,
            ))),
            &mut out,
        )
        .await;

    assert_eq!(out.len(), 1);
    let payload = serde_json::to_value(&out[0]).expect("serialize turn event");
    assert_eq!(payload["event_params"]["status"], json!("interrupted"));
    assert_eq!(
        payload["event_params"]["explicit_client_interrupt_requested_at_ms"],
        json!(null)
    );
    assert_eq!(payload["event_params"]["turn_error"], json!(null));
    assert_eq!(payload["event_params"]["codex_error_kind"], json!(null));
}

#[tokio::test]
async fn accepted_turn_interrupt_records_requested_at_on_turn_event() {
    let mut reducer = AnalyticsReducer::default();
    let mut out = Vec::new();

    ingest_turn_prerequisites(
        &mut reducer,
        &mut out,
        /*include_initialize*/ true,
        /*include_resolved_config*/ true,
        /*include_started*/ true,
        /*include_token_usage*/ false,
    )
    .await;
    reducer
        .ingest(
            AnalyticsFact::ExplicitClientInterruptRequest {
                connection_id: 7,
                request_id: RequestId::Integer(4),
                turn_id: "turn-2".to_string(),
                requested_at_ms: 1716000000123,
            },
            &mut out,
        )
        .await;
    reducer
        .ingest(
            AnalyticsFact::ClientResponse {
                connection_id: 7,
                request_id: RequestId::Integer(4),
                response: Box::new(sample_turn_interrupt_response()),
                thread_originator: None,
            },
            &mut out,
        )
        .await;
    reducer
        .ingest(
            AnalyticsFact::Notification(Box::new(sample_turn_completed_notification(
                "thread-2",
                "turn-2",
                AppServerTurnStatus::Interrupted,
                /*codex_error_info*/ None,
            ))),
            &mut out,
        )
        .await;

    assert_eq!(out.len(), 1);
    let payload = serde_json::to_value(&out[0]).expect("serialize turn event");
    assert_eq!(
        payload["event_params"]["explicit_client_interrupt_requested_at_ms"],
        json!(1716000000123_u64)
    );
}

#[tokio::test]
async fn accepted_turn_interrupt_retries_preserve_earliest_requested_at() {
    let mut reducer = AnalyticsReducer::default();
    let mut out = Vec::new();

    ingest_turn_prerequisites(
        &mut reducer,
        &mut out,
        /*include_initialize*/ true,
        /*include_resolved_config*/ true,
        /*include_started*/ true,
        /*include_token_usage*/ false,
    )
    .await;
    reducer
        .ingest(
            AnalyticsFact::ExplicitClientInterruptRequest {
                connection_id: 7,
                request_id: RequestId::Integer(4),
                turn_id: "turn-2".to_string(),
                requested_at_ms: 1716000000123,
            },
            &mut out,
        )
        .await;
    reducer
        .ingest(
            AnalyticsFact::ExplicitClientInterruptRequest {
                connection_id: 7,
                request_id: RequestId::Integer(5),
                turn_id: "turn-2".to_string(),
                requested_at_ms: 1716000000456,
            },
            &mut out,
        )
        .await;
    reducer
        .ingest(
            AnalyticsFact::ClientResponse {
                connection_id: 7,
                request_id: RequestId::Integer(5),
                response: Box::new(sample_turn_interrupt_response()),
                thread_originator: None,
            },
            &mut out,
        )
        .await;
    reducer
        .ingest(
            AnalyticsFact::ClientResponse {
                connection_id: 7,
                request_id: RequestId::Integer(4),
                response: Box::new(sample_turn_interrupt_response()),
                thread_originator: None,
            },
            &mut out,
        )
        .await;
    reducer
        .ingest(
            AnalyticsFact::Notification(Box::new(sample_turn_completed_notification(
                "thread-2",
                "turn-2",
                AppServerTurnStatus::Interrupted,
                /*codex_error_info*/ None,
            ))),
            &mut out,
        )
        .await;

    assert_eq!(out.len(), 1);
    let payload = serde_json::to_value(&out[0]).expect("serialize turn event");
    assert_eq!(
        payload["event_params"]["explicit_client_interrupt_requested_at_ms"],
        json!(1716000000123_u64)
    );
}
