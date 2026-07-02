use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use pretty_assertions::assert_eq;
use tracing::Event;
use tracing::Level;
use tracing::Subscriber;
use tracing::field::Field;
use tracing::field::Visit;
use tracing_subscriber::Layer;
use tracing_subscriber::layer::Context;
use tracing_subscriber::prelude::*;
use tracing_subscriber::registry::LookupSpan;

use super::ExecServerTelemetry;

#[derive(Clone, Debug, PartialEq, Eq)]
struct CapturedEvent {
    level: Level,
    target: String,
    fields: BTreeMap<String, String>,
}

#[derive(Clone, Default)]
struct CaptureLayer {
    events: Arc<Mutex<Vec<CapturedEvent>>>,
}

impl CaptureLayer {
    fn events(&self) -> Vec<CapturedEvent> {
        self.events
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

impl<S> Layer<S> for CaptureLayer
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
{
    fn on_event(&self, event: &Event<'_>, _context: Context<'_, S>) {
        let mut visitor = FieldVisitor::default();
        event.record(&mut visitor);
        self.events
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(CapturedEvent {
                level: *event.metadata().level(),
                target: event.metadata().target().to_string(),
                fields: visitor.fields,
            });
    }
}

#[derive(Default)]
struct FieldVisitor {
    fields: BTreeMap<String, String>,
}

impl Visit for FieldVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.fields
            .insert(field.name().to_string(), format!("{value:?}"));
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.fields
            .insert(field.name().to_string(), value.to_string());
    }

    fn record_f64(&mut self, field: &Field, value: f64) {
        self.fields
            .insert(field.name().to_string(), value.to_string());
    }
}

#[test]
fn exec_server_timing_events_are_structured_info_logs() {
    let capture = CaptureLayer::default();
    let subscriber = tracing_subscriber::registry().with(capture.clone());

    tracing::subscriber::with_default(subscriber, || {
        tracing::callsite::rebuild_interest_cache();
        let telemetry = ExecServerTelemetry::default();
        let request_log_context = telemetry
            .request_log_context(&"request-1", Some("00-trace-parent"))
            .expect("INFO event should be enabled");
        telemetry.request_completed(
            Some(&request_log_context),
            "process/start",
            "success",
            Duration::from_millis(42),
        );
        telemetry.process_started("process-1").finish("success");
    });

    let events = capture.events();
    let request = events
        .iter()
        .find(|event| {
            event
                .fields
                .get("event.name")
                .is_some_and(|name| name == "codex.exec_server_request")
        })
        .expect("request timing event");
    assert_eq!(request.level, Level::INFO);
    assert_eq!(request.target, "codex_exec_server::telemetry");
    assert_eq!(
        request.fields,
        BTreeMap::from([
            ("duration_ms".to_string(), "42".to_string()),
            (
                "event.name".to_string(),
                "codex.exec_server_request".to_string(),
            ),
            (
                "message".to_string(),
                "exec-server request completed".to_string(),
            ),
            ("method".to_string(), "process/start".to_string()),
            ("request_id".to_string(), "request-1".to_string()),
            ("result".to_string(), "success".to_string()),
            ("traceparent".to_string(), "00-trace-parent".to_string()),
        ])
    );

    let process = events
        .iter()
        .find(|event| {
            event
                .fields
                .get("event.name")
                .is_some_and(|name| name == "codex.exec_server_process")
        })
        .expect("process timing event");
    assert_eq!(process.level, Level::INFO);
    assert_eq!(process.target, "codex_exec_server::telemetry");
    assert_eq!(
        process.fields.get("message").map(String::as_str),
        Some("exec-server process completed")
    );
    assert_eq!(
        process.fields.get("process_id").map(String::as_str),
        Some("process-1")
    );
    assert_eq!(process.fields.get("trace_id").map(String::as_str), Some(""));
    assert_eq!(
        process.fields.get("result").map(String::as_str),
        Some("success")
    );
    assert!(process.fields.contains_key("duration_ms"));
}

#[test]
fn disabled_info_events_do_not_capture_process_log_values() {
    let subscriber = tracing_subscriber::registry()
        .with(tracing_subscriber::filter::filter_fn(|_metadata| false));

    tracing::subscriber::with_default(subscriber, || {
        tracing::callsite::rebuild_interest_cache();
        let telemetry = ExecServerTelemetry::default();
        assert!(!telemetry.info_events_enabled());
        let process = telemetry.process_started("process-1");
        assert!(process.log_context.is_none());
    });
}
