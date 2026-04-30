use std::io;

use codex_config::ConfigLayerEntry;
use codex_config::ConfigLayerSource;
use codex_config::config_toml::ConfigLockToml;
use codex_config::config_toml::ConfigToml;
use codex_utils_absolute_path::AbsolutePathBuf;
use serde::Serialize;
use serde::de::DeserializeOwned;
use similar::TextDiff;

pub(crate) const CONFIG_LOCK_VERSION: u32 = 1;

pub(crate) async fn read_config_lock_from_path(path: &AbsolutePathBuf) -> io::Result<ConfigToml> {
    let contents = tokio::fs::read_to_string(path).await.map_err(|err| {
        config_lock_error(format!(
            "failed to read config lock file {}: {err}",
            path.display()
        ))
    })?;
    let lock_config: ConfigToml = toml::from_str(&contents).map_err(|err| {
        config_lock_error(format!(
            "failed to parse config lock file {}: {err}",
            path.display()
        ))
    })?;
    let Some(lock) = lock_config.config_lock.as_ref() else {
        return Err(config_lock_error(format!(
            "config lock file {} is missing [config_lock] metadata",
            path.display()
        )));
    };
    validate_config_lock_metadata_shape(lock)?;
    Ok(lock_config)
}

pub(crate) fn config_lock_metadata(cwd: &AbsolutePathBuf) -> ConfigLockToml {
    ConfigLockToml {
        version: CONFIG_LOCK_VERSION,
        codex_version: env!("CARGO_PKG_VERSION").to_string(),
        cwd: cwd.clone(),
    }
}

pub(crate) fn validate_config_lock_replay(
    expected_lock: &ConfigToml,
    actual_lock: &ConfigToml,
) -> io::Result<()> {
    match expected_lock.config_lock.as_ref() {
        Some(expected) => validate_config_lock_metadata_shape(expected)?,
        None => {
            return Err(config_lock_error(
                "config lock file is missing [config_lock] metadata",
            ));
        }
    }
    match actual_lock.config_lock.as_ref() {
        Some(actual) => validate_config_lock_metadata_shape(actual)?,
        None => {
            return Err(config_lock_error(
                "regenerated config lock is missing [config_lock] metadata",
            ));
        }
    }

    let expected_lock = config_lock_for_comparison(expected_lock);
    let actual_lock = config_lock_for_comparison(actual_lock);
    if expected_lock != actual_lock {
        let diff = compact_diff("config", &expected_lock, &actual_lock)
            .unwrap_or_else(|err| format!("failed to build config lock diff: {err}"));
        return Err(config_lock_error(format!(
            "replayed effective config does not match config lock: {diff}"
        )));
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
    config.config_lock_export_dir = None;
    config
}

fn validate_config_lock_metadata_shape(lock: &ConfigLockToml) -> io::Result<()> {
    if lock.version != CONFIG_LOCK_VERSION {
        return Err(config_lock_error(format!(
            "unsupported config lock version {}; expected {CONFIG_LOCK_VERSION}",
            lock.version
        )));
    }
    Ok(())
}

fn config_lock_for_comparison(config: &ConfigToml) -> ConfigToml {
    let mut config = config.clone();
    config.config_lock_file = None;
    config.config_lock_export_dir = None;
    config
}

fn config_lock_error(message: impl Into<String>) -> io::Error {
    io::Error::other(message.into())
}

fn compact_diff<T: Serialize>(root: &str, expected: &T, actual: &T) -> io::Result<String> {
    let expected = toml::to_string_pretty(expected).map_err(|err| {
        config_lock_error(format!(
            "failed to serialize expected {root} lock TOML: {err}"
        ))
    })?;
    let actual = toml::to_string_pretty(actual).map_err(|err| {
        config_lock_error(format!(
            "failed to serialize actual {root} lock TOML: {err}"
        ))
    })?;
    Ok(TextDiff::from_lines(&expected, &actual)
        .unified_diff()
        .context_radius(2)
        .header("expected", "actual")
        .to_string())
}

fn toml_value<T: Serialize>(value: &T, label: &str) -> io::Result<toml::Value> {
    toml::Value::try_from(value)
        .map_err(|err| config_lock_error(format!("failed to serialize {label}: {err}")))
}

pub(crate) fn toml_round_trip<T>(value: &impl Serialize, label: &'static str) -> io::Result<T>
where
    T: DeserializeOwned + Serialize,
{
    let value = toml_value(value, label)?;
    let toml = value.clone().try_into().map_err(|err| {
        config_lock_error(format!("failed to convert {label} to TOML shape: {err}"))
    })?;
    let represented_value = toml_value(&toml, label)?;
    if represented_value != value {
        return Err(config_lock_error(format!(
            "resolved {label} cannot be fully represented as TOML"
        )));
    }
    Ok(toml)
}
