//! Requirements layers are composed in the same order as config layers: lowest
//! precedence first, highest precedence last. Most fields use the same
//! TOML-level merge policy as config: lower-priority layers provide defaults,
//! and higher-priority layers override scalar/list values while recursively
//! extending tables.
//!
//! A few fields carry domain-specific meaning that raw TOML replacement would
//! break:
//! - `remote_sandbox_config` is evaluated within each layer before merging.
//! - `rules.prefix_rules` append high-priority rules first.
//! - `hooks` append high-priority event groups first while failing closed on
//!   active managed-dir conflicts.
//! - `permissions.filesystem.deny_read` is a high-priority-first union across
//!   layers.

use crate::ConfigRequirementsToml;
use crate::ConfigRequirementsWithSources;
use crate::RequirementSource;
use crate::Sourced;
use crate::merge::merge_toml_values;
use std::io;
use thiserror::Error;
use toml::Value as TomlValue;

use super::hooks::HookDirectoryField;
use super::hooks::HookMergeState;
use super::layer::ComposableRequirementsLayer;
use super::layer::RequirementsLayerEntry;
use super::permissions::DenyReadMergeState;

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
    layers: impl IntoIterator<Item = RequirementsLayerEntry>,
) -> Result<Option<ConfigRequirementsWithSources>, RequirementsCompositionError> {
    let hostname = crate::host_name();
    compose_requirements_for_hostname(layers, hostname.as_deref())
}

pub(super) fn compose_requirements_for_hostname(
    layers: impl IntoIterator<Item = RequirementsLayerEntry>,
    hostname: Option<&str>,
) -> Result<Option<ConfigRequirementsWithSources>, RequirementsCompositionError> {
    compose_requirements_for_hostname_and_hook_directory(
        layers,
        hostname,
        HookDirectoryField::current_platform(),
    )
}

pub(super) fn compose_requirements_for_hostname_and_hook_directory(
    layers: impl IntoIterator<Item = RequirementsLayerEntry>,
    hostname: Option<&str>,
    hook_directory_field: HookDirectoryField,
) -> Result<Option<ConfigRequirementsWithSources>, RequirementsCompositionError> {
    let mut stack = RequirementsLayerStack::new(hook_directory_field);
    for layer in layers {
        stack.add_layer(layer, hostname)?;
    }
    stack.compose()
}

struct RequirementsLayerStack {
    layers: Vec<ComposableRequirementsLayer>,
    hook_directory_field: HookDirectoryField,
}

impl RequirementsLayerStack {
    fn new(hook_directory_field: HookDirectoryField) -> Self {
        Self {
            layers: Vec::new(),
            hook_directory_field,
        }
    }

    fn add_layer(
        &mut self,
        layer: RequirementsLayerEntry,
        hostname: Option<&str>,
    ) -> Result<(), RequirementsCompositionError> {
        self.layers
            .push(ComposableRequirementsLayer::from_entry(layer, hostname)?);
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
            let domain_fields = &layer.domain_fields;
            super::rules::merge(&mut rules, domain_fields.rules.clone(), &layer.source);
            hooks.merge(
                &mut hooks_output,
                domain_fields.hooks.clone(),
                &layer.source,
            )?;
            deny_read.merge(domain_fields.permissions.clone(), &layer.source);
        }
        output.rules = rules;
        output.hooks = hooks_output;
        deny_read.apply_to(&mut output.permissions);

        let output_is_empty = output.clone().into_toml().is_empty();
        Ok((!output_is_empty).then_some(output))
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
        allowed_permission_profiles,
        default_permissions,
        remote_sandbox_config: _,
        allowed_web_search_modes,
        allow_managed_hooks_only,
        allow_appshots,
        computer_use,
        windows,
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
    set_sourced!(
        allowed_permission_profiles,
        &["allowed_permission_profiles"]
    );
    set_sourced!(default_permissions, &["default_permissions"]);
    set_sourced!(allowed_web_search_modes, &["allowed_web_search_modes"]);
    set_sourced!(allow_managed_hooks_only, &["allow_managed_hooks_only"]);
    set_sourced!(allow_appshots, &["allow_appshots"]);
    set_sourced!(computer_use, &["computer_use"]);
    set_sourced!(windows, &["windows"]);
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
        RequirementSource::composite(table_sources)
    } else {
        winning_source
    }
}

fn top_level_value_for_keys<'a>(value: &'a TomlValue, keys: &[&str]) -> Option<&'a TomlValue> {
    let table = value.as_table()?;
    keys.iter().find_map(|key| table.get(*key))
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
#[path = "stack_tests.rs"]
mod tests;
