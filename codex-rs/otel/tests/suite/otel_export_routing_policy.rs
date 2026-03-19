use codex_otel::AuthEnvTelemetryMetadata;
use codex_otel::OtelProvider;
use codex_otel::SessionTelemetry;
use codex_otel::TelemetryAuthMode;
use opentelemetry::KeyValue;
use opentelemetry::logs::AnyValue;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_sdk::logs::InMemoryLogExporter;
use opentelemetry_sdk::logs::SdkLogRecord;
use opentelemetry_sdk::logs::SdkLoggerProvider;
use opentelemetry_sdk::trace::InMemorySpanExporter;
use opentelemetry_sdk::trace::SdkTracerProvider;
use pretty_assertions::assert_eq;
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::path::PathBuf;
use tracing_subscriber::Layer;
use tracing_subscriber::filter::filter_fn;
use tracing_subscriber::layer::SubscriberExt;

use codex_protocol::ThreadId;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::protocol::SessionSource;
use codex_protocol::user_input::UserInput;

fn log_attributes(record: &SdkLogRecord) -> BTreeMap<String, String> {
    record
        .attributes_iter()
        .map(|(key, value)| (key.as_str().to_string(), any_value_to_string(value)))
        .collect()
}

fn span_event_attributes(event: &opentelemetry::trace::Event) -> BTreeMap<String, String> {
    event
        .attributes
        .iter()
        .map(|KeyValue { key, value, .. }| (key.as_str().to_string(), value.to_string()))
        .collect()
}

fn any_value_to_string(value: &AnyValue) -> String {
    match value {
        AnyValue::Int(value) => value.to_string(),
        AnyValue::Double(value) => value.to_string(),
        AnyValue::String(value) => value.as_str().to_string(),
        AnyValue::Boolean(value) => value.to_string(),
        AnyValue::Bytes(value) => String::from_utf8_lossy(value).into_owned(),
        AnyValue::ListAny(value) => format!("{value:?}"),
        AnyValue::Map(value) => format!("{value:?}"),
        _ => format!("{value:?}"),
    }
}

fn find_log_by_event_name<'a>(
    logs: &'a [opentelemetry_sdk::logs::in_memory_exporter::LogDataWithResource],
    event_name: &str,
) -> &'a opentelemetry_sdk::logs::in_memory_exporter::LogDataWithResource {
    logs.iter()
        .find(|log| {
            log_attributes(&log.record)
                .get("event.name")
                .is_some_and(|value| value == event_name)
        })
        .unwrap_or_else(|| panic!("missing log event: {event_name}"))
}

fn find_span_event_by_name_attr<'a>(
    events: &'a [opentelemetry::trace::Event],
    event_name: &str,
) -> &'a opentelemetry::trace::Event {
    events
        .iter()
        .find(|event| {
            span_event_attributes(event)
                .get("event.name")
                .is_some_and(|value| value == event_name)
        })
        .unwrap_or_else(|| panic!("missing span event: {event_name}"))
}

fn auth_env_metadata() -> AuthEnvTelemetryMetadata {
    AuthEnvTelemetryMetadata {
        openai_api_key_env_present: true,
        codex_api_key_env_present: false,
        codex_api_key_env_enabled: true,
        provider_env_key_name: Some("configured".to_string()),
        provider_env_key_present: Some(true),
        refresh_token_url_override_present: true,
    }
}

#[test]
fn otel_export_routing_policy_routes_user_prompt_log_and_trace_events() {
    let log_exporter = InMemoryLogExporter::default();
    let logger_provider = SdkLoggerProvider::builder()
        .with_simple_exporter(log_exporter.clone())
        .build();
    let span_exporter = InMemorySpanExporter::default();
    let tracer_provider = SdkTracerProvider::builder()
        .with_simple_exporter(span_exporter.clone())
        .build();
    let tracer = tracer_provider.tracer("sink-split-test");

    let subscriber = tracing_subscriber::registry()
        .with(
            opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge::new(
                &logger_provider,
            )
            .with_filter(filter_fn(OtelProvider::log_export_filter)),
        )
        .with(
            tracing_opentelemetry::layer()
                .with_tracer(tracer)
                .with_filter(filter_fn(OtelProvider::trace_export_filter)),
        );

    tracing::subscriber::with_default(subscriber, || {
        tracing::callsite::rebuild_interest_cache();
        let manager = SessionTelemetry::new(
            ThreadId::new(),
            "gpt-5.1",
            "gpt-5.1",
            Some("account-id".to_string()),
            Some("engineer@example.com".to_string()),
            Some(TelemetryAuthMode::ApiKey),
            "codex_exec".to_string(),
            true,
            "tty".to_string(),
            SessionSource::Cli,
        );
        let root_span = tracing::info_span!("root");
        let _root_guard = root_span.enter();
        manager.user_prompt(&[
            UserInput::Text {
                text: "super secret prompt".to_string(),
                text_elements: Vec::new(),
            },
            UserInput::Image {
                image_url: "https://example.com/image.png".to_string(),
            },
            UserInput::LocalImage {
                path: PathBuf::from("/tmp/secret.png"),
            },
        ]);
    });

    logger_provider.force_flush().expect("flush logs");
    tracer_provider.force_flush().expect("flush traces");

    let logs = log_exporter.get_emitted_logs().expect("log export");
    assert!(
        logs.iter()
            .all(|log| { log.record.target().map(Cow::as_ref) == Some("codex_otel.log_only") })
    );

    let prompt_log = find_log_by_event_name(&logs, "codex.user_prompt");
    let prompt_log_attrs = log_attributes(&prompt_log.record);
    assert_eq!(
        prompt_log_attrs.get("prompt").map(String::as_str),
        Some("super secret prompt")
    );
    assert_eq!(
        prompt_log_attrs.get("user.email").map(String::as_str),
        Some("engineer@example.com")
    );

    let spans = span_exporter.get_finished_spans().expect("span export");
    assert_eq!(spans.len(), 1);
    let span_events = &spans[0].events.events;
    assert_eq!(span_events.len(), 1);

    let prompt_trace_event = find_span_event_by_name_attr(span_events, "codex.user_prompt");
    let prompt_trace_attrs = span_event_attributes(prompt_trace_event);
    assert_eq!(
        prompt_trace_attrs.get("prompt_length").map(String::as_str),
        Some("19")
    );
    assert_eq!(
        prompt_trace_attrs
            .get("text_input_count")
            .map(String::as_str),
        Some("1")
    );
    assert_eq!(
        prompt_trace_attrs
            .get("image_input_count")
            .map(String::as_str),
        Some("1")
    );
    assert_eq!(
        prompt_trace_attrs
            .get("local_image_input_count")
            .map(String::as_str),
        Some("1")
    );
    assert!(!prompt_trace_attrs.contains_key("prompt"));
    assert!(!prompt_trace_attrs.contains_key("user.email"));
    assert!(!prompt_trace_attrs.contains_key("user.account_id"));
}

#[test]
fn otel_export_routing_policy_routes_tool_result_log_and_trace_events() {
    let log_exporter = InMemoryLogExporter::default();
    let logger_provider = SdkLoggerProvider::builder()
        .with_simple_exporter(log_exporter.clone())
        .build();
    let span_exporter = InMemorySpanExporter::default();
    let tracer_provider = SdkTracerProvider::builder()
        .with_simple_exporter(span_exporter.clone())
        .build();
    let tracer = tracer_provider.tracer("sink-split-test");

    let subscriber = tracing_subscriber::registry()
        .with(
            opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge::new(
                &logger_provider,
            )
            .with_filter(filter_fn(OtelProvider::log_export_filter)),
        )
        .with(
            tracing_opentelemetry::layer()
                .with_tracer(tracer)
                .with_filter(filter_fn(OtelProvider::trace_export_filter)),
        );

    tracing::subscriber::with_default(subscriber, || {
        tracing::callsite::rebuild_interest_cache();
        let manager = SessionTelemetry::new(
            ThreadId::new(),
            "gpt-5.1",
            "gpt-5.1",
            Some("account-id".to_string()),
            Some("engineer@example.com".to_string()),
            Some(TelemetryAuthMode::ApiKey),
            "codex_exec".to_string(),
            true,
            "tty".to_string(),
            SessionSource::Cli,
        );
        let root_span = tracing::info_span!("root");
        let _root_guard = root_span.enter();
        manager.tool_result_with_tags(
            "shell",
            "call-1",
            "secret arguments",
            std::time::Duration::from_millis(42),
            true,
            "secret output\nsecond line",
            &[],
            Some("internal-mcp"),
            Some("stdio"),
        );
    });

    logger_provider.force_flush().expect("flush logs");
    tracer_provider.force_flush().expect("flush traces");

    let logs = log_exporter.get_emitted_logs().expect("log export");
    assert!(
        logs.iter()
            .all(|log| { log.record.target().map(Cow::as_ref) == Some("codex_otel.log_only") })
    );

    let tool_log = find_log_by_event_name(&logs, "codex.tool_result");
    let tool_log_attrs = log_attributes(&tool_log.record);
    assert_eq!(
        tool_log_attrs.get("arguments").map(String::as_str),
        Some("secret arguments")
    );
    assert_eq!(
        tool_log_attrs.get("output").map(String::as_str),
        Some("secret output\nsecond line")
    );
    assert_eq!(
        tool_log_attrs.get("mcp_server").map(String::as_str),
        Some("internal-mcp")
    );

    let spans = span_exporter.get_finished_spans().expect("span export");
    assert_eq!(spans.len(), 1);
    let span_events = &spans[0].events.events;
    assert_eq!(span_events.len(), 1);

    let tool_trace_event = find_span_event_by_name_attr(span_events, "codex.tool_result");
    let tool_trace_attrs = span_event_attributes(tool_trace_event);
    assert_eq!(
        tool_trace_attrs.get("arguments_length").map(String::as_str),
        Some("16")
    );
    assert_eq!(
        tool_trace_attrs.get("output_length").map(String::as_str),
        Some("25")
    );
    assert_eq!(
        tool_trace_attrs
            .get("output_line_count")
            .map(String::as_str),
        Some("2")
    );
    assert_eq!(
        tool_trace_attrs.get("tool_origin").map(String::as_str),
        Some("mcp")
    );
    assert_eq!(
        tool_trace_attrs.get("mcp_tool").map(String::as_str),
        Some("true")
    );
    assert!(!tool_trace_attrs.contains_key("arguments"));
    assert!(!tool_trace_attrs.contains_key("output"));
    assert!(!tool_trace_attrs.contains_key("mcp_server"));
    assert!(!tool_trace_attrs.contains_key("mcp_server_origin"));
}

#[test]
fn otel_export_routing_policy_routes_auth_recovery_log_and_trace_events() {
    let log_exporter = InMemoryLogExporter::default();
    let logger_provider = SdkLoggerProvider::builder()
        .with_simple_exporter(log_exporter.clone())
        .build();
    let span_exporter = InMemorySpanExporter::default();
    let tracer_provider = SdkTracerProvider::builder()
        .with_simple_exporter(span_exporter.clone())
        .build();
    let tracer = tracer_provider.tracer("sink-split-test");

    let subscriber = tracing_subscriber::registry()
        .with(
            opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge::new(
                &logger_provider,
            )
            .with_filter(filter_fn(OtelProvider::log_export_filter)),
        )
        .with(
            tracing_opentelemetry::layer()
                .with_tracer(tracer)
                .with_filter(filter_fn(OtelProvider::trace_export_filter)),
        );

    tracing::subscriber::with_default(subscriber, || {
        tracing::callsite::rebuild_interest_cache();
        let manager = SessionTelemetry::new(
            ThreadId::new(),
            "gpt-5.1",
            "gpt-5.1",
            Some("account-id".to_string()),
            Some("engineer@example.com".to_string()),
            Some(TelemetryAuthMode::Chatgpt),
            "codex_exec".to_string(),
            true,
            "tty".to_string(),
            SessionSource::Cli,
        );
        let root_span = tracing::info_span!("root");
        let _root_guard = root_span.enter();
        manager.record_auth_recovery(
            "managed",
            "reload",
            "recovery_succeeded",
            Some("req-401"),
            Some("ray-401"),
            Some("missing_authorization_header"),
            Some("token_expired"),
            None,
            Some(true),
        );
    });

    logger_provider.force_flush().expect("flush logs");
    tracer_provider.force_flush().expect("flush traces");

    let logs = log_exporter.get_emitted_logs().expect("log export");
    let recovery_log = find_log_by_event_name(&logs, "codex.auth_recovery");
    let recovery_log_attrs = log_attributes(&recovery_log.record);
    assert_eq!(
        recovery_log_attrs.get("auth.mode").map(String::as_str),
        Some("managed")
    );
    assert_eq!(
        recovery_log_attrs.get("auth.step").map(String::as_str),
        Some("reload")
    );
    assert_eq!(
        recovery_log_attrs.get("auth.outcome").map(String::as_str),
        Some("recovery_succeeded")
    );
    assert_eq!(
        recovery_log_attrs
            .get("auth.request_id")
            .map(String::as_str),
        Some("req-401")
    );
    assert_eq!(
        recovery_log_attrs.get("auth.cf_ray").map(String::as_str),
        Some("ray-401")
    );
    assert_eq!(
        recovery_log_attrs.get("auth.error").map(String::as_str),
        Some("missing_authorization_header")
    );
    assert_eq!(
        recovery_log_attrs
            .get("auth.error_code")
            .map(String::as_str),
        Some("token_expired")
    );
    assert_eq!(
        recovery_log_attrs
            .get("auth.state_changed")
            .map(String::as_str),
        Some("true")
    );

    let spans = span_exporter.get_finished_spans().expect("span export");
    assert_eq!(spans.len(), 1);
    let span_events = &spans[0].events.events;
    assert_eq!(span_events.len(), 1);

    let recovery_trace_event = find_span_event_by_name_attr(span_events, "codex.auth_recovery");
    let recovery_trace_attrs = span_event_attributes(recovery_trace_event);
    assert_eq!(
        recovery_trace_attrs.get("auth.mode").map(String::as_str),
        Some("managed")
    );
    assert_eq!(
        recovery_trace_attrs.get("auth.step").map(String::as_str),
        Some("reload")
    );
    assert_eq!(
        recovery_trace_attrs.get("auth.outcome").map(String::as_str),
        Some("recovery_succeeded")
    );
    assert_eq!(
        recovery_trace_attrs
            .get("auth.request_id")
            .map(String::as_str),
        Some("req-401")
    );
    assert_eq!(
        recovery_trace_attrs.get("auth.cf_ray").map(String::as_str),
        Some("ray-401")
    );
    assert_eq!(
        recovery_trace_attrs.get("auth.error").map(String::as_str),
        Some("missing_authorization_header")
    );
    assert_eq!(
        recovery_trace_attrs
            .get("auth.error_code")
            .map(String::as_str),
        Some("token_expired")
    );
    assert_eq!(
        recovery_trace_attrs
            .get("auth.state_changed")
            .map(String::as_str),
        Some("true")
    );
}

#[test]
fn otel_export_routing_policy_routes_api_request_auth_observability() {
    let log_exporter = InMemoryLogExporter::default();
    let logger_provider = SdkLoggerProvider::builder()
        .with_simple_exporter(log_exporter.clone())
        .build();
    let span_exporter = InMemorySpanExporter::default();
    let tracer_provider = SdkTracerProvider::builder()
        .with_simple_exporter(span_exporter.clone())
        .build();
    let tracer = tracer_provider.tracer("sink-split-test");

    let subscriber = tracing_subscriber::registry()
        .with(
            opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge::new(
                &logger_provider,
            )
            .with_filter(filter_fn(OtelProvider::log_export_filter)),
        )
        .with(
            tracing_opentelemetry::layer()
                .with_tracer(tracer)
                .with_filter(filter_fn(OtelProvider::trace_export_filter)),
        );

    tracing::subscriber::with_default(subscriber, || {
        tracing::callsite::rebuild_interest_cache();
        let manager = SessionTelemetry::new(
            ThreadId::new(),
            "gpt-5.1",
            "gpt-5.1",
            Some("account-id".to_string()),
            Some("engineer@example.com".to_string()),
            Some(TelemetryAuthMode::Chatgpt),
            "codex_exec".to_string(),
            true,
            "tty".to_string(),
            SessionSource::Cli,
        )
        .with_auth_env(auth_env_metadata());
        let root_span = tracing::info_span!("root");
        let _root_guard = root_span.enter();
        manager.conversation_starts_with_endpoint_details(
            "openai",
            "chatgpt.com",
            "openai_chatgpt",
            "default",
            true,
            None,
            ReasoningSummary::Auto,
            None,
            None,
            AskForApproval::Never,
            SandboxPolicy::DangerFullAccess,
            Vec::new(),
            None,
        );
        manager.record_api_request_with_endpoint_details(
            1,
            Some(401),
            Some("http 401"),
            std::time::Duration::from_millis(42),
            true,
            Some("authorization"),
            true,
            Some("managed"),
            Some("refresh_token"),
            "/responses",
            Some("openai-project"),
            "chatgpt.com",
            "openai_chatgpt",
            "default",
            true,
            Some("req-401"),
            Some("ray-401"),
            Some("missing_authorization_header"),
            Some("token_expired"),
            Some("workspace_not_authorized_in_region"),
            Some("Workspace is not authorized in this region."),
        );
    });

    logger_provider.force_flush().expect("flush logs");
    tracer_provider.force_flush().expect("flush traces");

    let logs = log_exporter.get_emitted_logs().expect("log export");
    let conversation_log = find_log_by_event_name(&logs, "codex.conversation_starts");
    let conversation_log_attrs = log_attributes(&conversation_log.record);
    assert_eq!(
        conversation_log_attrs
            .get("client_origin")
            .map(String::as_str),
        Some("custom")
    );
    assert_eq!(
        conversation_log_attrs
            .get("base_url_origin")
            .map(String::as_str),
        Some("chatgpt.com")
    );
    assert_eq!(
        conversation_log_attrs.get("host_class").map(String::as_str),
        Some("openai_chatgpt")
    );
    assert_eq!(
        conversation_log_attrs
            .get("base_url_source")
            .map(String::as_str),
        Some("default")
    );
    assert_eq!(
        conversation_log_attrs
            .get("base_url_is_default")
            .map(String::as_str),
        Some("true")
    );
    assert_eq!(
        conversation_log_attrs
            .get("auth.env_openai_api_key_present")
            .map(String::as_str),
        Some("true")
    );
    assert_eq!(
        conversation_log_attrs
            .get("auth.env_provider_key_name")
            .map(String::as_str),
        Some("configured")
    );
    let request_log = find_log_by_event_name(&logs, "codex.api_request");
    let request_log_attrs = log_attributes(&request_log.record);
    assert_eq!(
        request_log_attrs.get("client_origin").map(String::as_str),
        Some("custom")
    );
    assert_eq!(
        request_log_attrs
            .get("provider_header_names")
            .map(String::as_str),
        Some("openai-project")
    );
    assert_eq!(
        request_log_attrs.get("base_url_origin").map(String::as_str),
        Some("chatgpt.com")
    );
    assert_eq!(
        request_log_attrs.get("host_class").map(String::as_str),
        Some("openai_chatgpt")
    );
    assert_eq!(
        request_log_attrs.get("base_url_source").map(String::as_str),
        Some("default")
    );
    assert_eq!(
        request_log_attrs
            .get("base_url_is_default")
            .map(String::as_str),
        Some("true")
    );
    assert_eq!(
        request_log_attrs
            .get("auth.header_attached")
            .map(String::as_str),
        Some("true")
    );
    assert_eq!(
        request_log_attrs
            .get("auth.header_name")
            .map(String::as_str),
        Some("authorization")
    );
    assert_eq!(
        request_log_attrs
            .get("auth.retry_after_unauthorized")
            .map(String::as_str),
        Some("true")
    );
    assert_eq!(
        request_log_attrs
            .get("auth.recovery_mode")
            .map(String::as_str),
        Some("managed")
    );
    assert_eq!(
        request_log_attrs
            .get("auth.recovery_phase")
            .map(String::as_str),
        Some("refresh_token")
    );
    assert_eq!(
        request_log_attrs.get("endpoint").map(String::as_str),
        Some("/responses")
    );
    assert_eq!(
        request_log_attrs
            .get("error_body_class")
            .map(String::as_str),
        Some("workspace_not_authorized_in_region")
    );
    assert_eq!(
        request_log_attrs
            .get("safe_error_message")
            .map(String::as_str),
        Some("Workspace is not authorized in this region.")
    );
    assert_eq!(
        request_log_attrs.get("auth.error").map(String::as_str),
        Some("missing_authorization_header")
    );
    assert_eq!(
        request_log_attrs
            .get("auth.env_codex_api_key_enabled")
            .map(String::as_str),
        Some("true")
    );
    assert_eq!(
        request_log_attrs
            .get("auth.env_refresh_token_url_override_present")
            .map(String::as_str),
        Some("true")
    );

    let spans = span_exporter.get_finished_spans().expect("span export");
    let conversation_trace_event =
        find_span_event_by_name_attr(&spans[0].events.events, "codex.conversation_starts");
    let conversation_trace_attrs = span_event_attributes(conversation_trace_event);
    assert_eq!(
        conversation_trace_attrs
            .get("auth.env_provider_key_present")
            .map(String::as_str),
        Some("true")
    );
    assert_eq!(
        conversation_trace_attrs
            .get("base_url_origin")
            .map(String::as_str),
        Some("chatgpt.com")
    );
    let request_trace_event =
        find_span_event_by_name_attr(&spans[0].events.events, "codex.api_request");
    let request_trace_attrs = span_event_attributes(request_trace_event);
    assert_eq!(
        request_trace_attrs.get("client_origin").map(String::as_str),
        Some("custom")
    );
    assert_eq!(
        request_trace_attrs
            .get("provider_header_names")
            .map(String::as_str),
        Some("openai-project")
    );
    assert_eq!(
        request_trace_attrs
            .get("auth.header_attached")
            .map(String::as_str),
        Some("true")
    );
    assert_eq!(
        request_trace_attrs
            .get("auth.header_name")
            .map(String::as_str),
        Some("authorization")
    );
    assert_eq!(
        request_trace_attrs
            .get("auth.retry_after_unauthorized")
            .map(String::as_str),
        Some("true")
    );
    assert_eq!(
        request_trace_attrs.get("endpoint").map(String::as_str),
        Some("/responses")
    );
    assert_eq!(
        request_trace_attrs
            .get("auth.env_openai_api_key_present")
            .map(String::as_str),
        Some("true")
    );
    assert_eq!(
        request_trace_attrs
            .get("error_body_class")
            .map(String::as_str),
        Some("workspace_not_authorized_in_region")
    );
}

#[test]
fn otel_export_routing_policy_routes_websocket_connect_auth_observability() {
    let log_exporter = InMemoryLogExporter::default();
    let logger_provider = SdkLoggerProvider::builder()
        .with_simple_exporter(log_exporter.clone())
        .build();
    let span_exporter = InMemorySpanExporter::default();
    let tracer_provider = SdkTracerProvider::builder()
        .with_simple_exporter(span_exporter.clone())
        .build();
    let tracer = tracer_provider.tracer("sink-split-test");

    let subscriber = tracing_subscriber::registry()
        .with(
            opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge::new(
                &logger_provider,
            )
            .with_filter(filter_fn(OtelProvider::log_export_filter)),
        )
        .with(
            tracing_opentelemetry::layer()
                .with_tracer(tracer)
                .with_filter(filter_fn(OtelProvider::trace_export_filter)),
        );

    tracing::subscriber::with_default(subscriber, || {
        tracing::callsite::rebuild_interest_cache();
        let manager = SessionTelemetry::new(
            ThreadId::new(),
            "gpt-5.1",
            "gpt-5.1",
            Some("account-id".to_string()),
            Some("engineer@example.com".to_string()),
            Some(TelemetryAuthMode::Chatgpt),
            "codex_exec".to_string(),
            true,
            "tty".to_string(),
            SessionSource::Cli,
        )
        .with_auth_env(auth_env_metadata());
        let root_span = tracing::info_span!("root");
        let _root_guard = root_span.enter();
        manager.record_websocket_connect_with_endpoint_details(
            std::time::Duration::from_millis(17),
            Some(401),
            Some("http 401"),
            true,
            Some("authorization"),
            true,
            Some("managed"),
            Some("reload"),
            "/responses",
            Some("openai-project"),
            "chatgpt.com",
            "openai_chatgpt",
            "default",
            true,
            false,
            Some("req-ws-401"),
            Some("ray-ws-401"),
            Some("missing_authorization_header"),
            Some("token_expired"),
            Some("workspace_not_authorized_in_region"),
            Some("Workspace is not authorized in this region."),
        );
    });

    logger_provider.force_flush().expect("flush logs");
    tracer_provider.force_flush().expect("flush traces");

    let logs = log_exporter.get_emitted_logs().expect("log export");
    let connect_log = find_log_by_event_name(&logs, "codex.websocket_connect");
    let connect_log_attrs = log_attributes(&connect_log.record);
    assert_eq!(
        connect_log_attrs.get("client_origin").map(String::as_str),
        Some("custom")
    );
    assert_eq!(
        connect_log_attrs
            .get("provider_header_names")
            .map(String::as_str),
        Some("openai-project")
    );
    assert_eq!(
        connect_log_attrs.get("base_url_origin").map(String::as_str),
        Some("chatgpt.com")
    );
    assert_eq!(
        connect_log_attrs.get("host_class").map(String::as_str),
        Some("openai_chatgpt")
    );
    assert_eq!(
        connect_log_attrs.get("base_url_source").map(String::as_str),
        Some("default")
    );
    assert_eq!(
        connect_log_attrs
            .get("base_url_is_default")
            .map(String::as_str),
        Some("true")
    );
    assert_eq!(
        connect_log_attrs
            .get("auth.header_attached")
            .map(String::as_str),
        Some("true")
    );
    assert_eq!(
        connect_log_attrs
            .get("auth.header_name")
            .map(String::as_str),
        Some("authorization")
    );
    assert_eq!(
        connect_log_attrs.get("auth.error").map(String::as_str),
        Some("missing_authorization_header")
    );
    assert_eq!(
        connect_log_attrs.get("endpoint").map(String::as_str),
        Some("/responses")
    );
    assert_eq!(
        connect_log_attrs
            .get("auth.connection_reused")
            .map(String::as_str),
        Some("false")
    );
    assert_eq!(
        connect_log_attrs
            .get("error_body_class")
            .map(String::as_str),
        Some("workspace_not_authorized_in_region")
    );
    assert_eq!(
        connect_log_attrs
            .get("safe_error_message")
            .map(String::as_str),
        Some("Workspace is not authorized in this region.")
    );
    assert_eq!(
        connect_log_attrs
            .get("auth.env_provider_key_name")
            .map(String::as_str),
        Some("configured")
    );

    let spans = span_exporter.get_finished_spans().expect("span export");
    let connect_trace_event =
        find_span_event_by_name_attr(&spans[0].events.events, "codex.websocket_connect");
    let connect_trace_attrs = span_event_attributes(connect_trace_event);
    assert_eq!(
        connect_trace_attrs.get("client_origin").map(String::as_str),
        Some("custom")
    );
    assert_eq!(
        connect_trace_attrs
            .get("auth.recovery_phase")
            .map(String::as_str),
        Some("reload")
    );
    assert_eq!(
        connect_trace_attrs
            .get("auth.env_refresh_token_url_override_present")
            .map(String::as_str),
        Some("true")
    );
    assert_eq!(
        connect_trace_attrs
            .get("provider_header_names")
            .map(String::as_str),
        Some("openai-project")
    );
}

#[test]
fn otel_export_routing_policy_routes_websocket_request_transport_observability() {
    let log_exporter = InMemoryLogExporter::default();
    let logger_provider = SdkLoggerProvider::builder()
        .with_simple_exporter(log_exporter.clone())
        .build();
    let span_exporter = InMemorySpanExporter::default();
    let tracer_provider = SdkTracerProvider::builder()
        .with_simple_exporter(span_exporter.clone())
        .build();
    let tracer = tracer_provider.tracer("sink-split-test");

    let subscriber = tracing_subscriber::registry()
        .with(
            opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge::new(
                &logger_provider,
            )
            .with_filter(filter_fn(OtelProvider::log_export_filter)),
        )
        .with(
            tracing_opentelemetry::layer()
                .with_tracer(tracer)
                .with_filter(filter_fn(OtelProvider::trace_export_filter)),
        );

    tracing::subscriber::with_default(subscriber, || {
        tracing::callsite::rebuild_interest_cache();
        let manager = SessionTelemetry::new(
            ThreadId::new(),
            "gpt-5.1",
            "gpt-5.1",
            Some("account-id".to_string()),
            Some("engineer@example.com".to_string()),
            Some(TelemetryAuthMode::Chatgpt),
            "codex_exec".to_string(),
            true,
            "tty".to_string(),
            SessionSource::Cli,
        )
        .with_auth_env(auth_env_metadata());
        let root_span = tracing::info_span!("root");
        let _root_guard = root_span.enter();
        manager.record_websocket_request(
            std::time::Duration::from_millis(23),
            Some("stream error"),
            true,
        );
    });

    logger_provider.force_flush().expect("flush logs");
    tracer_provider.force_flush().expect("flush traces");

    let logs = log_exporter.get_emitted_logs().expect("log export");
    let request_log = find_log_by_event_name(&logs, "codex.websocket_request");
    let request_log_attrs = log_attributes(&request_log.record);
    assert_eq!(
        request_log_attrs
            .get("auth.connection_reused")
            .map(String::as_str),
        Some("true")
    );
    assert_eq!(
        request_log_attrs.get("error.message").map(String::as_str),
        Some("stream error")
    );
    assert_eq!(
        request_log_attrs
            .get("auth.env_openai_api_key_present")
            .map(String::as_str),
        Some("true")
    );

    let spans = span_exporter.get_finished_spans().expect("span export");
    let request_trace_event =
        find_span_event_by_name_attr(&spans[0].events.events, "codex.websocket_request");
    let request_trace_attrs = span_event_attributes(request_trace_event);
    assert_eq!(
        request_trace_attrs
            .get("auth.connection_reused")
            .map(String::as_str),
        Some("true")
    );
    assert_eq!(
        request_trace_attrs
            .get("auth.env_provider_key_present")
            .map(String::as_str),
        Some("true")
    );
}
