//! Strict config validation built on top of serde's ignored-field tracking.

use crate::diagnostics::ConfigDiagnosticSource;
use crate::diagnostics::ConfigError;
use crate::diagnostics::config_error_from_toml_for_source;
use crate::diagnostics::default_range;
use crate::diagnostics::span_for_config_path;
use crate::diagnostics::span_for_toml_key_path;
use crate::diagnostics::text_range_from_span;
use codex_features::is_known_feature_key;
use serde::de::DeserializeOwned;
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

#[cfg(any(target_os = "macos", test))]
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

pub(crate) fn ignored_toml_value_field<T: DeserializeOwned>(value: TomlValue) -> Option<String> {
    let mut ignored_paths = Vec::new();
    let result: Result<T, _> = serde_ignored::deserialize(value, |ignored_path| {
        let path_segments = ignored_path_segments(&ignored_path);
        if !path_segments.is_empty() {
            ignored_paths.push(path_segments);
        }
    });
    if result.is_err() {
        return None;
    }

    ignored_paths
        .into_iter()
        .next()
        .map(|path_segments| path_segments.join("."))
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

#[cfg(test)]
#[path = "strict_config_tests.rs"]
mod tests;
