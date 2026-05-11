use crate::install_wfp_filters_for_account;
use crate::setup_error::sanitize_setup_metric_tag_value;
use anyhow::Result;
use codex_otel::OtelExporter;
use codex_otel::OtelProvider;
use codex_otel::OtelSettings;
use codex_otel::StatsigMetricsSettings;
use std::collections::BTreeMap;
use std::path::Path;

const WFP_SETUP_SERVICE_NAME: &str = "codex-windows-sandbox-setup";
const WFP_SETUP_SUCCESS_METRIC: &str = "codex.windows_sandbox.wfp_setup_success";
const WFP_SETUP_FAILURE_METRIC: &str = "codex.windows_sandbox.wfp_setup_failure";

#[derive(Debug, Clone, Copy)]
enum WfpSetupMetricOutcome {
    Success,
    Failure,
}

struct WfpSetupMetric {
    outcome: WfpSetupMetricOutcome,
    target_account: String,
    installed_filter_count: usize,
    error: Option<String>,
}

fn panic_payload_to_string(panic_payload: Box<dyn std::any::Any + Send>) -> String {
    match panic_payload.downcast::<String>() {
        Ok(message) => *message,
        Err(panic_payload) => match panic_payload.downcast::<&'static str>() {
            Ok(message) => (*message).to_string(),
            Err(_) => "unknown panic payload".to_string(),
        },
    }
}

fn build_wfp_metrics_provider(
    codex_home: &Path,
    otel: Option<&StatsigMetricsSettings>,
) -> Result<Option<OtelProvider>> {
    let Some(otel) = otel else {
        return Ok(None);
    };
    // The setup helper cannot call codex-core's OTEL builder because core
    // depends on this crate, so the parent process passes only the resolved
    // Statsig environment in the elevation payload. Other exporters are
    // intentionally omitted from this helper path.
    OtelProvider::from(&OtelSettings {
        environment: otel.environment.clone(),
        service_name: WFP_SETUP_SERVICE_NAME.to_string(),
        service_version: env!("CARGO_PKG_VERSION").to_string(),
        codex_home: codex_home.to_path_buf(),
        exporter: OtelExporter::None,
        trace_exporter: OtelExporter::None,
        metrics_exporter: OtelExporter::Statsig,
        runtime_metrics: false,
        span_attributes: BTreeMap::new(),
        tracestate: BTreeMap::new(),
    })
    .map_err(|err| anyhow::anyhow!("failed to initialize WFP setup metrics provider: {err}"))
}

fn emit_wfp_setup_metric(
    codex_home: &Path,
    otel: Option<&StatsigMetricsSettings>,
    metric: &WfpSetupMetric,
) -> Result<()> {
    let Some(provider) = build_wfp_metrics_provider(codex_home, otel)? else {
        return Ok(());
    };
    if let Some(metrics) = provider.metrics() {
        let target_account = sanitize_setup_metric_tag_value(&metric.target_account);
        match metric.outcome {
            WfpSetupMetricOutcome::Success => {
                let installed_filter_count = metric.installed_filter_count.to_string();
                metrics.counter(
                    WFP_SETUP_SUCCESS_METRIC,
                    /*inc*/ 1,
                    &[
                        ("target_account", target_account.as_str()),
                        ("installed_filter_count", installed_filter_count.as_str()),
                    ],
                )?;
            }
            WfpSetupMetricOutcome::Failure => {
                let mut tags = vec![("target_account", target_account.as_str())];
                let error_tag = metric.error.as_deref().map(sanitize_setup_metric_tag_value);
                if let Some(error) = error_tag.as_deref() {
                    tags.push(("message", error));
                }
                metrics.counter(WFP_SETUP_FAILURE_METRIC, /*inc*/ 1, &tags)?;
            }
        }
    }
    provider.shutdown();
    Ok(())
}

fn emit_wfp_setup_metric_safely<F>(
    codex_home: &Path,
    otel: Option<&StatsigMetricsSettings>,
    offline_username: &str,
    metric: &WfpSetupMetric,
    log: &mut F,
) where
    F: FnMut(&str),
{
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        emit_wfp_setup_metric(codex_home, otel, metric)
    }));
    match result {
        Ok(Ok(())) => {}
        Ok(Err(err)) => log(&format!(
            "failed to emit WFP setup metric for {offline_username}: {err}"
        )),
        Err(panic_payload) => {
            let error = panic_payload_to_string(panic_payload);
            log(&format!(
                "WFP setup metric emission panicked for {offline_username}: {error}"
            ));
        }
    }
}

pub fn install_wfp_filters<F>(
    codex_home: &Path,
    offline_username: &str,
    otel: Option<&StatsigMetricsSettings>,
    mut log: F,
) -> Result<()>
where
    F: FnMut(&str),
{
    let (metric, install_result) = evaluate_wfp_install(offline_username, &mut log, || {
        install_wfp_filters_for_account(offline_username)
    });

    emit_wfp_setup_metric_safely(codex_home, otel, offline_username, &metric, &mut log);
    install_result
}

fn evaluate_wfp_install<F, I>(
    offline_username: &str,
    log: &mut F,
    install: I,
) -> (WfpSetupMetric, Result<()>)
where
    F: FnMut(&str),
    I: FnOnce() -> Result<usize>,
{
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(install)) {
        Ok(Ok(installed_filter_count)) => {
            log(&format!(
                "WFP setup succeeded for {offline_username} with {installed_filter_count} installed filters"
            ));
            (
                WfpSetupMetric {
                    outcome: WfpSetupMetricOutcome::Success,
                    target_account: offline_username.to_string(),
                    installed_filter_count,
                    error: None,
                },
                Ok(()),
            )
        }
        Ok(Err(err)) => {
            let error = err.to_string();
            log(&format!("WFP setup failed for {offline_username}: {error}"));
            (
                WfpSetupMetric {
                    outcome: WfpSetupMetricOutcome::Failure,
                    target_account: offline_username.to_string(),
                    installed_filter_count: 0,
                    error: Some(error.clone()),
                },
                Err(anyhow::anyhow!(
                    "WFP setup failed for {offline_username}: {error}"
                )),
            )
        }
        Err(panic_payload) => {
            let error = panic_payload_to_string(panic_payload);
            log(&format!(
                "WFP setup panicked for {offline_username}: {error}"
            ));
            (
                WfpSetupMetric {
                    outcome: WfpSetupMetricOutcome::Failure,
                    target_account: offline_username.to_string(),
                    installed_filter_count: 0,
                    error: Some(format!("panic: {error}")),
                },
                Err(anyhow::anyhow!(
                    "WFP setup panicked for {offline_username}: {error}"
                )),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evaluate_wfp_install_returns_ok_after_success() {
        let mut messages = Vec::new();
        let mut log = |message: &str| messages.push(message.to_string());

        let (metric, result) = evaluate_wfp_install("offline", &mut log, || Ok(3));

        assert!(result.is_ok());
        assert!(matches!(metric.outcome, WfpSetupMetricOutcome::Success));
        assert_eq!(metric.installed_filter_count, 3);
        assert_eq!(metric.error, None);
        assert_eq!(
            messages,
            vec!["WFP setup succeeded for offline with 3 installed filters"]
        );
    }

    #[test]
    fn evaluate_wfp_install_returns_err_after_install_failure() {
        let mut messages = Vec::new();
        let mut log = |message: &str| messages.push(message.to_string());

        let (metric, result) =
            evaluate_wfp_install("offline", &mut log, || Err(anyhow::anyhow!("driver error")));

        let error = result.expect_err("install failures must bubble out");
        assert_eq!(
            error.to_string(),
            "WFP setup failed for offline: driver error"
        );
        assert!(matches!(metric.outcome, WfpSetupMetricOutcome::Failure));
        assert_eq!(metric.installed_filter_count, 0);
        assert_eq!(metric.error.as_deref(), Some("driver error"));
        assert_eq!(messages, vec!["WFP setup failed for offline: driver error"]);
    }

    #[test]
    fn evaluate_wfp_install_returns_err_after_install_panic() {
        let mut messages = Vec::new();
        let mut log = |message: &str| messages.push(message.to_string());

        let (metric, result) =
            evaluate_wfp_install("offline", &mut log, || panic!("unexpected installer panic"));

        let error = result.expect_err("installer panics must bubble out");
        assert_eq!(
            error.to_string(),
            "WFP setup panicked for offline: unexpected installer panic"
        );
        assert!(matches!(metric.outcome, WfpSetupMetricOutcome::Failure));
        assert_eq!(metric.installed_filter_count, 0);
        assert_eq!(
            metric.error.as_deref(),
            Some("panic: unexpected installer panic")
        );
        assert_eq!(
            messages,
            vec!["WFP setup panicked for offline: unexpected installer panic"]
        );
    }
}
