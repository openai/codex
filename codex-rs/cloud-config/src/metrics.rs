use codex_config::CloudConfigBundle;

const CLOUD_CONFIG_BUNDLE_FETCH_ATTEMPT_METRIC: &str = "codex.cloud_config_bundle.fetch_attempt";
const CLOUD_CONFIG_BUNDLE_FETCH_FINAL_METRIC: &str = "codex.cloud_config_bundle.fetch_final";
const CLOUD_CONFIG_BUNDLE_LOAD_METRIC: &str = "codex.cloud_config_bundle.load";

pub(crate) fn emit_fetch_attempt_metric(
    trigger: &str,
    attempt: usize,
    outcome: &str,
    status_code: Option<u16>,
) {
    let attempt_tag = attempt.to_string();
    let status_code_tag = status_code_tag(status_code);
    emit_metric(
        CLOUD_CONFIG_BUNDLE_FETCH_ATTEMPT_METRIC,
        vec![
            ("trigger", trigger.to_string()),
            ("attempt", attempt_tag),
            ("outcome", outcome.to_string()),
            ("status_code", status_code_tag),
        ],
    );
}

pub(crate) fn emit_fetch_final_metric(
    trigger: &str,
    outcome: &str,
    reason: &str,
    attempt_count: usize,
    status_code: Option<u16>,
    bundle: Option<&CloudConfigBundle>,
) {
    let attempt_count_tag = attempt_count.to_string();
    let status_code_tag = status_code_tag(status_code);
    emit_metric(
        CLOUD_CONFIG_BUNDLE_FETCH_FINAL_METRIC,
        vec![
            ("trigger", trigger.to_string()),
            ("outcome", outcome.to_string()),
            ("reason", reason.to_string()),
            ("attempt_count", attempt_count_tag),
            ("status_code", status_code_tag),
            ("bundle_shape", bundle_shape_tag(bundle)),
        ],
    );
}

pub(crate) fn emit_load_metric(trigger: &str, outcome: &str, bundle: Option<&CloudConfigBundle>) {
    emit_metric(
        CLOUD_CONFIG_BUNDLE_LOAD_METRIC,
        vec![
            ("trigger", trigger.to_string()),
            ("outcome", outcome.to_string()),
            ("bundle_shape", bundle_shape_tag(bundle)),
        ],
    );
}

pub(crate) fn bundle_shape_tag(bundle: Option<&CloudConfigBundle>) -> String {
    let Some(bundle) = bundle else {
        return "none".to_string();
    };

    let mut sources = Vec::new();
    if !bundle.config_toml.enterprise_managed.is_empty() {
        sources.push("enterprise_config");
    }
    if !bundle.requirements_toml.enterprise_managed.is_empty() {
        sources.push("enterprise_requirements");
    }

    if sources.is_empty() {
        "empty".to_string()
    } else {
        sources.sort_unstable();
        sources.join(",")
    }
}

fn status_code_tag(status_code: Option<u16>) -> String {
    status_code
        .map(|status_code| status_code.to_string())
        .unwrap_or_else(|| "none".to_string())
}

fn emit_metric(metric_name: &str, tags: Vec<(&str, String)>) {
    if let Some(metrics) = codex_otel::global() {
        let tag_refs = tags
            .iter()
            .map(|(key, value)| (*key, value.as_str()))
            .collect::<Vec<_>>();
        let _ = metrics.counter(metric_name, /*inc*/ 1, &tag_refs);
    }
}
