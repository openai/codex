use codex_protocol::config_types::SandboxMode;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::SandboxPolicy;
use serde::Deserialize;

use crate::config::Constrained;
use crate::config::ConstraintError;

/// Normalized version of [`ConfigRequirementsToml`] after deserialization and
/// normalization.
#[derive(Debug, Clone, PartialEq)]
pub struct ConfigRequirements {
    pub approval_policy: Constrained<AskForApproval>,
    pub sandbox_policy: Constrained<SandboxPolicy>,
}

impl Default for ConfigRequirements {
    fn default() -> Self {
        Self {
            approval_policy: Constrained::allow_any_from_default(),
            sandbox_policy: Constrained::allow_any(SandboxPolicy::ReadOnly),
        }
    }
}

/// Base config deserialized from /etc/codex/requirements.toml or MDM.
#[derive(Deserialize, Debug, Clone, Default, PartialEq)]
pub struct ConfigRequirementsToml {
    pub allowed_approval_policies: Option<Vec<AskForApproval>>,
    pub allowed_sandbox_modes: Option<Vec<SandboxModeRequirement>>,
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
    /// For every field in `other` that is `Some`, if the corresponding field in
    /// `self` is `None`, copy the value from `other` into `self`.
    pub fn merge_unset_fields(&mut self, mut other: ConfigRequirementsToml) {
        macro_rules! fill_missing_take {
            ($base:expr, $other:expr, { $($field:ident),+ $(,)? }) => {
                $(
                    if $base.$field.is_none() {
                        if let Some(value) = $other.$field.take() {
                            $base.$field = Some(value);
                        }
                    }
                )+
            };
        }

        fill_missing_take!(self, other, { allowed_approval_policies, allowed_sandbox_modes });
    }
}

impl TryFrom<ConfigRequirementsToml> for ConfigRequirements {
    type Error = ConstraintError;

    fn try_from(toml: ConfigRequirementsToml) -> Result<Self, Self::Error> {
        let ConfigRequirementsToml {
            allowed_approval_policies,
            allowed_sandbox_modes,
        } = toml;
        let approval_policy: Constrained<AskForApproval> = match allowed_approval_policies {
            Some(policies) => {
                if let Some(first) = policies.first() {
                    Constrained::allow_values(*first, policies)?
                } else {
                    return Err(ConstraintError::empty_field("allowed_approval_policies"));
                }
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
            Some(modes) => {
                if !modes.contains(&SandboxModeRequirement::ReadOnly) {
                    return Err(ConstraintError::invalid_value(
                        "allowed_sandbox_modes",
                        "must include 'read-only' to allow any SandboxPolicy",
                    ));
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
                        Err(ConstraintError::invalid_value(
                            format!("{candidate:?}"),
                            format!("{modes:?}"),
                        ))
                    }
                })?
            }
            None => Constrained::allow_any(default_sandbox_policy),
        };
        Ok(ConfigRequirements {
            approval_policy,
            sandbox_policy,
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

    #[test]
    fn merge_unset_fields_only_fills_missing_values() -> Result<()> {
        let source: ConfigRequirementsToml = from_str(
            r#"
                allowed_approval_policies = ["on-request"]
            "#,
        )?;

        let mut empty_target: ConfigRequirementsToml = from_str(
            r#"
                # intentionally left unset
            "#,
        )?;
        empty_target.merge_unset_fields(source.clone());
        assert_eq!(
            empty_target.allowed_approval_policies,
            Some(vec![AskForApproval::OnRequest])
        );

        let mut populated_target: ConfigRequirementsToml = from_str(
            r#"
                allowed_approval_policies = ["never"]
            "#,
        )?;
        populated_target.merge_unset_fields(source);
        assert_eq!(
            populated_target.allowed_approval_policies,
            Some(vec![AskForApproval::Never])
        );
        Ok(())
    }

    #[test]
    fn deserialize_allowed_approval_policies() -> Result<()> {
        let toml_str = r#"
            allowed_approval_policies = ["untrusted", "on-request"]
        "#;
        let config: ConfigRequirementsToml = from_str(toml_str)?;
        let requirements = ConfigRequirements::try_from(config)?;

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
                candidate: "OnFailure".into(),
                allowed: "[UnlessTrusted, OnRequest]".into(),
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
                candidate: "Never".into(),
                allowed: "[UnlessTrusted, OnRequest]".into(),
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
        let requirements = ConfigRequirements::try_from(config)?;

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
                candidate: "DangerFullAccess".into(),
                allowed: "[ReadOnly, WorkspaceWrite]".into(),
            })
        );
        assert_eq!(
            requirements
                .sandbox_policy
                .can_set(&SandboxPolicy::ExternalSandbox {
                    network_access: NetworkAccess::Restricted,
                }),
            Err(ConstraintError::InvalidValue {
                candidate: "ExternalSandbox { network_access: Restricted }".into(),
                allowed: "[ReadOnly, WorkspaceWrite]".into(),
            })
        );

        Ok(())
    }
}
