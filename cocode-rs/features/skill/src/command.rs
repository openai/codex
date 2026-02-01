//! Skill command types.
//!
//! Defines the prompt-based skill commands and slash commands that users
//! can invoke. Each skill is represented as a [`SkillPromptCommand`] with
//! associated metadata and prompt content.

use serde::Deserialize;
use serde::Serialize;
use std::fmt;

/// A skill that injects a prompt into the conversation.
///
/// This is the primary representation of a loaded skill. The prompt text
/// is either read from a file (referenced in `SKILL.toml`) or specified
/// inline in the TOML metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillPromptCommand {
    /// Unique skill name (used as the slash command identifier).
    pub name: String,

    /// Human-readable description shown in help/completion.
    pub description: String,

    /// Prompt text injected when the skill is invoked.
    pub prompt: String,

    /// Optional list of tools the skill is allowed to use.
    #[serde(default)]
    pub allowed_tools: Option<Vec<String>>,

    /// Optional interface with hook definitions.
    /// Populated from SKILL.toml when hooks are defined.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interface: Option<crate::interface::SkillInterface>,
}

impl fmt::Display for SkillPromptCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "/{} - {}", self.name, self.description)
    }
}

/// A slash command that can be invoked by the user.
///
/// Slash commands include both skill-based commands and system/plugin commands.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlashCommand {
    /// Command name (without leading slash).
    pub name: String,

    /// Human-readable description.
    pub description: String,

    /// The type of command.
    pub command_type: CommandType,
}

impl fmt::Display for SlashCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = match self.command_type {
            CommandType::Skill => "skill",
            CommandType::System => "system",
            CommandType::Plugin => "plugin",
        };
        write!(f, "/{} [{}] - {}", self.name, kind, self.description)
    }
}

/// The type of a slash command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommandType {
    /// A user-defined skill loaded from SKILL.toml.
    Skill,

    /// A built-in system command (e.g., /help, /clear).
    System,

    /// A plugin-provided command.
    Plugin,
}

impl fmt::Display for CommandType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Skill => write!(f, "skill"),
            Self::System => write!(f, "system"),
            Self::Plugin => write!(f, "plugin"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skill_prompt_command_display() {
        let cmd = SkillPromptCommand {
            name: "commit".to_string(),
            description: "Generate a commit message".to_string(),
            prompt: "Analyze the diff...".to_string(),
            allowed_tools: None,
            interface: None,
        };
        assert_eq!(cmd.to_string(), "/commit - Generate a commit message");
    }

    #[test]
    fn test_slash_command_display() {
        let cmd = SlashCommand {
            name: "review".to_string(),
            description: "Review code changes".to_string(),
            command_type: CommandType::Skill,
        };
        assert_eq!(cmd.to_string(), "/review [skill] - Review code changes");
    }

    #[test]
    fn test_command_type_display() {
        assert_eq!(CommandType::Skill.to_string(), "skill");
        assert_eq!(CommandType::System.to_string(), "system");
        assert_eq!(CommandType::Plugin.to_string(), "plugin");
    }

    #[test]
    fn test_skill_prompt_command_serialize_roundtrip() {
        let cmd = SkillPromptCommand {
            name: "test".to_string(),
            description: "A test skill".to_string(),
            prompt: "Do something".to_string(),
            allowed_tools: Some(vec!["read".to_string(), "write".to_string()]),
            interface: None,
        };
        let json = serde_json::to_string(&cmd).expect("serialize");
        let deserialized: SkillPromptCommand = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deserialized.name, "test");
        assert_eq!(
            deserialized.allowed_tools,
            Some(vec!["read".to_string(), "write".to_string()])
        );
    }

    #[test]
    fn test_skill_prompt_command_deserialize_no_tools() {
        let json = r#"{"name":"x","description":"d","prompt":"p"}"#;
        let cmd: SkillPromptCommand = serde_json::from_str(json).expect("deserialize");
        assert_eq!(cmd.name, "x");
        assert!(cmd.allowed_tools.is_none());
    }
}
