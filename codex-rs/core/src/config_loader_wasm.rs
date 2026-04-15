use crate::config::ConfigToml;
use codex_app_server_protocol::ConfigLayerSource;
use codex_config::CONFIG_TOML_FILE;
use codex_config::ConfigRequirementsWithSources;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_absolute_path::AbsolutePathBufGuard;
use std::io;
use std::path::Path;
use toml::Value as TomlValue;

pub use codex_config::AppRequirementToml;
pub use codex_config::AppsRequirementsToml;
pub use codex_config::CloudRequirementsLoadError;
pub use codex_config::CloudRequirementsLoadErrorCode;
pub use codex_config::CloudRequirementsLoader;
pub use codex_config::ConfigError;
pub use codex_config::ConfigLayerEntry;
pub use codex_config::ConfigLayerStack;
pub use codex_config::ConfigLayerStackOrdering;
pub use codex_config::ConfigLoadError;
pub use codex_config::ConfigRequirements;
pub use codex_config::ConfigRequirementsToml;
pub use codex_config::ConstrainedWithSource;
pub use codex_config::FeatureRequirementsToml;
pub use codex_config::LoaderOverrides;
pub use codex_config::McpServerIdentity;
pub use codex_config::McpServerRequirement;
pub use codex_config::NetworkConstraints;
pub use codex_config::NetworkDomainPermissionToml;
pub use codex_config::NetworkDomainPermissionsToml;
pub use codex_config::NetworkRequirementsToml;
pub use codex_config::NetworkUnixSocketPermissionToml;
pub use codex_config::NetworkUnixSocketPermissionsToml;
pub use codex_config::RequirementSource;
pub use codex_config::ResidencyRequirement;
pub use codex_config::SandboxModeRequirement;
pub use codex_config::Sourced;
pub use codex_config::TextPosition;
pub use codex_config::TextRange;
pub use codex_config::WebSearchModeRequirement;
pub(crate) use codex_config::build_cli_overrides_layer;
pub(crate) use codex_config::config_error_from_toml;
pub use codex_config::default_project_root_markers;
pub use codex_config::format_config_error;
pub use codex_config::format_config_error_with_source;
pub(crate) use codex_config::io_error_from_config_error;
pub use codex_config::merge_toml_values;
pub use codex_config::project_root_markers_from_config;

pub(crate) async fn first_layer_config_error(layers: &ConfigLayerStack) -> Option<ConfigError> {
    codex_config::first_layer_config_error::<ConfigToml>(layers, CONFIG_TOML_FILE).await
}

pub(crate) async fn first_layer_config_error_from_entries(
    layers: &[ConfigLayerEntry],
) -> Option<ConfigError> {
    codex_config::first_layer_config_error_from_entries::<ConfigToml>(layers, CONFIG_TOML_FILE)
        .await
}

pub async fn load_config_layers_state(
    _codex_home: &Path,
    cwd: Option<AbsolutePathBuf>,
    cli_overrides: &[(String, TomlValue)],
    _overrides: LoaderOverrides,
    cloud_requirements: CloudRequirementsLoader,
) -> io::Result<ConfigLayerStack> {
    let mut requirements_toml = ConfigRequirementsWithSources::default();
    if let Some(requirements) = cloud_requirements.get().await.map_err(io::Error::other)? {
        requirements_toml.merge_unset_fields(RequirementSource::CloudRequirements, requirements);
    }

    let mut layers = Vec::<ConfigLayerEntry>::new();
    if !cli_overrides.is_empty() {
        let cli_layer = build_cli_overrides_layer(cli_overrides);
        let base_dir = cwd
            .as_ref()
            .map(AbsolutePathBuf::as_path)
            .unwrap_or_else(|| Path::new("."));
        let _ = AbsolutePathBuf::from_absolute_path(base_dir)?;
        layers.push(ConfigLayerEntry::new(
            ConfigLayerSource::SessionFlags,
            resolve_relative_paths_in_config_toml(cli_layer, base_dir)?,
        ));
    }

    let requirements = requirements_toml
        .clone()
        .try_into()
        .map_err(io::Error::other)?;
    ConfigLayerStack::new(layers, requirements, requirements_toml.into_toml())
}

pub(crate) fn resolve_relative_paths_in_config_toml(
    value_from_config_toml: TomlValue,
    base_dir: &Path,
) -> io::Result<TomlValue> {
    let _guard = AbsolutePathBufGuard::new(base_dir);
    let Ok(resolved) = value_from_config_toml.clone().try_into::<ConfigToml>() else {
        return Ok(value_from_config_toml);
    };
    drop(_guard);

    let resolved_value = TomlValue::try_from(resolved).map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Failed to serialize resolved config: {err}"),
        )
    })?;

    Ok(copy_shape_from_original(
        &value_from_config_toml,
        &resolved_value,
    ))
}

fn copy_shape_from_original(original: &TomlValue, resolved: &TomlValue) -> TomlValue {
    match (original, resolved) {
        (TomlValue::Table(original_table), TomlValue::Table(resolved_table)) => {
            let mut table = toml::map::Map::new();
            for (key, original_value) in original_table {
                let resolved_value = resolved_table.get(key).unwrap_or(original_value);
                table.insert(
                    key.clone(),
                    copy_shape_from_original(original_value, resolved_value),
                );
            }
            TomlValue::Table(table)
        }
        (TomlValue::Array(original_array), TomlValue::Array(resolved_array)) => {
            let mut items = Vec::new();
            for (index, original_value) in original_array.iter().enumerate() {
                let resolved_value = resolved_array.get(index).unwrap_or(original_value);
                items.push(copy_shape_from_original(original_value, resolved_value));
            }
            TomlValue::Array(items)
        }
        (_, resolved_value) => resolved_value.clone(),
    }
}
