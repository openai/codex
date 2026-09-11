//! Strict config validation built on top of serde's ignored-field tracking.

use crate::CONFIG_TOML_FILE;
use crate::ConfigLayerStack;
use crate::ConfigRequirementsToml;
use crate::RequirementsLayerEntry;
use crate::config_toml::ConfigToml;
use crate::diagnostics::ConfigDiagnosticSource;
use crate::diagnostics::ConfigError;
use crate::diagnostics::config_error_from_toml_for_source;
use crate::diagnostics::default_range;
use crate::diagnostics::span_for_config_path;
use crate::diagnostics::span_for_toml_key_path;
use crate::diagnostics::text_range_from_span;
use crate::format_config_layer_source;
use crate::requirements_layers::strip_cloud_auth_requirements;
use codex_features::is_known_feature_key;
use codex_utils_absolute_path::AbsolutePathBufGuard;
use serde::de::DeserializeOwned;
use std::collections::BTreeSet;
use std::fmt::Write;
use std::path::Path;
use toml::Value as TomlValue;

pub fn config_error_from_ignored_toml_fields<T: DeserializeOwned>(
    path: impl AsRef<Path>,
    contents: &str,
) -> Option<ConfigError> {
    let source = ConfigDiagnosticSource::Path(path.as_ref());
    match toml::from_str::<TomlValue>(contents) {
        Ok(value) => {
            config_error_from_ignored_toml_value_fields_for_source::<T>(source, contents, value)
        }
        Err(err) => Some(config_error_from_toml_for_source(source, contents, err)),
    }
}

pub(crate) fn config_error_from_ignored_toml_value_fields<T: DeserializeOwned>(
    path: impl AsRef<Path>,
    contents: &str,
    value: TomlValue,
) -> Option<ConfigError> {
    config_error_from_ignored_toml_value_fields_for_source::<T>(
        ConfigDiagnosticSource::Path(path.as_ref()),
        contents,
        value,
    )
}

pub(crate) fn config_error_from_ignored_toml_value_fields_for_source_name<T: DeserializeOwned>(
    source_name: &str,
    contents: &str,
    value: TomlValue,
) -> Option<ConfigError> {
    config_error_from_ignored_toml_value_fields_for_source::<T>(
        ConfigDiagnosticSource::DisplayName(source_name),
        contents,
        value,
    )
}

fn config_error_from_ignored_toml_value_fields_for_source<T: DeserializeOwned>(
    source: ConfigDiagnosticSource<'_>,
    contents: &str,
    value: TomlValue,
) -> Option<ConfigError> {
    let unknown_feature_paths = unknown_feature_toml_value_path(&value);
    let mut ignored_paths = Vec::new();
    let mut ignored_callback = |ignored_path: serde_ignored::Path<'_>| {
        let path_segments = ignored_path_segments(&ignored_path);
        if !path_segments.is_empty() {
            ignored_paths.push(path_segments);
        }
    };
    let deserializer = serde_ignored::Deserializer::new(value, &mut ignored_callback);
    let result: Result<T, _> = serde_path_to_error::deserialize(deserializer);

    match result {
        Ok(_) => unknown_field_error_from_paths(source, contents, ignored_paths)
            .or_else(|| unknown_field_error_from_paths(source, contents, unknown_feature_paths)),
        Err(err) => {
            let path_hint = err.path().clone();
            let toml_err = err.into_inner();
            let range = span_for_config_path(contents, &path_hint)
                .or_else(|| toml_err.span())
                .map(|span| text_range_from_span(contents, span))
                .unwrap_or_else(default_range);
            Some(ConfigError::new(
                source.to_path_buf(),
                range,
                toml_err.message(),
            ))
        }
    }
}

pub(crate) fn ignored_toml_value_fields<T: DeserializeOwned>(value: TomlValue) -> Vec<Vec<String>> {
    let mut ignored_paths = Vec::new();
    let result: Result<T, _> = serde_ignored::deserialize(value, |ignored_path| {
        let path_segments = ignored_path_segments(&ignored_path);
        if !path_segments.is_empty() {
            ignored_paths.push(path_segments);
        }
    });
    if result.is_err() {
        return Vec::new();
    }

    ignored_paths
}

pub(crate) fn unknown_feature_toml_value_field(value: &TomlValue) -> Option<String> {
    unknown_feature_toml_value_path(value)
        .into_iter()
        .next()
        .map(|path_segments| path_segments.join("."))
}

fn unknown_field_error_from_paths(
    source: ConfigDiagnosticSource<'_>,
    contents: &str,
    ignored_paths: Vec<Vec<String>>,
) -> Option<ConfigError> {
    let path_segments = ignored_paths.into_iter().next()?;
    let ignored_path = path_segments.join(".");
    let range = span_for_toml_key_path(contents, &path_segments)
        .map(|span| text_range_from_span(contents, span))
        .unwrap_or_else(default_range);
    Some(ConfigError::new(
        source.to_path_buf(),
        range,
        format!("unknown configuration field `{ignored_path}`"),
    ))
}

fn unknown_feature_toml_value_path(value: &TomlValue) -> Vec<Vec<String>> {
    let Some(root) = value.as_table() else {
        return Vec::new();
    };

    let mut paths = Vec::new();
    push_unknown_feature_paths(&mut paths, &["features"], root.get("features"));

    if let Some(profiles) = root.get("profiles").and_then(TomlValue::as_table) {
        for (profile_name, profile) in profiles {
            let prefix = ["profiles", profile_name.as_str(), "features"];
            let features = profile
                .as_table()
                .and_then(|profile| profile.get("features"));
            push_unknown_feature_paths(&mut paths, &prefix, features);
        }
    }

    paths
}

fn push_unknown_feature_paths(
    paths: &mut Vec<Vec<String>>,
    prefix: &[&str],
    features: Option<&TomlValue>,
) {
    let Some(features) = features.and_then(TomlValue::as_table) else {
        return;
    };

    for feature_key in features
        .keys()
        .map(String::as_str)
        .filter(|key| !is_known_feature_key(key))
    {
        let mut path = prefix
            .iter()
            .map(|segment| (*segment).to_string())
            .collect::<Vec<_>>();
        path.push(feature_key.to_string());
        paths.push(path);
    }
}

fn ignored_path_segments(path: &serde_ignored::Path<'_>) -> Vec<String> {
    let mut segments = Vec::new();
    push_ignored_path_segments(path, &mut segments);
    segments
}

fn push_ignored_path_segments(path: &serde_ignored::Path<'_>, segments: &mut Vec<String>) {
    match path {
        serde_ignored::Path::Root => {}
        serde_ignored::Path::Seq { parent, index } => {
            push_ignored_path_segments(parent, segments);
            segments.push(index.to_string());
        }
        serde_ignored::Path::Map { parent, key } => {
            push_ignored_path_segments(parent, segments);
            segments.push(key.clone());
        }
        serde_ignored::Path::Some { parent }
        | serde_ignored::Path::NewtypeStruct { parent }
        | serde_ignored::Path::NewtypeVariant { parent } => {
            push_ignored_path_segments(parent, segments);
        }
    }
}

pub(crate) fn ignored_config_warning(
    config: &ConfigLayerStack,
    requirements: &[RequirementsLayerEntry],
) -> Option<String> {
    let mut fields = BTreeSet::new();
    // Validate the merged config so incomplete layer fragments (e.g. a provider
    // override) do not hide diagnostics.
    let effective = config.effective_config();
    let unknown_features = unknown_feature_toml_value_path(&effective);
    for path in ignored_toml_value_fields::<ConfigToml>(effective)
        .into_iter()
        .chain(unknown_features)
    {
        // Origins only track leaf values; an ignored field can be a whole table.
        for layer in config.layers_high_to_low().filter(|layer| {
            path.iter()
                .try_fold(&layer.config, |value, key| match value {
                    toml::Value::Array(items) => {
                        key.parse::<usize>().ok().and_then(|i| items.get(i))
                    }
                    _ => value.get(key),
                })
                .is_some()
        }) {
            fields.insert((
                format_config_layer_source(&layer.name, CONFIG_TOML_FILE),
                path.clone(),
            ));
        }
    }

    // Requirements use a different schema and lose unknown fields during
    // composition. Inspect their original layers, with the same path bases.
    // Normal loading remains responsible for reporting parse/type errors.
    for layer in requirements {
        let Ok((source, mut value, base_dir)) = layer.clone().into_raw_parts() else {
            continue;
        };
        let _guard = base_dir
            .as_ref()
            .map(|base_dir| AbsolutePathBufGuard::new(base_dir.as_path()));
        strip_cloud_auth_requirements(&source, &mut value);
        fields.extend(
            ignored_toml_value_fields::<ConfigRequirementsToml>(value)
                .into_iter()
                .map(|path| (format!("requirements ({source})"), path)),
        );
    }

    if fields.is_empty() {
        return None;
    }
    let count = fields.len();
    let setting = if count == 1 { "setting" } else { "settings" };
    let mut warning = format!(
        "Codex is ignoring {count} unrecognized configuration {setting}. Check for typos or deprecated settings."
    );
    for (source, path) in fields.iter().take(3) {
        let source = bounded_label(source);
        let key = bounded_label(&path.join("."));
        let hint = match path
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
            .as_slice()
        {
            ["network_proxy"] => {
                " Use [permissions.<name>.network] for network settings, or [experimental_network] in requirements.toml for enforced network policy."
            }
            ["allowed_permissions"] => {
                " Use [allowed_permission_profiles] and default_permissions; the old allowlist is ignored even when both forms are present."
            }
            ["include_view_image_tool"]
            | ["features", "include_view_image_tool"]
            | ["profiles", _, "include_view_image_tool"]
            | ["profiles", _, "features", "include_view_image_tool"] => {
                " Use [features].view_image to configure the image tool."
            }
            _ => "",
        };
        let _ = write!(warning, "\n  {source}: `{key}` is ignored.{hint}");
    }
    if fields.len() > 3 {
        let remaining = fields.len() - 3;
        let _ = write!(warning, "\n  ... and {remaining} more ignored settings.");
    }
    Some(warning)
}

fn bounded_label(value: &str) -> String {
    let mut label = String::new();
    for character in value.chars() {
        if label.len() >= 200 {
            label.push('…');
            break;
        }
        if character.is_control() {
            label.extend(character.escape_default());
        } else {
            label.push(character);
        }
    }
    label
}

#[cfg(test)]
#[path = "strict_config_tests.rs"]
mod tests;
