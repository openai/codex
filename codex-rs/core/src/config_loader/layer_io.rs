use super::LoaderOverrides;
use super::macos::load_managed_admin_config_layer;
use super::overrides::default_empty_table;
use crate::config::CONFIG_TOML_FILE;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use tokio::fs;
use toml::Value as TomlValue;

#[cfg(unix)]
const CODEX_MANAGED_CONFIG_SYSTEM_PATH: &str = "/etc/codex/managed_config.toml";

#[derive(Debug, Clone)]
pub(super) struct LoadedConfigLayers {
    pub base: TomlValue,
    pub managed_config: Option<TomlValue>,
    pub managed_preferences: Option<TomlValue>,
}

pub(super) async fn load_config_layers_internal(
    codex_home: &Path,
    overrides: LoaderOverrides,
) -> io::Result<LoadedConfigLayers> {
    #[cfg(target_os = "macos")]
    let LoaderOverrides {
        managed_config_path,
        managed_preferences_base64,
    } = overrides;

    #[cfg(not(target_os = "macos"))]
    let LoaderOverrides {
        managed_config_path,
    } = overrides;

    let managed_config_path =
        managed_config_path.unwrap_or_else(|| managed_config_default_path(codex_home));

    let user_config_path = codex_home.join(CONFIG_TOML_FILE);
    let user_config = read_config_from_path(&user_config_path, true).await?;
    let managed_config = read_config_from_path(&managed_config_path, false).await?;

    #[cfg(target_os = "macos")]
    let managed_preferences =
        load_managed_admin_config_layer(managed_preferences_base64.as_deref()).await?;

    #[cfg(not(target_os = "macos"))]
    let managed_preferences = load_managed_admin_config_layer(None).await?;

    Ok(LoadedConfigLayers {
        base: user_config.unwrap_or_else(default_empty_table),
        managed_config,
        managed_preferences,
    })
}

pub(super) async fn read_config_from_path(
    path: &Path,
    log_missing_as_info: bool,
) -> io::Result<Option<TomlValue>> {
    match fs::read_to_string(path).await {
        Ok(contents) => match toml::from_str::<TomlValue>(&contents) {
            Ok(value) => Ok(Some(value)),
            Err(err) => {
                tracing::error!("Failed to parse {}: {err}", path.display());
                Err(io::Error::new(io::ErrorKind::InvalidData, err))
            }
        },
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            if log_missing_as_info {
                tracing::info!("{} not found, using defaults", path.display());
            } else {
                tracing::debug!("{} not found", path.display());
            }
            Ok(None)
        }
        Err(err) => {
            tracing::error!("Failed to read {}: {err}", path.display());
            Err(err)
        }
    }
}

/// Return the default managed config path (honoring `CODEX_MANAGED_CONFIG_PATH`).
pub(super) fn managed_config_default_path(codex_home: &Path) -> PathBuf {
    if let Ok(path) = std::env::var("CODEX_MANAGED_CONFIG_PATH") {
        return PathBuf::from(path);
    }

    #[cfg(unix)]
    {
        let _ = codex_home;
        PathBuf::from(CODEX_MANAGED_CONFIG_SYSTEM_PATH)
    }

    #[cfg(not(unix))]
    {
        codex_home.join("managed_config.toml")
    }
}
