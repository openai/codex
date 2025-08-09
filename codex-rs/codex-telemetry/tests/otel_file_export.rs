#![cfg(feature = "otel")]

use std::fs;
use std::path::PathBuf;

use codex_telemetry as telemetry;
use tempfile::TempDir;
use tokio;
use tracing_subscriber::prelude::*;

fn latest_trace_file(dir: &PathBuf) -> Option<PathBuf> {
    let traces_dir = dir.join("traces");
    let mut entries: Vec<_> = fs::read_dir(&traces_dir)
        .ok()?
        .filter_map(|e| e.ok())
        .collect();
    if entries.is_empty() {
        return None;
    }
    entries.sort_by_key(|e| e.metadata().and_then(|m| m.modified()).ok());
    entries.last().map(|e| e.path())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_exporter_writes_span_json() {
    // Arrange
    let tmp = TempDir::new().expect("temp dir");
    let codex_home = tmp.path().to_path_buf();

    let (guard, tracer) = telemetry::build_layer(&telemetry::Settings {
        enabled: true,
        exporter: telemetry::Exporter::OtlpFile {
            path: PathBuf::new(),
            rotate_mb: Some(100),
        },
        service_name: "codex-test".to_string(),
        service_version: "0.0.0".to_string(),
        codex_home: Some(codex_home.clone()),
    })
    .expect("build otel layer");

    let otel_layer = tracing_opentelemetry::OpenTelemetryLayer::new(tracer);
    let subscriber = tracing_subscriber::registry().with(otel_layer);

    // Act: create and drop a span within a scoped subscriber.
    tracing::subscriber::with_default(subscriber, || {
        let span = tracing::info_span!("test.span", test_id = %"123");
        let _entered = span.entered();
    });

    // Drop guard to flush provider and batch processor.
    drop(guard);

    // Assert: a traces file exists and contains a JSON line with the span name.
    let file = latest_trace_file(&codex_home).expect("traces file should exist");
    let contents = fs::read_to_string(&file).expect("read traces file");
    assert!(!contents.is_empty(), "traces file should not be empty");

    // Parse first line and check basic OTLP shape and span name.
    let first_line = contents.lines().next().expect("at least one line");
    let v: serde_json::Value = serde_json::from_str(first_line).expect("valid json line");

    let span_name = v["resourceSpans"][0]["scopeSpans"][0]["spans"][0]["name"]
        .as_str()
        .unwrap_or("");
    assert_eq!(span_name, "test.span");

    // Ensure required fields exist
    assert!(v["resourceSpans"][0]["scopeSpans"][0]["spans"][0]["traceId"].is_string());
    assert!(v["resourceSpans"][0]["scopeSpans"][0]["spans"][0]["spanId"].is_string());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn session_span_flushes_on_shutdown() {
    // Arrange
    let tmp = TempDir::new().expect("temp dir");
    let codex_home = tmp.path().to_path_buf();

    let (guard, tracer) = telemetry::build_layer(&telemetry::Settings {
        enabled: true,
        exporter: telemetry::Exporter::OtlpFile {
            path: PathBuf::new(),
            rotate_mb: Some(100),
        },
        service_name: "codex-test".to_string(),
        service_version: "0.0.0".to_string(),
        codex_home: Some(codex_home.clone()),
    })
    .expect("build otel layer");

    let otel_layer = tracing_opentelemetry::OpenTelemetryLayer::new(tracer);
    let subscriber = tracing_subscriber::registry().with(otel_layer);

    // Act: create a session span via helper in a scoped subscriber, then drop guard to flush
    tracing::subscriber::with_default(subscriber, || {
        let span = telemetry::make_session_span("s1", "model-x", "provider-y");
        drop(span);
    });

    // Drop guard to flush provider and batch processor.
    drop(guard);

    // Assert: traces contain a span named "codex.session"
    let file = latest_trace_file(&codex_home).expect("traces file should exist");
    let contents = fs::read_to_string(&file).expect("read traces file");
    let mut found = false;
    for line in contents.lines() {
        let v: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if let Some(name) = v["resourceSpans"][0]["scopeSpans"][0]["spans"][0]["name"].as_str() {
            if name == "codex.session" {
                found = true;
                break;
            }
        }
    }
    assert!(found, "expected a codex.session span to be exported");
}
