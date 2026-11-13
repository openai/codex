//! Read compatible spellings of the shared network policy and preserve source attribution.

use crate::NetworkRequirementsToml;
use crate::RequirementSource;
use crate::Sourced;
use crate::merge::merge_toml_values;
use toml::Value;

use super::layer::ComposableRequirementsLayer;
use super::layer::remove_nested_field_and_prune_empty;
use super::layer::remove_top_level_field;
use super::stack::RequirementsCompositionError;

/// Fold each validated source's network table before the regular merge. The
/// submitted TOML distinguishes empty replacement lists from omitted lists.
pub(super) fn compose(
    layers: &mut [ComposableRequirementsLayer],
) -> Result<Option<Box<Sourced<NetworkRequirementsToml>>>, RequirementsCompositionError> {
    let mut merged = Value::Table(toml::Table::new());
    let mut sources = Vec::new();
    let mut has_legacy = false;
    let mut has_nested = false;
    for layer in layers {
        let regular_toml = &mut layer.regular_toml;
        let legacy = remove_top_level_field(regular_toml, "network_proxy")
            .or_else(|| remove_top_level_field(regular_toml, "experimental_network"));
        let nested = remove_nested_field_and_prune_empty(regular_toml, &["permissions", "network"]);
        has_legacy |= legacy.is_some();
        has_nested |= nested.is_some();
        let value = match (legacy, nested) {
            (Some(Value::Table(legacy)), Some(Value::Table(nested))) => {
                // The effective rules already agree. A deny can shadow an allow,
                // but an explicit empty allowlist clears inherited allows.
                if let (Some(left), Some(right)) = (
                    legacy.get("allowed_domains").and_then(Value::as_array),
                    nested.get("allowed_domains").and_then(Value::as_array),
                ) && left.is_empty() != right.is_empty()
                {
                    return Err(RequirementsCompositionError::Parse {
                        layer_source: layer.source.clone(),
                        message: "legacy and permissions.network requirements specify different allowed_domains replacements".into(),
                    });
                }
                // Keep the legacy spelling for the executor.
                Value::Table(legacy)
            }
            (Some(value), None) | (None, Some(value)) => value,
            (None, None) => continue,
            (Some(_), Some(_)) => {
                return Err(RequirementsCompositionError::Parse {
                    layer_source: layer.source.clone(),
                    message: "shared network requirements must be tables".into(),
                });
            }
        };
        sources.push(layer.source.clone());
        merge_toml_values(&mut merged, &value);
    }
    if sources.is_empty() {
        return Ok(None);
    }
    // Until list composition can preserve competing replacements, require
    // multiple contributing sources to use the same surface.
    let mixed_sources = sources.len() > 1 && has_legacy && has_nested;
    let source = RequirementSource::composite(sources.into_iter().rev());
    if mixed_sources {
        return Err(RequirementsCompositionError::Parse {
            layer_source: source,
            message: "mixed legacy and permissions.network requirements".to_string(),
        });
    }
    let value = merged.try_into().map_err(|err: toml::de::Error| {
        RequirementsCompositionError::ComposedParse {
            message: err.to_string(),
        }
    })?;
    Ok(Some(Box::new(Sourced::new(value, source))))
}
