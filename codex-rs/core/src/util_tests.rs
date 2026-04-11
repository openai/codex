use super::*;
use codex_feedback::FeedbackRequestTags;
use codex_feedback::emit_feedback_request_tags;
use codex_feedback::emit_feedback_request_tags_with_auth_env;
use codex_login::AuthEnvTelemetry;
use codex_protocol::protocol::FileChange;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::path::PathBuf;
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
    tags: Arc<Mutex<BTreeMap<String, String>>>,
    event_count: Arc<Mutex<usize>>,
}

impl<S> Layer<S> for TagCollectorLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        if event.metadata().target() != "feedback_tags" {
            return;
        }
        let mut visitor = TagCollectorVisitor::default();
        event.record(&mut visitor);
        self.tags.lock().unwrap().extend(visitor.tags);
        *self.event_count.lock().unwrap() += 1;
    }
}

#[test]
fn emit_feedback_request_tags_records_sentry_feedback_fields() {
    let tags = Arc::new(Mutex::new(BTreeMap::new()));
    let event_count = Arc::new(Mutex::new(0));
    let _guard = tracing_subscriber::registry()
        .with(TagCollectorLayer {
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
fn emit_feedback_request_tags_preserves_auth_env_fields_for_legacy_emitters() {
    let tags = Arc::new(Mutex::new(BTreeMap::new()));
    let event_count = Arc::new(Mutex::new(0));
    let _guard = tracing_subscriber::registry()
        .with(TagCollectorLayer {
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

#[test]
fn decodes_partial_function_call_arguments() {
    let input = r#"{"input":"*** Begin Patch\n*** Add File: hello.txt\n+hello"#;
    assert_eq!(
        apply_patch_text_from_tool_input_prefix(input),
        Some("*** Begin Patch\n*** Add File: hello.txt\n+hello".to_string())
    );
}

#[test]
fn keeps_raw_custom_tool_input() {
    assert_eq!(
        apply_patch_text_from_tool_input_prefix("*** Begin Patch"),
        Some("*** Begin Patch".to_string())
    );
}

#[test]
fn streams_function_call_apply_patch_changes() {
    let mut stream = ApplyPatchInputStream::default();
    assert!(
        stream
            .push_delta("call-1".to_string(), r#"{"input":"*** Begin Patch\n"#)
            .is_none()
    );

    let event_1 = stream
        .push_delta("call-1".to_string(), r#"*** Add File: hello.txt\n+hello"#)
        .expect("progress event");
    let expected_changes_1 = HashMap::from([(
        PathBuf::from("hello.txt"),
        FileChange::Add {
            content: "hello\n".to_string(),
        },
    )]);
    assert_eq!(event_1.call_id, "call-1");
    assert_eq!(event_1.active_path, Some(PathBuf::from("hello.txt")));
    assert_eq!(event_1.changes, expected_changes_1);

    let event_2 = stream
        .push_delta("call-1".to_string(), r#"\n+world"#)
        .expect("progress event");
    let expected_changes_2 = HashMap::from([(
        PathBuf::from("hello.txt"),
        FileChange::Add {
            content: "hello\nworld\n".to_string(),
        },
    )]);
    assert_eq!(event_2.call_id, "call-1");
    assert_eq!(event_2.active_path, Some(PathBuf::from("hello.txt")));
    assert_eq!(event_2.changes, expected_changes_2);
}

#[test]
fn streams_raw_apply_patch_changes() {
    let mut stream = ApplyPatchInputStream::default();
    assert!(
        stream
            .push_delta("call-1".to_string(), "*** Begin Patch\n")
            .is_none()
    );

    let event = stream
        .push_delta("call-1".to_string(), "*** Add File: hello.txt\n+hello")
        .expect("progress event");
    let expected_changes = HashMap::from([(
        PathBuf::from("hello.txt"),
        FileChange::Add {
            content: "hello\n".to_string(),
        },
    )]);
    assert_eq!(event.call_id, "call-1");
    assert_eq!(event.active_path, Some(PathBuf::from("hello.txt")));
    assert_eq!(event.changes, expected_changes);
}
