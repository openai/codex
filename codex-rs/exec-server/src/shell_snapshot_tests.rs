use std::collections::BTreeMap;
use std::collections::HashMap;

use codex_otel::MetricsClient;
use codex_otel::MetricsConfig;
use codex_protocol::config_types::ShellEnvironmentPolicyInherit;
use opentelemetry_sdk::metrics::InMemoryMetricExporter;
use opentelemetry_sdk::metrics::data::AggregatedMetrics;
use opentelemetry_sdk::metrics::data::MetricData;
use pretty_assertions::assert_eq;
use test_case::test_case;

use super::CapturePurpose;
use super::MAX_SNAPSHOT_ATTEMPTS;
use super::MAX_SNAPSHOT_BYTES;
use super::SNAPSHOT_RETRY_BACKOFF;
use super::ShellSnapshotCache;
use super::parse_snapshot;
use crate::process_sandbox::prepare_exec_request_with_telemetry;
use crate::process_telemetry::ProcessTelemetry;
use crate::protocol::ExecEnvPolicy;
use crate::protocol::ExecParams;
use crate::protocol::ProcessId;
use crate::protocol::ShellInfo;
use crate::protocol::ShellSnapshotRequest;
use crate::telemetry::ExecServerTelemetry;

#[test_case(1, CapturePurpose::Execution; "succeeds_on_first_attempt")]
#[test_case(2, CapturePurpose::Execution; "recovers_on_second_attempt")]
#[test_case(3, CapturePurpose::Execution; "recovers_on_last_attempt")]
#[test_case(4, CapturePurpose::Execution; "stops_after_three_failures")]
#[test_case(2, CapturePurpose::Prewarm; "failed_prewarm_releases_waiting_command")]
#[test_case(3, CapturePurpose::Prewarm; "failed_prewarm_preserves_last_attempt")]
#[test_case(4, CapturePurpose::Prewarm; "failed_prewarm_preserves_retry_limit")]
#[tokio::test]
async fn snapshot_failure_retries_are_bounded_and_single_flight(
    recovery_attempt: usize,
    initial_purpose: CapturePurpose,
) -> anyhow::Result<()> {
    let prewarm_fails_first = initial_purpose == CapturePurpose::Prewarm;
    let home = tempfile::TempDir::new()?;
    let profile = home.path().join(".bashrc");
    std::fs::write(&profile, "printf x >> \"$HOME/captures\"\nexit 7\n")?;
    let params = ExecParams {
        metadata: Default::default(),
        process_id: ProcessId::from("snapshot-retry"),
        argv: vec![
            "/bin/bash".to_string(),
            "-lc".to_string(),
            "true".to_string(),
        ],
        cwd: codex_utils_path_uri::PathUri::from_host_native_path(home.path())?,
        env: HashMap::from([
            (
                "HOME".to_string(),
                home.path().to_string_lossy().into_owned(),
            ),
            ("PATH".to_string(), "/usr/bin:/bin".to_string()),
        ]),
        env_policy: None,
        shell_snapshot: Some(ShellSnapshotRequest {
            scope_id: "attachment-1".to_string(),
            shell: ShellInfo {
                name: "bash".to_string(),
                path: "/bin/bash".to_string(),
            },
        }),
        tty: false,
        pipe_stdin: false,
        arg0: None,
        sandbox: None,
        enforce_managed_network: false,
        managed_network: None,
        network_proxy: None,
    };
    let cache = ShellSnapshotCache::default();
    let metrics = MetricsClient::new(
        MetricsConfig::in_memory(
            "test",
            "codex-exec-server",
            env!("CARGO_PKG_VERSION"),
            InMemoryMetricExporter::default(),
        )
        .with_runtime_reader(),
    )?;
    let telemetry = ExecServerTelemetry::new(metrics.clone());

    for attempt in 1..=5 {
        if attempt == recovery_attempt {
            std::fs::write(
                &profile,
                "printf x >> \"$HOME/captures\"\nprofile_helper() { printf recovered; }\n",
            )?;
        }
        let mut prepared = prepare_exec_request_with_telemetry(
            &params,
            params.env.clone(),
            /*runtime_paths*/ None,
            /*network_policy_decider*/ None,
            /*network_policy_audit_observer*/ None,
            &ProcessTelemetry::default(),
        )
        .await
        .expect("prepare capture");
        let mut concurrent = prepare_exec_request_with_telemetry(
            &params,
            params.env.clone(),
            /*runtime_paths*/ None,
            /*network_policy_decider*/ None,
            /*network_policy_audit_observer*/ None,
            &ProcessTelemetry::default(),
        )
        .await
        .expect("prepare concurrent capture");
        let prewarming = prewarm_fails_first && attempt == 1;
        let purpose = if prewarming {
            CapturePurpose::Prewarm
        } else {
            CapturePurpose::Execution
        };
        let (first, second) = tokio::join!(
            biased;
            cache.prepare(&params, &mut prepared, &telemetry, purpose),
            cache.prepare(&params, &mut concurrent, &telemetry, CapturePurpose::Execution),
        );
        let first = if prewarming {
            first.expect_err("prewarm must report failure without caching it");
            None
        } else {
            first.expect("capture failure must preserve command fallback")
        };
        let second = second.expect("waiting command must complete even when prewarm fails");
        for (prepared, reader) in [(&mut prepared, first), (&mut concurrent, second)] {
            if let Some(reader) = reader {
                let fd = std::os::fd::AsRawFd::as_raw_fd(&reader);
                prepared.command[2] =
                    prepared.command[2].replace(&format!("/dev/fd/{fd}"), "/dev/fd/SNAPSHOT");
            }
        }
        assert_eq!(
            (&prepared.command, &prepared.env),
            (&concurrent.command, &concurrent.env)
        );

        tokio::time::pause();
        if attempt < recovery_attempt || recovery_attempt > MAX_SNAPSHOT_ATTEMPTS {
            cache
                .prepare(
                    &params,
                    &mut prepared,
                    &telemetry,
                    CapturePurpose::Execution,
                )
                .await
                .expect("capture must stay cached during backoff");
            assert_eq!(
                (&prepared.command, &prepared.env),
                (&params.argv, &params.env)
            );
        } else {
            assert_ne!(prepared.command, params.argv);
        }
        assert_eq!(
            std::fs::read_to_string(home.path().join("captures"))?,
            "x".repeat(
                attempt.min(recovery_attempt).min(MAX_SNAPSHOT_ATTEMPTS)
                    + usize::from(prewarm_fails_first)
            )
        );
        tokio::time::advance(SNAPSHOT_RETRY_BACKOFF).await;
        tokio::time::resume();
    }

    let snapshot = metrics.snapshot()?;
    let mut counters = BTreeMap::new();
    let mut durations = BTreeMap::new();
    for metric in snapshot
        .scope_metrics()
        .flat_map(opentelemetry_sdk::metrics::data::ScopeMetrics::metrics)
    {
        match metric.name() {
            "codex.shell_snapshot" => {
                let AggregatedMetrics::U64(MetricData::Sum(sum)) = metric.data() else {
                    panic!("expected shell snapshot counter");
                };
                for point in sum.data_points() {
                    let tags = point
                        .attributes()
                        .map(|attribute| (attribute.key.to_string(), attribute.value.to_string()))
                        .collect::<BTreeMap<_, _>>();
                    counters.insert(tags, point.value());
                }
            }
            "codex.shell_snapshot.duration_ms" => {
                let AggregatedMetrics::F64(MetricData::Histogram(histogram)) = metric.data() else {
                    panic!("expected shell snapshot duration histogram");
                };
                for point in histogram.data_points() {
                    let tags = point
                        .attributes()
                        .map(|attribute| (attribute.key.to_string(), attribute.value.to_string()))
                        .collect::<BTreeMap<_, _>>();
                    durations.insert(tags, point.count());
                }
            }
            _ => {}
        }
    }
    let mut expected_counters = BTreeMap::new();
    let mut expected_durations = BTreeMap::new();
    let captures = prewarm_fails_first
        .then_some(("prewarm", 1))
        .into_iter()
        .chain(
            (1..=recovery_attempt.min(MAX_SNAPSHOT_ATTEMPTS)).map(|attempt| ("execution", attempt)),
        );
    for (purpose, attempt) in captures {
        let success = purpose == "execution" && attempt == recovery_attempt;
        let mut tags = BTreeMap::from([
            ("version".to_string(), "v2".to_string()),
            ("success".to_string(), success.to_string()),
            ("purpose".to_string(), purpose.to_string()),
            ("attempt".to_string(), attempt.to_string()),
            ("shell".to_string(), "bash".to_string()),
            ("sandbox".to_string(), "none".to_string()),
        ]);
        if !success {
            tags.insert("failure_reason".to_string(), "nonzero_exit".to_string());
        }
        expected_durations.insert(tags.clone(), /*value*/ 1);
        expected_counters.insert(tags, /*value*/ 1);
    }
    assert_eq!(
        (counters, durations),
        (expected_counters, expected_durations)
    );
    Ok(())
}

#[test]
fn snapshot_size_limit_counts_state_and_environment_before_filtering() {
    let half = "x".repeat(MAX_SNAPSHOT_BYTES / 2);
    let oversized = format!("# Snapshot file\n# {half}\n\0\0\0FILTERED={half}\0");
    let policy = ExecEnvPolicy {
        inherit: ShellEnvironmentPolicyInherit::All,
        ignore_default_excludes: false,
        exclude: vec!["FILTERED".to_string()],
        r#set: HashMap::new(),
        include_only: Vec::new(),
    };
    assert!(parse_snapshot(ShellType::Bash, oversized.as_bytes(), Some(&policy)).is_err());
}

#[test]
fn snapshot_filters_profile_exports_after_capture() {
    let policy = ExecEnvPolicy {
        inherit: ShellEnvironmentPolicyInherit::All,
        ignore_default_excludes: false,
        exclude: vec!["PROFILE_DENIED".to_string()],
        r#set: HashMap::from([("PROFILE_ALLOWED".to_string(), "override".to_string())]),
        include_only: vec!["PROFILE_*".to_string()],
    };
    let snapshot = parse_snapshot(
        ShellType::Bash,
        b"profile \xff noise\n# Snapshot file\nfunction profile_helper() { :; }\n\0alias profile_alias='profile_helper'\n\0PROFILE_DENIED\0export PROFILE_DENIED=denied\n\0NON_UTF8\0export NON_UTF8='\xff'\n\0\0PROFILE_ALLOWED=profile\0PROFILE_DENIED=denied\0PROFILE_SECRET=secret\0PWD=/tmp\0NON_UTF8=\xff\0",
        Some(&policy),
    )
    .expect("snapshot should parse");

    assert_eq!(
        snapshot.environment,
        HashMap::from([("PROFILE_ALLOWED".to_string(), "override".to_string())])
    );
    assert_eq!(
        snapshot.state,
        "# Snapshot file\nfunction profile_helper() { :; }\nalias profile_alias='profile_helper'\n"
    );
}

#[test]
fn snapshot_preserves_profile_exports_with_restrictive_inheritance() {
    for inherit in [
        ShellEnvironmentPolicyInherit::None,
        ShellEnvironmentPolicyInherit::Core,
    ] {
        let policy = ExecEnvPolicy {
            inherit,
            ignore_default_excludes: false,
            exclude: vec!["PROFILE_DENIED".to_string()],
            r#set: HashMap::new(),
            include_only: Vec::new(),
        };
        let snapshot = parse_snapshot(
            ShellType::Bash,
            b"# Snapshot file\n\0\0\0PROFILE_ALLOWED=profile\0SDKROOT=/sdk\0PROFILE_SECRET=secret\0PROFILE_DENIED=denied\0",
            Some(&policy),
        )
        .expect("snapshot should parse");

        assert_eq!(
            snapshot.environment,
            HashMap::from([
                ("PROFILE_ALLOWED".to_string(), "profile".to_string()),
                ("SDKROOT".to_string(), "/sdk".to_string()),
            ])
        );
    }
}

#[test]
fn snapshot_caches_only_unmanaged_proxy_state() {
    for (exports, expected) in [
        (
            "PROFILE_ALLOWED=profile\0HTTP_PROXY=http://127.0.0.1:4321\0CODEX_NETWORK_PROXY_ACTIVE=1\0CODEX_NETWORK_PROXY_CREDENTIAL_BROKER_ACTIVE=1\0",
            HashMap::from([("PROFILE_ALLOWED".to_string(), "profile".to_string())]),
        ),
        (
            "PROFILE_ALLOWED=profile\0HTTP_PROXY=http://user-proxy.example\0",
            HashMap::from([
                ("PROFILE_ALLOWED".to_string(), "profile".to_string()),
                (
                    "HTTP_PROXY".to_string(),
                    "http://user-proxy.example".to_string(),
                ),
            ]),
        ),
    ] {
        let output = format!("# Snapshot file\n\0\0\0{exports}");
        let snapshot = parse_snapshot(ShellType::Bash, output.as_bytes(), /*env_policy*/ None)
            .expect("snapshot should parse");

        assert_eq!(snapshot.environment, expected);
    }
}
use codex_shell_command::shell_detect::ShellType;
