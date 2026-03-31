use super::ApiTelemetry;
use super::AuthRequestTelemetryContext;
use super::ModelClient;
use super::PendingUnauthorizedRetry;
use super::RequestRouteTelemetry;
use super::UnauthorizedRecoveryExecution;
use super::emit_terminal_auth_failure_after_failed_recovery;
use crate::auth_env_telemetry::AuthEnvTelemetry;
use crate::response_debug_context::ResponseDebugContext;
use codex_api::ApiError;
use codex_api::RequestTelemetry;
use codex_api::WebsocketTelemetry;
use codex_otel::SessionTelemetry;
use codex_protocol::ThreadId;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use http::StatusCode;
use pretty_assertions::assert_eq;
use serde_json::json;
use serial_test::serial;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::Mutex;

fn test_model_client(session_source: SessionSource) -> ModelClient {
    let provider = crate::model_provider_info::create_oss_provider_with_base_url(
        "https://example.com/v1",
        crate::model_provider_info::WireApi::Responses,
    );
    ModelClient::new(
        /*auth_manager*/ None,
        ThreadId::new(),
        provider,
        session_source,
        /*model_verbosity*/ None,
        /*enable_request_compression*/ false,
        /*include_timing_metrics*/ false,
        /*beta_features_header*/ None,
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
        /*account_id*/ None,
        /*account_email*/ None,
        /*auth_mode*/ None,
        "test-originator".to_string(),
        /*log_user_prompts*/ false,
        "test-terminal".to_string(),
        SessionSource::Cli,
    )
}

fn empty_auth_env_telemetry() -> AuthEnvTelemetry {
    AuthEnvTelemetry {
        openai_api_key_env_present: false,
        codex_api_key_env_present: false,
        codex_api_key_env_enabled: false,
        provider_env_key_name: None,
        provider_env_key_present: None,
        refresh_token_url_override_present: false,
    }
}

type AuthFailureReportCollector = Arc<Mutex<Vec<BTreeMap<String, String>>>>;

fn install_auth_failure_report_collector() -> (
    AuthFailureReportCollector,
    crate::auth::AuthFailureReporterGuard,
) {
    let reported = Arc::new(Mutex::new(Vec::new()));
    let guard = crate::auth::set_auth_failure_reporter({
        let reported = Arc::clone(&reported);
        Arc::new(move |fields| {
            reported
                .lock()
                .expect("report collector poisoned")
                .push(fields);
            true
        })
    });
    (reported, guard)
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
        .summarize_memories(
            Vec::new(),
            &model_info,
            /*effort*/ None,
            &session_telemetry,
        )
        .await
        .expect("empty summarize request should succeed");
    assert_eq!(output.len(), 0);
}

#[test]
fn auth_request_telemetry_context_tracks_attached_auth_and_retry_phase() {
    let auth_context = AuthRequestTelemetryContext::new(
        Some(crate::auth::AuthMode::Chatgpt),
        &crate::api_bridge::CoreAuthProvider::for_test(Some("access-token"), Some("workspace-123")),
        /*has_followup_unauthorized_retry*/ true,
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
#[serial(auth_failure_reporter)]
fn api_telemetry_skips_auth_failure_while_followup_retry_remains() {
    let (reported, _reporter_guard) = install_auth_failure_report_collector();

    let telemetry = ApiTelemetry::new(
        test_session_telemetry(),
        AuthRequestTelemetryContext {
            auth_mode: Some("Chatgpt"),
            auth_header_attached: true,
            auth_header_name: Some("authorization"),
            has_followup_unauthorized_retry: true,
            retry_after_unauthorized: false,
            recovery_mode: Some("managed"),
            recovery_phase: Some("refresh_token"),
        },
        RequestRouteTelemetry::for_endpoint("/responses"),
        empty_auth_env_telemetry(),
        /*emit_sentry_auth_failures*/ true,
    );

    telemetry.on_request(
        /*attempt*/ 1,
        Some(StatusCode::UNAUTHORIZED),
        /*error*/ None,
        Default::default(),
    );

    assert!(reported.lock().unwrap().is_empty());
}

#[test]
#[serial(auth_failure_reporter)]
fn api_telemetry_emits_auth_failure_once_no_followup_retry_remains() {
    let (reported, _reporter_guard) = install_auth_failure_report_collector();

    let telemetry = ApiTelemetry::new(
        test_session_telemetry(),
        AuthRequestTelemetryContext {
            auth_mode: Some("Chatgpt"),
            auth_header_attached: true,
            auth_header_name: Some("authorization"),
            has_followup_unauthorized_retry: false,
            retry_after_unauthorized: true,
            recovery_mode: Some("managed"),
            recovery_phase: Some("refresh_token"),
        },
        RequestRouteTelemetry::for_endpoint("/responses"),
        empty_auth_env_telemetry(),
        /*emit_sentry_auth_failures*/ true,
    );

    telemetry.on_request(
        /*attempt*/ 2,
        Some(StatusCode::UNAUTHORIZED),
        /*error*/ None,
        Default::default(),
    );

    let reported = reported.lock().expect("report collector poisoned");
    assert_eq!(reported.len(), 1);
    assert_eq!(
        reported[0].get("endpoint").map(String::as_str),
        Some("/responses")
    );
    assert_eq!(
        reported[0]
            .get("auth_retry_after_unauthorized")
            .map(String::as_str),
        Some("true")
    );
}

#[test]
#[serial(auth_failure_reporter)]
fn api_telemetry_skips_sentry_auth_failure_for_non_openai_provider() {
    let (reported, _reporter_guard) = install_auth_failure_report_collector();

    let telemetry = ApiTelemetry::new(
        test_session_telemetry(),
        AuthRequestTelemetryContext {
            auth_mode: Some("Chatgpt"),
            auth_header_attached: true,
            auth_header_name: Some("authorization"),
            has_followup_unauthorized_retry: true,
            retry_after_unauthorized: true,
            recovery_mode: Some("managed"),
            recovery_phase: Some("refresh_token"),
        },
        RequestRouteTelemetry::for_endpoint("/responses"),
        empty_auth_env_telemetry(),
        /*emit_sentry_auth_failures*/ false,
    );

    telemetry.on_request(
        /*attempt*/ 1,
        Some(StatusCode::UNAUTHORIZED),
        /*error*/ None,
        Default::default(),
    );

    assert!(reported.lock().unwrap().is_empty());
}

#[test]
#[serial(auth_failure_reporter)]
fn websocket_handshake_failure_emits_auth_failure_when_no_followup_retry_remains() {
    let (reported, _reporter_guard) = install_auth_failure_report_collector();

    let telemetry = ApiTelemetry::new(
        test_session_telemetry(),
        AuthRequestTelemetryContext {
            auth_mode: Some("Chatgpt"),
            auth_header_attached: true,
            auth_header_name: Some("authorization"),
            has_followup_unauthorized_retry: false,
            retry_after_unauthorized: true,
            recovery_mode: Some("managed"),
            recovery_phase: Some("refresh_token"),
        },
        RequestRouteTelemetry::for_endpoint("/responses"),
        empty_auth_env_telemetry(),
        /*emit_sentry_auth_failures*/ true,
    );

    telemetry.on_ws_request(
        Default::default(),
        Some(&ApiError::Transport(codex_api::TransportError::Http {
            status: StatusCode::UNAUTHORIZED,
            url: None,
            headers: None,
            body: None,
        })),
        /*connection_reused*/ false,
    );

    let reported = reported.lock().expect("report collector poisoned");
    assert_eq!(reported.len(), 1);
    assert_eq!(
        reported[0].get("endpoint").map(String::as_str),
        Some("/responses")
    );
    assert_eq!(
        reported[0]
            .get("auth_retry_after_unauthorized")
            .map(String::as_str),
        Some("true")
    );
}

#[test]
#[serial(auth_failure_reporter)]
fn websocket_reused_connection_failure_emits_auth_failure_even_with_followup_retry_available() {
    let (reported, _reporter_guard) = install_auth_failure_report_collector();

    let telemetry = ApiTelemetry::new(
        test_session_telemetry(),
        AuthRequestTelemetryContext {
            auth_mode: Some("Chatgpt"),
            auth_header_attached: true,
            auth_header_name: Some("authorization"),
            has_followup_unauthorized_retry: true,
            retry_after_unauthorized: false,
            recovery_mode: Some("managed"),
            recovery_phase: Some("refresh_token"),
        },
        RequestRouteTelemetry::for_endpoint("/responses"),
        empty_auth_env_telemetry(),
        /*emit_sentry_auth_failures*/ true,
    );

    telemetry.on_ws_request(
        Default::default(),
        Some(&ApiError::Transport(codex_api::TransportError::Http {
            status: StatusCode::UNAUTHORIZED,
            url: None,
            headers: None,
            body: None,
        })),
        /*connection_reused*/ true,
    );

    let reported = reported.lock().expect("report collector poisoned");
    assert_eq!(reported.len(), 1);
    assert_eq!(
        reported[0].get("endpoint").map(String::as_str),
        Some("/responses")
    );
    assert_eq!(
        reported[0]
            .get("auth_connection_reused")
            .map(String::as_str),
        Some("true")
    );
}

#[test]
#[serial(auth_failure_reporter)]
fn failed_recovery_before_retry_emits_terminal_auth_failure() {
    let (reported, _reporter_guard) = install_auth_failure_report_collector();

    let auth_context = AuthRequestTelemetryContext {
        auth_mode: Some("Chatgpt"),
        auth_header_attached: true,
        auth_header_name: Some("authorization"),
        has_followup_unauthorized_retry: true,
        retry_after_unauthorized: false,
        recovery_mode: Some("managed"),
        recovery_phase: Some("reload"),
    };
    let debug = ResponseDebugContext {
        request_id: Some("req_failed_recovery".to_string()),
        cf_ray: Some("ray_failed_recovery".to_string()),
        auth_error: None,
        auth_error_code: Some("token_expired".to_string()),
    };

    emit_terminal_auth_failure_after_failed_recovery(
        auth_context,
        RequestRouteTelemetry::for_endpoint("/responses"),
        &empty_auth_env_telemetry(),
        &debug,
        /*emit_sentry_auth_failures*/ true,
    );

    let reported = reported.lock().expect("report collector poisoned");
    assert_eq!(reported.len(), 1);
    assert_eq!(
        reported[0].get("endpoint").map(String::as_str),
        Some("/responses")
    );
    assert_eq!(
        reported[0].get("auth_recovery_mode").map(String::as_str),
        Some("managed")
    );
    assert_eq!(
        reported[0].get("auth_recovery_phase").map(String::as_str),
        Some("reload")
    );
    assert_eq!(
        reported[0]
            .get("auth_retry_after_unauthorized")
            .map(String::as_str),
        Some("false")
    );
    assert_eq!(
        reported[0].get("auth_request_id").map(String::as_str),
        Some("req_failed_recovery")
    );
}

#[test]
#[serial(auth_failure_reporter)]
fn refresh_token_failed_recovery_before_retry_does_not_emit_terminal_auth_failure() {
    let (reported, _reporter_guard) = install_auth_failure_report_collector();

    let auth_context = AuthRequestTelemetryContext {
        auth_mode: Some("Chatgpt"),
        auth_header_attached: true,
        auth_header_name: Some("authorization"),
        has_followup_unauthorized_retry: true,
        retry_after_unauthorized: false,
        recovery_mode: Some("managed"),
        recovery_phase: Some("refresh_token"),
    };
    let debug = ResponseDebugContext {
        request_id: Some("req_refresh_failed".to_string()),
        cf_ray: Some("ray_refresh_failed".to_string()),
        auth_error: None,
        auth_error_code: Some("token_expired".to_string()),
    };

    emit_terminal_auth_failure_after_failed_recovery(
        auth_context,
        RequestRouteTelemetry::for_endpoint("/responses"),
        &empty_auth_env_telemetry(),
        &debug,
        /*emit_sentry_auth_failures*/ true,
    );

    assert!(
        reported
            .lock()
            .expect("report collector poisoned")
            .is_empty()
    );
}
