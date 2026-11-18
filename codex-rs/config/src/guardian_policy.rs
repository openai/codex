//! Defines the managed review policy boundary and its ordered publication snapshot.
//! Reviewer permissions, model requirements, and policy text always travel together.

use crate::CloudConfigBundleBinding;
use crate::ConfigLayerEntry;
use crate::ConfigLayerSource;
use crate::ConfigRequirements;
use crate::ConfigRequirementsToml;
use crate::ConfigRequirementsWithSources;

/// Parsed and raw forms of the requirements projected by the Guardian loader.
#[derive(Debug)]
pub(crate) struct GuardianPolicy {
    pub(crate) requirements: ConfigRequirements,
    pub(crate) requirements_toml: ConfigRequirementsToml,
}

/// Copies the same Guardian requirements during publication and preservation.
pub(crate) fn copy_guardian_requirements(
    parsed: &mut ConfigRequirements,
    raw: &mut ConfigRequirementsToml,
    source: &ConfigRequirements,
    source_raw: &ConfigRequirementsToml,
) {
    parsed.approvals_reviewer = source.approvals_reviewer.clone();
    parsed.auto_review_required_models = source.auto_review_required_models.clone();
    parsed.guardian_policy_config_source = source.guardian_policy_config_source.clone();
    parsed.guardian_extra_policy_source = source.guardian_extra_policy_source.clone();
    raw.allowed_approvals_reviewers
        .clone_from(&source_raw.allowed_approvals_reviewers);
    raw.auto_review.clone_from(&source_raw.auto_review);
    raw.guardian_policy_config
        .clone_from(&source_raw.guardian_policy_config);
    raw.guardian_extra_policy
        .clone_from(&source_raw.guardian_extra_policy);
}

/// One managed review policy observation. Revisions are allocated before loading;
/// publication records even unchanged observations to reject older in-flight work.
#[derive(Debug)]
pub struct GuardianPolicySnapshot {
    pub revision: u64,
    pub binding: Option<CloudConfigBundleBinding>,
    pub(crate) policy: Box<GuardianPolicy>,
    pub(crate) managed_config: Vec<GuardianFallbackLayer>,
}

impl GuardianPolicySnapshot {
    pub(crate) fn from_requirements(
        revision: u64,
        requirements: ConfigRequirementsWithSources,
        binding: Option<CloudConfigBundleBinding>,
        managed_config: Vec<GuardianFallbackLayer>,
    ) -> std::io::Result<Self> {
        let parsed = ConfigRequirements::try_from(requirements.clone())?;
        Ok(Self {
            revision,
            binding,
            policy: Box::new(GuardianPolicy {
                requirements: parsed,
                requirements_toml: requirements.into_toml(),
            }),
            managed_config,
        })
    }
}

/// The projected `auto_review` value and precedence of an enabled config layer.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct GuardianFallbackLayer {
    pub(crate) source: ConfigLayerSource,
    pub(crate) value: toml::Value,
}

impl GuardianFallbackLayer {
    pub(crate) fn new(source: ConfigLayerSource, mut value: toml::Value) -> Self {
        if let Some(table) = value.as_table_mut() {
            table.retain(|key, _| key == "policy");
        }
        Self { source, value }
    }

    pub(crate) fn project(layer: &ConfigLayerEntry) -> Option<Self> {
        // An omitted section contributes nothing. A table, even an empty one,
        // replaces an earlier scalar, and a surviving scalar fails validation.
        let value = match layer.config.get("auto_review")? {
            toml::Value::Table(table) => toml::Value::Table(
                table
                    .get("policy")
                    .map(|value| ("policy".to_owned(), value.clone()))
                    .into_iter()
                    .collect(),
            ),
            value => value.clone(),
        };
        Some(Self {
            source: layer.name.clone(),
            value,
        })
    }
}
