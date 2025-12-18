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
use crate::config_loader::config_requirements::ConfigRequirementsToml;
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

/// On Unix systems, load requirements from this file path, if present.
const DEFAULT_REQUIREMENTS_TOML_FILE_UNIX: &str = "/etc/codex/requirements.toml";

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
    let mut config_requirements_toml = ConfigRequirementsToml::default();

    // TODO(mbolin): Support an entry in MDM for config requirements and use it
    // with `config_requirements_toml.merge_unset_fields(...)`, if present.

    // Honor /etc/codex/requirements.toml.
    if cfg!(unix) {
        load_requirements_toml(
            &mut config_requirements_toml,
            DEFAULT_REQUIREMENTS_TOML_FILE_UNIX,
        )
        .await?;
    }

    // Make a best-effort to support the legacy `managed_config.toml` as a
    // requirements specification.
    let loaded_config_layers = layer_io::load_config_layers_internal(codex_home, overrides).await?;
    load_requirements_from_legacy_scheme(
        &mut config_requirements_toml,
        loaded_config_layers.clone(),
    )
    .await?;

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

    ConfigLayerStack::new(layers, config_requirements_toml.try_into()?)
}

/// If available, apply requirements from `/etc/codex/requirements.toml` to
/// `config_requirements_toml` by filling in any unset fields.
async fn load_requirements_toml(
    config_requirements_toml: &mut ConfigRequirementsToml,
    requirements_toml_file: impl AsRef<Path>,
) -> io::Result<()> {
    match tokio::fs::read_to_string(&requirements_toml_file).await {
        Ok(contents) => {
            let requirements_config: ConfigRequirementsToml =
                toml::from_str(&contents).map_err(|e| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "Error parsing requirements file {}: {e}",
                            requirements_toml_file.as_ref().display(),
                        ),
                    )
                })?;
            config_requirements_toml.merge_unset_fields(requirements_config);
        }
        Err(e) => {
            if e.kind() != io::ErrorKind::NotFound {
                return Err(io::Error::new(
                    e.kind(),
                    format!(
                        "Failed to read requirements file {}: {e}",
                        requirements_toml_file.as_ref().display(),
                    ),
                ));
            }
        }
    }

    Ok(())
}

async fn load_requirements_from_legacy_scheme(
    config_requirements_toml: &mut ConfigRequirementsToml,
    loaded_config_layers: LoadedConfigLayers,
) -> io::Result<()> {
    // In this implementation, earlier layers cannot be overwritten by later
    // layers, so list managed_config_from_mdm first because it has the highest
    // precedence.
    let LoadedConfigLayers {
        managed_config,
        managed_config_from_mdm,
    } = loaded_config_layers;
    for config in [
        managed_config_from_mdm,
        managed_config.map(|c| c.managed_config),
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

        let new_requirements_toml = ConfigRequirementsToml::from(legacy_config);
        config_requirements_toml.merge_unset_fields(new_requirements_toml);
    }

    Ok(())
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

impl From<LegacyManagedConfigToml> for ConfigRequirementsToml {
    fn from(legacy: LegacyManagedConfigToml) -> Self {
        let mut config_requirements_toml = ConfigRequirementsToml::default();

        let LegacyManagedConfigToml { approval_policy } = legacy;
        if let Some(approval_policy) = approval_policy {
            config_requirements_toml.allowed_approval_policies = Some(vec![approval_policy]);
        }

        config_requirements_toml
    }
}
