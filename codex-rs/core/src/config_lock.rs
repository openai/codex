use std::io;

use codex_config::ConfigLayerEntry;
use codex_config::ConfigLayerSource;
use codex_config::config_toml::ConfigLockToml;
use codex_config::config_toml::ConfigToml;
use codex_config::version_for_toml;
use codex_utils_absolute_path::AbsolutePathBuf;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value as JsonValue;

use crate::ThreadConfigSnapshot;

pub(crate) const CONFIG_LOCK_VERSION: u32 = 1;
const MAX_LOCK_DIFFS: usize = 5;
const MAX_DIFF_VALUE_CHARS: usize = 120;

pub(crate) async fn read_config_lock_from_path(path: &AbsolutePathBuf) -> io::Result<ConfigToml> {
    let contents = tokio::fs::read_to_string(path)
        .await
        .with_context_io(|| format!("failed to read config lock file {}", path.display()))?;
    let lock_config: ConfigToml = toml::from_str(&contents).map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("failed to parse config lock file {}: {err}", path.display()),
        )
    })?;
    let Some(lock) = lock_config.config_lock.as_ref() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "config lock file {} is missing [config_lock] metadata",
                path.display()
            ),
        ));
    };
    validate_config_lock_metadata_shape(lock)?;
    Ok(lock_config)
}

pub(crate) fn config_lock_metadata(
    config: &ConfigToml,
    session: &ThreadConfigSnapshot,
) -> io::Result<ConfigLockToml> {
    Ok(ConfigLockToml {
        version: CONFIG_LOCK_VERSION,
        codex_version: env!("CARGO_PKG_VERSION").to_string(),
        config_sha256: config_sha256(config)?,
        session_sha256: session_sha256(session)?,
    })
}

pub(crate) fn validate_config_lock_replay(
    expected: &ConfigLockToml,
    expected_config: &ConfigToml,
    actual_config: &ConfigToml,
    actual_session: &ThreadConfigSnapshot,
) -> io::Result<()> {
    validate_config_lock_metadata_shape(expected)?;

    let expected_config_sha = config_sha256(expected_config)?;
    if expected.config_sha256 != expected_config_sha {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "config lock file contents do not match [config_lock].config_sha256 \
                 (stored {}, current {}); regenerate the lock after editing config values",
                expected.config_sha256, expected_config_sha
            ),
        ));
    }

    let actual_config_sha = config_sha256(actual_config)?;
    if expected.config_sha256 != actual_config_sha {
        let expected_config = config_without_lock_controls(expected_config);
        let actual_config = config_without_lock_controls(actual_config);
        let diff = compact_diff("config", &expected_config, &actual_config)
            .unwrap_or_else(|err| format!("failed to build config lock diff: {err}"));
        return Err(lock_hash_mismatch(
            "config",
            &expected.config_sha256,
            &actual_config_sha,
            diff,
        ));
    }

    let actual_session_sha = session_sha256(actual_session)?;
    if expected.session_sha256 != actual_session_sha {
        return Err(lock_hash_mismatch(
            "session config",
            &expected.session_sha256,
            &actual_session_sha,
            String::new(),
        ));
    }

    Ok(())
}

pub(crate) fn lock_layer_from_config(
    lock_path: &AbsolutePathBuf,
    lock_config: &ConfigToml,
) -> io::Result<ConfigLayerEntry> {
    let value = toml_value(&config_without_lock_controls(lock_config), "config lock")?;
    Ok(ConfigLayerEntry::new(
        ConfigLayerSource::User {
            file: lock_path.clone(),
        },
        value,
    ))
}

pub(crate) fn config_without_lock_controls(config: &ConfigToml) -> ConfigToml {
    let mut config = config.clone();
    config.config_lock = None;
    config.config_lock_file = None;
    config.config_snapshot_export_dir = None;
    config
}

fn validate_config_lock_metadata_shape(lock: &ConfigLockToml) -> io::Result<()> {
    if lock.version != CONFIG_LOCK_VERSION {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "unsupported config lock version {}; expected {CONFIG_LOCK_VERSION}",
                lock.version
            ),
        ));
    }
    if lock.config_sha256.is_empty() || lock.session_sha256.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "config lock metadata must include config_sha256 and session_sha256",
        ));
    }
    Ok(())
}

fn config_sha256(config: &ConfigToml) -> io::Result<String> {
    let config = config_without_lock_controls(config);
    Ok(version_for_toml(&toml_value(
        &config,
        "config lock config",
    )?))
}

fn session_sha256(session: &ThreadConfigSnapshot) -> io::Result<String> {
    Ok(version_for_toml(&toml_value(
        session,
        "config lock session",
    )?))
}

fn lock_hash_mismatch(
    section: &str,
    expected_sha: &str,
    actual_sha: &str,
    diff: String,
) -> io::Error {
    let suffix = if diff.is_empty() {
        String::new()
    } else {
        format!(": {diff}")
    };
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!(
            "resolved {section} hash does not match config lock \
             (expected {expected_sha}, actual {actual_sha}){suffix}"
        ),
    )
}

fn compact_diff<T: Serialize>(root: &str, expected: &T, actual: &T) -> io::Result<String> {
    let expected = serde_json::to_value(expected).map_err(diff_serialize_error)?;
    let actual = serde_json::to_value(actual).map_err(diff_serialize_error)?;
    let mut diffs = Vec::new();
    let truncated = collect_value_diffs(root, &expected, &actual, &mut diffs);
    if truncated {
        diffs.push("...".to_string());
    }
    Ok(diffs.join("; "))
}

fn diff_serialize_error(err: serde_json::Error) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("failed to serialize config lock diff value: {err}"),
    )
}

fn collect_value_diffs(
    path: &str,
    expected: &JsonValue,
    actual: &JsonValue,
    diffs: &mut Vec<String>,
) -> bool {
    if expected == actual {
        return false;
    }
    if diffs.len() >= MAX_LOCK_DIFFS {
        return true;
    }

    match (expected, actual) {
        (JsonValue::Object(expected), JsonValue::Object(actual)) => {
            let mut keys = expected.keys().chain(actual.keys()).collect::<Vec<_>>();
            keys.sort();
            keys.dedup();
            for key in keys {
                let child_path = format!("{path}.{key}");
                match (expected.get(key), actual.get(key)) {
                    (Some(expected), Some(actual)) => {
                        if collect_value_diffs(&child_path, expected, actual, diffs) {
                            return true;
                        }
                    }
                    (Some(expected), None) => {
                        push_diff(&child_path, expected, &JsonValue::Null, diffs);
                    }
                    (None, Some(actual)) => {
                        push_diff(&child_path, &JsonValue::Null, actual, diffs);
                    }
                    (None, None) => {}
                }
                if diffs.len() >= MAX_LOCK_DIFFS {
                    return true;
                }
            }
            false
        }
        (JsonValue::Array(expected), JsonValue::Array(actual)) => {
            if expected.len() != actual.len() {
                push_summary_diff(
                    path,
                    format!("[len {}]", expected.len()),
                    format!("[len {}]", actual.len()),
                    diffs,
                );
            }
            for (index, (expected, actual)) in expected.iter().zip(actual.iter()).enumerate() {
                let child_path = format!("{path}[{index}]");
                if collect_value_diffs(&child_path, expected, actual, diffs) {
                    return true;
                }
                if diffs.len() >= MAX_LOCK_DIFFS {
                    return true;
                }
            }
            false
        }
        _ => {
            push_diff(path, expected, actual, diffs);
            false
        }
    }
}

fn push_diff(path: &str, expected: &JsonValue, actual: &JsonValue, diffs: &mut Vec<String>) {
    push_summary_diff(
        path,
        summarize_value(expected),
        summarize_value(actual),
        diffs,
    );
}

fn push_summary_diff(path: &str, expected: String, actual: String, diffs: &mut Vec<String>) {
    if diffs.len() < MAX_LOCK_DIFFS {
        diffs.push(format!("{path}: expected {expected}, actual {actual}"));
    }
}

fn summarize_value(value: &JsonValue) -> String {
    match value {
        JsonValue::Null => "null".to_string(),
        JsonValue::Bool(value) => value.to_string(),
        JsonValue::Number(value) => value.to_string(),
        JsonValue::String(value) => summarize_string(value),
        JsonValue::Array(values) => format!("[len {}]", values.len()),
        JsonValue::Object(_) => "{...}".to_string(),
    }
}

fn summarize_string(value: &str) -> String {
    let escaped =
        serde_json::to_string(value).unwrap_or_else(|_| "\"<invalid string>\"".to_string());
    if escaped.chars().count() <= MAX_DIFF_VALUE_CHARS {
        return escaped;
    }
    let max_inner_chars = MAX_DIFF_VALUE_CHARS.saturating_sub(5);
    let escaped_inner = escaped
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or(escaped.as_str());
    let truncated = escaped_inner
        .chars()
        .take(max_inner_chars)
        .collect::<String>();
    format!("\"{truncated}...\"")
}

fn toml_value<T: Serialize>(value: &T, label: &str) -> io::Result<toml::Value> {
    toml::Value::try_from(value).map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("failed to serialize {label}: {err}"),
        )
    })
}

pub(crate) fn toml_round_trip<T>(value: &impl Serialize, label: &'static str) -> io::Result<T>
where
    T: DeserializeOwned + Serialize,
{
    let value = toml_value(value, label)?;
    let toml = value.clone().try_into().map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("failed to convert {label} to TOML shape: {err}"),
        )
    })?;
    let represented_value = toml_value(&toml, label)?;
    if represented_value != value {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("resolved {label} cannot be fully represented as TOML"),
        ));
    }
    Ok(toml)
}

trait IoContext<T> {
    fn with_context_io(self, context: impl FnOnce() -> String) -> io::Result<T>;
}

impl<T> IoContext<T> for io::Result<T> {
    fn with_context_io(self, context: impl FnOnce() -> String) -> io::Result<T> {
        self.map_err(|err| io::Error::new(err.kind(), format!("{}: {err}", context())))
    }
}
