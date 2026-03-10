use codex_otel::SessionTelemetry;
use codex_otel::TelemetryAuthMode;
use codex_otel::otel_provider::OtelProvider;
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
