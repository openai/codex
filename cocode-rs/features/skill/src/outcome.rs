//! Skill loading outcome types.
//!
//! Loading skills uses a fail-open strategy: if one skill fails to load
//! (e.g., malformed TOML, missing prompt file), it is reported as a
//! [`SkillLoadOutcome::Failed`] but does not prevent other skills from
//! being loaded successfully.

use crate::command::SkillPromptCommand;
use crate::source::SkillSource;
use std::path::PathBuf;

/// The result of attempting to load a single skill.
///
/// This enum captures both success and failure cases to support the
/// fail-open loading strategy.
#[derive(Debug, Clone)]
pub enum SkillLoadOutcome {
    /// The skill was loaded and validated successfully.
    Success {
        /// The loaded skill command.
        skill: SkillPromptCommand,

        /// Where the skill was loaded from.
        source: SkillSource,
    },

    /// The skill failed to load.
    Failed {
        /// Path to the skill directory that failed.
        path: PathBuf,

        /// Human-readable error description.
        error: String,
    },
}

impl SkillLoadOutcome {
    /// Returns `true` if this outcome is a success.
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Success { .. })
    }

    /// Returns the skill name if this outcome is a success.
    pub fn skill_name(&self) -> Option<&str> {
        match self {
            Self::Success { skill, .. } => Some(&skill.name),
            Self::Failed { .. } => None,
        }
    }

    /// Converts a successful outcome into the skill command, or `None`.
    pub fn into_skill(self) -> Option<SkillPromptCommand> {
        match self {
            Self::Success { skill, .. } => Some(skill),
            Self::Failed { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_success() -> SkillLoadOutcome {
        SkillLoadOutcome::Success {
            skill: SkillPromptCommand {
                name: "test".to_string(),
                description: "A test skill".to_string(),
                prompt: "Do something".to_string(),
                allowed_tools: None,
                interface: None,
            },
            source: SkillSource::Bundled,
        }
    }

    fn make_failed() -> SkillLoadOutcome {
        SkillLoadOutcome::Failed {
            path: PathBuf::from("/bad/skill"),
            error: "parse error".to_string(),
        }
    }

    #[test]
    fn test_is_success() {
        assert!(make_success().is_success());
        assert!(!make_failed().is_success());
    }

    #[test]
    fn test_skill_name() {
        assert_eq!(make_success().skill_name(), Some("test"));
        assert_eq!(make_failed().skill_name(), None);
    }

    #[test]
    fn test_into_skill() {
        let skill = make_success().into_skill();
        assert!(skill.is_some());
        assert_eq!(skill.as_ref().map(|s| s.name.as_str()), Some("test"));

        let skill = make_failed().into_skill();
        assert!(skill.is_none());
    }
}
