#![cfg(feature = "otel")]

use std::fs;
use std::path::Path;
use std::path::PathBuf;

use codex_otel::config::OtelExporter;
use codex_otel::config::OtelSampler;
use codex_otel::config::OtelSettings;
use codex_otel::otel_provider::OtelProvider;
use tempfile::TempDir;
use tracing_subscriber::prelude::*;
use walkdir::WalkDir;

fn latest_trace_file(dir: &Path) -> Option<PathBuf> {
    let traces_dir = dir.join("traces");

    WalkDir::new(&traces_dir)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| {
            let mtime = e.metadata().ok()?.modified().ok()?;
            Some((mtime, e.into_path()))
        })
        .max_by_key(|(mtime, _)| *mtime)
        .map(|(_, path)| path)
}
fn settings(codex_home: PathBuf) -> OtelSettings {
    OtelSettings {
        enabled: true,
        environment: "test".to_string(),
        service_name: "codex-test".to_string(),
        service_version: "0.0.0".to_string(),
        codex_home,
        sampler: OtelSampler::AlwaysOn,
        exporter: OtelExporter::OtlpFile,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_exporter_writes_span_json() {
    let tmp = TempDir::new().expect("temp dir");
    let codex_home = tmp.path().to_path_buf();

    let provider = OtelProvider::from(&settings(codex_home.clone()))
        .unwrap()
        .expect("otel provider");
    let tracer = provider.tracer();

    let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);
    let subscriber = tracing_subscriber::registry().with(otel_layer);

    tracing::subscriber::with_default(subscriber, || {
        let span = tracing::info_span!("test.span", test_id = %"123");
        let _entered = span.entered();
    });

    drop(provider);

    let file = latest_trace_file(&codex_home).expect("traces file should exist");
    let contents = fs::read_to_string(&file).expect("read traces file");
    assert!(!contents.is_empty(), "traces file should not be empty");

    let first_line = contents.lines().next().expect("at least one line");
    let v: serde_json::Value = serde_json::from_str(first_line).expect("valid json line");

    let span_name = v["resourceSpans"][0]["scopeSpans"][0]["spans"][0]["name"]
        .as_str()
        .unwrap_or("");
    assert_eq!(span_name, "test.span");

    assert!(v["resourceSpans"][0]["scopeSpans"][0]["spans"][0]["traceId"].is_string());
    assert!(v["resourceSpans"][0]["scopeSpans"][0]["spans"][0]["spanId"].is_string());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn session_span_flushes_on_shutdown() {
    let tmp = TempDir::new().expect("temp dir");
    let codex_home = tmp.path().to_path_buf();

    let provider = OtelProvider::from(&settings(codex_home.clone()))
        .unwrap()
        .expect("otel provider");
    let tracer = provider.tracer();

    let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);
    let subscriber = tracing_subscriber::registry().with(otel_layer);

    tracing::subscriber::with_default(subscriber, || {
        let span = tracing::info_span!("codex.session", test_id = %"123");
        drop(span);
    });

    drop(provider);

    let file = latest_trace_file(&codex_home).expect("traces file should exist");
    let contents = fs::read_to_string(&file).expect("read traces file");
    let mut found = false;
    for line in contents.lines() {
        let v: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if let Some(name) = v["resourceSpans"][0]["scopeSpans"][0]["spans"][0]["name"].as_str()
            && name == "codex.session"
        {
            found = true;
            break;
        }
    }
    assert!(found, "expected a codex.session span to be exported");
}
