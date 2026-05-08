use std::collections::BTreeMap;
use std::fmt::Display;

use codex_config::types::DEFAULT_OTEL_ENVIRONMENT;
use codex_config::types::OtelConfig;
use codex_config::types::OtelConfigToml;
use codex_config::types::OtelExporterKind;

pub(crate) fn resolve_config(
    config: OtelConfigToml,
    startup_warnings: &mut Vec<String>,
) -> OtelConfig {
    let log_user_prompt = config.log_user_prompt.unwrap_or(false);
    let environment = config
        .environment
        .unwrap_or_else(|| DEFAULT_OTEL_ENVIRONMENT.to_string());
    let exporter = config.exporter.unwrap_or(OtelExporterKind::None);
    // OTLP HTTP endpoints are signal-specific in our config, so enabling log
    // export must not implicitly send spans to a /v1/logs endpoint.
    let trace_exporter = config.trace_exporter.unwrap_or(OtelExporterKind::None);
    let metrics_exporter = config.metrics_exporter.unwrap_or(OtelExporterKind::Statsig);
    // Provider initialization installs process-global OTEL state. Sanitize
    // user-editable trace metadata here so malformed config is reported as a
    // startup warning instead of making startup fail.
    let span_attributes = resolve_span_attributes(config.span_attributes, startup_warnings);
    let tracestate = resolve_tracestate(config.tracestate, startup_warnings);

    OtelConfig {
        log_user_prompt,
        environment,
        exporter,
        trace_exporter,
        metrics_exporter,
        span_attributes,
        tracestate,
    }
}

fn resolve_span_attributes(
    span_attributes: Option<BTreeMap<String, String>>,
    startup_warnings: &mut Vec<String>,
) -> BTreeMap<String, String> {
    let Some(span_attributes) = span_attributes else {
        return BTreeMap::new();
    };

    let mut valid_attributes = BTreeMap::new();
    for (key, value) in span_attributes {
        let attribute = BTreeMap::from([(key.clone(), value.clone())]);
        if let Err(err) = codex_otel::validate_span_attributes(&attribute) {
            push_invalid_config_warning("otel.span_attributes", err, startup_warnings);
            continue;
        }
        valid_attributes.insert(key, value);
    }

    valid_attributes
}

fn resolve_tracestate(
    tracestate: Option<BTreeMap<String, BTreeMap<String, String>>>,
    startup_warnings: &mut Vec<String>,
) -> BTreeMap<String, BTreeMap<String, String>> {
    let Some(tracestate) = tracestate else {
        return BTreeMap::new();
    };

    let mut valid_entries = BTreeMap::new();
    for (member_key, fields) in tracestate {
        let fields = resolve_tracestate_member_fields(&member_key, fields, startup_warnings);
        if fields.is_empty() {
            continue;
        }
        if let Err(err) = codex_otel::validate_tracestate_member(&member_key, &fields) {
            push_invalid_config_warning("otel.tracestate", err, startup_warnings);
            continue;
        }
        valid_entries.insert(member_key, fields);
    }

    // Tracestate members can be valid individually while the combined W3C
    // tracestate header is not, so validate the filtered set before handing it
    // to provider initialization.
    if let Err(err) = codex_otel::validate_tracestate_entries(&valid_entries) {
        push_invalid_config_warning("otel.tracestate", err, startup_warnings);
        return BTreeMap::new();
    }

    valid_entries
}

fn resolve_tracestate_member_fields(
    member_key: &str,
    fields: BTreeMap<String, String>,
    startup_warnings: &mut Vec<String>,
) -> BTreeMap<String, String> {
    let mut valid_fields = BTreeMap::new();
    for (field_key, value) in fields {
        let field = BTreeMap::from([(field_key.clone(), value.clone())]);
        if let Err(err) = codex_otel::validate_tracestate_member(member_key, &field) {
            push_invalid_config_warning("otel.tracestate", err, startup_warnings);
            continue;
        }
        valid_fields.insert(field_key, value);
    }
    valid_fields
}

fn push_invalid_config_warning(
    config_key: &str,
    err: impl Display,
    startup_warnings: &mut Vec<String>,
) {
    let message = format!("Ignoring invalid `{config_key}` config: {err}");
    tracing::warn!("{message}");
    startup_warnings.push(message);
}
