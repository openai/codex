use crate::RequirementSource;
use crate::Sourced;
use crate::types::OtelConfigToml;
use std::collections::BTreeMap;

/// Merges one OTEL requirements layer into an output assembled high-to-low.
///
/// `log_user_prompt` and `environment` use the first value supplied by a
/// higher-priority layer. Each exporter is also selected as a complete value:
/// for example, the endpoint from one layer is never combined with the headers
/// or TLS settings from another. Span attributes and tracestate are merged by
/// key instead.
pub(super) fn merge(
    output: &mut Option<Sourced<OtelConfigToml>>,
    incoming: Option<OtelConfigToml>,
    source: &RequirementSource,
) {
    let Some(incoming) = incoming.filter(|value| value != &OtelConfigToml::default()) else {
        return;
    };
    let Some(output) = output.as_mut() else {
        *output = Some(Sourced::new(incoming, source.clone()));
        return;
    };

    let OtelConfigToml {
        log_user_prompt,
        environment,
        exporter,
        trace_exporter,
        metrics_exporter,
        span_attributes,
        tracestate,
    } = incoming;

    macro_rules! fill_exact_leaf {
        ($field:ident) => {
            if output.value.$field.is_none() {
                output.value.$field = $field;
            }
        };
    }

    fill_exact_leaf!(log_user_prompt);
    fill_exact_leaf!(environment);
    fill_exact_leaf!(exporter);
    fill_exact_leaf!(trace_exporter);
    fill_exact_leaf!(metrics_exporter);
    merge_map(&mut output.value.span_attributes, span_attributes);
    merge_nested_map(&mut output.value.tracestate, tracestate);
    super::stack::merge_output_source(&mut output.source, source);
}

fn merge_map(
    output: &mut Option<BTreeMap<String, String>>,
    incoming: Option<BTreeMap<String, String>>,
) {
    let Some(incoming) = incoming else {
        return;
    };
    let output = output.get_or_insert_default();
    for (key, value) in incoming {
        output.entry(key).or_insert(value);
    }
}

fn merge_nested_map(
    output: &mut Option<BTreeMap<String, BTreeMap<String, String>>>,
    incoming: Option<BTreeMap<String, BTreeMap<String, String>>>,
) {
    let Some(incoming) = incoming else {
        return;
    };
    let output = output.get_or_insert_default();
    for (member, fields) in incoming {
        let output_fields = output.entry(member).or_default();
        for (key, value) in fields {
            output_fields.entry(key).or_insert(value);
        }
    }
}
