use self::hooks::HookDirectoryField;
use self::hooks::HookMergeState;
use self::mcp::McpMergeState;
use crate::AppsRequirementsToml;
use crate::ConfigRequirementsToml;
use crate::ConfigRequirementsWithSources;
use crate::FeatureRequirementsToml;
use crate::RequirementSource;
use crate::RequirementsExecPolicyToml;
use crate::Sourced;
use crate::config_requirements::merge_app_requirements_descending;
use serde::Deserialize;
use serde::Serialize;
use std::fmt;
use thiserror::Error;

mod hooks;
mod mcp;
mod network;
mod permissions;

// Cloud requirements are delivered as already-prioritized TOML fragments. This
// module parses each fragment into the requirements domain object, applies any
// per-host requirements inside that fragment, and folds the parsed layers in
// bundle order.
//
// Keep the top-level field dispatch here. Domain-specific mergers live in the
// sibling modules, but this file owns the exhaustive destructuring of
// ConfigRequirementsToml so adding a new requirements field forces an explicit
// merge policy decision.

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CloudRequirementsFragment {
    pub id: String,
    pub name: String,
    pub contents: String,
}

impl CloudRequirementsFragment {
    fn source_ref(&self) -> CloudRequirementsFragmentSource {
        CloudRequirementsFragmentSource {
            id: self.id.clone(),
            name: self.name.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CloudRequirementsFragmentSource {
    pub id: String,
    pub name: String,
}

impl CloudRequirementsFragmentSource {
    pub(super) fn requirement_source(&self) -> RequirementSource {
        RequirementSource::EnterpriseManaged {
            id: self.id.clone(),
            name: self.name.clone(),
        }
    }
}

impl fmt::Display for CloudRequirementsFragmentSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.name, self.id)
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CloudRequirementsCompositionError {
    #[error("failed to parse cloud requirements fragment {fragment}: {message}")]
    Parse {
        fragment: CloudRequirementsFragmentSource,
        message: String,
    },
    #[error(
        "failed to compose cloud requirements field `{field}` between {existing_fragment} and {incoming_fragment}: {message}"
    )]
    Conflict {
        field: String,
        existing_fragment: CloudRequirementsFragmentSource,
        incoming_fragment: CloudRequirementsFragmentSource,
        message: String,
    },
}

pub fn compose_cloud_requirements(
    fragments: impl IntoIterator<Item = CloudRequirementsFragment>,
) -> Result<Option<ConfigRequirementsWithSources>, CloudRequirementsCompositionError> {
    let hostname = crate::host_name();
    compose_cloud_requirements_for_hostname(fragments, hostname.as_deref())
}

fn compose_cloud_requirements_for_hostname(
    fragments: impl IntoIterator<Item = CloudRequirementsFragment>,
    hostname: Option<&str>,
) -> Result<Option<ConfigRequirementsWithSources>, CloudRequirementsCompositionError> {
    compose_cloud_requirements_for_hostname_and_hook_directory(
        fragments,
        hostname,
        HookDirectoryField::current_platform(),
    )
}

fn compose_cloud_requirements_for_hostname_and_hook_directory(
    fragments: impl IntoIterator<Item = CloudRequirementsFragment>,
    hostname: Option<&str>,
    hook_directory_field: HookDirectoryField,
) -> Result<Option<ConfigRequirementsWithSources>, CloudRequirementsCompositionError> {
    let mut accumulator = CloudRequirementsAccumulator::new(hook_directory_field);
    for fragment in fragments {
        let source_ref = fragment.source_ref();
        let mut requirements: ConfigRequirementsToml =
            toml::from_str(&fragment.contents).map_err(|err| {
                CloudRequirementsCompositionError::Parse {
                    fragment: source_ref.clone(),
                    message: err.to_string(),
                }
            })?;
        requirements.apply_remote_sandbox_config(hostname);
        accumulator.merge_layer(source_ref, requirements)?;
    }
    accumulator.finish()
}

struct CloudRequirementsAccumulator {
    output: ConfigRequirementsWithSources,
    hooks: HookMergeState,
    mcp: McpMergeState,
}

impl CloudRequirementsAccumulator {
    fn new(hook_directory_field: HookDirectoryField) -> Self {
        Self {
            output: ConfigRequirementsWithSources::default(),
            hooks: HookMergeState::new(hook_directory_field),
            mcp: McpMergeState::default(),
        }
    }

    fn merge_layer(
        &mut self,
        source_ref: CloudRequirementsFragmentSource,
        mut requirements: ConfigRequirementsToml,
    ) -> Result<(), CloudRequirementsCompositionError> {
        if requirements
            .guardian_policy_config
            .as_deref()
            .is_some_and(|value| value.trim().is_empty())
        {
            requirements.guardian_policy_config = None;
        }

        // Destructure without `..` so new requirements fields fail to compile
        // until this cloud-composition path chooses the correct merge policy.
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
            hooks,
            mcp_servers,
            plugins,
            apps,
            rules,
            enforce_residency,
            network,
            permissions,
            guardian_policy_config,
        } = requirements;

        fill_first(
            &mut self.output.allowed_approval_policies,
            allowed_approval_policies,
            &source_ref,
        );
        fill_first(
            &mut self.output.allowed_approvals_reviewers,
            allowed_approvals_reviewers,
            &source_ref,
        );
        fill_first(
            &mut self.output.allowed_sandbox_modes,
            allowed_sandbox_modes,
            &source_ref,
        );
        fill_first(
            &mut self.output.allowed_permissions,
            allowed_permissions,
            &source_ref,
        );
        fill_first(
            &mut self.output.allowed_web_search_modes,
            allowed_web_search_modes,
            &source_ref,
        );
        fill_first(
            &mut self.output.allow_managed_hooks_only,
            allow_managed_hooks_only,
            &source_ref,
        );
        fill_first(&mut self.output.allow_appshots, allow_appshots, &source_ref);
        fill_first(&mut self.output.computer_use, computer_use, &source_ref);
        self.merge_feature_requirements(feature_requirements, &source_ref);
        self.hooks
            .merge(&mut self.output.hooks, hooks, &source_ref)?;
        self.mcp
            .merge_mcp_servers(&mut self.output.mcp_servers, mcp_servers, &source_ref)?;
        self.mcp
            .merge_plugins(&mut self.output.plugins, plugins, &source_ref)?;
        self.merge_apps(apps, &source_ref);
        self.merge_rules(rules, &source_ref);
        fill_first(
            &mut self.output.enforce_residency,
            enforce_residency,
            &source_ref,
        );
        network::merge_network(&mut self.output.network, network, &source_ref);
        permissions::merge_permissions(&mut self.output.permissions, permissions, &source_ref);
        fill_first(
            &mut self.output.guardian_policy_config,
            guardian_policy_config,
            &source_ref,
        );

        Ok(())
    }

    fn merge_feature_requirements(
        &mut self,
        incoming: Option<FeatureRequirementsToml>,
        source_ref: &CloudRequirementsFragmentSource,
    ) {
        let Some(incoming) = incoming.filter(|value| !value.is_empty()) else {
            return;
        };
        let Some(existing) = self.output.feature_requirements.as_mut() else {
            self.output.feature_requirements =
                Some(Sourced::new(incoming, source_ref.requirement_source()));
            return;
        };

        for (feature, enabled) in incoming.entries {
            if let std::collections::btree_map::Entry::Vacant(entry) =
                existing.value.entries.entry(feature)
            {
                entry.insert(enabled);
                merge_output_source(&mut existing.source, source_ref);
            }
        }
    }

    fn merge_apps(
        &mut self,
        incoming: Option<AppsRequirementsToml>,
        source_ref: &CloudRequirementsFragmentSource,
    ) {
        let Some(incoming) = incoming.filter(|apps| !apps.is_empty()) else {
            return;
        };
        let Some(existing) = self.output.apps.as_mut() else {
            self.output.apps = Some(Sourced::new(incoming, source_ref.requirement_source()));
            return;
        };

        merge_app_requirements_descending(&mut existing.value, incoming);
        merge_output_source(&mut existing.source, source_ref);
    }

    fn merge_rules(
        &mut self,
        incoming: Option<RequirementsExecPolicyToml>,
        source_ref: &CloudRequirementsFragmentSource,
    ) {
        let Some(incoming) = incoming else {
            return;
        };
        let Some(existing) = self.output.rules.as_mut() else {
            self.output.rules = Some(Sourced::new(incoming, source_ref.requirement_source()));
            return;
        };

        existing.value.prefix_rules.extend(incoming.prefix_rules);
        merge_output_source(&mut existing.source, source_ref);
    }

    fn finish(
        self,
    ) -> Result<Option<ConfigRequirementsWithSources>, CloudRequirementsCompositionError> {
        let output_is_empty = self.output.clone().into_toml().is_empty();
        Ok((!output_is_empty).then_some(self.output))
    }
}

fn fill_first<T>(
    target: &mut Option<Sourced<T>>,
    incoming: Option<T>,
    source_ref: &CloudRequirementsFragmentSource,
) {
    if target.is_none()
        && let Some(value) = incoming
    {
        *target = Some(Sourced::new(value, source_ref.requirement_source()));
    }
}

pub(super) fn merge_output_source(
    existing: &mut RequirementSource,
    incoming: &CloudRequirementsFragmentSource,
) {
    let incoming = incoming.requirement_source();
    if *existing != incoming {
        *existing = RequirementSource::CloudRequirements;
    }
}

pub(super) fn composition_conflict(
    field: String,
    existing_fragment: CloudRequirementsFragmentSource,
    incoming_fragment: CloudRequirementsFragmentSource,
    message: impl Into<String>,
) -> CloudRequirementsCompositionError {
    CloudRequirementsCompositionError::Conflict {
        field,
        existing_fragment,
        incoming_fragment,
        message: message.into(),
    }
}

#[cfg(test)]
mod tests;
