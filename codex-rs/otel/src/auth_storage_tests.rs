//! Exercises buffered storage observations and bounded diagnostics.

use super::*;
use crate::MetricsClient;
use crate::MetricsConfig;
use opentelemetry_sdk::metrics::InMemoryMetricExporter;
use opentelemetry_sdk::metrics::data::AggregatedMetrics;
use opentelemetry_sdk::metrics::data::MetricData;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;

// Provider installation is process-global. Isolate these cases even under Bazel/libtest,
// which otherwise runs all unit tests in a shared process.
fn run_in_subprocess() -> bool {
    if std::env::var_os("CODEX_AUTH_STORAGE_TEST_CHILD").is_some() {
        return false;
    }
    let thread = std::thread::current();
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", thread.name().unwrap(), "--nocapture"])
        .env("CODEX_AUTH_STORAGE_TEST_CHILD", "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    true
}

#[test]
fn startup_fallback_exports_one_logical_operation_without_error_contents() -> crate::Result<()> {
    if run_in_subprocess() {
        return Ok(());
    }
    let mut telemetry = AuthStorageOriginator::from_client_name("Codex Desktop").sync_scope(|| {
        StorageTelemetry::new(
            CredentialKind::Codex,
            StoreMode::Auto,
            Store::Secrets,
            Operation::Save,
            AuthStorageOriginator::current(),
        )
    });
    let logs = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let writer = LogWriter(logs.clone());
    let subscriber = tracing_subscriber::fmt()
        .without_time()
        .with_ansi(false)
        .with_target(false)
        .with_writer(move || writer.clone())
        .finish();
    let failure: Result<(), &str> = Err("sensitive error contents");
    telemetry.record_save_attempt(Store::Secrets, &failure);
    let success: Result<(), &str> = Ok(());
    telemetry.record_save_attempt(Store::File, &success);
    telemetry.record_secure_error(&std::io::Error::new(
        std::io::ErrorKind::PermissionDenied,
        "private account",
    ));
    tracing::subscriber::with_default(subscriber, || drop(telemetry));
    let logs = String::from_utf8(logs.lock().unwrap().clone()).unwrap();
    assert_eq!(logs.lines().count(), 1);
    assert!(logs.contains("secure_error=\"access_denied\""), "{logs}");
    assert!(logs.contains("originator=\"codex_desktop\""), "{logs}");
    assert!(logs.contains("outcome=\"success\""), "{logs}");
    assert!(!logs.contains("private account") && !logs.contains("sensitive error contents"));

    let metrics = crate::install_global_metrics(MetricsClient::new(
        MetricsConfig::in_memory("test", "test", "1", InMemoryMetricExporter::default())
            .with_runtime_reader(),
    )?);
    let snapshot = metrics.snapshot()?;
    let mut counts = Vec::new();
    let mut durations = Vec::new();
    for metric in snapshot
        .scope_metrics()
        .flat_map(opentelemetry_sdk::metrics::data::ScopeMetrics::metrics)
    {
        match metric.data() {
            AggregatedMetrics::U64(MetricData::Sum(sum)) => {
                assert_eq!(metric.name(), "codex.auth_storage.operation");
                for point in sum.data_points() {
                    counts.push((
                        point.value(),
                        point
                            .attributes()
                            .map(|attribute| {
                                (attribute.key.to_string(), attribute.value.to_string())
                            })
                            .collect::<BTreeMap<_, _>>(),
                    ));
                }
            }
            AggregatedMetrics::F64(MetricData::Histogram(histogram)) => {
                assert_eq!(metric.name(), "codex.auth_storage.duration");
                durations.extend(
                    histogram
                        .data_points()
                        .map(opentelemetry_sdk::metrics::data::HistogramDataPoint::count),
                );
            }
            _ => panic!("unexpected metric"),
        }
    }
    assert_eq!(
        counts,
        vec![(
            1,
            BTreeMap::from([
                ("credential_kind".into(), "codex".into()),
                ("store_mode".into(), "auto".into()),
                ("selected_store".into(), "secrets".into()),
                ("actual_store".into(), "file".into()),
                ("operation".into(), "save".into()),
                ("secure_outcome".into(), "error".into()),
                ("outcome".into(), "success".into()),
                ("fallback_reason".into(), "secure_error".into()),
                ("secure_error".into(), "access_denied".into()),
                ("storage_phase".into(), "policy".into()),
                ("originator".into(), "codex_desktop".into()),
            ])
        )]
    );
    assert_eq!(durations, vec![1]);
    drop(StorageTelemetry::new(
        CredentialKind::Codex,
        StoreMode::Auto,
        Store::Secrets,
        Operation::Save,
        AuthStorageOriginator::current(),
    ));
    assert!(
        metrics
            .snapshot()?
            .scope_metrics()
            .all(|scope| scope.metrics().next().is_none())
    );
    StorageTelemetry::new(
        CredentialKind::Mcp,
        StoreMode::Auto,
        Store::Secrets,
        Operation::RefreshPersist,
        AuthStorageOriginator::current(),
    )
    .record_save_attempt(Store::Secrets, &Ok::<(), ()>(()));
    let snapshot = metrics.snapshot()?;
    let mut names: Vec<_> = snapshot
        .scope_metrics()
        .flat_map(opentelemetry_sdk::metrics::data::ScopeMetrics::metrics)
        .map(opentelemetry_sdk::metrics::data::Metric::name)
        .collect();
    names.sort();
    assert_eq!(
        names,
        vec![
            "codex.auth_storage.refresh_persist",
            "codex.auth_storage.refresh_persist.duration"
        ]
    );
    metrics.shutdown()
}

#[test]
fn missing_credentials_and_failed_fallback_keep_distinct_outcomes() {
    if run_in_subprocess() {
        return;
    }
    let mut telemetry = StorageTelemetry::new(
        CredentialKind::Mcp,
        StoreMode::Auto,
        Store::DirectKeyring,
        Operation::Load,
        AuthStorageOriginator::current(),
    );
    let missing: Result<Option<String>, &str> = Ok(None);
    telemetry.record_load_attempt(Store::DirectKeyring, &missing);
    let failure: Result<Option<String>, &str> = Err("file unavailable");
    telemetry.record_load_attempt(Store::File, &failure);
    assert_eq!(
        (
            telemetry.observation.secure_outcome,
            telemetry.observation.outcome,
            telemetry.observation.actual_store
        ),
        (Outcome::NotFound, Outcome::Error, Store::File)
    );
}

#[test]
fn secure_error_classification_is_independent_of_attempt_recording_order() {
    if run_in_subprocess() {
        return;
    }
    for store in [Store::DirectKeyring, Store::Secrets] {
        for classify_first in [true, false] {
            let mut telemetry = StorageTelemetry::new(
                CredentialKind::Codex,
                StoreMode::Auto,
                store,
                Operation::Save,
                AuthStorageOriginator::current(),
            );
            let failure = Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "private account",
            ));
            if classify_first {
                telemetry.record_secure_error(failure.as_ref().unwrap_err());
            }
            telemetry.record_save_attempt(store, &failure);
            if !classify_first {
                telemetry.record_secure_error(failure.as_ref().unwrap_err());
            }
            telemetry.record_save_attempt(Store::File, &Ok::<(), ()>(()));
            assert_eq!(
                (
                    telemetry.observation.secure_outcome,
                    telemetry.observation.outcome,
                    telemetry.observation.secure_error,
                ),
                (Outcome::Error, Outcome::Success, "access_denied")
            );
            telemetry.record_save_attempt(store, &Ok::<(), ()>(()));
            assert_eq!(
                (
                    telemetry.observation.secure_outcome,
                    telemetry.observation.secure_error,
                ),
                (Outcome::Success, "none")
            );
        }
    }
}

#[derive(Clone)]
struct LogWriter(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);
impl std::io::Write for LogWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
