use super::*;
use crate::auth_env_telemetry::AuthEnvTelemetry;
use serial_test::serial;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::Mutex;
use tracing::Event;
use tracing::Subscriber;
use tracing::field::Visit;
use tracing_subscriber::Layer;
use tracing_subscriber::layer::Context;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::util::SubscriberInitExt;

#[test]
fn feedback_tags_macro_compiles() {
    #[derive(Debug)]
    struct OnlyDebug;

    feedback_tags!(model = "gpt-5", cached = true, debug_only = OnlyDebug);
}

#[derive(Default)]
struct TagCollectorVisitor {
    tags: BTreeMap<String, String>,
}

impl Visit for TagCollectorVisitor {
    fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
        self.tags
            .insert(field.name().to_string(), value.to_string());
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        self.tags
            .insert(field.name().to_string(), value.to_string());
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.tags
            .insert(field.name().to_string(), format!("{value:?}"));
    }
}

#[derive(Clone)]
struct TagCollectorLayer {
    target: &'static str,
    tags: Arc<Mutex<BTreeMap<String, String>>>,
    event_count: Arc<Mutex<usize>>,
}

impl<S> Layer<S> for TagCollectorLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        if event.metadata().target() != self.target {
            return;
        }
        let mut visitor = TagCollectorVisitor::default();
        event.record(&mut visitor);
        self.tags.lock().unwrap().extend(visitor.tags);
        *self.event_count.lock().unwrap() += 1;
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
fn emit_feedback_request_tags_records_sentry_feedback_fields() {
    let tags = Arc::new(Mutex::new(BTreeMap::new()));
    let event_count = Arc::new(Mutex::new(0));
    let _guard = tracing_subscriber::registry()
        .with(TagCollectorLayer {
            target: "feedback_tags",
            tags: tags.clone(),
            event_count: event_count.clone(),
        })
        .set_default();

    let auth_env = AuthEnvTelemetry {
        openai_api_key_env_present: true,
        codex_api_key_env_present: false,
        codex_api_key_env_enabled: true,
        provider_env_key_name: Some("configured".to_string()),
        provider_env_key_present: Some(true),
        refresh_token_url_override_present: true,
    };

    emit_feedback_request_tags_with_auth_env(
        &FeedbackRequestTags {
            endpoint: "/responses",
            auth_header_attached: true,
            auth_header_name: Some("authorization"),
            auth_mode: Some("chatgpt"),
            auth_retry_after_unauthorized: Some(false),
            auth_recovery_mode: Some("managed"),
            auth_recovery_phase: Some("refresh_token"),
            auth_connection_reused: Some(true),
            auth_request_id: Some("req-123"),
            auth_cf_ray: Some("ray-123"),
            auth_error: Some("missing_authorization_header"),
            auth_error_code: Some("token_expired"),
            auth_recovery_followup_success: Some(true),
            auth_recovery_followup_status: Some(200),
        },
        &auth_env,
    );

    let tags = tags.lock().unwrap().clone();
    assert_eq!(
        tags.get("endpoint").map(String::as_str),
        Some("\"/responses\"")
    );
    assert_eq!(
        tags.get("auth_header_attached").map(String::as_str),
        Some("true")
    );
    assert_eq!(
        tags.get("auth_header_name").map(String::as_str),
        Some("\"authorization\"")
    );
    assert_eq!(
        tags.get("auth_env_openai_api_key_present")
            .map(String::as_str),
        Some("true")
    );
    assert_eq!(
        tags.get("auth_env_codex_api_key_present")
            .map(String::as_str),
        Some("false")
    );
    assert_eq!(
        tags.get("auth_env_codex_api_key_enabled")
            .map(String::as_str),
        Some("true")
    );
    assert_eq!(
        tags.get("auth_env_provider_key_name").map(String::as_str),
        Some("\"configured\"")
    );
    assert_eq!(
        tags.get("auth_env_provider_key_present")
            .map(String::as_str),
        Some("\"true\"")
    );
    assert_eq!(
        tags.get("auth_env_refresh_token_url_override_present")
            .map(String::as_str),
        Some("true")
    );
    assert_eq!(
        tags.get("auth_request_id").map(String::as_str),
        Some("\"req-123\"")
    );
    assert_eq!(
        tags.get("auth_error_code").map(String::as_str),
        Some("\"token_expired\"")
    );
    assert_eq!(
        tags.get("auth_recovery_followup_success")
            .map(String::as_str),
        Some("\"true\"")
    );
    assert_eq!(
        tags.get("auth_recovery_followup_status")
            .map(String::as_str),
        Some("\"200\"")
    );
    assert_eq!(*event_count.lock().unwrap(), 1);
}

#[test]
fn emit_feedback_auth_recovery_tags_preserves_401_specific_fields() {
    let tags = Arc::new(Mutex::new(BTreeMap::new()));
    let event_count = Arc::new(Mutex::new(0));
    let _guard = tracing_subscriber::registry()
        .with(TagCollectorLayer {
            target: "feedback_tags",
            tags: tags.clone(),
            event_count: event_count.clone(),
        })
        .set_default();

    emit_feedback_auth_recovery_tags(
        "managed",
        "refresh_token",
        "recovery_succeeded",
        Some("req-401"),
        Some("ray-401"),
        Some("missing_authorization_header"),
        Some("token_expired"),
    );

    let tags = tags.lock().unwrap().clone();
    assert_eq!(
        tags.get("auth_401_request_id").map(String::as_str),
        Some("\"req-401\"")
    );
    assert_eq!(
        tags.get("auth_401_cf_ray").map(String::as_str),
        Some("\"ray-401\"")
    );
    assert_eq!(
        tags.get("auth_401_error").map(String::as_str),
        Some("\"missing_authorization_header\"")
    );
    assert_eq!(
        tags.get("auth_401_error_code").map(String::as_str),
        Some("\"token_expired\"")
    );
    assert_eq!(*event_count.lock().unwrap(), 1);
}

#[test]
fn emit_feedback_auth_recovery_tags_clears_stale_401_fields() {
    let tags = Arc::new(Mutex::new(BTreeMap::new()));
    let event_count = Arc::new(Mutex::new(0));
    let _guard = tracing_subscriber::registry()
        .with(TagCollectorLayer {
            target: "feedback_tags",
            tags: tags.clone(),
            event_count: event_count.clone(),
        })
        .set_default();

    emit_feedback_auth_recovery_tags(
        "managed",
        "refresh_token",
        "recovery_failed_transient",
        Some("req-401-a"),
        Some("ray-401-a"),
        Some("missing_authorization_header"),
        Some("token_expired"),
    );
    emit_feedback_auth_recovery_tags(
        "managed",
        "done",
        "recovery_not_run",
        Some("req-401-b"),
        /*auth_cf_ray*/ None,
        /*auth_error*/ None,
        /*auth_error_code*/ None,
    );

    let tags = tags.lock().unwrap().clone();
    assert_eq!(
        tags.get("auth_401_request_id").map(String::as_str),
        Some("\"req-401-b\"")
    );
    assert_eq!(
        tags.get("auth_401_cf_ray").map(String::as_str),
        Some("\"\"")
    );
    assert_eq!(tags.get("auth_401_error").map(String::as_str), Some("\"\""));
    assert_eq!(
        tags.get("auth_401_error_code").map(String::as_str),
        Some("\"\"")
    );
    assert_eq!(*event_count.lock().unwrap(), 2);
}

#[test]
fn emit_feedback_request_tags_preserves_latest_auth_fields_after_unauthorized() {
    let tags = Arc::new(Mutex::new(BTreeMap::new()));
    let event_count = Arc::new(Mutex::new(0));
    let _guard = tracing_subscriber::registry()
        .with(TagCollectorLayer {
            target: "feedback_tags",
            tags: tags.clone(),
            event_count: event_count.clone(),
        })
        .set_default();

    emit_feedback_request_tags(&FeedbackRequestTags {
        endpoint: "/responses",
        auth_header_attached: true,
        auth_header_name: Some("authorization"),
        auth_mode: Some("chatgpt"),
        auth_retry_after_unauthorized: Some(true),
        auth_recovery_mode: Some("managed"),
        auth_recovery_phase: Some("refresh_token"),
        auth_connection_reused: None,
        auth_request_id: Some("req-123"),
        auth_cf_ray: Some("ray-123"),
        auth_error: Some("missing_authorization_header"),
        auth_error_code: Some("token_expired"),
        auth_recovery_followup_success: Some(false),
        auth_recovery_followup_status: Some(401),
    });

    let tags = tags.lock().unwrap().clone();
    assert_eq!(
        tags.get("auth_request_id").map(String::as_str),
        Some("\"req-123\"")
    );
    assert_eq!(
        tags.get("auth_cf_ray").map(String::as_str),
        Some("\"ray-123\"")
    );
    assert_eq!(
        tags.get("auth_error").map(String::as_str),
        Some("\"missing_authorization_header\"")
    );
    assert_eq!(
        tags.get("auth_error_code").map(String::as_str),
        Some("\"token_expired\"")
    );
    assert_eq!(
        tags.get("auth_recovery_followup_success")
            .map(String::as_str),
        Some("\"false\"")
    );
    assert_eq!(*event_count.lock().unwrap(), 1);
}

#[test]
#[serial(auth_failure_reporter)]
fn emit_sentry_auth_failure_event_keeps_post_refresh_retry_reports_when_override_present() {
    let (reported, _reporter_guard) = install_auth_failure_report_collector();

    let auth_env = AuthEnvTelemetry {
        openai_api_key_env_present: true,
        codex_api_key_env_present: false,
        codex_api_key_env_enabled: true,
        provider_env_key_name: Some("configured".to_string()),
        provider_env_key_present: Some(true),
        refresh_token_url_override_present: true,
    };

    emit_sentry_auth_failure_event_with_auth_env(
        &FeedbackRequestTags {
            endpoint: "/responses",
            auth_header_attached: true,
            auth_header_name: Some("authorization"),
            auth_mode: Some("chatgpt"),
            auth_retry_after_unauthorized: Some(false),
            auth_recovery_mode: Some("managed"),
            auth_recovery_phase: Some("refresh_token"),
            auth_connection_reused: Some(false),
            auth_request_id: Some("req-123"),
            auth_cf_ray: Some("ray-123"),
            auth_error: Some("missing_authorization_header"),
            auth_error_code: Some("token_expired"),
            auth_recovery_followup_success: None,
            auth_recovery_followup_status: None,
        },
        &auth_env,
    );

    let reported = reported.lock().expect("report collector poisoned");
    assert_eq!(reported.len(), 1);
    assert_eq!(
        reported[0].get("endpoint").map(String::as_str),
        Some("/responses")
    );
    assert_eq!(
        reported[0].get("auth_recovery_phase").map(String::as_str),
        Some("refresh_token")
    );
    assert_eq!(
        reported[0].get("auth_request_id").map(String::as_str),
        Some("req-123")
    );
}

#[test]
#[serial(auth_failure_reporter)]
fn emit_sentry_auth_failure_event_keeps_non_refresh_reports_when_override_present() {
    let (reported, _reporter_guard) = install_auth_failure_report_collector();

    let auth_env = AuthEnvTelemetry {
        openai_api_key_env_present: true,
        codex_api_key_env_present: false,
        codex_api_key_env_enabled: true,
        provider_env_key_name: Some("configured".to_string()),
        provider_env_key_present: Some(true),
        refresh_token_url_override_present: true,
    };

    emit_sentry_auth_failure_event_with_auth_env(
        &FeedbackRequestTags {
            endpoint: "/responses/compact",
            auth_header_attached: true,
            auth_header_name: Some("authorization"),
            auth_mode: Some("openai_api_key"),
            auth_retry_after_unauthorized: Some(false),
            auth_recovery_mode: None,
            auth_recovery_phase: None,
            auth_connection_reused: None,
            auth_request_id: Some("req-non-refresh"),
            auth_cf_ray: Some("ray-non-refresh"),
            auth_error: Some("missing_authorization_header"),
            auth_error_code: Some("invalid_api_key"),
            auth_recovery_followup_success: None,
            auth_recovery_followup_status: None,
        },
        &auth_env,
    );

    let reported = reported.lock().expect("report collector poisoned");
    assert_eq!(reported.len(), 1);
    assert_eq!(
        reported[0].get("endpoint").map(String::as_str),
        Some("/responses/compact")
    );
    assert_eq!(
        reported[0].get("auth_request_id").map(String::as_str),
        Some("req-non-refresh")
    );
}

#[test]
#[serial(auth_failure_reporter)]
fn emit_sentry_auth_failure_event_omits_missing_followup_metadata() {
    let (reported, _reporter_guard) = install_auth_failure_report_collector();

    emit_sentry_auth_failure_event_with_auth_env(
        &FeedbackRequestTags {
            endpoint: "/responses/compact",
            auth_header_attached: true,
            auth_header_name: Some("authorization"),
            auth_mode: Some("chatgpt"),
            auth_retry_after_unauthorized: Some(false),
            auth_recovery_mode: None,
            auth_recovery_phase: None,
            auth_connection_reused: None,
            auth_request_id: Some("req-no-followup"),
            auth_cf_ray: Some("ray-no-followup"),
            auth_error: Some("plain body"),
            auth_error_code: Some("token_expired"),
            auth_recovery_followup_success: None,
            auth_recovery_followup_status: None,
        },
        &AuthEnvTelemetry {
            openai_api_key_env_present: false,
            codex_api_key_env_present: false,
            codex_api_key_env_enabled: true,
            provider_env_key_name: None,
            provider_env_key_present: None,
            refresh_token_url_override_present: false,
        },
    );

    let reported = reported.lock().expect("report collector poisoned");
    assert_eq!(reported.len(), 1);
    assert_eq!(reported[0].get("auth_recovery_followup_success"), None);
    assert_eq!(reported[0].get("auth_recovery_followup_status"), None);
}

#[test]
#[serial(auth_failure_reporter)]
fn emit_sentry_auth_failure_event_uses_direct_reporter_when_available() {
    let (reported, _reporter_guard) = install_auth_failure_report_collector();

    emit_sentry_auth_failure_event_with_auth_env(
        &FeedbackRequestTags {
            endpoint: "/responses",
            auth_header_attached: true,
            auth_header_name: Some("authorization"),
            auth_mode: Some("chatgpt"),
            auth_retry_after_unauthorized: Some(true),
            auth_recovery_mode: Some("managed"),
            auth_recovery_phase: Some("refresh_token"),
            auth_connection_reused: Some(false),
            auth_request_id: Some("req-direct"),
            auth_cf_ray: Some("ray-direct"),
            auth_error: Some("plain body"),
            auth_error_code: Some("token_expired"),
            auth_recovery_followup_success: Some(false),
            auth_recovery_followup_status: Some(401),
        },
        &AuthEnvTelemetry {
            openai_api_key_env_present: false,
            codex_api_key_env_present: false,
            codex_api_key_env_enabled: true,
            provider_env_key_name: Some("OPENAI_API_KEY".to_string()),
            provider_env_key_present: Some(false),
            refresh_token_url_override_present: false,
        },
    );

    let reported = reported.lock().expect("report collector poisoned");
    assert_eq!(reported.len(), 1);
    assert_eq!(
        reported[0].get("auth_request_id").map(String::as_str),
        Some("req-direct")
    );
    assert_eq!(reported[0].get("auth_error").map(String::as_str), None);
}

#[test]
fn emit_feedback_request_tags_preserves_auth_env_fields_for_legacy_emitters() {
    let tags = Arc::new(Mutex::new(BTreeMap::new()));
    let event_count = Arc::new(Mutex::new(0));
    let _guard = tracing_subscriber::registry()
        .with(TagCollectorLayer {
            target: "feedback_tags",
            tags: tags.clone(),
            event_count: event_count.clone(),
        })
        .set_default();

    let auth_env = AuthEnvTelemetry {
        openai_api_key_env_present: true,
        codex_api_key_env_present: true,
        codex_api_key_env_enabled: true,
        provider_env_key_name: Some("configured".to_string()),
        provider_env_key_present: Some(true),
        refresh_token_url_override_present: true,
    };

    emit_feedback_request_tags_with_auth_env(
        &FeedbackRequestTags {
            endpoint: "/responses",
            auth_header_attached: true,
            auth_header_name: Some("authorization"),
            auth_mode: Some("chatgpt"),
            auth_retry_after_unauthorized: Some(false),
            auth_recovery_mode: Some("managed"),
            auth_recovery_phase: Some("refresh_token"),
            auth_connection_reused: Some(true),
            auth_request_id: Some("req-123"),
            auth_cf_ray: Some("ray-123"),
            auth_error: Some("missing_authorization_header"),
            auth_error_code: Some("token_expired"),
            auth_recovery_followup_success: Some(true),
            auth_recovery_followup_status: Some(200),
        },
        &auth_env,
    );
    emit_feedback_request_tags(&FeedbackRequestTags {
        endpoint: "/responses",
        auth_header_attached: true,
        auth_header_name: None,
        auth_mode: None,
        auth_retry_after_unauthorized: None,
        auth_recovery_mode: None,
        auth_recovery_phase: None,
        auth_connection_reused: None,
        auth_request_id: None,
        auth_cf_ray: None,
        auth_error: None,
        auth_error_code: None,
        auth_recovery_followup_success: None,
        auth_recovery_followup_status: None,
    });

    let tags = tags.lock().unwrap().clone();
    assert_eq!(
        tags.get("auth_header_name").map(String::as_str),
        Some("\"\"")
    );
    assert_eq!(tags.get("auth_mode").map(String::as_str), Some("\"\""));
    assert_eq!(
        tags.get("auth_request_id").map(String::as_str),
        Some("\"\"")
    );
    assert_eq!(tags.get("auth_cf_ray").map(String::as_str), Some("\"\""));
    assert_eq!(tags.get("auth_error").map(String::as_str), Some("\"\""));
    assert_eq!(
        tags.get("auth_error_code").map(String::as_str),
        Some("\"\"")
    );
    assert_eq!(
        tags.get("auth_env_openai_api_key_present")
            .map(String::as_str),
        Some("true")
    );
    assert_eq!(
        tags.get("auth_env_codex_api_key_present")
            .map(String::as_str),
        Some("true")
    );
    assert_eq!(
        tags.get("auth_env_codex_api_key_enabled")
            .map(String::as_str),
        Some("true")
    );
    assert_eq!(
        tags.get("auth_env_provider_key_name").map(String::as_str),
        Some("\"configured\"")
    );
    assert_eq!(
        tags.get("auth_env_provider_key_present")
            .map(String::as_str),
        Some("\"true\"")
    );
    assert_eq!(
        tags.get("auth_env_refresh_token_url_override_present")
            .map(String::as_str),
        Some("true")
    );
    assert_eq!(
        tags.get("auth_recovery_followup_success")
            .map(String::as_str),
        Some("\"\"")
    );
    assert_eq!(
        tags.get("auth_recovery_followup_status")
            .map(String::as_str),
        Some("\"\"")
    );
    assert_eq!(*event_count.lock().unwrap(), 2);
}

#[test]
fn normalize_thread_name_trims_and_rejects_empty() {
    assert_eq!(normalize_thread_name("   "), None);
    assert_eq!(
        normalize_thread_name("  my thread  "),
        Some("my thread".to_string())
    );
}

#[test]
fn resume_command_prefers_name_over_id() {
    let thread_id = ThreadId::from_string("123e4567-e89b-12d3-a456-426614174000").unwrap();
    let command = resume_command(Some("my-thread"), Some(thread_id));
    assert_eq!(command, Some("codex resume my-thread".to_string()));
}

#[test]
fn resume_command_with_only_id() {
    let thread_id = ThreadId::from_string("123e4567-e89b-12d3-a456-426614174000").unwrap();
    let command = resume_command(/*thread_name*/ None, Some(thread_id));
    assert_eq!(
        command,
        Some("codex resume 123e4567-e89b-12d3-a456-426614174000".to_string())
    );
}

#[test]
fn resume_command_with_no_name_or_id() {
    let command = resume_command(/*thread_name*/ None, /*thread_id*/ None);
    assert_eq!(command, None);
}

#[test]
fn resume_command_quotes_thread_name_when_needed() {
    let command = resume_command(Some("-starts-with-dash"), /*thread_id*/ None);
    assert_eq!(
        command,
        Some("codex resume -- -starts-with-dash".to_string())
    );

    let command = resume_command(Some("two words"), /*thread_id*/ None);
    assert_eq!(command, Some("codex resume 'two words'".to_string()));

    let command = resume_command(Some("quote'case"), /*thread_id*/ None);
    assert_eq!(command, Some("codex resume \"quote'case\"".to_string()));
}
