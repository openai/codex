use codex_protocol::config_types::SandboxMode;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::SandboxPolicy;
use codex_utils_absolute_path::AbsolutePathBuf;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fmt;

use super::requirements_exec_policy::RequirementsExecPolicy;
use super::requirements_exec_policy::RequirementsExecPolicyToml;
use crate::config::Constrained;
use crate::config::ConstraintError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RequirementSource {
    Unknown,
    MdmManagedPreferences { domain: String, key: String },
    CloudRequirements,
    SystemRequirementsToml { file: AbsolutePathBuf },
    LegacyManagedConfigTomlFromFile { file: AbsolutePathBuf },
    LegacyManagedConfigTomlFromMdm,
}

impl fmt::Display for RequirementSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RequirementSource::Unknown => write!(f, "<unspecified>"),
            RequirementSource::MdmManagedPreferences { domain, key } => {
                write!(f, "MDM {domain}:{key}")
            }
            RequirementSource::CloudRequirements => {
                write!(f, "cloud requirements")
            }
            RequirementSource::SystemRequirementsToml { file } => {
                write!(f, "{}", file.as_path().display())
            }
            RequirementSource::LegacyManagedConfigTomlFromFile { file } => {
                write!(f, "{}", file.as_path().display())
            }
            RequirementSource::LegacyManagedConfigTomlFromMdm => {
                write!(f, "MDM managed_config.toml (legacy)")
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConstrainedWithSource<T> {
    pub value: Constrained<T>,
    pub source: Option<RequirementSource>,
}

impl<T> ConstrainedWithSource<T> {
    pub fn new(value: Constrained<T>, source: Option<RequirementSource>) -> Self {
        Self { value, source }
    }
}

impl<T> std::ops::Deref for ConstrainedWithSource<T> {
    type Target = Constrained<T>;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T> std::ops::DerefMut for ConstrainedWithSource<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}

/// Normalized version of [`ConfigRequirementsToml`] after deserialization and
/// normalization.
#[derive(Debug, Clone, PartialEq)]
pub struct ConfigRequirements {
    pub approval_policy: ConstrainedWithSource<AskForApproval>,
    pub sandbox_policy: ConstrainedWithSource<SandboxPolicy>,
    pub mcp_servers: Option<Sourced<BTreeMap<String, McpServerRequirement>>>,
    pub(crate) exec_policy: Option<Sourced<RequirementsExecPolicy>>,
    pub enforce_residency: ConstrainedWithSource<Option<ResidencyRequirement>>,
}

impl Default for ConfigRequirements {
    fn default() -> Self {
        Self {
            approval_policy: ConstrainedWithSource::new(
                Constrained::allow_any_from_default(),
                None,
            ),
            sandbox_policy: ConstrainedWithSource::new(
                Constrained::allow_any(SandboxPolicy::ReadOnly),
                None,
            ),
            mcp_servers: None,
            exec_policy: None,
            enforce_residency: ConstrainedWithSource::new(Constrained::allow_any(None), None),
        }
    }
}

impl ConfigRequirements {
    pub fn exec_policy_source(&self) -> Option<&RequirementSource> {
        self.exec_policy.as_ref().map(|policy| &policy.source)
    }
}

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(untagged)]
pub enum McpServerIdentity {
    Command { command: String },
    Url { url: String },
}

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct McpServerRequirement {
    pub identity: McpServerIdentity,
}

/// Base config deserialized from /etc/codex/requirements.toml or MDM.
#[derive(Deserialize, Debug, Clone, Default, PartialEq)]
pub struct ConfigRequirementsToml {
    pub allowed_approval_policies: Option<Vec<AskForApproval>>,
    pub allowed_sandbox_modes: Option<Vec<SandboxModeRequirement>>,
    pub mcp_servers: Option<BTreeMap<String, McpServerRequirement>>,
    pub rules: Option<RequirementsExecPolicyToml>,
    pub enforce_residency: Option<ResidencyRequirement>,
}

/// Value paired with the requirement source it came from, for better error
/// messages.
#[derive(Debug, Clone, PartialEq)]
pub struct Sourced<T> {
    pub value: T,
    pub source: RequirementSource,
}

impl<T> Sourced<T> {
    pub fn new(value: T, source: RequirementSource) -> Self {
        Self { value, source }
    }
}

impl<T> std::ops::Deref for Sourced<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ConfigRequirementsWithSources {
    pub allowed_approval_policies: Option<Sourced<Vec<AskForApproval>>>,
    pub allowed_sandbox_modes: Option<Sourced<Vec<SandboxModeRequirement>>>,
    pub mcp_servers: Option<Sourced<BTreeMap<String, McpServerRequirement>>>,
    pub rules: Option<Sourced<RequirementsExecPolicyToml>>,
    pub enforce_residency: Option<Sourced<ResidencyRequirement>>,
}

impl ConfigRequirementsWithSources {
    pub fn merge_unset_fields(&mut self, source: RequirementSource, other: ConfigRequirementsToml) {
        // For every field in `other` that is `Some`, if the corresponding field
        // in `self` is `None`, copy the value from `other` into `self`.
        macro_rules! fill_missing_take {
            ($base:expr, $other:expr, $source:expr, { $($field:ident),+ $(,)? }) => {
                // Destructure without `..` so adding fields to `ConfigRequirementsToml`
                // forces this merge logic to be updated.
                let ConfigRequirementsToml { $($field: _,)+ } = &$other;

                $(
                    if $base.$field.is_none()
                        && let Some(value) = $other.$field.take()
                    {
                        $base.$field = Some(Sourced::new(value, $source.clone()));
                    }
                )+
            };
        }

        let mut other = other;
        fill_missing_take!(
            self,
            other,
            source,
            {
                allowed_approval_policies,
                allowed_sandbox_modes,
                mcp_servers,
                rules,
                enforce_residency,
            }
        );
    }

    pub fn into_toml(self) -> ConfigRequirementsToml {
        let ConfigRequirementsWithSources {
            allowed_approval_policies,
            allowed_sandbox_modes,
            mcp_servers,
            rules,
            enforce_residency,
        } = self;
        ConfigRequirementsToml {
            allowed_approval_policies: allowed_approval_policies.map(|sourced| sourced.value),
            allowed_sandbox_modes: allowed_sandbox_modes.map(|sourced| sourced.value),
            mcp_servers: mcp_servers.map(|sourced| sourced.value),
            rules: rules.map(|sourced| sourced.value),
            enforce_residency: enforce_residency.map(|sourced| sourced.value),
        }
    }
}

/// Currently, `external-sandbox` is not supported in config.toml, but it is
/// supported through programmatic use.
#[derive(Deserialize, Debug, Clone, Copy, PartialEq)]
pub enum SandboxModeRequirement {
    #[serde(rename = "read-only")]
    ReadOnly,

    #[serde(rename = "workspace-write")]
    WorkspaceWrite,

    #[serde(rename = "danger-full-access")]
    DangerFullAccess,

    #[serde(rename = "external-sandbox")]
    ExternalSandbox,
}

impl From<SandboxMode> for SandboxModeRequirement {
    fn from(mode: SandboxMode) -> Self {
        match mode {
            SandboxMode::ReadOnly => SandboxModeRequirement::ReadOnly,
            SandboxMode::WorkspaceWrite => SandboxModeRequirement::WorkspaceWrite,
            SandboxMode::DangerFullAccess => SandboxModeRequirement::DangerFullAccess,
        }
    }
}

#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ResidencyRequirement {
    Us,
}

impl ConfigRequirementsToml {
    pub fn is_empty(&self) -> bool {
        self.allowed_approval_policies.is_none()
            && self.allowed_sandbox_modes.is_none()
            && self.mcp_servers.is_none()
            && self.rules.is_none()
            && self.enforce_residency.is_none()
    }
}

impl TryFrom<ConfigRequirementsWithSources> for ConfigRequirements {
    type Error = ConstraintError;

    fn try_from(toml: ConfigRequirementsWithSources) -> Result<Self, Self::Error> {
        let ConfigRequirementsWithSources {
            allowed_approval_policies,
            allowed_sandbox_modes,
            mcp_servers,
            rules,
            enforce_residency,
        } = toml;

        let approval_policy = match allowed_approval_policies {
            Some(Sourced {
                value: policies,
                source: requirement_source,
            }) => {
                let Some(initial_value) = policies.first().copied() else {
                    return Err(ConstraintError::empty_field("allowed_approval_policies"));
                };

                let requirement_source_for_error = requirement_source.clone();
                let constrained = Constrained::new(initial_value, move |candidate| {
                    if policies.contains(candidate) {
                        Ok(())
                    } else {
                        Err(ConstraintError::InvalidValue {
                            field_name: "approval_policy",
                            candidate: format!("{candidate:?}"),
                            allowed: format!("{policies:?}"),
                            requirement_source: requirement_source_for_error.clone(),
                        })
                    }
                })?;
                ConstrainedWithSource::new(constrained, Some(requirement_source))
            }
            None => ConstrainedWithSource::new(Constrained::allow_any_from_default(), None),
        };

        // TODO(gt): `ConfigRequirementsToml` should let the author specify the
        // default `SandboxPolicy`? Should do this for `AskForApproval` too?
        //
        // Currently, we force ReadOnly as the default policy because two of
        // the other variants (WorkspaceWrite, ExternalSandbox) require
        // additional parameters. Ultimately, we should expand the config
        // format to allow specifying those parameters.
        let default_sandbox_policy = SandboxPolicy::ReadOnly;
        let sandbox_policy = match allowed_sandbox_modes {
            Some(Sourced {
                value: modes,
                source: requirement_source,
            }) => {
                if !modes.contains(&SandboxModeRequirement::ReadOnly) {
                    return Err(ConstraintError::InvalidValue {
                        field_name: "allowed_sandbox_modes",
                        candidate: format!("{modes:?}"),
                        allowed: "must include 'read-only' to allow any SandboxPolicy".to_string(),
                        requirement_source,
                    });
                };

                let requirement_source_for_error = requirement_source.clone();
                let constrained = Constrained::new(default_sandbox_policy, move |candidate| {
                    let mode = match candidate {
                        SandboxPolicy::ReadOnly => SandboxModeRequirement::ReadOnly,
                        SandboxPolicy::WorkspaceWrite { .. } => {
                            SandboxModeRequirement::WorkspaceWrite
                        }
                        SandboxPolicy::DangerFullAccess => SandboxModeRequirement::DangerFullAccess,
                        SandboxPolicy::ExternalSandbox { .. } => {
                            SandboxModeRequirement::ExternalSandbox
                        }
                    };
                    if modes.contains(&mode) {
                        Ok(())
                    } else {
                        Err(ConstraintError::InvalidValue {
                            field_name: "sandbox_mode",
                            candidate: format!("{mode:?}"),
                            allowed: format!("{modes:?}"),
                            requirement_source: requirement_source_for_error.clone(),
                        })
                    }
                })?;
                ConstrainedWithSource::new(constrained, Some(requirement_source))
            }
            None => {
                ConstrainedWithSource::new(Constrained::allow_any(default_sandbox_policy), None)
            }
        };
        let exec_policy = match rules {
            Some(Sourced { value, source }) => {
                let policy = value.to_requirements_policy().map_err(|err| {
                    ConstraintError::ExecPolicyParse {
                        requirement_source: source.clone(),
                        reason: err.to_string(),
                    }
                })?;
                Some(Sourced::new(policy, source))
            }
            None => None,
        };

        let enforce_residency = match enforce_residency {
            Some(Sourced {
                value: residency,
                source: requirement_source,
            }) => {
                let required = Some(residency);
                let requirement_source_for_error = requirement_source.clone();
                let constrained = Constrained::new(required, move |candidate| {
                    if candidate == &required {
                        Ok(())
                    } else {
                        Err(ConstraintError::InvalidValue {
                            field_name: "enforce_residency",
                            candidate: format!("{candidate:?}"),
                            allowed: format!("{required:?}"),
                            requirement_source: requirement_source_for_error.clone(),
                        })
                    }
                })?;
                ConstrainedWithSource::new(constrained, Some(requirement_source))
            }
            None => ConstrainedWithSource::new(Constrained::allow_any(None), None),
        };
        Ok(ConfigRequirements {
            approval_policy,
            sandbox_policy,
            mcp_servers,
            exec_policy,
            enforce_residency,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use codex_execpolicy::Decision;
    use codex_execpolicy::Evaluation;
    use codex_execpolicy::RuleMatch;
    use codex_protocol::protocol::NetworkAccess;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use pretty_assertions::assert_eq;
    use toml::from_str;

    fn tokens(cmd: &[&str]) -> Vec<String> {
        cmd.iter().map(std::string::ToString::to_string).collect()
    }

    fn with_unknown_source(toml: ConfigRequirementsToml) -> ConfigRequirementsWithSources {
        let ConfigRequirementsToml {
            allowed_approval_policies,
            allowed_sandbox_modes,
            mcp_servers,
            rules,
            enforce_residency,
        } = toml;
        ConfigRequirementsWithSources {
            allowed_approval_policies: allowed_approval_policies
                .map(|value| Sourced::new(value, RequirementSource::Unknown)),
            allowed_sandbox_modes: allowed_sandbox_modes
                .map(|value| Sourced::new(value, RequirementSource::Unknown)),
            mcp_servers: mcp_servers.map(|value| Sourced::new(value, RequirementSource::Unknown)),
            rules: rules.map(|value| Sourced::new(value, RequirementSource::Unknown)),
            enforce_residency: enforce_residency
                .map(|value| Sourced::new(value, RequirementSource::Unknown)),
        }
    }

    #[test]
    fn merge_unset_fields_copies_every_field_and_sets_sources() {
        let mut target = ConfigRequirementsWithSources::default();
        let source = RequirementSource::LegacyManagedConfigTomlFromMdm;

        let allowed_approval_policies = vec![AskForApproval::UnlessTrusted, AskForApproval::Never];
        let allowed_sandbox_modes = vec![
            SandboxModeRequirement::WorkspaceWrite,
            SandboxModeRequirement::DangerFullAccess,
        ];
        let enforce_residency = ResidencyRequirement::Us;
        let enforce_source = source.clone();

        // Intentionally constructed without `..Default::default()` so adding a new field to
        // `ConfigRequirementsToml` forces this test to be updated.
        let other = ConfigRequirementsToml {
            allowed_approval_policies: Some(allowed_approval_policies.clone()),
            allowed_sandbox_modes: Some(allowed_sandbox_modes.clone()),
            mcp_servers: None,
            rules: None,
            enforce_residency: Some(enforce_residency),
        };

        target.merge_unset_fields(source.clone(), other);

        assert_eq!(
            target,
            ConfigRequirementsWithSources {
                allowed_approval_policies: Some(Sourced::new(
                    allowed_approval_policies,
                    source.clone()
                )),
                allowed_sandbox_modes: Some(Sourced::new(allowed_sandbox_modes, source)),
                mcp_servers: None,
                rules: None,
                enforce_residency: Some(Sourced::new(enforce_residency, enforce_source)),
            }
        );
    }

    #[test]
    fn merge_unset_fields_fills_missing_values() -> Result<()> {
        let source: ConfigRequirementsToml = from_str(
            r#"
                allowed_approval_policies = ["on-request"]
            "#,
        )?;

        let source_location = RequirementSource::MdmManagedPreferences {
            domain: "com.codex".to_string(),
            key: "allowed_approval_policies".to_string(),
        };

        let mut empty_target = ConfigRequirementsWithSources::default();
        empty_target.merge_unset_fields(source_location.clone(), source);
        assert_eq!(
            empty_target,
            ConfigRequirementsWithSources {
                allowed_approval_policies: Some(Sourced::new(
                    vec![AskForApproval::OnRequest],
                    source_location,
                )),
                allowed_sandbox_modes: None,
                mcp_servers: None,
                rules: None,
                enforce_residency: None,
            }
        );
        Ok(())
    }

    #[test]
    fn merge_unset_fields_does_not_overwrite_existing_values() -> Result<()> {
        let existing_source = RequirementSource::LegacyManagedConfigTomlFromMdm;
        let mut populated_target = ConfigRequirementsWithSources::default();
        let populated_requirements: ConfigRequirementsToml = from_str(
            r#"
                allowed_approval_policies = ["never"]
            "#,
        )?;
        populated_target.merge_unset_fields(existing_source.clone(), populated_requirements);

        let source: ConfigRequirementsToml = from_str(
            r#"
                allowed_approval_policies = ["on-request"]
            "#,
        )?;
        let source_location = RequirementSource::MdmManagedPreferences {
            domain: "com.codex".to_string(),
            key: "allowed_approval_policies".to_string(),
        };
        populated_target.merge_unset_fields(source_location, source);

        assert_eq!(
            populated_target,
            ConfigRequirementsWithSources {
                allowed_approval_policies: Some(Sourced::new(
                    vec![AskForApproval::Never],
                    existing_source,
                )),
                allowed_sandbox_modes: None,
                mcp_servers: None,
                rules: None,
                enforce_residency: None,
            }
        );
        Ok(())
    }

    #[test]
    fn constraint_error_includes_requirement_source() -> Result<()> {
        let source: ConfigRequirementsToml = from_str(
            r#"
                allowed_approval_policies = ["on-request"]
                allowed_sandbox_modes = ["read-only"]
            "#,
        )?;

        let requirements_toml_file = if cfg!(windows) {
            "C:\\etc\\codex\\requirements.toml"
        } else {
            "/etc/codex/requirements.toml"
        };
        let requirements_toml_file = AbsolutePathBuf::from_absolute_path(requirements_toml_file)?;
        let source_location = RequirementSource::SystemRequirementsToml {
            file: requirements_toml_file,
        };

        let mut target = ConfigRequirementsWithSources::default();
        target.merge_unset_fields(source_location.clone(), source);
        let requirements = ConfigRequirements::try_from(target)?;

        assert_eq!(
            requirements.approval_policy.can_set(&AskForApproval::Never),
            Err(ConstraintError::InvalidValue {
                field_name: "approval_policy",
                candidate: "Never".into(),
                allowed: "[OnRequest]".into(),
                requirement_source: source_location.clone(),
            })
        );
        assert_eq!(
            requirements
                .sandbox_policy
                .can_set(&SandboxPolicy::DangerFullAccess),
            Err(ConstraintError::InvalidValue {
                field_name: "sandbox_mode",
                candidate: "DangerFullAccess".into(),
                allowed: "[ReadOnly]".into(),
                requirement_source: source_location,
            })
        );

        Ok(())
    }

    #[test]
    fn constraint_error_includes_cloud_requirements_source() -> Result<()> {
        let source: ConfigRequirementsToml = from_str(
            r#"
                allowed_approval_policies = ["on-request"]
            "#,
        )?;

        let source_location = RequirementSource::CloudRequirements;

        let mut target = ConfigRequirementsWithSources::default();
        target.merge_unset_fields(source_location.clone(), source);
        let requirements = ConfigRequirements::try_from(target)?;

        assert_eq!(
            requirements.approval_policy.can_set(&AskForApproval::Never),
            Err(ConstraintError::InvalidValue {
                field_name: "approval_policy",
                candidate: "Never".into(),
                allowed: "[OnRequest]".into(),
                requirement_source: source_location,
            })
        );

        Ok(())
    }

    #[test]
    fn constrained_fields_store_requirement_source() -> Result<()> {
        let source: ConfigRequirementsToml = from_str(
            r#"
                allowed_approval_policies = ["on-request"]
                allowed_sandbox_modes = ["read-only"]
                enforce_residency = "us"
            "#,
        )?;

        let source_location = RequirementSource::CloudRequirements;
        let mut target = ConfigRequirementsWithSources::default();
        target.merge_unset_fields(source_location.clone(), source);
        let requirements = ConfigRequirements::try_from(target)?;

        assert_eq!(
            requirements.approval_policy.source,
            Some(source_location.clone())
        );
        assert_eq!(
            requirements.sandbox_policy.source,
            Some(source_location.clone())
        );
        assert_eq!(requirements.enforce_residency.source, Some(source_location));

        Ok(())
    }

    #[test]
    fn deserialize_allowed_approval_policies() -> Result<()> {
        let toml_str = r#"
            allowed_approval_policies = ["untrusted", "on-request"]
        "#;
        let config: ConfigRequirementsToml = from_str(toml_str)?;
        let requirements: ConfigRequirements = with_unknown_source(config).try_into()?;

        assert_eq!(
            requirements.approval_policy.value(),
            AskForApproval::UnlessTrusted,
            "currently, there is no way to specify the default value for approval policy in the toml, so it picks the first allowed value"
        );
        assert!(
            requirements
                .approval_policy
                .can_set(&AskForApproval::UnlessTrusted)
                .is_ok()
        );
        assert_eq!(
            requirements
                .approval_policy
                .can_set(&AskForApproval::OnFailure),
            Err(ConstraintError::InvalidValue {
                field_name: "approval_policy",
                candidate: "OnFailure".into(),
                allowed: "[UnlessTrusted, OnRequest]".into(),
                requirement_source: RequirementSource::Unknown,
            })
        );
        assert!(
            requirements
                .approval_policy
                .can_set(&AskForApproval::OnRequest)
                .is_ok()
        );
        assert_eq!(
            requirements.approval_policy.can_set(&AskForApproval::Never),
            Err(ConstraintError::InvalidValue {
                field_name: "approval_policy",
                candidate: "Never".into(),
                allowed: "[UnlessTrusted, OnRequest]".into(),
                requirement_source: RequirementSource::Unknown,
            })
        );
        assert!(
            requirements
                .sandbox_policy
                .can_set(&SandboxPolicy::ReadOnly)
                .is_ok()
        );

        Ok(())
    }

    #[test]
    fn deserialize_allowed_sandbox_modes() -> Result<()> {
        let toml_str = r#"
            allowed_sandbox_modes = ["read-only", "workspace-write"]
        "#;
        let config: ConfigRequirementsToml = from_str(toml_str)?;
        let requirements: ConfigRequirements = with_unknown_source(config).try_into()?;

        let root = if cfg!(windows) { "C:\\repo" } else { "/repo" };
        assert!(
            requirements
                .sandbox_policy
                .can_set(&SandboxPolicy::ReadOnly)
                .is_ok()
        );
        assert!(
            requirements
                .sandbox_policy
                .can_set(&SandboxPolicy::WorkspaceWrite {
                    writable_roots: vec![AbsolutePathBuf::from_absolute_path(root)?],
                    network_access: false,
                    exclude_tmpdir_env_var: false,
                    exclude_slash_tmp: false,
                })
                .is_ok()
        );
        assert_eq!(
            requirements
                .sandbox_policy
                .can_set(&SandboxPolicy::DangerFullAccess),
            Err(ConstraintError::InvalidValue {
                field_name: "sandbox_mode",
                candidate: "DangerFullAccess".into(),
                allowed: "[ReadOnly, WorkspaceWrite]".into(),
                requirement_source: RequirementSource::Unknown,
            })
        );
        assert_eq!(
            requirements
                .sandbox_policy
                .can_set(&SandboxPolicy::ExternalSandbox {
                    network_access: NetworkAccess::Restricted,
                }),
            Err(ConstraintError::InvalidValue {
                field_name: "sandbox_mode",
                candidate: "ExternalSandbox".into(),
                allowed: "[ReadOnly, WorkspaceWrite]".into(),
                requirement_source: RequirementSource::Unknown,
            })
        );

        Ok(())
    }

    #[test]
    fn deserialize_mcp_server_requirements() -> Result<()> {
        let toml_str = r#"
            [mcp_servers.docs.identity]
            command = "codex-mcp"

            [mcp_servers.remote.identity]
            url = "https://example.com/mcp"
        "#;
        let requirements: ConfigRequirements =
            with_unknown_source(from_str(toml_str)?).try_into()?;

        assert_eq!(
            requirements.mcp_servers,
            Some(Sourced::new(
                BTreeMap::from([
                    (
                        "docs".to_string(),
                        McpServerRequirement {
                            identity: McpServerIdentity::Command {
                                command: "codex-mcp".to_string(),
                            },
                        },
                    ),
                    (
                        "remote".to_string(),
                        McpServerRequirement {
                            identity: McpServerIdentity::Url {
                                url: "https://example.com/mcp".to_string(),
                            },
                        },
                    ),
                ]),
                RequirementSource::Unknown,
            ))
        );
        Ok(())
    }

    #[test]
    fn deserialize_exec_policy_requirements() -> Result<()> {
        let toml_str = r#"
            [rules]
            prefix_rules = [
                { pattern = [{ token = "rm" }], decision = "forbidden" },
            ]
        "#;
        let config: ConfigRequirementsToml = from_str(toml_str)?;
        let requirements: ConfigRequirements = with_unknown_source(config).try_into()?;
        let policy = requirements.exec_policy.expect("exec policy").value;

        assert_eq!(
            policy.as_ref().check(&tokens(&["rm", "-rf"]), &|_| {
                panic!("rule should match so heuristic should not be called");
            }),
            Evaluation {
                decision: Decision::Forbidden,
                matched_rules: vec![RuleMatch::PrefixRuleMatch {
                    matched_prefix: tokens(&["rm"]),
                    decision: Decision::Forbidden,
                    justification: None,
                }],
            }
        );

        Ok(())
    }

    #[test]
    fn exec_policy_error_includes_requirement_source() -> Result<()> {
        let toml_str = r#"
            [rules]
            prefix_rules = [
                { pattern = [{ token = "rm" }] },
            ]
        "#;
        let config: ConfigRequirementsToml = from_str(toml_str)?;
        let requirements_toml_file =
            AbsolutePathBuf::from_absolute_path("/etc/codex/requirements.toml")?;
        let source_location = RequirementSource::SystemRequirementsToml {
            file: requirements_toml_file,
        };

        let mut requirements_with_sources = ConfigRequirementsWithSources::default();
        requirements_with_sources.merge_unset_fields(source_location.clone(), config);
        let err = ConfigRequirements::try_from(requirements_with_sources)
            .expect_err("invalid exec policy");

        assert_eq!(
            err,
            ConstraintError::ExecPolicyParse {
                requirement_source: source_location,
                reason: "rules prefix_rule at index 0 is missing a decision".to_string(),
            }
        );

        Ok(())
    }
}
