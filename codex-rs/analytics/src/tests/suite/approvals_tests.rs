//! User and Guardian approval events, item summaries, and permission privacy tests.

use crate::GuardianAdditionalPermissions;
use crate::GuardianApprovalRequestSource;
use crate::GuardianReviewDecision;
use crate::GuardianReviewEventParams;
use crate::GuardianReviewFailureReason;
use crate::GuardianReviewTerminalStatus;
use crate::GuardianReviewedAction;
use crate::events::AppServerRpcTransport;
use crate::events::CodexAppServerClientMetadata;
use crate::events::CodexReviewEventParams;
use crate::events::CodexReviewEventRequest;
use crate::events::CodexRuntimeMetadata;
use crate::events::ReviewResolution;
use crate::events::ReviewStatus;
use crate::events::ReviewSubjectKind;
use crate::events::ReviewTrigger;
use crate::events::Reviewer;
use crate::events::TrackEventRequest;
use crate::facts::AnalyticsFact;
use crate::facts::CodexCompactionEvent;
use crate::facts::CompactionImplementation;
use crate::facts::CompactionPhase;
use crate::facts::CompactionReason;
use crate::facts::CompactionStatus;
use crate::facts::CompactionStrategy;
use crate::facts::CompactionTrigger;
use crate::facts::CustomAnalyticsFact;
use crate::facts::SubAgentThreadStartedInput;
use crate::reducer::AnalyticsReducer;
use crate::tests::support::ingest_complete_child_turn;
use crate::tests::support::ingest_completed_command_execution_item;
use crate::tests::support::ingest_review_prerequisites;
use crate::tests::support::sample_initialize_fact;
use crate::tests::support::sample_runtime_metadata;
use crate::tests::support::sample_thread_start_response;
use crate::tests::support::sample_turn_start_request;
use crate::tests::support::sample_turn_start_response;
use crate::tests::support::sample_turn_token_usage_fact;
use codex_app_server_protocol::ClientInfo;
use codex_app_server_protocol::CommandExecutionApprovalDecision;
use codex_app_server_protocol::CommandExecutionRequestApprovalParams;
use codex_app_server_protocol::CommandExecutionRequestApprovalResponse;
use codex_app_server_protocol::GuardianApprovalReview;
use codex_app_server_protocol::GuardianApprovalReviewAction;
use codex_app_server_protocol::GuardianApprovalReviewStatus;
use codex_app_server_protocol::GuardianCommandSource as AppServerGuardianCommandSource;
use codex_app_server_protocol::InitializeCapabilities;
use codex_app_server_protocol::InitializeParams;
use codex_app_server_protocol::ItemGuardianApprovalReviewCompletedNotification;
use codex_app_server_protocol::PermissionsRequestApprovalParams;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::RequestPermissionProfile;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::ServerResponse;
use codex_login::default_client::DEFAULT_ORIGINATOR;
use codex_protocol::approvals::NetworkApprovalProtocol;
use codex_protocol::models::AdditionalPermissionProfile;
use codex_protocol::models::NetworkPermissions as CoreNetworkPermissions;
use codex_protocol::models::SandboxPermissions;
use codex_protocol::protocol::GuardianCommandSource;
use codex_protocol::protocol::SubAgentSource;
use codex_protocol::protocol::ThreadSource;
use codex_protocol::request_permissions::PermissionGrantScope as CorePermissionGrantScope;
use codex_protocol::request_permissions::RequestPermissionProfile as CoreRequestPermissionProfile;
use codex_protocol::request_permissions::RequestPermissionsResponse as CoreRequestPermissionsResponse;
use codex_utils_absolute_path::test_support::PathBufExt;
use codex_utils_absolute_path::test_support::test_path_buf;
use pretty_assertions::assert_eq;
use serde_json::json;

fn sample_command_approval_request(request_id: i64, approval_id: Option<&str>) -> ServerRequest {
    ServerRequest::CommandExecutionRequestApproval {
        request_id: RequestId::Integer(request_id),
        params: CommandExecutionRequestApprovalParams {
            kind: Default::default(),
            thread_id: "thread-1".to_string(),
            turn_id: "turn-1".to_string(),
            item_id: "item-1".to_string(),
            started_at_ms: 1_000,
            approval_id: approval_id.map(str::to_string),
            environment_id: None,
            reason: None,
            network_approval_context: None,
            command: Some("echo hi".to_string()),
            cwd: None,
            command_actions: None,
            additional_permissions: None,
            proposed_execpolicy_amendment: None,
            proposed_network_policy_amendments: None,
            available_decisions: None,
        },
    }
}

fn sample_command_approval_response(
    request_id: i64,
    decision: CommandExecutionApprovalDecision,
) -> ServerResponse {
    ServerResponse::CommandExecutionRequestApproval {
        request_id: RequestId::Integer(request_id),
        response: CommandExecutionRequestApprovalResponse { decision },
    }
}

fn sample_permissions_approval_request(request_id: i64) -> ServerRequest {
    ServerRequest::PermissionsRequestApproval {
        request_id: RequestId::Integer(request_id),
        params: PermissionsRequestApprovalParams {
            thread_id: "thread-1".to_string(),
            turn_id: "turn-1".to_string(),
            item_id: "permissions-1".to_string(),
            environment_id: None,
            started_at_ms: 1_000,
            cwd: test_path_buf("/tmp").abs().into(),
            reason: Some("need network".to_string()),
            permissions: RequestPermissionProfile {
                network: Some(codex_app_server_protocol::AdditionalNetworkPermissions {
                    enabled: Some(true),
                }),
                file_system: None,
            },
        },
    }
}

fn sample_effective_permissions_approval_response(
    permissions: CoreRequestPermissionProfile,
    scope: CorePermissionGrantScope,
) -> CoreRequestPermissionsResponse {
    CoreRequestPermissionsResponse {
        permissions,
        scope,
        strict_auto_review: false,
    }
}

fn sample_guardian_review_completed(
    review_id: &str,
    target_item_id: Option<&str>,
    status: GuardianApprovalReviewStatus,
    action: GuardianApprovalReviewAction,
) -> ServerNotification {
    ServerNotification::ItemGuardianApprovalReviewCompleted(
        ItemGuardianApprovalReviewCompletedNotification {
            thread_id: "thread-1".to_string(),
            turn_id: "turn-1".to_string(),
            started_at_ms: 1_000,
            completed_at_ms: 1_042,
            review_id: review_id.to_string(),
            target_item_id: target_item_id.map(str::to_string),
            decision_source: codex_app_server_protocol::AutoReviewDecisionSource::Agent,
            review: GuardianApprovalReview {
                status,
                risk_level: None,
                user_authorization: None,
                rationale: None,
            },
            action,
        },
    )
}

#[test]
fn review_event_serializes_expected_shape() {
    let event = TrackEventRequest::ReviewEvent(CodexReviewEventRequest {
        event_type: "codex_review_event",
        event_params: CodexReviewEventParams {
            thread_id: "thread-1".to_string(),
            turn_id: "turn-1".to_string(),
            item_id: None,
            review_id: "review-1".to_string(),
            app_server_client: CodexAppServerClientMetadata {
                product_client_id: "codex_tui".to_string(),
                client_name: Some("codex-tui".to_string()),
                client_version: Some("1.2.3".to_string()),
                rpc_transport: AppServerRpcTransport::Websocket,
                experimental_api_enabled: Some(true),
            },
            runtime: CodexRuntimeMetadata {
                codex_rs_version: "0.99.0".to_string(),
                runtime_os: "macos".to_string(),
                runtime_os_version: "15.3.1".to_string(),
                runtime_arch: "aarch64".to_string(),
            },
            thread_source: Some(ThreadSource::Subagent),
            subagent_source: Some("thread_spawn".to_string()),
            parent_thread_id: Some("parent-thread-1".to_string()),
            subject_kind: ReviewSubjectKind::NetworkAccess,
            subject_name: "network_access".to_string(),
            reviewer: Reviewer::User,
            trigger: ReviewTrigger::NetworkPolicyDenial,
            status: ReviewStatus::Approved,
            resolution: ReviewResolution::NetworkPolicyAmendment,
            started_at_ms: 123,
            completed_at_ms: 125,
            duration_ms: Some(2),
        },
    });

    let payload = serde_json::to_value(&event).expect("serialize review event");
    assert_eq!(
        payload,
        json!({
            "event_type": "codex_review_event",
            "event_params": {
                "thread_id": "thread-1",
                "turn_id": "turn-1",
                "item_id": null,
                "review_id": "review-1",
                "app_server_client": {
                    "product_client_id": "codex_tui",
                    "client_name": "codex-tui",
                    "client_version": "1.2.3",
                    "rpc_transport": "websocket",
                    "experimental_api_enabled": true
                },
                "runtime": {
                    "codex_rs_version": "0.99.0",
                    "runtime_os": "macos",
                    "runtime_os_version": "15.3.1",
                    "runtime_arch": "aarch64"
                },
                "thread_source": "subagent",
                "subagent_source": "thread_spawn",
                "parent_thread_id": "parent-thread-1",
                "subject_kind": "network_access",
                "subject_name": "network_access",
                "reviewer": "user",
                "trigger": "network_policy_denial",
                "status": "approved",
                "resolution": "network_policy_amendment",
                "started_at_ms": 123,
                "completed_at_ms": 125,
                "duration_ms": 2
            }
        })
    );
}

#[tokio::test]
async fn guardian_review_event_ingests_custom_fact_with_optional_target_item() {
    let mut reducer = AnalyticsReducer::default();
    let mut events = Vec::new();

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
                request_id: RequestId::Integer(1),
                response: Box::new(sample_thread_start_response(
                    "thread-guardian",
                    /*ephemeral*/ false,
                    "gpt-5",
                )),
                thread_originator: None,
            },
            &mut events,
        )
        .await;
    events.clear();

    reducer
        .ingest(
            AnalyticsFact::Custom(CustomAnalyticsFact::GuardianReview(Box::new(
                GuardianReviewEventParams {
                    thread_id: "thread-guardian".to_string(),
                    turn_id: "turn-guardian".to_string(),
                    review_id: "review-guardian".to_string(),
                    target_item_id: None,
                    approval_request_source: GuardianApprovalRequestSource::DelegatedSubagent,
                    reviewed_action: GuardianReviewedAction::NetworkAccess {
                        protocol: NetworkApprovalProtocol::Https,
                        port: 443,
                    },
                    reviewed_action_truncated: false,
                    decision: GuardianReviewDecision::Denied,
                    terminal_status: GuardianReviewTerminalStatus::TimedOut,
                    failure_reason: Some(GuardianReviewFailureReason::Timeout),
                    attempt_count: 1,
                    risk_level: None,
                    user_authorization: None,
                    outcome: None,
                    guardian_thread_id: None,
                    guardian_session_kind: None,
                    guardian_model: None,
                    guardian_reasoning_effort: None,
                    guardian_default_review_model_id: Some("codex-auto-review".to_string()),
                    guardian_catalog_contains_auto_review: Some(false),
                    guardian_review_model_overridden: Some(false),
                    guardian_review_model_override: None,
                    guardian_model_provider_id: Some("openai".to_string()),
                    had_prior_review_context: None,
                    review_timeout_ms: 90_000,
                    tool_call_count: None,
                    time_to_first_token_ms: None,
                    completion_latency_ms: Some(90_000),
                    started_at: 100,
                    completed_at: Some(190),
                    input_tokens: None,
                    cached_input_tokens: None,
                    cache_write_input_tokens: None,
                    output_tokens: None,
                    reasoning_output_tokens: None,
                    total_tokens: None,
                },
            ))),
            &mut events,
        )
        .await;

    let payload = serde_json::to_value(&events).expect("serialize events");
    assert_eq!(payload.as_array().expect("events array").len(), 1);
    assert_eq!(payload[0]["event_type"], "codex_guardian_review");
    assert_eq!(
        payload[0]["event_params"]["session_id"],
        "session-thread-guardian"
    );
    assert_eq!(payload[0]["event_params"]["thread_id"], "thread-guardian");
    assert_eq!(payload[0]["event_params"]["turn_id"], "turn-guardian");
    assert_eq!(payload[0]["event_params"]["review_id"], "review-guardian");
    assert_eq!(payload[0]["event_params"]["target_item_id"], json!(null));
    assert_eq!(
        payload[0]["event_params"]["approval_request_source"],
        "delegated_subagent"
    );
    assert_eq!(
        payload[0]["event_params"]["app_server_client"]["product_client_id"],
        DEFAULT_ORIGINATOR
    );
    assert_eq!(
        payload[0]["event_params"]["runtime"]["codex_rs_version"],
        "0.1.0"
    );
    assert_eq!(
        payload[0]["event_params"]["reviewed_action"]["type"],
        "network_access"
    );
    assert_eq!(
        payload[0]["event_params"]["reviewed_action"]["protocol"],
        "https"
    );
    assert_eq!(payload[0]["event_params"]["reviewed_action"]["port"], 443);
    assert!(payload[0]["event_params"].get("retry_reason").is_none());
    assert!(payload[0]["event_params"].get("rationale").is_none());
    assert!(
        payload[0]["event_params"]["reviewed_action"]
            .get("target")
            .is_none()
    );
    assert!(
        payload[0]["event_params"]["reviewed_action"]
            .get("host")
            .is_none()
    );
    assert_eq!(payload[0]["event_params"]["terminal_status"], "timed_out");
    assert_eq!(payload[0]["event_params"]["failure_reason"], "timeout");
    assert_eq!(payload[0]["event_params"]["attempt_count"], 1);
    assert_eq!(payload[0]["event_params"]["review_timeout_ms"], 90_000);
    assert_eq!(
        payload[0]["event_params"]["guardian_default_review_model_id"],
        "codex-auto-review"
    );
    assert_eq!(
        payload[0]["event_params"]["guardian_catalog_contains_auto_review"],
        false
    );
    assert_eq!(
        payload[0]["event_params"]["guardian_review_model_overridden"],
        false
    );
    assert_eq!(
        payload[0]["event_params"]["guardian_review_model_override"],
        json!(null)
    );
    assert_eq!(
        payload[0]["event_params"]["guardian_model_provider_id"],
        "openai"
    );
}

#[tokio::test]
async fn command_execution_approval_response_publishes_user_review_event() {
    for (kind, approval_id, subject, trigger) in [
        (None, None, "command_execution", "initial"),
        (
            None,
            Some("execve-approval"),
            "command_execution",
            "execve_intercept",
        ),
        (
            Some("writeStdin"),
            Some("stdin-approval"),
            "write_stdin",
            "initial",
        ),
    ] {
        let mut reducer = AnalyticsReducer::default();
        let mut events = Vec::new();

        ingest_review_prerequisites(&mut reducer, &mut events).await;
        let mut request = serde_json::to_value(sample_command_approval_request(
            /*request_id*/ 41,
            approval_id,
        ))
        .expect("serialize approval request");
        // Missing kind models requests from older app-servers.
        if let Some(kind) = kind {
            request["params"]["kind"] = json!(kind);
        } else {
            request["params"].as_object_mut().unwrap().remove("kind");
        }
        reducer
            .ingest(
                AnalyticsFact::ServerRequest {
                    connection_id: 7,
                    request: Box::new(
                        serde_json::from_value(request).expect("deserialize approval request"),
                    ),
                },
                &mut events,
            )
            .await;
        assert!(events.is_empty());

        reducer
            .ingest(
                AnalyticsFact::ServerResponse {
                    completed_at_ms: 1_042,
                    response: Box::new(sample_command_approval_response(
                        /*request_id*/ 41,
                        CommandExecutionApprovalDecision::Accept,
                    )),
                },
                &mut events,
            )
            .await;

        let payload = serde_json::to_value(&events).expect("serialize events");
        assert_eq!(payload.as_array().expect("events array").len(), 1);
        assert_eq!(payload[0]["event_type"], "codex_review_event");
        assert_eq!(payload[0]["event_params"]["thread_id"], "thread-1");
        assert_eq!(payload[0]["event_params"]["turn_id"], "turn-1");
        assert_eq!(payload[0]["event_params"]["item_id"], "item-1");
        assert_eq!(payload[0]["event_params"]["review_id"], "user:41");
        assert_eq!(payload[0]["event_params"]["thread_source"], "user");
        assert_eq!(payload[0]["event_params"]["subject_kind"], subject);
        assert_eq!(payload[0]["event_params"]["subject_name"], subject);
        assert_eq!(payload[0]["event_params"]["reviewer"], "user");
        assert_eq!(payload[0]["event_params"]["trigger"], trigger);
        assert_eq!(payload[0]["event_params"]["status"], "approved");
        assert_eq!(payload[0]["event_params"]["started_at_ms"], 1_000);
        assert_eq!(payload[0]["event_params"]["completed_at_ms"], 1_042);
        assert_eq!(payload[0]["event_params"]["duration_ms"], 42);

        // Stdin reviews must not count toward the parent command's approval summary.
        events.clear();
        ingest_completed_command_execution_item(&mut reducer, &mut events, "thread-1", "item-1")
            .await;
        let item = serde_json::to_value(&events[0]).expect("serialize tool item event");
        assert_eq!(
            item["event_params"]["review_count"],
            u64::from(kind.is_none())
        );
    }
}

#[tokio::test]
async fn permissions_reviews_emit_events_without_denormalizing_onto_tool_items() {
    let mut reducer = AnalyticsReducer::default();
    let mut events = Vec::new();

    ingest_review_prerequisites(&mut reducer, &mut events).await;
    reducer
        .ingest(
            AnalyticsFact::ServerRequest {
                connection_id: 7,
                request: Box::new(sample_permissions_approval_request(/*request_id*/ 51)),
            },
            &mut events,
        )
        .await;
    assert!(events.is_empty());

    reducer
        .ingest(
            AnalyticsFact::EffectivePermissionsApprovalResponse {
                completed_at_ms: 1_042,
                request_id: RequestId::Integer(51),
                response: Box::new(sample_effective_permissions_approval_response(
                    CoreRequestPermissionProfile::default(),
                    CorePermissionGrantScope::Turn,
                )),
            },
            &mut events,
        )
        .await;

    let payload = serde_json::to_value(&events).expect("serialize events");
    assert_eq!(payload.as_array().expect("events array").len(), 1);
    assert_eq!(payload[0]["event_type"], "codex_review_event");
    assert_eq!(payload[0]["event_params"]["review_id"], "user:51");
    assert_eq!(payload[0]["event_params"]["subject_kind"], "permissions");
    assert_eq!(payload[0]["event_params"]["reviewer"], "user");
    assert_eq!(payload[0]["event_params"]["status"], "denied");
    assert_eq!(payload[0]["event_params"]["resolution"], "none");

    events.clear();
    ingest_completed_command_execution_item(&mut reducer, &mut events, "thread-1", "permissions-1")
        .await;

    let payload = serde_json::to_value(&events[0]).expect("serialize tool item event");
    assert_eq!(payload["event_params"]["item_id"], "permissions-1");
    assert_eq!(payload["event_params"]["review_count"], 0);
    assert_eq!(payload["event_params"]["user_review_count"], 0);
    assert_eq!(payload["event_params"]["guardian_review_count"], 0);
}

#[tokio::test]
async fn effective_session_permissions_response_publishes_session_user_review_event() {
    let mut reducer = AnalyticsReducer::default();
    let mut events = Vec::new();

    ingest_review_prerequisites(&mut reducer, &mut events).await;
    reducer
        .ingest(
            AnalyticsFact::ServerRequest {
                connection_id: 7,
                request: Box::new(sample_permissions_approval_request(/*request_id*/ 52)),
            },
            &mut events,
        )
        .await;

    reducer
        .ingest(
            AnalyticsFact::EffectivePermissionsApprovalResponse {
                completed_at_ms: 1_042,
                request_id: RequestId::Integer(52),
                response: Box::new(sample_effective_permissions_approval_response(
                    CoreRequestPermissionProfile {
                        network: Some(CoreNetworkPermissions {
                            enabled: Some(true),
                        }),
                        file_system: None,
                    },
                    CorePermissionGrantScope::Session,
                )),
            },
            &mut events,
        )
        .await;

    let payload = serde_json::to_value(&events).expect("serialize events");
    assert_eq!(payload.as_array().expect("events array").len(), 1);
    assert_eq!(payload[0]["event_type"], "codex_review_event");
    assert_eq!(payload[0]["event_params"]["review_id"], "user:52");
    assert_eq!(payload[0]["event_params"]["subject_kind"], "permissions");
    assert_eq!(payload[0]["event_params"]["reviewer"], "user");
    assert_eq!(payload[0]["event_params"]["status"], "approved");
    assert_eq!(payload[0]["event_params"]["resolution"], "session_approval");
}

#[tokio::test]
async fn aborted_server_request_publishes_aborted_user_review_event_once() {
    let mut reducer = AnalyticsReducer::default();
    let mut events = Vec::new();

    ingest_review_prerequisites(&mut reducer, &mut events).await;
    reducer
        .ingest(
            AnalyticsFact::ServerRequest {
                connection_id: 7,
                request: Box::new(sample_command_approval_request(
                    /*request_id*/ 61, /*approval_id*/ None,
                )),
            },
            &mut events,
        )
        .await;
    reducer
        .ingest(
            AnalyticsFact::ServerRequestAborted {
                completed_at_ms: 1_042,
                request_id: RequestId::Integer(61),
            },
            &mut events,
        )
        .await;

    let payload = serde_json::to_value(&events).expect("serialize events");
    assert_eq!(payload.as_array().expect("events array").len(), 1);
    assert_eq!(payload[0]["event_params"]["review_id"], "user:61");
    assert_eq!(payload[0]["event_params"]["status"], "aborted");
    assert_eq!(payload[0]["event_params"]["resolution"], "none");

    events.clear();
    reducer
        .ingest(
            AnalyticsFact::ServerResponse {
                completed_at_ms: 1_043,
                response: Box::new(sample_command_approval_response(
                    /*request_id*/ 61,
                    CommandExecutionApprovalDecision::Accept,
                )),
            },
            &mut events,
        )
        .await;
    assert!(events.is_empty());
}

#[tokio::test]
async fn guardian_completed_notification_publishes_review_event_with_thread_metadata() {
    let mut reducer = AnalyticsReducer::default();
    let mut events = Vec::new();

    ingest_review_prerequisites(&mut reducer, &mut events).await;
    reducer
        .ingest(
            AnalyticsFact::Notification(Box::new(sample_guardian_review_completed(
                "guardian-review-1",
                Some("item-1"),
                GuardianApprovalReviewStatus::Denied,
                GuardianApprovalReviewAction::Command {
                    source: AppServerGuardianCommandSource::Shell,
                    command: "echo hi".to_string(),
                    cwd: test_path_buf("/tmp").abs().into(),
                },
            ))),
            &mut events,
        )
        .await;

    let payload = serde_json::to_value(&events[0]).expect("serialize review event");
    assert_eq!(payload["event_type"], "codex_review_event");
    assert_eq!(payload["event_params"]["review_id"], "guardian-review-1");
    assert_eq!(payload["event_params"]["item_id"], "item-1");
    assert_eq!(payload["event_params"]["thread_source"], "user");
    assert_eq!(payload["event_params"]["subject_kind"], "command_execution");
    assert_eq!(payload["event_params"]["reviewer"], "guardian");
    assert_eq!(payload["event_params"]["status"], "denied");
    assert_eq!(payload["event_params"]["started_at_ms"], 1_000);
    assert_eq!(payload["event_params"]["completed_at_ms"], 1_042);
    assert_eq!(payload["event_params"]["duration_ms"], 42);
}

#[tokio::test]
async fn terminal_reviews_denormalize_counts_onto_tool_item_events() {
    let mut reducer = AnalyticsReducer::default();
    let mut events = Vec::new();

    ingest_review_prerequisites(&mut reducer, &mut events).await;
    reducer
        .ingest(
            AnalyticsFact::ServerRequest {
                connection_id: 7,
                request: Box::new(sample_command_approval_request(
                    /*request_id*/ 71, /*approval_id*/ None,
                )),
            },
            &mut events,
        )
        .await;
    reducer
        .ingest(
            AnalyticsFact::ServerResponse {
                completed_at_ms: 1_042,
                response: Box::new(sample_command_approval_response(
                    /*request_id*/ 71,
                    CommandExecutionApprovalDecision::AcceptForSession,
                )),
            },
            &mut events,
        )
        .await;
    events.clear();

    reducer
        .ingest(
            AnalyticsFact::Notification(Box::new(sample_guardian_review_completed(
                "guardian-stdin-review-1",
                Some("item-1"),
                GuardianApprovalReviewStatus::Denied,
                GuardianApprovalReviewAction::WriteStdin {
                    approval_id: "stdin-approval-1".to_string(),
                    process_id: "42".to_string(),
                    stdin: "confirm\n".to_string(),
                    cwd: test_path_buf("/tmp").abs().into(),
                },
            ))),
            &mut events,
        )
        .await;
    let review_payload = serde_json::to_value(&events[0]).expect("serialize review event");
    assert_eq!(
        review_payload["event_params"]["subject_kind"],
        "write_stdin"
    );
    events.clear();

    ingest_completed_command_execution_item(&mut reducer, &mut events, "thread-1", "item-1").await;

    let payload = serde_json::to_value(&events[0]).expect("serialize tool item event");
    assert_eq!(payload["event_params"]["review_count"], 1);
    assert_eq!(payload["event_params"]["user_review_count"], 1);
    assert_eq!(payload["event_params"]["guardian_review_count"], 0);
    assert_eq!(
        payload["event_params"]["final_approval_outcome"],
        "user_approved_for_session"
    );
}

#[tokio::test]
async fn item_review_summaries_do_not_cross_threads_with_reused_item_ids() {
    let mut reducer = AnalyticsReducer::default();
    let mut events = Vec::new();

    ingest_review_prerequisites(&mut reducer, &mut events).await;
    reducer
        .ingest(
            AnalyticsFact::ClientResponse {
                connection_id: 7,
                request_id: RequestId::Integer(2),
                response: Box::new(sample_thread_start_response(
                    "thread-2", /*ephemeral*/ false, "gpt-5",
                )),
                thread_originator: None,
            },
            &mut events,
        )
        .await;
    events.clear();

    reducer
        .ingest(
            AnalyticsFact::ServerRequest {
                connection_id: 7,
                request: Box::new(sample_command_approval_request(
                    /*request_id*/ 72, /*approval_id*/ None,
                )),
            },
            &mut events,
        )
        .await;
    reducer
        .ingest(
            AnalyticsFact::ServerResponse {
                completed_at_ms: 1_042,
                response: Box::new(sample_command_approval_response(
                    /*request_id*/ 72,
                    CommandExecutionApprovalDecision::Accept,
                )),
            },
            &mut events,
        )
        .await;
    events.clear();

    ingest_completed_command_execution_item(&mut reducer, &mut events, "thread-2", "item-1").await;

    let payload = serde_json::to_value(&events[0]).expect("serialize tool item event");
    assert_eq!(payload["event_params"]["thread_id"], "thread-2");
    assert_eq!(payload["event_params"]["item_id"], "item-1");
    assert_eq!(payload["event_params"]["review_count"], 0);
    assert_eq!(payload["event_params"]["user_review_count"], 0);
    assert_eq!(payload["event_params"]["guardian_review_count"], 0);
    assert_eq!(payload["event_params"]["final_approval_outcome"], "unknown");
}

#[tokio::test]
async fn guardian_events_keep_thread_source_and_originator_with_explicit_turn_connection() {
    let mut reducer = AnalyticsReducer::default();
    let mut events = Vec::new();
    let parent_thread_id =
        codex_protocol::ThreadId::from_string("44444444-4444-4444-4444-444444444444")
            .expect("valid parent thread id");
    let parent_thread_id_string = parent_thread_id.to_string();

    reducer
        .ingest(
            AnalyticsFact::Initialize {
                connection_id: 7,
                params: InitializeParams {
                    client_info: ClientInfo {
                        name: "parent-client".to_string(),
                        title: None,
                        version: "1.0.0".to_string(),
                    },
                    capabilities: None,
                },
                product_client_id: "parent-client".to_string(),
                runtime: sample_runtime_metadata(),
                rpc_transport: AppServerRpcTransport::Stdio,
            },
            &mut events,
        )
        .await;
    reducer
        .ingest(
            AnalyticsFact::ClientResponse {
                connection_id: 7,
                request_id: RequestId::Integer(1),
                response: Box::new(sample_thread_start_response(
                    &parent_thread_id_string,
                    /*ephemeral*/ false,
                    "gpt-5",
                )),
                thread_originator: None,
            },
            &mut events,
        )
        .await;

    reducer
        .ingest(
            AnalyticsFact::Custom(CustomAnalyticsFact::SubAgentThreadStarted(
                SubAgentThreadStartedInput {
                    session_id: "session-root".to_string(),
                    thread_id: "thread-review".to_string(),
                    parent_thread_id: Some(parent_thread_id.to_string()),
                    forked_from_thread_id: None,
                    product_client_id: "parent-client".to_string(),
                    client_name: Some("parent-client".to_string()),
                    client_version: Some("1.0.0".to_string()),
                    model: "gpt-5".to_string(),
                    ephemeral: false,
                    thread_source: Some(ThreadSource::GuardianReview),
                    subagent_source: SubAgentSource::Other("guardian".to_string()),
                    created_at: 130,
                },
            )),
            &mut events,
        )
        .await;

    events.clear();
    reducer
        .ingest(
            AnalyticsFact::Custom(CustomAnalyticsFact::Compaction(Box::new(
                CodexCompactionEvent {
                    thread_id: "thread-review".to_string(),
                    turn_id: "turn-compact".to_string(),
                    trigger: CompactionTrigger::Manual,
                    reason: CompactionReason::UserRequested,
                    implementation: CompactionImplementation::Responses,
                    phase: CompactionPhase::StandaloneTurn,
                    strategy: CompactionStrategy::Memento,
                    status: CompactionStatus::Completed,
                    codex_error_kind: None,
                    codex_error_http_status_code: None,
                    active_context_tokens_before: 131_000,
                    active_context_tokens_after: 64_000,
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
    assert_eq!(payload[0]["event_params"]["session_id"], "session-root");
    assert_eq!(payload[0]["event_params"]["thread_id"], "thread-review");
    assert_eq!(
        payload[0]["event_params"]["app_server_client"]["product_client_id"],
        "parent-client"
    );
    assert_eq!(
        payload[0]["event_params"]["parent_thread_id"],
        "44444444-4444-4444-4444-444444444444"
    );

    events.clear();
    ingest_complete_child_turn(&mut reducer, &mut events, "thread-review", "turn-inherited").await;
    let [TrackEventRequest::TurnEvent(event)] = events.as_slice() else {
        panic!("expected one turn event");
    };
    let params = &event.event_params;
    assert_eq!(params.session_id, "session-root");
    assert_eq!(params.thread_source, Some(ThreadSource::GuardianReview));
    assert_eq!(params.subagent_source.as_deref(), Some("guardian"));
    assert_eq!(
        params.parent_thread_id.as_deref(),
        Some("44444444-4444-4444-4444-444444444444")
    );
    assert_eq!(params.app_server_client.product_client_id, "parent-client");
    assert_eq!(params.runtime.codex_rs_version, "0.1.0");

    reducer
        .ingest(
            AnalyticsFact::Custom(CustomAnalyticsFact::TurnTokenUsage(Box::new(
                sample_turn_token_usage_fact("thread-review", "turn-inherited"),
            ))),
            &mut events,
        )
        .await;
    assert_eq!(events.len(), 1);

    events.clear();
    reducer
        .ingest(sample_initialize_fact(/*connection_id*/ 8), &mut events)
        .await;
    reducer
        .ingest(
            AnalyticsFact::ClientRequest {
                connection_id: 8,
                request_id: RequestId::Integer(3),
                request: Box::new(sample_turn_start_request(
                    "thread-review",
                    /*request_id*/ 3,
                )),
            },
            &mut events,
        )
        .await;
    reducer
        .ingest(
            AnalyticsFact::ClientResponse {
                connection_id: 8,
                request_id: RequestId::Integer(3),
                response: Box::new(sample_turn_start_response("turn-explicit")),
                thread_originator: None,
            },
            &mut events,
        )
        .await;
    ingest_complete_child_turn(&mut reducer, &mut events, "thread-review", "turn-explicit").await;
    let [TrackEventRequest::TurnEvent(event)] = events.as_slice() else {
        panic!("expected one turn event");
    };
    assert_eq!(
        event.event_params.app_server_client.product_client_id,
        "parent-client"
    );
    assert_eq!(
        event.event_params.app_server_client.client_name.as_deref(),
        Some("codex-tui")
    );
}

#[test]
fn execve_serializes_enabled_network_permissions() {
    let permissions: AdditionalPermissionProfile = serde_json::from_value(json!({
        "network": { "enabled": true },
    }))
    .expect("network permissions");

    let action = GuardianReviewedAction::Execve {
        source: GuardianCommandSource::UnifiedExec,
        additional_permissions: Some(GuardianAdditionalPermissions::from(&permissions)),
    };

    assert_eq!(
        serde_json::to_value(action).expect("serialize action"),
        json!({
            "type": "execve",
            "source": "unified_exec",
            "additional_permissions": {
                "network": { "enabled": true },
            },
        }),
    );
}

#[test]
fn unified_exec_serializes_disabled_network_permissions() {
    let permissions: AdditionalPermissionProfile = serde_json::from_value(json!({
        "network": { "enabled": false },
    }))
    .expect("network permissions");

    let action = GuardianReviewedAction::UnifiedExec {
        sandbox_permissions: SandboxPermissions::WithAdditionalPermissions,
        additional_permissions: Some(GuardianAdditionalPermissions::from(&permissions)),
        tty: false,
    };

    assert_eq!(
        serde_json::to_value(action).expect("serialize action"),
        json!({
            "type": "unified_exec",
            "sandbox_permissions": "with_additional_permissions",
            "additional_permissions": {
                "network": { "enabled": false },
            },
            "tty": false,
        }),
    );
}

#[test]
fn permission_metadata_preserves_absent_and_empty_requests() {
    for (input, expected) in [
        (json!(null), json!(null)),
        (json!({}), json!({ "network": null })),
        (
            json!({ "network": { "enabled": null } }),
            json!({
                "network": { "enabled": null },
            }),
        ),
    ] {
        let permissions: Option<AdditionalPermissionProfile> =
            serde_json::from_value(input).expect("optional permissions");
        let metadata = permissions
            .as_ref()
            .map(GuardianAdditionalPermissions::from);

        assert_eq!(
            serde_json::to_value(metadata).expect("serialize permissions"),
            expected,
        );
    }
}
