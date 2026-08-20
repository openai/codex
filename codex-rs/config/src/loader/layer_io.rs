#[cfg(target_os = "macos")]
use super::macos::ManagedAdminConfigLayer;
#[cfg(target_os = "macos")]
use super::macos::load_managed_admin_config_layer;
use crate::config_toml::ConfigToml;
use crate::diagnostics::config_error_from_toml;
use crate::diagnostics::io_error_from_config_error;
use crate::state::LoaderOverrides;
use crate::strict_config::config_error_from_ignored_toml_value_fields;
use codex_file_system::ExecutorFileSystem;
#[cfg(windows)]
use codex_file_system::GetMetadataOptions;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_absolute_path::AbsolutePathBufGuard;
use codex_utils_path_uri::PathUri;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use toml::Value as TomlValue;

#[cfg(unix)]
const CODEX_MANAGED_CONFIG_SYSTEM_PATH: &str = "/etc/codex/managed_config.toml";

#[derive(Debug, Clone)]
pub(super) struct MangedConfigFromFile {
    pub managed_config: TomlValue,
    pub file: AbsolutePathBuf,
}

#[derive(Debug, Clone)]
pub(super) struct ManagedConfigFromMdm {
    pub managed_config: TomlValue,
    pub raw_toml: String,
}

#[derive(Debug, Clone)]
pub(super) struct LoadedConfigLayers {
    /// If present, data read from a file such as `/etc/codex/managed_config.toml`.
    pub managed_config: Option<MangedConfigFromFile>,
    /// If present, data read from managed preferences (macOS only).
    pub managed_config_from_mdm: Option<ManagedConfigFromMdm>,
    /// User-facing warnings discovered while loading managed configuration.
    pub startup_warnings: Vec<String>,
}

pub(super) async fn load_config_layers_internal(
    fs: &dyn ExecutorFileSystem,
    codex_home: &Path,
    overrides: LoaderOverrides,
    strict_config: bool,
) -> io::Result<LoadedConfigLayers> {
    #[cfg(target_os = "macos")]
    let LoaderOverrides {
        managed_config_path,
        managed_preferences_base64,
        ..
    } = overrides;

    #[cfg(not(target_os = "macos"))]
    let LoaderOverrides {
        managed_config_path,
        ..
    } = overrides;

    #[cfg(windows)]
    let ignore_default_managed_config = managed_config_path.is_none();
    #[cfg(not(windows))]
    let ignore_default_managed_config = false;

    let managed_config_path = AbsolutePathBuf::from_absolute_path(
        managed_config_path.unwrap_or_else(|| managed_config_default_path(codex_home)),
    )?;

    #[cfg(windows)]
    let startup_warnings = if ignore_default_managed_config {
        let path_uri = PathUri::from_abs_path(&managed_config_path);
        match fs
            .get_metadata(
                &path_uri,
                GetMetadataOptions::default(),
                /*sandbox*/ None,
            )
            .await
        {
            Ok(_) => vec![format!(
                "Ignoring deprecated managed config file at {}; CODEX_HOME/managed_config.toml is no longer supported on Windows. Use %ProgramData%\\OpenAI\\Codex\\requirements.toml for enforced settings or config.toml for defaults.",
                managed_config_path.as_path().display()
            )],
            Err(err) if err.kind() == io::ErrorKind::NotFound => Vec::new(),
            Err(err) => {
                tracing::debug!(
                    error = %err,
                    path = %managed_config_path.as_path().display(),
                    "Failed to check deprecated managed config file"
                );
                Vec::new()
            }
        }
    } else {
        Vec::new()
    };
    #[cfg(not(windows))]
    let startup_warnings = Vec::new();

    let managed_config = if ignore_default_managed_config {
        None
    } else {
        read_config_from_path(
            fs,
            &managed_config_path,
            /*log_missing_as_info*/ false,
            strict_config,
        )
        .await?
        .map(|loaded| MangedConfigFromFile {
            managed_config: loaded,
            file: managed_config_path.clone(),
        })
    };

    #[cfg(target_os = "macos")]
    let managed_preferences = load_managed_admin_config_layer(
        managed_preferences_base64.as_deref(),
        strict_config,
        codex_home,
    )
    .await?
    .map(map_managed_admin_layer);

    #[cfg(not(target_os = "macos"))]
    let managed_preferences = None;

    Ok(LoadedConfigLayers {
        managed_config,
        managed_config_from_mdm: managed_preferences,
        startup_warnings,
    })
}

#[cfg(target_os = "macos")]
fn map_managed_admin_layer(layer: ManagedAdminConfigLayer) -> ManagedConfigFromMdm {
    let ManagedAdminConfigLayer { config, raw_toml } = layer;
    ManagedConfigFromMdm {
        managed_config: config,
        raw_toml,
    }
}

pub(super) async fn read_config_from_path(
    fs: &dyn ExecutorFileSystem,
    path: &AbsolutePathBuf,
    log_missing_as_info: bool,
    strict_config: bool,
) -> io::Result<Option<TomlValue>> {
    let path_uri = PathUri::from_abs_path(path);
    match fs
        .read_file_text(&path_uri, Default::default(), /*sandbox*/ None)
        .await
    {
        Ok(contents) => match toml::from_str::<TomlValue>(&contents) {
            Ok(value) => {
                if strict_config {
                    validate_config_toml_strictly(path, &contents, &value)?;
                }
                Ok(Some(value))
            }
            Err(err) => {
                tracing::error!("Failed to parse {}: {err}", path.as_path().display());
                let config_error = config_error_from_toml(path.as_path(), &contents, err.clone());
                Err(io_error_from_config_error(
                    io::ErrorKind::InvalidData,
                    config_error,
                    Some(err),
                ))
            }
        },
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            if log_missing_as_info {
                tracing::info!("{} not found, using defaults", path.as_path().display());
            } else {
                tracing::debug!("{} not found", path.as_path().display());
            }
            Ok(None)
        }
        Err(err) => {
            tracing::error!("Failed to read {}: {err}", path.as_path().display());
            Err(err)
        }
    }
}

fn validate_config_toml_strictly(
    path: &AbsolutePathBuf,
    contents: &str,
    value: &TomlValue,
) -> io::Result<()> {
    let Some(base_dir) = path.as_path().parent() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Config file {} has no parent directory", path.display()),
        ));
    };
    let _guard = AbsolutePathBufGuard::new(base_dir);
    if let Some(config_error) = config_error_from_ignored_toml_value_fields::<ConfigToml>(
        path.as_path(),
        contents,
        value.clone(),
    ) {
        return Err(io_error_from_config_error(
            io::ErrorKind::InvalidData,
            config_error,
            /*source*/ None,
        ));
    }

    Ok(())
}

/// Return the legacy managed config path.
///
/// On Windows, the default path is only checked so callers can warn that it is ignored.
pub(super) fn managed_config_default_path(codex_home: &Path) -> PathBuf {
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
