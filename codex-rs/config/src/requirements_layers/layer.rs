use crate::ConfigRequirementsToml;
use crate::ManagedHooksRequirementsToml;
use crate::RequirementSource;
use crate::RequirementsExecPolicyToml;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_absolute_path::AbsolutePathBufGuard;
use toml::Value as TomlValue;

use super::stack::RequirementsCompositionError;

#[derive(Clone, Debug)]
pub struct RequirementsLayerEntry {
    pub(super) source: RequirementSource,
    toml: RequirementsLayerToml,
    base_dir: Option<AbsolutePathBuf>,
}

impl RequirementsLayerEntry {
    pub fn from_toml(source: RequirementSource, contents: impl Into<String>) -> Self {
        Self {
            source,
            toml: RequirementsLayerToml::String(contents.into()),
            base_dir: None,
        }
    }

    pub fn from_toml_value(source: RequirementSource, value: TomlValue) -> Self {
        Self {
            source,
            toml: RequirementsLayerToml::Value(value),
            base_dir: None,
        }
    }

    pub fn with_base_dir(mut self, base_dir: AbsolutePathBuf) -> Self {
        self.base_dir = Some(base_dir);
        self
    }
}

#[derive(Clone, Debug)]
enum RequirementsLayerToml {
    String(String),
    Value(TomlValue),
}

#[derive(Clone, Debug)]
pub(super) struct ComposableRequirementsLayer {
    pub(super) source: RequirementSource,
    pub(super) regular_toml: TomlValue,
    pub(super) domain_fields: DomainMergedRequirementsFields,
}

impl ComposableRequirementsLayer {
    pub(super) fn from_entry(
        layer: RequirementsLayerEntry,
        hostname: Option<&str>,
    ) -> Result<Self, RequirementsCompositionError> {
        let RequirementsLayerEntry {
            source,
            toml,
            base_dir,
        } = layer;
        let (mut regular_toml, mut requirements) = {
            let _guard = base_dir
                .as_ref()
                .map(|base_dir| AbsolutePathBufGuard::new(base_dir.as_path()));
            let regular_toml = parse_layer_toml(&toml, &source)?;
            let requirements = parse_layer_requirements(&toml, &source)?;
            (regular_toml, requirements)
        };

        requirements.apply_remote_sandbox_config(hostname);
        materialize_resolved_path_requirements(&mut regular_toml, &requirements)?;
        materialize_remote_sandbox_config(&mut regular_toml, &requirements)?;
        strip_special_fields(&mut regular_toml);

        Ok(Self {
            source,
            regular_toml,
            domain_fields: DomainMergedRequirementsFields {
                rules: requirements.rules,
                hooks: requirements.hooks,
                permissions: requirements.permissions,
            },
        })
    }
}

#[derive(Clone, Debug)]
pub(super) struct DomainMergedRequirementsFields {
    pub(super) rules: Option<RequirementsExecPolicyToml>,
    pub(super) hooks: Option<ManagedHooksRequirementsToml>,
    pub(super) permissions: Option<crate::config_requirements::PermissionsRequirementsToml>,
}

fn parse_layer_toml(
    toml: &RequirementsLayerToml,
    source: &RequirementSource,
) -> Result<TomlValue, RequirementsCompositionError> {
    match toml {
        RequirementsLayerToml::String(contents) => {
            toml::from_str(contents).map_err(|err: toml::de::Error| {
                RequirementsCompositionError::Parse {
                    layer_source: source.clone(),
                    message: err.to_string(),
                }
            })
        }
        RequirementsLayerToml::Value(value) => Ok(value.clone()),
    }
}

fn parse_layer_requirements(
    toml: &RequirementsLayerToml,
    source: &RequirementSource,
) -> Result<ConfigRequirementsToml, RequirementsCompositionError> {
    match toml {
        RequirementsLayerToml::String(contents) => {
            toml::from_str(contents).map_err(|err: toml::de::Error| {
                RequirementsCompositionError::Parse {
                    layer_source: source.clone(),
                    message: err.to_string(),
                }
            })
        }
        RequirementsLayerToml::Value(value) => {
            value.clone().try_into().map_err(|err: toml::de::Error| {
                RequirementsCompositionError::Parse {
                    layer_source: source.clone(),
                    message: err.to_string(),
                }
            })
        }
    }
}

fn materialize_resolved_path_requirements(
    layer_toml: &mut TomlValue,
    requirements: &ConfigRequirementsToml,
) -> Result<(), RequirementsCompositionError> {
    let Some(table) = layer_toml.as_table_mut() else {
        return Ok(());
    };

    for (key, value) in [
        ("sqlite_home", requirements.sqlite_home.as_ref()),
        ("log_dir", requirements.log_dir.as_ref()),
        (
            "model_catalog_json",
            requirements.model_catalog_json.as_ref(),
        ),
    ] {
        if let Some(value) = value {
            table.insert(key.to_string(), toml_value_from_serializable(value)?);
        }
    }

    Ok(())
}

fn materialize_remote_sandbox_config(
    layer_toml: &mut TomlValue,
    requirements: &ConfigRequirementsToml,
) -> Result<(), RequirementsCompositionError> {
    remove_top_level_field(layer_toml, "remote_sandbox_config");
    let Some(allowed_sandbox_modes) = requirements.allowed_sandbox_modes.as_ref() else {
        return Ok(());
    };
    let Some(table) = layer_toml.as_table_mut() else {
        return Ok(());
    };
    table.insert(
        "allowed_sandbox_modes".to_string(),
        toml_value_from_serializable(allowed_sandbox_modes)?,
    );
    Ok(())
}

fn toml_value_from_serializable<T: serde::Serialize>(
    value: T,
) -> Result<TomlValue, RequirementsCompositionError> {
    TomlValue::try_from(value).map_err(|err| RequirementsCompositionError::ComposedParse {
        message: err.to_string(),
    })
}

fn strip_special_fields(layer_toml: &mut TomlValue) {
    remove_top_level_field(layer_toml, "rules");
    remove_top_level_field(layer_toml, "hooks");
    remove_nested_field_and_prune_empty(layer_toml, &["permissions", "filesystem", "deny_read"]);
}

fn remove_top_level_field(value: &mut TomlValue, key: &str) -> Option<TomlValue> {
    value.as_table_mut()?.remove(key)
}

fn remove_nested_field_and_prune_empty(value: &mut TomlValue, path: &[&str]) -> Option<TomlValue> {
    let (key, remaining) = path.split_first()?;
    let table = value.as_table_mut()?;
    if remaining.is_empty() {
        return table.remove(*key);
    }

    let removed = table
        .get_mut(*key)
        .and_then(|child| remove_nested_field_and_prune_empty(child, remaining));
    if table
        .get(*key)
        .and_then(TomlValue::as_table)
        .is_some_and(toml::map::Map::is_empty)
    {
        table.remove(*key);
    }
    removed
}
