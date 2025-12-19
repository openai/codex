use codex_protocol::protocol::AskForApproval;
use serde::Deserialize;

use crate::config::Constrained;
use crate::config::ConstraintError;

/// Normalized version of [`ConfigRequirementsToml`] after deserialization and
/// normalization.
#[derive(Debug, Clone, PartialEq)]
pub struct ConfigRequirements {
    pub approval_policy: Constrained<AskForApproval>,
}

impl Default for ConfigRequirements {
    fn default() -> Self {
        Self {
            approval_policy: Constrained::allow_any_from_default(),
        }
    }
}

/// Base config deserialized from /etc/codex/requirements.toml or MDM.
#[derive(Deserialize, Debug, Clone, Default, PartialEq)]
pub struct ConfigRequirementsToml {
    pub allowed_approval_policies: Option<Vec<AskForApproval>>,
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

        fill_missing_take!(self, other, { allowed_approval_policies });
    }
}

impl TryFrom<ConfigRequirementsToml> for ConfigRequirements {
    type Error = ConstraintError;

    fn try_from(toml: ConfigRequirementsToml) -> Result<Self, Self::Error> {
        let approval_policy: Constrained<AskForApproval> = match toml.allowed_approval_policies {
            Some(policies) => {
                let default_value = AskForApproval::default();
                if policies.contains(&default_value) {
                    Constrained::allow_values(default_value, policies)?
                } else if let Some(first) = policies.first() {
                    Constrained::allow_values(*first, policies)?
                } else {
                    return Err(ConstraintError::empty_field("allowed_approval_policies"));
                }
            }
            None => Constrained::allow_any_from_default(),
        };
        Ok(ConfigRequirements { approval_policy })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
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
}
