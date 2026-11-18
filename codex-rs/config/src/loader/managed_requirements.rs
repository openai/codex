//! Reads managed requirements in precedence order. Full configuration and Guardian
//! policy compose and validate their own views of those sources.

use super::layer_io;
use super::layer_io::LoadedConfigLayers;
use super::load_requirements_toml;
#[cfg(target_os = "macos")]
use super::macos;
use super::requirements_layers_from_legacy_scheme;
use super::system_requirements_toml_file_with_overrides;
use crate::CloudConfigBundleLoader;
use crate::CloudConfigBundleSnapshot;
use crate::ConfigLayerSource;
use crate::ConfigRequirements;
use crate::ConfigRequirementsToml;
use crate::GuardianPolicySnapshot;
use crate::RequirementsLayerEntry;
use crate::compose_requirements;
use crate::config_requirements::ConfigRequirementsWithSources;
use crate::guardian_policy::GuardianFallbackLayer;
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
        bundle_requirements = bundle
            .requirements_toml
            .into_layers()
            .into_iter()
            .map(|layer| layer.with_base_dir(base_dir.clone()))
            .collect();
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

/// Loads only managed review requirements, preserving their provenance and source validity.
pub async fn load_guardian_policy_snapshot(
    fs: &dyn ExecutorFileSystem,
    codex_home: &Path,
    overrides: &LoaderOverrides,
    cloud_config_bundle: &CloudConfigBundleLoader,
    revision: u64,
) -> io::Result<GuardianPolicySnapshot> {
    let mut legacy = layer_io::load_config_layers_internal(
        fs, codex_home, overrides, /*strict_config*/ false,
    )
    .await?;
    let snapshot = if overrides.ignore_managed_requirements {
        CloudConfigBundleSnapshot {
            config_server: None,
            bundle: Ok(None),
            binding: None,
        }
    } else {
        cloud_config_bundle.get_snapshot().await
    };
    let base = AbsolutePathBuf::from_absolute_path(codex_home)?;
    let bundle = snapshot
        .bundle
        .map_err(io::Error::other)?
        .unwrap_or_default();
    // Cloud fragments arrive highest priority first; merging proceeds lowest first.
    let mut managed_config = Vec::new();
    for fragment in bundle.config_toml.enterprise_managed.into_iter().rev() {
        let mut table: toml::Table =
            toml::from_str(&fragment.contents).map_err(io::Error::other)?;
        if let Some(value) = table.remove("auto_review") {
            managed_config.push(GuardianFallbackLayer::new(
                ConfigLayerSource::EnterpriseManaged {
                    id: fragment.id,
                    name: fragment.name,
                },
                value,
            ));
        }
    }
    // Preserve the fallbacks before narrowing legacy config to reviewer requirements.
    let mut project_legacy = |source, config: &mut toml::Value| {
        if let Some(table) = config.as_table_mut() {
            if let Some(value) = table.remove("auto_review") {
                managed_config.push(GuardianFallbackLayer::new(source, value));
            }
            table.retain(|key, _| key == "approvals_reviewer");
        }
    };
    if let Some(layer) = &mut legacy.managed_config {
        project_legacy(
            ConfigLayerSource::LegacyManagedConfigTomlFromFile {
                file: layer.file.clone(),
            },
            &mut layer.managed_config,
        );
    }
    if let Some(layer) = &mut legacy.managed_config_from_mdm {
        project_legacy(
            ConfigLayerSource::LegacyManagedConfigTomlFromMdm,
            &mut layer.managed_config,
        );
    }
    let layers = bundle
        .requirements_toml
        .into_layers()
        .into_iter()
        .map(|layer| layer.with_base_dir(base.clone()))
        .collect();
    let requirements_layers =
        load_requirements_layers(fs, codex_home, overrides, legacy, layers).await?;
    let requirements_layers = requirements_layers
        .into_iter()
        .map(|layer| {
            let (source, mut value, base_dir) = layer.into_raw_parts()?;
            if let Some(table) = value.as_table_mut() {
                table.retain(|key, _| {
                    matches!(
                        key,
                        "allowed_approvals_reviewers"
                            | "auto_review"
                            | "guardian_policy_config"
                            | "guardian_extra_policy"
                            | "config_server"
                    )
                });
            }
            let layer = RequirementsLayerEntry::from_toml_value(source, value);
            Ok::<_, io::Error>(match base_dir {
                Some(base) => layer.with_base_dir(base),
                None => layer,
            })
        })
        .collect::<io::Result<Vec<_>>>()?;
    let mut requirements = compose_requirements(requirements_layers)?.unwrap_or_default();
    if requirements.config_server.take().map(|source| source.value) != snapshot.config_server {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "Required managed configuration has not been loaded",
        ));
    }
    // Fallbacks are validated after retained thread layers have been merged.
    GuardianPolicySnapshot::from_requirements(
        revision,
        requirements,
        snapshot.binding,
        managed_config,
    )
}

/// Composes all managed requirements and retains source layers for diagnostics.
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
    let loaded_config_layers =
        layer_io::load_config_layers_internal(fs, codex_home, overrides, strict_config).await?;
    let requirements_layers = load_requirements_layers(
        fs,
        codex_home,
        overrides,
        loaded_config_layers.clone(),
        bundle_requirements,
    )
    .await?;
    let mut requirements = compose_requirements(requirements_layers.clone())?.unwrap_or_default();
    if overrides.ignore_login_requirements {
        requirements.allowed_login_methods = None;
        requirements.allowed_chatgpt_workspaces = None;
    }
    Ok((requirements, loaded_config_layers, requirements_layers))
}

/// Collects ordered requirement layers, converting the supplied legacy configuration.
async fn load_requirements_layers(
    fs: &dyn ExecutorFileSystem,
    codex_home: &Path,
    overrides: &LoaderOverrides,
    legacy_config: LoadedConfigLayers,
    bundle_requirements: Vec<RequirementsLayerEntry>,
) -> io::Result<Vec<RequirementsLayerEntry>> {
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

    if !overrides.ignore_managed_requirements {
        requirements_layers.extend(system_requirements_layer);
        requirements_layers.extend(bundle_requirements);
        requirements_layers.extend(requirements_layers_from_legacy_scheme(
            legacy_config,
            codex_home,
        )?);
        requirements_layers.extend(managed_preferences_requirements_layer);
    }
    Ok(requirements_layers)
}
