use codex_protocol::config_types::SandboxMode;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::SandboxPolicy;
use codex_utils_absolute_path::AbsolutePathBuf;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fmt;

use crate::config::Constrained;
use crate::config::ConstraintError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RequirementSource {
    Unknown,
    MdmManagedPreferences { domain: String, key: String },
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

/// Normalized version of [`ConfigRequirementsToml`] after deserialization and
/// normalization.
#[derive(Debug, Clone, PartialEq)]
pub struct ConfigRequirements {
    pub approval_policy: Constrained<AskForApproval>,
    pub sandbox_policy: Constrained<SandboxPolicy>,
    pub mcp_servers: Option<BTreeMap<String, McpServerRequirement>>,
}

impl Default for ConfigRequirements {
    fn default() -> Self {
        Self {
            approval_policy: Constrained::allow_any_from_default(),
            sandbox_policy: Constrained::allow_any(SandboxPolicy::ReadOnly),
            mcp_servers: None,
        }
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
            }
        );
    }

    pub fn into_toml(self) -> ConfigRequirementsToml {
        let ConfigRequirementsWithSources {
            allowed_approval_policies,
            allowed_sandbox_modes,
            mcp_servers,
        } = self;
        ConfigRequirementsToml {
            allowed_approval_policies: allowed_approval_policies.map(|sourced| sourced.value),
            allowed_sandbox_modes: allowed_sandbox_modes.map(|sourced| sourced.value),
            mcp_servers: mcp_servers.map(|sourced| sourced.value),
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

impl ConfigRequirementsToml {
    pub fn is_empty(&self) -> bool {
        self.allowed_approval_policies.is_none()
            && self.allowed_sandbox_modes.is_none()
            && self.mcp_servers.is_none()
    }
}

impl TryFrom<ConfigRequirementsWithSources> for ConfigRequirements {
    type Error = ConstraintError;

    fn try_from(toml: ConfigRequirementsWithSources) -> Result<Self, Self::Error> {
        let ConfigRequirementsWithSources {
            allowed_approval_policies,
            allowed_sandbox_modes,
            mcp_servers,
        } = toml;

        let approval_policy: Constrained<AskForApproval> = match allowed_approval_policies {
            Some(Sourced {
                value: policies,
                source: requirement_source,
            }) => {
                let Some(initial_value) = policies.first().copied() else {
                    return Err(ConstraintError::empty_field("allowed_approval_policies"));
                };

                Constrained::new(initial_value, move |candidate| {
                    if policies.contains(candidate) {
                        Ok(())
                    } else {
                        Err(ConstraintError::InvalidValue {
                            field_name: "approval_policy",
                            candidate: format!("{candidate:?}"),
                            allowed: format!("{policies:?}"),
                            requirement_source: requirement_source.clone(),
                        })
                    }
                })?
            }
            None => Constrained::allow_any_from_default(),
        };

        // TODO(gt): `ConfigRequirementsToml` should let the author specify the
        // default `SandboxPolicy`? Should do this for `AskForApproval` too?
        //
        // Currently, we force ReadOnly as the default policy because two of
        // the other variants (WorkspaceWrite, ExternalSandbox) require
        // additional parameters. Ultimately, we should expand the config
        // format to allow specifying those parameters.
        let default_sandbox_policy = SandboxPolicy::ReadOnly;
        let sandbox_policy: Constrained<SandboxPolicy> = match allowed_sandbox_modes {
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

                Constrained::new(default_sandbox_policy, move |candidate| {
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
                            requirement_source: requirement_source.clone(),
                        })
                    }
                })?
            }
            None => Constrained::allow_any(default_sandbox_policy),
        };
        Ok(ConfigRequirements {
            approval_policy,
            sandbox_policy,
            mcp_servers: mcp_servers.map(|sourced| sourced.value),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use codex_protocol::protocol::NetworkAccess;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use pretty_assertions::assert_eq;
    use toml::from_str;

    fn with_unknown_source(toml: ConfigRequirementsToml) -> ConfigRequirementsWithSources {
        let ConfigRequirementsToml {
            allowed_approval_policies,
            allowed_sandbox_modes,
            mcp_servers,
        } = toml;
        ConfigRequirementsWithSources {
            allowed_approval_policies: allowed_approval_policies
                .map(|value| Sourced::new(value, RequirementSource::Unknown)),
            allowed_sandbox_modes: allowed_sandbox_modes
                .map(|value| Sourced::new(value, RequirementSource::Unknown)),
            mcp_servers: mcp_servers.map(|value| Sourced::new(value, RequirementSource::Unknown)),
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

        // Intentionally constructed without `..Default::default()` so adding a new field to
        // `ConfigRequirementsToml` forces this test to be updated.
        let other = ConfigRequirementsToml {
            allowed_approval_policies: Some(allowed_approval_policies.clone()),
            allowed_sandbox_modes: Some(allowed_sandbox_modes.clone()),
            mcp_servers: None,
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
            Some(BTreeMap::from([
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
            ]))
        );
        Ok(())
    }
}
