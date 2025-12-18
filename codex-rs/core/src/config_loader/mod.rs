mod config_requirements;
mod fingerprint;
mod layer_io;
#[cfg(target_os = "macos")]
mod macos;
mod merge;
mod overrides;
mod state;

#[cfg(test)]
mod tests;

use crate::config::CONFIG_TOML_FILE;
use crate::config_loader::layer_io::LoadedConfigLayers;
use codex_app_server_protocol::ConfigLayerSource;
use codex_protocol::protocol::AskForApproval;
use codex_utils_absolute_path::AbsolutePathBuf;
use serde::Deserialize;
use std::io;
use std::path::Path;
use toml::Value as TomlValue;

pub use config_requirements::ConfigRequirements;
pub use merge::merge_toml_values;
pub use state::ConfigLayerEntry;
pub use state::ConfigLayerStack;
pub use state::LoaderOverrides;

/// To build up the set of admin-enforced constraints, we build up from multiple
/// configuration layers in the following order, but a constraint defined in an
/// earlier layer cannot be overridden by a later layer:
///
/// - admin:    managed preferences (*)
/// - system    `/etc/codex/requirements.toml`
///
/// For backwards compatibility, we also load from
/// `/etc/codex/managed_config.toml` and map it to
/// `/etc/codex/requirements.toml`.
///
/// Configuration is built up from multiple layers in the following order:
///
/// - admin:    managed preferences (*)
/// - system    `/etc/codex/config.toml`
/// - user      `${CODEX_HOME}/config.toml`
/// - cwd       `${PWD}/config.toml`
/// - tree      parent directories up to root looking for `./.codex/config.toml`
/// - repo      `$(git rev-parse --show-toplevel)/.codex/config.toml`
/// - runtime   e.g., --config flags, model selector in UI
///
/// (*) Only available on macOS via managed device profiles.
///
/// See https://developers.openai.com/codex/security for details.
pub async fn load_config_layers_state(
    codex_home: &Path,
    cli_overrides: &[(String, TomlValue)],
    overrides: LoaderOverrides,
) -> io::Result<ConfigLayerStack> {
    let loaded_config_layers = layer_io::load_config_layers_internal(codex_home, overrides).await?;
    let requirements = load_requirements_from_legacy_scheme(loaded_config_layers.clone()).await?;

    // TODO(mbolin): Honor /etc/codex/requirements.toml.

    let mut layers = Vec::<ConfigLayerEntry>::new();

    // TODO(mbolin): Honor managed preferences (macOS only).
    // TODO(mbolin): Honor /etc/codex/config.toml.

    // Add a layer for $CODEX_HOME/config.toml if it exists. Note if the file
    // exists, but is malformed, then this error should be propagated to the
    // user.
    let user_file = AbsolutePathBuf::resolve_path_against_base(CONFIG_TOML_FILE, codex_home)?;
    match tokio::fs::read_to_string(&user_file).await {
        Ok(contents) => {
            let user_config: TomlValue = toml::from_str(&contents).map_err(|e| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "Error parsing user config file {}: {e}",
                        user_file.as_path().display(),
                    ),
                )
            })?;
            layers.push(ConfigLayerEntry::new(
                ConfigLayerSource::User { file: user_file },
                user_config,
            ));
        }
        Err(e) => {
            if e.kind() != io::ErrorKind::NotFound {
                return Err(io::Error::new(
                    e.kind(),
                    format!(
                        "Failed to read user config file {}: {e}",
                        user_file.as_path().display(),
                    ),
                ));
            }
        }
    }

    // TODO(mbolin): Add layers for cwd, tree, and repo config files.

    // Add a layer for runtime overrides from the CLI or UI, if any exist.
    if !cli_overrides.is_empty() {
        let cli_overrides_layer = overrides::build_cli_overrides_layer(cli_overrides);
        layers.push(ConfigLayerEntry::new(
            ConfigLayerSource::SessionFlags,
            cli_overrides_layer,
        ));
    }

    // Make a best-effort to support the legacy `managed_config.toml` as a
    // config layer on top of everything else. For fields in
    // `managed_config.toml` that do not have an equivalent in
    // `ConfigRequirements`, note users can still override these values on a
    // per-turn basis in the TUI and VS Code.
    let LoadedConfigLayers {
        managed_config,
        managed_config_from_mdm,
    } = loaded_config_layers;
    if let Some(config) = managed_config {
        layers.push(ConfigLayerEntry::new(
            ConfigLayerSource::LegacyManagedConfigTomlFromFile {
                file: config.file.clone(),
            },
            config.managed_config,
        ));
    }
    if let Some(config) = managed_config_from_mdm {
        layers.push(ConfigLayerEntry::new(
            ConfigLayerSource::LegacyManagedConfigTomlFromMdm,
            config,
        ));
    }

    ConfigLayerStack::new(layers, requirements)
}

async fn load_requirements_from_legacy_scheme(
    loaded_config_layers: LoadedConfigLayers,
) -> io::Result<ConfigRequirements> {
    let mut config_requirements = ConfigRequirements::default();

    // In this implementation, later layers override earlier layers, so list
    // managed_config_from_mdm last because it has the highest precedence.
    let LoadedConfigLayers {
        managed_config,
        managed_config_from_mdm,
    } = loaded_config_layers;
    for config in [
        managed_config.map(|c| c.managed_config),
        managed_config_from_mdm,
    ]
    .into_iter()
    .flatten()
    {
        let legacy_config: LegacyManagedConfigToml =
            config.try_into().map_err(|err: toml::de::Error| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Failed to parse config requirements as TOML: {err}"),
                )
            })?;

        let LegacyManagedConfigToml { approval_policy } = legacy_config;
        if let Some(approval_policy) = approval_policy {
            config_requirements.approval_policy =
                crate::config::Constrained::allow_only(approval_policy);
        }
    }

    Ok(config_requirements)
}

/// The legacy mechanism for specifying admin-enforced configuration is to read
/// from a file like `/etc/codex/managed_config.toml` that has the same
/// structure as `config.toml` where fields like `approval_policy` can specify
/// exactly one value rather than a list of allowed values.
///
/// If present, re-interpret `managed_config.toml` as a `requirements.toml`
/// where each specified field is treated as a constraint allowing only that
/// value.
#[derive(Deserialize, Debug, Clone, Default, PartialEq)]
struct LegacyManagedConfigToml {
    approval_policy: Option<AskForApproval>,
}
