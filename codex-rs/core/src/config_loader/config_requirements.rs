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
