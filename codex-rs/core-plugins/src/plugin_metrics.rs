use crate::script_attribution::normalized_relative_script_path;
use codex_plugin::PluginId;
use codex_protocol::items::is_safe_plugin_relative_path;
use codex_utils_absolute_path::AbsolutePathBuf;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fs::File;
use std::io::Read;

const ANALYTICS_MANIFEST_FILE: &str = "analytics.yaml";
const MAX_ANALYTICS_MANIFEST_BYTES: u64 = 64 * 1024;
const MAX_IDENTIFIER_LEN: usize = 64;
const MAX_DIMENSIONS_PER_MEASUREMENT: usize = 8;

/// The manifest declaration for one numeric measurement.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PluginMeasurementDefinition {
    pub enum_dimensions: BTreeMap<String, BTreeSet<String>>,
}

/// Custom metrics allowed for one trusted plugin script operation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PluginMetricsOperation {
    pub operation_name: String,
    pub measurements: BTreeMap<String, PluginMeasurementDefinition>,
}

/// A metrics operation bound to identity from a fresh trusted command lookup.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedPluginMetricsOperation {
    pub plugin_id: PluginId,
    pub operation: PluginMetricsOperation,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AnalyticsManifest {
    version: u32,
    #[serde(with = "serde_with::rust::maps_duplicate_key_is_error")]
    operations: BTreeMap<String, OperationDeclaration>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OperationDeclaration {
    path: String,
    #[serde(with = "serde_with::rust::maps_duplicate_key_is_error")]
    measurements: BTreeMap<String, MeasurementDeclaration>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MeasurementDeclaration {
    #[serde(default, with = "serde_with::rust::maps_duplicate_key_is_error")]
    dimensions: BTreeMap<String, Vec<String>>,
}

pub(crate) fn load_plugin_metrics_operations(
    plugin_root: &AbsolutePathBuf,
) -> Option<BTreeMap<String, PluginMetricsOperation>> {
    let manifest_path = plugin_root.join(ANALYTICS_MANIFEST_FILE);
    let canonical_manifest_path = manifest_path.canonicalize().ok()?;
    if canonical_manifest_path != manifest_path || !manifest_path.as_path().is_file() {
        return None;
    }

    let mut contents = Vec::new();
    File::open(manifest_path.as_path())
        .ok()?
        .take(MAX_ANALYTICS_MANIFEST_BYTES + 1)
        .read_to_end(&mut contents)
        .ok()?;
    if contents.len() as u64 > MAX_ANALYTICS_MANIFEST_BYTES {
        return None;
    }
    let manifest: AnalyticsManifest = serde_yaml::from_slice(&contents).ok()?;
    validate_manifest(manifest, plugin_root)
}

fn validate_manifest(
    manifest: AnalyticsManifest,
    plugin_root: &AbsolutePathBuf,
) -> Option<BTreeMap<String, PluginMetricsOperation>> {
    if manifest.version != 1 || manifest.operations.is_empty() {
        return None;
    }

    let mut operations_by_path = BTreeMap::new();
    for (operation_name, operation) in manifest.operations {
        if !valid_identifier(&operation_name) || operation.measurements.is_empty() {
            return None;
        }
        let normalized_path = validated_operation_path(plugin_root, &operation.path)?;
        let mut measurements = BTreeMap::new();
        for (measurement_name, measurement) in operation.measurements {
            if !valid_identifier(&measurement_name)
                || measurement.dimensions.len() > MAX_DIMENSIONS_PER_MEASUREMENT
            {
                return None;
            }
            let mut enum_dimensions = BTreeMap::new();
            for (dimension_name, values) in measurement.dimensions {
                if !valid_identifier(&dimension_name) || values.is_empty() {
                    return None;
                }
                let value_count = values.len();
                let values = values.into_iter().collect::<BTreeSet<_>>();
                if values.len() != value_count
                    || values.iter().any(|value| !valid_identifier(value))
                {
                    return None;
                }
                enum_dimensions.insert(dimension_name, values);
            }
            measurements.insert(
                measurement_name,
                PluginMeasurementDefinition { enum_dimensions },
            );
        }
        if operations_by_path
            .insert(
                normalized_path,
                PluginMetricsOperation {
                    operation_name,
                    measurements,
                },
            )
            .is_some()
        {
            return None;
        }
    }
    Some(operations_by_path)
}

fn validated_operation_path(plugin_root: &AbsolutePathBuf, path: &str) -> Option<String> {
    let normalized_path = path.strip_prefix("./").unwrap_or(path);
    if !is_safe_plugin_relative_path(normalized_path) {
        return None;
    }
    let script = plugin_root.join(normalized_path).canonicalize().ok()?;
    if !script.as_path().is_file() {
        return None;
    }
    let canonical_relative_path = script.as_path().strip_prefix(plugin_root.as_path()).ok()?;
    (normalized_relative_script_path(canonical_relative_path)?.as_str() == normalized_path)
        .then(|| normalized_path.to_string())
}

fn valid_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    matches!(chars.next(), Some('a'..='z'))
        && value.len() <= MAX_IDENTIFIER_LEN
        && chars.all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        })
}
