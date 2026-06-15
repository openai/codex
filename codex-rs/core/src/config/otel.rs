use std::collections::BTreeMap;
use std::fmt::Display;

use codex_config::RequirementSource;
use codex_config::Sourced;
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

/// Applies required OTEL settings to user-configured OTEL settings.
///
/// Required settings and complete exporters replace their configured values,
/// while span attributes and tracestate are merged by key. Invalid required
/// trace metadata returns an error; configured conflicts produce startup
/// warnings.
pub(super) fn apply_requirement(
    configured: &mut Option<OtelConfigToml>,
    requirement: &Sourced<OtelConfigToml>,
    startup_warnings: &mut Vec<String>,
) -> std::io::Result<()> {
    let Sourced {
        value: required,
        source,
    } = requirement;
    validate_requirement(required)?;

    let OtelConfigToml {
        log_user_prompt,
        environment,
        exporter,
        trace_exporter,
        metrics_exporter,
        span_attributes,
        tracestate,
    } = required;
    let configured = configured.get_or_insert_default();
    let mut conflict = false;

    conflict |= super::requirements::replace_required_leaf(
        &mut configured.log_user_prompt,
        log_user_prompt,
    );
    conflict |=
        super::requirements::replace_required_leaf(&mut configured.environment, environment);
    conflict |= super::requirements::replace_required_leaf(&mut configured.exporter, exporter);
    conflict |=
        super::requirements::replace_required_leaf(&mut configured.trace_exporter, trace_exporter);
    conflict |= super::requirements::replace_required_leaf(
        &mut configured.metrics_exporter,
        metrics_exporter,
    );

    if let Some(required) = span_attributes.as_ref() {
        let configured = configured.span_attributes.get_or_insert_default();
        conflict |= required
            .iter()
            .any(|(key, value)| configured.get(key).is_some_and(|current| current != value));
        configured.extend(required.clone());
    }
    if let Some(required) = tracestate.as_ref() {
        let configured_tracestate = configured.tracestate.take().unwrap_or_default();
        conflict |= required.iter().any(|(member, required_fields)| {
            configured_tracestate
                .get(member)
                .is_some_and(|configured_fields| {
                    required_fields.iter().any(|(key, value)| {
                        configured_fields
                            .get(key)
                            .is_some_and(|current| current != value)
                    })
                })
        });
        configured.tracestate = Some(merge_required_tracestate(
            configured_tracestate,
            required.clone(),
            source,
            startup_warnings,
        ));
    }

    super::requirements::push_structured_requirement_override_warning(
        "otel",
        conflict,
        source,
        startup_warnings,
    );
    Ok(())
}

fn validate_requirement(requirement: &OtelConfigToml) -> std::io::Result<()> {
    if let Some(span_attributes) = requirement.span_attributes.as_ref() {
        codex_otel::validate_span_attributes(span_attributes).map_err(|err| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("invalid required `otel.span_attributes`: {err}"),
            )
        })?;
    }
    if let Some(tracestate) = requirement.tracestate.as_ref() {
        codex_otel::validate_tracestate_entries(tracestate).map_err(|err| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("invalid required `otel.tracestate`: {err}"),
            )
        })?;
    }
    Ok(())
}

/// Merges user-configured tracestate into a validated required base.
///
/// Required member fields always win. Other configured fields are admitted one
/// at a time only when the complete candidate remains W3C-valid; rejected
/// fields produce startup warnings rather than erasing required metadata.
fn merge_required_tracestate(
    configured: BTreeMap<String, BTreeMap<String, String>>,
    required: BTreeMap<String, BTreeMap<String, String>>,
    source: &RequirementSource,
    startup_warnings: &mut Vec<String>,
) -> BTreeMap<String, BTreeMap<String, String>> {
    let mut effective = required.clone();
    for (member_key, configured_fields) in configured {
        for (field_key, value) in configured_fields {
            if required
                .get(&member_key)
                .is_some_and(|required_fields| required_fields.contains_key(&field_key))
            {
                continue;
            }

            let mut candidate = effective.clone();
            candidate
                .entry(member_key.clone())
                .or_default()
                .insert(field_key.clone(), value);
            match codex_otel::validate_tracestate_entries(&candidate) {
                Ok(()) => effective = candidate,
                Err(err) => {
                    let config_key = format!("otel.tracestate.{member_key}.{field_key}");
                    let message = format!(
                        "Ignoring configured `{config_key}` because it would invalidate tracestate required by {source}: {err}"
                    );
                    tracing::warn!("{message}");
                    startup_warnings.push(message);
                }
            }
        }
    }
    effective
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
