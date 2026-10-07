//! Check outcome accounting and timings without installing process-global metrics.

use super::super::backend_tests::deny_glob;
use super::super::backend_tests::sandbox_request;
use super::*;
use codex_otel::MetricsConfig;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::FileSystemSpecialPath;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_sandboxing::SandboxType;
use codex_utils_absolute_path::AbsolutePathBuf;
use opentelemetry_sdk::metrics::InMemoryMetricExporter;
use opentelemetry_sdk::metrics::data::AggregatedMetrics;
use opentelemetry_sdk::metrics::data::MetricData;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use std::fs;

#[test]
fn records_every_outcome_and_times_preparation_without_paths() -> anyhow::Result<()> {
    let metrics = MetricsClient::new(
        MetricsConfig::in_memory(
            "test",
            "sandbox-integrity",
            env!("CARGO_PKG_VERSION"),
            InMemoryMetricExporter::default(),
        )
        .with_runtime_reader(),
    )?;
    let temp = tempfile::tempdir()?;
    let root = AbsolutePathBuf::from_absolute_path(temp.path())?.canonicalize()?;
    let writable = root.join("writable");
    let protected = root.join("protected");
    fs::write(&writable, "fixture")?;
    fs::write(&protected, "fixture")?;
    let policy = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry::new(root.clone().into(), FileSystemAccessMode::Write),
        FileSystemSandboxEntry::new(protected.clone().into(), FileSystemAccessMode::Read),
    ]);
    let telemetry = IntegrityMetrics::new(Some(&metrics), "test", &policy);
    let checker = FileContentsChecker::new(&policy, &root)?;
    for (dependency, path) in [
        ("writable", Ok(writable.into_path_buf())),
        ("protected", Ok(protected.into_path_buf())),
        ("missing", Ok(root.join("missing").into_path_buf())),
        ("directory", Ok(root.to_path_buf())),
    ] {
        let target = DependencyTarget::inspect(path);
        let result = target
            .as_ref()
            .ok()
            .map(|target| checker.check(&target.path));
        telemetry.record_check("contents", dependency, &target, result);
    }
    telemetry.record_check(
        "contents",
        "preparation",
        &DependencyTarget::inspect(Ok(root.join("writable").into_path_buf())),
        /*result*/ None,
    );
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .max_blocking_threads(/*val*/ 1)
        .build()?;
    let broad_policy = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry::new(
            FileSystemPath::Special {
                value: FileSystemSpecialPath::Root,
            },
            FileSystemAccessMode::Write,
        ),
        FileSystemSandboxEntry::new(root.join("protected").into(), FileSystemAccessMode::Deny),
    ]);
    let mut request = sandbox_request(&root, &broad_policy);
    // The same policy is checked when configured, but skipped for an approved escalation.
    runtime.block_on(run_checks_with_metrics(&request, Some(&metrics)));
    request.sandbox_override = SandboxOverride::EscalatedSandboxWithRestrictions;
    runtime.block_on(run_checks_with_metrics(&request, Some(&metrics)));
    request.sandbox_override = SandboxOverride::NoOverride;
    request.sandbox = SandboxType::None;
    runtime.block_on(run_checks_with_metrics(&request, Some(&metrics)));
    request.sandbox = backend::SANDBOX_TYPE;
    request.permission_profile = PermissionProfile::Disabled;
    runtime.block_on(run_checks_with_metrics(&request, Some(&metrics)));
    let mut expected_preparations = 1;
    if cfg!(any(target_os = "linux", target_os = "windows")) {
        let policy = FileSystemSandboxPolicy::restricted(vec![
            FileSystemSandboxEntry::new(
                FileSystemPath::Special {
                    value: FileSystemSpecialPath::Root,
                },
                FileSystemAccessMode::Write,
            ),
            deny_glob(format!("{}/absent/*.key", root.display())),
        ]);
        request.permission_profile =
            PermissionProfile::from_runtime_permissions(&policy, NetworkSandboxPolicy::Restricted);
        runtime.block_on(run_checks_with_metrics(&request, Some(&metrics)));
        // Preparation still runs, but no dependency receives another check result.
        expected_preparations += 1;
    }
    request = sandbox_request(&root, &policy);
    runtime.block_on(async {
        tokio::time::pause();
        // Occupy the only worker; dropping release also unblocks it on assertion failure.
        let (release, wait) = std::sync::mpsc::channel::<()>();
        let (started, ready) = tokio::sync::oneshot::channel();
        let blocker = tokio::task::spawn_blocking(move || {
            let _ = started.send(());
            let _ = wait.recv();
        });
        ready.await?;
        let mut check = Box::pin(run_checks_with_metrics(&request, Some(&metrics)));
        assert!(futures::poll!(&mut check).is_pending());
        tokio::time::advance(CHECK_TIMEOUT + Duration::from_millis(/*millis*/ 1)).await;
        assert!(futures::poll!(&mut check).is_ready());
        drop(release);
        blocker.await?;
        Ok::<_, anyhow::Error>(())
    })?;
    // Wait for the abandoned worker before checking that it emitted no late results.
    drop(runtime);
    expected_preparations += 1;
    let snapshot = metrics.snapshot()?;
    let mut outcomes = BTreeMap::new();
    let mut durations = Vec::new();
    for metric in snapshot
        .scope_metrics()
        .flat_map(opentelemetry_sdk::metrics::data::ScopeMetrics::metrics)
    {
        match metric.data() {
            AggregatedMetrics::U64(MetricData::Sum(sum)) => {
                assert_eq!(metric.name(), "codex.sandbox_integrity.check");
                for point in sum.data_points() {
                    let attributes: BTreeMap<_, _> = point
                        .attributes()
                        .map(|tag| (tag.key.as_str(), tag.value.as_str().into_owned()))
                        .collect();
                    assert_eq!(attributes["checker"], "contents");
                    if attributes["backend"] == "test"
                        || attributes["error_kind"] == "timeout"
                        || matches!(
                            attributes["dependency"].as_str(),
                            "codex" | "sandbox_launcher" | "preparation"
                        )
                    {
                        *outcomes
                            .entry(
                                ["dependency", "outcome", "error_kind"]
                                    .map(|key| attributes[key].clone()),
                            )
                            .or_insert(0) += point.value();
                    }
                }
            }
            AggregatedMetrics::F64(MetricData::Histogram(histogram)) => {
                assert_eq!(
                    histogram
                        .data_points()
                        .map(opentelemetry_sdk::metrics::data::HistogramDataPoint::count)
                        .sum::<u64>(),
                    expected_preparations
                );
                durations.push(metric.name().to_owned());
            }
            _ => panic!("unexpected metric type"),
        }
    }
    let mut expected_outcomes = vec![
        ["codex", "finding", "none"],
        ["writable", "finding", "none"],
        ["protected", "clean", "none"],
        ["missing", "missing", "other"],
        ["directory", "error", "other"],
        ["preparation", "error", "preparation"],
        ["preparation", "error", "timeout"],
    ];
    if matches!(
        backend::SANDBOX_TYPE,
        SandboxType::LinuxSeccomp | SandboxType::WindowsMxc
    ) {
        expected_outcomes.push(["sandbox_launcher", "missing", "other"]);
    }
    assert_eq!(
        outcomes,
        expected_outcomes
            .into_iter()
            .map(|tags| (tags.map(str::to_owned), 1))
            .collect::<BTreeMap<_, _>>()
    );
    durations.sort();
    assert_eq!(
        durations,
        vec![
            "codex.sandbox_integrity.duration_ms",
            "codex.sandbox_integrity.preparation_ms"
        ]
    );
    assert!(!format!("{snapshot:?}").contains(root.as_path().to_str().expect("temporary path")));
    Ok(())
}
