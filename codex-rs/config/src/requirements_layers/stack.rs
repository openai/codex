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
//! - Incompatible definitions of the same named MCP server fail composition
//!   instead of silently combining legacy identities with matcher rules.
//! - `permissions.filesystem.deny_read` is a high-priority-first union across
//!   layers.

use crate::ConfigRequirementsToml;
use crate::ConfigRequirementsWithSources;
use crate::RequirementSource;
use crate::Sourced;
use crate::merge::merge_toml_values;
use std::cell::OnceCell;
use std::collections::BTreeMap;
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
    compose_requirements_with_hostname_resolver(layers, crate::host_name)
}

#[cfg(test)]
pub(super) fn compose_requirements_for_hostname(
    layers: impl IntoIterator<Item = RequirementsLayerEntry>,
    hostname: Option<&str>,
) -> Result<Option<ConfigRequirementsWithSources>, RequirementsCompositionError> {
    let hostname = hostname.map(str::to_string);
    compose_requirements_with_hostname_resolver_and_hook_directory(
        layers,
        move || hostname.clone(),
        HookDirectoryField::current_platform(),
    )
}

#[cfg(test)]
pub(super) fn compose_requirements_for_hostname_and_hook_directory(
    layers: impl IntoIterator<Item = RequirementsLayerEntry>,
    hostname: Option<&str>,
    hook_directory_field: HookDirectoryField,
) -> Result<Option<ConfigRequirementsWithSources>, RequirementsCompositionError> {
    let hostname = hostname.map(str::to_string);
    compose_requirements_with_hostname_resolver_and_hook_directory(
        layers,
        move || hostname.clone(),
        hook_directory_field,
    )
}

fn compose_requirements_with_hostname_resolver(
    layers: impl IntoIterator<Item = RequirementsLayerEntry>,
    hostname_resolver: impl Fn() -> Option<String>,
) -> Result<Option<ConfigRequirementsWithSources>, RequirementsCompositionError> {
    compose_requirements_with_hostname_resolver_and_hook_directory(
        layers,
        hostname_resolver,
        HookDirectoryField::current_platform(),
    )
}

fn compose_requirements_with_hostname_resolver_and_hook_directory(
    layers: impl IntoIterator<Item = RequirementsLayerEntry>,
    hostname_resolver: impl Fn() -> Option<String>,
    hook_directory_field: HookDirectoryField,
) -> Result<Option<ConfigRequirementsWithSources>, RequirementsCompositionError> {
    // Evaluate every layer in this composition against the same hostname while
    // keeping resolution lazy when no layer needs remote sandbox matching.
    let hostname = OnceCell::new();
    let cached_hostname_resolver = || hostname.get_or_init(&hostname_resolver).clone();
    let mut stack = RequirementsLayerStack::new(hook_directory_field);
    for layer in layers {
        stack.add_layer(layer, &cached_hostname_resolver)?;
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
        hostname_resolver: &dyn Fn() -> Option<String>,
    ) -> Result<(), RequirementsCompositionError> {
        self.layers.push(ComposableRequirementsLayer::from_entry(
            layer,
            hostname_resolver,
        )?);
        Ok(())
    }

    fn compose(
        self,
    ) -> Result<Option<ConfigRequirementsWithSources>, RequirementsCompositionError> {
        let Self {
            layers,
            hook_directory_field,
        } = self;

        validate_mcp_server_requirement_shapes(&layers)?;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum McpServerRequirementShape {
    LegacyCommandIdentity,
    LegacyUrlIdentity,
    CommandMatcher,
    ExactUrlMatcher,
    PrefixUrlMatcher,
    RegexUrlMatcher,
}

impl std::fmt::Display for McpServerRequirementShape {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LegacyCommandIdentity => f.write_str("legacy command identity"),
            Self::LegacyUrlIdentity => f.write_str("legacy URL identity"),
            Self::CommandMatcher => f.write_str("command matcher"),
            Self::ExactUrlMatcher => f.write_str("exact URL matcher"),
            Self::PrefixUrlMatcher => f.write_str("prefix URL matcher"),
            Self::RegexUrlMatcher => f.write_str("regex URL matcher"),
        }
    }
}

fn validate_mcp_server_requirement_shapes(
    layers: &[ComposableRequirementsLayer],
) -> Result<(), RequirementsCompositionError> {
    let mut seen = BTreeMap::<String, (McpServerRequirementShape, RequirementSource)>::new();
    for layer in layers {
        if let Some(requirements) = table_at_path(&layer.regular_toml, &["mcp_servers"]) {
            validate_mcp_server_requirement_table(
                requirements,
                "mcp_servers",
                &layer.source,
                &mut seen,
            )?;
        }

        let Some(plugins) = table_at_path(&layer.regular_toml, &["plugins"]) else {
            continue;
        };
        for plugin_name in plugins.keys() {
            let Some(requirements) = table_at_path(
                &layer.regular_toml,
                &["plugins", plugin_name, "mcp_servers"],
            ) else {
                continue;
            };
            validate_mcp_server_requirement_table(
                requirements,
                &format!("plugins.\"{plugin_name}\".mcp_servers"),
                &layer.source,
                &mut seen,
            )?;
        }
    }
    Ok(())
}

fn validate_mcp_server_requirement_table(
    requirements: &toml::Table,
    table_path: &str,
    source: &RequirementSource,
    seen: &mut BTreeMap<String, (McpServerRequirementShape, RequirementSource)>,
) -> Result<(), RequirementsCompositionError> {
    for (server_name, requirement) in requirements {
        let Some(shape) = mcp_server_requirement_shape(requirement) else {
            continue;
        };
        let field = format!("{table_path}.{server_name}");
        if let Some((existing_shape, existing_source)) = seen.get(&field)
            && *existing_shape != shape
        {
            return Err(composition_conflict(
                field,
                existing_source.clone(),
                source.clone(),
                format!(
                    "cannot combine {existing_shape} with {shape}; define this MCP server using one requirement form"
                ),
            ));
        }
        seen.insert(field, (shape, source.clone()));
    }
    Ok(())
}

fn mcp_server_requirement_shape(value: &TomlValue) -> Option<McpServerRequirementShape> {
    let requirement = value.as_table()?;
    if let Some(identity) = requirement.get("identity").and_then(TomlValue::as_table) {
        if identity.contains_key("command") {
            return Some(McpServerRequirementShape::LegacyCommandIdentity);
        }
        if identity.contains_key("url") {
            return Some(McpServerRequirementShape::LegacyUrlIdentity);
        }
        return None;
    }
    if requirement.contains_key("command") {
        return Some(McpServerRequirementShape::CommandMatcher);
    }
    let matcher = requirement.get("url")?.as_table()?;
    match matcher.get("match")?.as_str()? {
        "exact" => Some(McpServerRequirementShape::ExactUrlMatcher),
        "prefix" => Some(McpServerRequirementShape::PrefixUrlMatcher),
        "regex" => Some(McpServerRequirementShape::RegexUrlMatcher),
        _ => None,
    }
}

fn table_at_path<'a>(mut value: &'a TomlValue, path: &[&str]) -> Option<&'a toml::Table> {
    for key in path {
        value = value.as_table()?.get(*key)?;
    }
    value.as_table()
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
        allow_remote_control,
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
    set_sourced!(allow_remote_control, &["allow_remote_control"]);
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
