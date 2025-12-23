use super::LoaderOverrides;
#[cfg(target_os = "macos")]
use super::macos::load_managed_admin_config_layer;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use tokio::fs;
use toml::Value as TomlValue;

#[cfg(unix)]
const CODEX_MANAGED_CONFIG_SYSTEM_PATH: &str = "/etc/codex/managed_config.toml";

#[derive(Debug, Clone)]
pub(super) struct MangedConfigFromFile {
    pub managed_config: TomlValue,
    pub file: AbsolutePathBuf,
}

#[derive(Debug, Clone)]
pub(super) struct LoadedConfigLayers {
    /// If present, data read from a file such as `/etc/codex/managed_config.toml`.
    pub managed_config: Option<MangedConfigFromFile>,
    /// If present, data read from managed preferences (macOS only).
    pub managed_config_from_mdm: Option<TomlValue>,
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

    let managed_config_path = AbsolutePathBuf::from_absolute_path(
        managed_config_path.unwrap_or_else(|| managed_config_default_path(codex_home)),
    )?;

    let managed_config = read_config_from_path(&managed_config_path, false)
        .await?
        .map(|managed_config| MangedConfigFromFile {
            managed_config,
            file: managed_config_path.clone(),
        });

    #[cfg(target_os = "macos")]
    let managed_preferences =
        load_managed_admin_config_layer(managed_preferences_base64.as_deref()).await?;

    #[cfg(not(target_os = "macos"))]
    let managed_preferences = None;

    Ok(LoadedConfigLayers {
        managed_config,
        managed_config_from_mdm: managed_preferences,
    })
}

pub(super) async fn read_config_from_path(
    path: impl AsRef<Path>,
    log_missing_as_info: bool,
) -> io::Result<Option<TomlValue>> {
    match fs::read_to_string(path.as_ref()).await {
        Ok(contents) => match toml::from_str::<TomlValue>(&contents) {
            Ok(value) => Ok(Some(value)),
            Err(err) => {
                tracing::error!("Failed to parse {}: {err}", path.as_ref().display());
                Err(io::Error::new(io::ErrorKind::InvalidData, err))
            }
        },
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            if log_missing_as_info {
                tracing::info!("{} not found, using defaults", path.as_ref().display());
            } else {
                tracing::debug!("{} not found", path.as_ref().display());
            }
            Ok(None)
        }
        Err(err) => {
            tracing::error!("Failed to read {}: {err}", path.as_ref().display());
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
