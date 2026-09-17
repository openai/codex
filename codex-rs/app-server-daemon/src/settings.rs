use std::path::Path;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use anyhow::ensure;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::Map;
use serde_json::Value;
use tokio::fs;

pub(crate) const DEFAULT_UPDATE_INTERVAL_MINUTES: u32 = 60;
pub(crate) const DEFAULT_SHUTDOWN_GRACE_SECONDS: u32 = 60;
pub(crate) const MAX_SHUTDOWN_GRACE_SECONDS: u32 = 5 * 60;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DaemonSettings {
    pub(crate) remote_control_enabled: bool,
    pub(crate) auto_update_enabled: bool,
    pub(crate) update_interval_minutes: u32,
    pub(crate) shutdown_grace_seconds: u32,
}

impl Default for DaemonSettings {
    fn default() -> Self {
        Self {
            remote_control_enabled: false,
            auto_update_enabled: true,
            update_interval_minutes: DEFAULT_UPDATE_INTERVAL_MINUTES,
            shutdown_grace_seconds: DEFAULT_SHUTDOWN_GRACE_SECONDS,
        }
    }
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StopSettings {
    shutdown_grace_seconds: Option<u32>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredSettings {
    #[serde(default)]
    remote_control_enabled: bool,
    #[serde(default = "default_shutdown_grace_seconds")]
    shutdown_grace_seconds: u32,
    #[serde(default)]
    updater: UpdaterSettings,
}

impl Default for StoredSettings {
    fn default() -> Self {
        Self {
            remote_control_enabled: false,
            shutdown_grace_seconds: DEFAULT_SHUTDOWN_GRACE_SECONDS,
            updater: UpdaterSettings::default(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UpdaterSettings {
    #[serde(default = "default_auto_update_enabled")]
    pub(crate) auto_update_enabled: bool,
    #[serde(default = "default_update_interval_minutes")]
    pub(crate) update_interval_minutes: u32,
}

impl Default for UpdaterSettings {
    fn default() -> Self {
        Self {
            auto_update_enabled: default_auto_update_enabled(),
            update_interval_minutes: default_update_interval_minutes(),
        }
    }
}

fn default_auto_update_enabled() -> bool {
    true
}

fn default_update_interval_minutes() -> u32 {
    DEFAULT_UPDATE_INTERVAL_MINUTES
}

fn default_shutdown_grace_seconds() -> u32 {
    DEFAULT_SHUTDOWN_GRACE_SECONDS
}

fn validate_shutdown_grace(seconds: u32) -> Result<()> {
    ensure!(
        seconds <= MAX_SHUTDOWN_GRACE_SECONDS,
        "shutdown grace must be between 0 and {MAX_SHUTDOWN_GRACE_SECONDS} seconds"
    );
    Ok(())
}

impl UpdaterSettings {
    pub(crate) async fn load(settings_file: &Path) -> Result<Self> {
        let settings: StoredSettings = read_settings(settings_file).await?;
        validate_shutdown_grace(settings.shutdown_grace_seconds)?;
        let settings = settings.updater;
        settings.validate()?;
        Ok(settings)
    }

    pub(crate) fn validate(&self) -> Result<()> {
        ensure!(
            self.update_interval_minutes > 0,
            "update interval must be positive"
        );
        Ok(())
    }

    pub(crate) fn update_interval(&self, minute: Duration) -> Duration {
        minute * self.update_interval_minutes
    }
}

impl DaemonSettings {
    pub(crate) async fn load(path: &Path) -> Result<Self> {
        let settings: StoredSettings = read_settings(path).await?;
        settings.updater.validate()?;
        validate_shutdown_grace(settings.shutdown_grace_seconds)?;
        Ok(Self {
            remote_control_enabled: settings.remote_control_enabled,
            auto_update_enabled: settings.updater.auto_update_enabled,
            update_interval_minutes: settings.updater.update_interval_minutes,
            shutdown_grace_seconds: settings.shutdown_grace_seconds,
        })
    }

    pub(crate) async fn load_for_stop(path: &Path) -> Self {
        // Stop must work even when settings are unreadable or partially edited.
        let shutdown_grace_seconds = read_settings::<StopSettings>(path)
            .await
            .ok()
            .and_then(|settings| settings.shutdown_grace_seconds)
            .filter(|&seconds| seconds <= MAX_SHUTDOWN_GRACE_SECONDS)
            .unwrap_or(DEFAULT_SHUTDOWN_GRACE_SECONDS);
        Self {
            shutdown_grace_seconds,
            ..Self::default()
        }
    }

    pub(crate) async fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await.with_context(|| {
                format!(
                    "failed to create daemon settings directory {}",
                    parent.display()
                )
            })?;
        }
        let mut settings: Map<String, Value> = read_settings(path).await?;
        settings.insert(
            "remoteControlEnabled".to_string(),
            Value::Bool(self.remote_control_enabled),
        );
        let contents =
            serde_json::to_vec_pretty(&settings).context("failed to serialize settings")?;
        let temporary_path = path.with_extension("tmp");
        fs::write(&temporary_path, contents)
            .await
            .with_context(|| {
                format!(
                    "failed to write daemon settings {}",
                    temporary_path.display()
                )
            })?;
        fs::rename(&temporary_path, path)
            .await
            .with_context(|| format!("failed to replace daemon settings {}", path.display()))
    }
}

pub(crate) async fn telemetry_tags(path: &Path) -> Result<[(&'static str, &'static str); 4]> {
    let raw: Map<String, Value> = read_settings(path).await?;
    // Validate the same snapshot used for presence checks, using the existing settings parser.
    let settings: StoredSettings = serde_json::from_value(Value::Object(raw.clone()))?;
    settings.updater.validate()?;
    validate_shutdown_grace(settings.shutdown_grace_seconds)?;
    let updater = raw.get("updater");
    let presence = |configured| if configured { "configured" } else { "default" };
    Ok([
        (
            "auto_update",
            if settings.updater.auto_update_enabled {
                "enabled"
            } else {
                "disabled"
            },
        ),
        (
            "auto_update_setting",
            presence(updater.is_some_and(|value| value.get("autoUpdateEnabled").is_some())),
        ),
        (
            "update_interval_setting",
            presence(updater.is_some_and(|value| value.get("updateIntervalMinutes").is_some())),
        ),
        (
            "shutdown_grace_setting",
            presence(raw.contains_key("shutdownGraceSeconds")),
        ),
    ])
}

async fn read_settings<T: DeserializeOwned + Default>(path: &Path) -> Result<T> {
    let contents = match fs::read_to_string(path).await {
        Ok(contents) => contents,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(T::default()),
        Err(err) => {
            return Err(err).with_context(|| format!("failed to read settings {}", path.display()));
        }
    };
    serde_json::from_str(&contents)
        .with_context(|| format!("failed to parse settings {}", path.display()))
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
