use std::io;

use codex_config::ConfigLayerSource;
use codex_config::config_toml::ConfigToml;
use codex_utils_absolute_path::AbsolutePathBuf;
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::ThreadConfigSnapshot;

pub(crate) const CONFIG_LOCK_VERSION: u32 = 1;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct ConfigLockFile {
    pub version: u32,
    pub codex_version: String,
    pub replay_config: ConfigToml,
    pub session: ThreadConfigSnapshot,
}

impl ConfigLockFile {
    pub(crate) fn new(replay_config: ConfigToml, session: ThreadConfigSnapshot) -> Self {
        Self {
            version: CONFIG_LOCK_VERSION,
            codex_version: env!("CARGO_PKG_VERSION").to_string(),
            replay_config,
            session,
        }
    }

    pub(crate) async fn read_from_path(path: &AbsolutePathBuf) -> io::Result<Self> {
        let contents = tokio::fs::read_to_string(path)
            .await
            .with_context_io(|| format!("failed to read config lock file {}", path.display()))?;
        let lock: Self = toml::from_str(&contents).map_err(|err| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("failed to parse config lock file {}: {err}", path.display()),
            )
        })?;
        if lock.version != CONFIG_LOCK_VERSION {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "unsupported config lock version {}; expected {CONFIG_LOCK_VERSION}",
                    lock.version
                ),
            ));
        }
        Ok(lock)
    }

    pub(crate) fn validate_replay(&self, actual: &Self) -> io::Result<()> {
        if self.replay_config != actual.replay_config {
            return Err(lock_mismatch("replay config"));
        }
        if self.session != actual.session {
            return Err(lock_mismatch("session config"));
        }
        Ok(())
    }
}

pub(crate) fn lock_layer_from_config(
    lock_path: &AbsolutePathBuf,
    lock: &ConfigLockFile,
) -> io::Result<codex_config::ConfigLayerEntry> {
    let value = toml_value(&lock.replay_config, "replay config")?;
    Ok(codex_config::ConfigLayerEntry::new(
        ConfigLayerSource::User {
            file: lock_path.clone(),
        },
        value,
    ))
}

fn lock_mismatch(section: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("resolved {section} does not match config lock"),
    )
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
