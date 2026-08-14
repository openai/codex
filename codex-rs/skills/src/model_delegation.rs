use serde::Deserialize;

/// Model requested for work governed by a skill.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SkillModel {
    Luna,
}

#[cfg(test)]
#[path = "model_delegation_tests.rs"]
mod tests;
