use self::hooks::HookDirectoryField;
use self::hooks::HookMergeState;
use self::permissions::DenyReadMergeState;
use crate::ConfigRequirementsToml;
use crate::ConfigRequirementsWithSources;
use crate::ManagedHooksRequirementsToml;
use crate::RequirementSource;
use crate::RequirementsExecPolicyToml;
use crate::Sourced;
use crate::merge::merge_toml_values;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_absolute_path::AbsolutePathBufGuard;
use std::io;
use thiserror::Error;
use toml::Value as TomlValue;

mod hooks;
mod permissions;

// Requirements layers are composed in the same order as config layers: lowest
// precedence first, highest precedence last. Most fields use the same
// TOML-level merge policy as config: lower-priority layers provide defaults,
// and higher-priority layers override scalar/list values while recursively
// extending tables.
//
// A few fields carry domain-specific meaning that raw TOML replacement would
// break:
// - `remote_sandbox_config` is evaluated within each layer before merging.
// - `rules.prefix_rules` append high-priority rules first.
// - `hooks` append high-priority event groups first while failing closed on
//   active managed-dir conflicts.
// - `permissions.filesystem.deny_read` is a high-priority-first union across
//   layers.

#[derive(Clone, Debug)]
pub struct RequirementsLayer {
    source: RequirementSource,
    toml: RequirementsLayerToml,
    base_dir: Option<AbsolutePathBuf>,
}

impl RequirementsLayer {
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

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RequirementsCompositionError {
    #[error("failed to parse requirements layer {layer_source}: {message}")]
    Parse {
        layer_source: RequirementSource,
        message: String,
    },
    #[error("failed to parse merged requirements: {message}")]
    ComposedParse { message: String },
    #[error(
        "failed to compose requirements field `{field}` between {existing_source} and {incoming_source}: {message}"
    )]
    Conflict {
        field: String,
        existing_source: RequirementSource,
        incoming_source: RequirementSource,
        message: String,
    },
}

impl From<RequirementsCompositionError> for io::Error {
    fn from(error: RequirementsCompositionError) -> Self {
        io::Error::new(io::ErrorKind::InvalidData, error)
    }
}

pub fn compose_requirements(
    layers: impl IntoIterator<Item = RequirementsLayer>,
) -> Result<Option<ConfigRequirementsWithSources>, RequirementsCompositionError> {
    let hostname = crate::host_name();
    compose_requirements_for_hostname(layers, hostname.as_deref())
}

fn compose_requirements_for_hostname(
    layers: impl IntoIterator<Item = RequirementsLayer>,
    hostname: Option<&str>,
) -> Result<Option<ConfigRequirementsWithSources>, RequirementsCompositionError> {
    compose_requirements_for_hostname_and_hook_directory(
        layers,
        hostname,
        HookDirectoryField::current_platform(),
    )
}

fn compose_requirements_for_hostname_and_hook_directory(
    layers: impl IntoIterator<Item = RequirementsLayer>,
    hostname: Option<&str>,
    hook_directory_field: HookDirectoryField,
) -> Result<Option<ConfigRequirementsWithSources>, RequirementsCompositionError> {
    let mut composer = RequirementsComposer::new(hook_directory_field);
    for layer in layers {
        composer.add_layer(layer, hostname)?;
    }
    composer.compose()
}

#[derive(Clone, Debug)]
struct ComposableRequirementsLayer {
    source: RequirementSource,
    regular_toml: TomlValue,
    custom_fields: CustomMergedRequirementsFields,
}

#[derive(Clone, Debug)]
struct CustomMergedRequirementsFields {
    rules: Option<RequirementsExecPolicyToml>,
    hooks: Option<ManagedHooksRequirementsToml>,
    permissions: Option<crate::config_requirements::PermissionsRequirementsToml>,
}

struct RequirementsComposer {
    layers: Vec<ComposableRequirementsLayer>,
    hook_directory_field: HookDirectoryField,
}

impl RequirementsComposer {
    fn new(hook_directory_field: HookDirectoryField) -> Self {
        Self {
            layers: Vec::new(),
            hook_directory_field,
        }
    }

    fn add_layer(
        &mut self,
        layer: RequirementsLayer,
        hostname: Option<&str>,
    ) -> Result<(), RequirementsCompositionError> {
        let RequirementsLayer {
            source,
            toml,
            base_dir,
        } = layer;
        let _guard = base_dir
            .as_ref()
            .map(|base_dir| AbsolutePathBufGuard::new(base_dir.as_path()));
        let mut regular_toml = parse_layer_toml(&toml, &source)?;
        let mut requirements = parse_layer_requirements(&toml, &source)?;

        requirements.apply_remote_sandbox_config(hostname);
        materialize_remote_sandbox_config(&mut regular_toml, &requirements)?;
        strip_special_fields(&mut regular_toml);

        self.layers.push(ComposableRequirementsLayer {
            source,
            regular_toml,
            custom_fields: CustomMergedRequirementsFields {
                rules: requirements.rules,
                hooks: requirements.hooks,
                permissions: requirements.permissions,
            },
        });
        Ok(())
    }

    fn compose(
        self,
    ) -> Result<Option<ConfigRequirementsWithSources>, RequirementsCompositionError> {
        let Self {
            layers,
            hook_directory_field,
        } = self;

        let mut merged_toml = TomlValue::Table(toml::map::Map::new());
        for layer in &layers {
            merge_toml_values(&mut merged_toml, &layer.regular_toml);
        }

        let requirements: ConfigRequirementsToml =
            merged_toml.try_into().map_err(|err: toml::de::Error| {
                RequirementsCompositionError::ComposedParse {
                    message: err.to_string(),
                }
            })?;
        let mut output = ConfigRequirementsWithSources::default();
        populate_merged_regular_fields_with_sources(&mut output, requirements, &layers);
        let mut rules = None;
        let mut hooks = HookMergeState::new(hook_directory_field);
        let mut hooks_output = None;
        let mut deny_read = DenyReadMergeState::default();
        // Regular TOML fields are folded low-to-high like config. These custom
        // fields append or union values, so process them high-to-low to keep
        // priority order visible in the output.
        for layer in layers.iter().rev() {
            let custom_fields = &layer.custom_fields;
            merge_rules(&mut rules, custom_fields.rules.clone(), &layer.source);
            hooks.merge(
                &mut hooks_output,
                custom_fields.hooks.clone(),
                &layer.source,
            )?;
            deny_read.merge(custom_fields.permissions.clone(), &layer.source);
        }
        output.rules = rules;
        output.hooks = hooks_output;
        deny_read.apply_to(&mut output.permissions);

        let output_is_empty = output.clone().into_toml().is_empty();
        Ok((!output_is_empty).then_some(output))
    }
}

fn merge_rules(
    target: &mut Option<Sourced<RequirementsExecPolicyToml>>,
    incoming: Option<RequirementsExecPolicyToml>,
    source: &RequirementSource,
) {
    let Some(incoming) = incoming else {
        return;
    };
    let Some(existing) = target.as_mut() else {
        *target = Some(Sourced::new(incoming, source.clone()));
        return;
    };

    let RequirementsExecPolicyToml { prefix_rules } = incoming;
    existing.value.prefix_rules.extend(prefix_rules);
    merge_output_source(&mut existing.source, source);
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

fn populate_merged_regular_fields_with_sources(
    output: &mut ConfigRequirementsWithSources,
    requirements: ConfigRequirementsToml,
    layers: &[ComposableRequirementsLayer],
) {
    macro_rules! set_sourced {
        ($field:ident, $keys:expr) => {
            if let Some(value) = $field {
                output.$field = Some(Sourced::new(
                    value,
                    source_for_top_level_keys(layers, $keys),
                ));
            }
        };
    }

    // Destructure without `..` so every new requirements field must choose
    // whether it belongs in the regular TOML merge path or in a special merger.
    let ConfigRequirementsToml {
        allowed_approval_policies,
        allowed_approvals_reviewers,
        allowed_sandbox_modes,
        allowed_permissions,
        remote_sandbox_config: _,
        allowed_web_search_modes,
        allow_managed_hooks_only,
        allow_appshots,
        computer_use,
        feature_requirements,
        hooks: _,
        mcp_servers,
        plugins,
        apps,
        rules: _,
        enforce_residency,
        network,
        permissions,
        guardian_policy_config,
    } = requirements;

    set_sourced!(allowed_approval_policies, &["allowed_approval_policies"]);
    set_sourced!(
        allowed_approvals_reviewers,
        &["allowed_approvals_reviewers"]
    );
    set_sourced!(allowed_sandbox_modes, &["allowed_sandbox_modes"]);
    set_sourced!(allowed_permissions, &["allowed_permissions"]);
    set_sourced!(allowed_web_search_modes, &["allowed_web_search_modes"]);
    set_sourced!(allow_managed_hooks_only, &["allow_managed_hooks_only"]);
    set_sourced!(allow_appshots, &["allow_appshots"]);
    set_sourced!(computer_use, &["computer_use"]);
    set_sourced!(feature_requirements, &["features", "feature_requirements"]);
    set_sourced!(mcp_servers, &["mcp_servers"]);
    set_sourced!(plugins, &["plugins"]);
    set_sourced!(apps, &["apps"]);
    set_sourced!(enforce_residency, &["enforce_residency"]);
    set_sourced!(network, &["experimental_network"]);
    set_sourced!(permissions, &["permissions"]);

    if let Some(guardian_policy_config) =
        guardian_policy_config.filter(|value| !value.trim().is_empty())
    {
        output.guardian_policy_config = Some(Sourced::new(
            guardian_policy_config,
            source_for_top_level_keys(layers, &["guardian_policy_config"]),
        ));
    }
}

fn source_for_top_level_keys(
    layers: &[ComposableRequirementsLayer],
    keys: &[&str],
) -> RequirementSource {
    let matching_layers = layers
        .iter()
        .filter_map(|layer| {
            top_level_value_for_keys(&layer.regular_toml, keys).map(|value| (&layer.source, value))
        })
        .collect::<Vec<_>>();
    let Some((winning_source, winning_value)) = matching_layers.last() else {
        return RequirementSource::Unknown;
    };
    let winning_source = (*winning_source).clone();

    if !winning_value.is_table() {
        return winning_source;
    }

    let table_sources = matching_layers
        .into_iter()
        .rev()
        .filter_map(|(source, value)| value.is_table().then_some(source.clone()))
        .collect::<Vec<_>>();
    if table_sources.len() > 1 {
        return RequirementSource::composite(table_sources);
    }

    winning_source
}

fn top_level_value_for_keys<'a>(value: &'a TomlValue, keys: &[&str]) -> Option<&'a TomlValue> {
    let table = value.as_table()?;
    keys.iter().find_map(|key| table.get(*key))
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

pub(super) fn merge_output_source(existing: &mut RequirementSource, incoming: &RequirementSource) {
    if existing != incoming {
        *existing = RequirementSource::composite([existing.clone(), incoming.clone()]);
    }
}

pub(super) fn composition_conflict(
    field: String,
    existing_source: RequirementSource,
    incoming_source: RequirementSource,
    message: impl Into<String>,
) -> RequirementsCompositionError {
    RequirementsCompositionError::Conflict {
        field,
        existing_source,
        incoming_source,
        message: message.into(),
    }
}

#[cfg(test)]
mod tests;
