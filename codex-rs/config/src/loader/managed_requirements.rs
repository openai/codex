//! Loads managed requirements independently of user and project configuration,
//! retaining source layers for diagnostics during full configuration loads.

use super::layer_io;
use super::layer_io::LoadedConfigLayers;
use super::load_requirements_toml;
#[cfg(target_os = "macos")]
use super::macos;
use super::requirements_layers_from_legacy_scheme;
use super::system_requirements_toml_file_with_overrides;
use crate::ConfigRequirements;
use crate::ConfigRequirementsToml;
use crate::RequirementsLayerEntry;
use crate::compose_requirements;
use crate::config_requirements::ConfigRequirementsWithSources;
use crate::state::ConfigLoadOptions;
use crate::state::LoaderOverrides;
use codex_file_system::ExecutorFileSystem;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::io;
use std::path::Path;

/// Loads current managed requirements without reading system defaults, user or
/// project settings, or thread-provided configuration. Legacy managed config
/// remains a requirements source.
pub async fn load_managed_requirements_state(
    fs: &dyn ExecutorFileSystem,
    codex_home: &Path,
    options: impl Into<ConfigLoadOptions>,
) -> io::Result<ConfigRequirementsToml> {
    let ConfigLoadOptions {
        loader_overrides: overrides,
        strict_config,
        cloud_config_bundle,
    } = options.into();
    if overrides.ignore_managed_requirements {
        return Ok(ConfigRequirementsToml::default());
    }

    let mut bundle_requirements = Vec::new();
    if let Some(bundle) = cloud_config_bundle.get().await.map_err(io::Error::other)? {
        let base_dir = AbsolutePathBuf::from_absolute_path(codex_home)?;
        bundle_requirements = bundle.requirements_toml.into_layers(&base_dir);
    }
    let (requirements, _, _) = load_requirements_from_sources(
        fs,
        codex_home,
        &overrides,
        strict_config,
        bundle_requirements,
    )
    .await?;
    let _: ConfigRequirements = requirements.clone().try_into()?;
    let requirements_toml = requirements.into_toml();
    // Full config loads also validate required provider definitions when
    // constructing ConfigLayerStack, including providers not in use by this thread.
    let _ = crate::model_provider_requirements::to_config(&requirements_toml)?;
    Ok(requirements_toml)
}

/// Composes the same requirements for full config loads and policy-only checks.
/// The loaded legacy config and source layers are also returned for the full
/// config stack and its ignored-field diagnostics.
pub(super) async fn load_requirements_from_sources(
    fs: &dyn ExecutorFileSystem,
    codex_home: &Path,
    overrides: &LoaderOverrides,
    strict_config: bool,
    bundle_requirements: Vec<RequirementsLayerEntry>,
) -> io::Result<(
    ConfigRequirementsWithSources,
    LoadedConfigLayers,
    Vec<RequirementsLayerEntry>,
)> {
    let mut requirements_layers = Vec::new();
    let mut system_requirements_layer = None;
    let managed_preferences_requirements_layer;
    if !overrides.ignore_managed_requirements {
        #[cfg(target_os = "macos")]
        {
            let base_dir = AbsolutePathBuf::from_absolute_path(codex_home)?;
            managed_preferences_requirements_layer = macos::load_managed_admin_requirements_layer(
                overrides
                    .macos_managed_config_requirements_base64
                    .as_deref(),
            )
            .await?
            .map(|layer| layer.with_base_dir(base_dir));
        }
        #[cfg(not(target_os = "macos"))]
        {
            managed_preferences_requirements_layer = None;
        }

        let requirements_file = system_requirements_toml_file_with_overrides(overrides)?;
        system_requirements_layer = load_requirements_toml(fs, &requirements_file).await?;
    } else {
        managed_preferences_requirements_layer = None;
    }

    let loaded_config_layers =
        layer_io::load_config_layers_internal(fs, codex_home, overrides.clone(), strict_config)
            .await?;
    if !overrides.ignore_managed_requirements {
        requirements_layers.extend(system_requirements_layer);
        requirements_layers.extend(bundle_requirements);
        // Legacy managed_config.toml contributes approval and sandbox requirements.
        requirements_layers.extend(requirements_layers_from_legacy_scheme(
            loaded_config_layers.clone(),
            codex_home,
        )?);
        requirements_layers.extend(managed_preferences_requirements_layer);
    }

    let mut requirements = compose_requirements(requirements_layers.clone())?.unwrap_or_default();
    if overrides.ignore_login_requirements {
        requirements.allowed_login_methods = None;
        requirements.allowed_chatgpt_workspaces = None;
    }
    Ok((requirements, loaded_config_layers, requirements_layers))
}
