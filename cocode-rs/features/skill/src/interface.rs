//! Skill interface definition (`SKILL.toml` schema).
//!
//! Each skill directory contains a `SKILL.toml` file that describes the
//! skill's metadata and prompt content. This module defines the
//! deserialization target for that file.

use serde::{Deserialize, Serialize};

/// Metadata and content of a skill, as defined in `SKILL.toml`.
///
/// A skill must have a `name` and `description`. The prompt content can be
/// provided either inline (`prompt_inline`) or by referencing an external
/// file (`prompt_file`). If both are specified, `prompt_file` takes
/// precedence.
///
/// # Example SKILL.toml
///
/// ```toml
/// name = "commit"
/// description = "Generate a commit message from staged changes"
/// prompt_file = "prompt.md"
/// allowed_tools = ["Bash", "Read"]
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillInterface {
    /// Unique skill name (used as slash-command identifier).
    pub name: String,

    /// Human-readable description.
    pub description: String,

    /// Path to an external file containing the prompt text.
    /// Relative to the skill directory.
    #[serde(default)]
    pub prompt_file: Option<String>,

    /// Inline prompt text (used when `prompt_file` is not set).
    #[serde(default)]
    pub prompt_inline: Option<String>,

    /// Tools the skill is allowed to invoke.
    #[serde(default)]
    pub allowed_tools: Option<Vec<String>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_full() {
        let toml_str = r#"
name = "commit"
description = "Generate a commit message"
prompt_file = "prompt.md"
allowed_tools = ["Bash", "Read"]
"#;
        let iface: SkillInterface = toml::from_str(toml_str).expect("parse SKILL.toml");
        assert_eq!(iface.name, "commit");
        assert_eq!(iface.description, "Generate a commit message");
        assert_eq!(iface.prompt_file, Some("prompt.md".to_string()));
        assert!(iface.prompt_inline.is_none());
        assert_eq!(
            iface.allowed_tools,
            Some(vec!["Bash".to_string(), "Read".to_string()])
        );
    }

    #[test]
    fn test_deserialize_inline_prompt() {
        let toml_str = r#"
name = "review"
description = "Review code"
prompt_inline = "Please review the following code changes."
"#;
        let iface: SkillInterface = toml::from_str(toml_str).expect("parse SKILL.toml");
        assert_eq!(iface.name, "review");
        assert_eq!(
            iface.prompt_inline,
            Some("Please review the following code changes.".to_string())
        );
        assert!(iface.prompt_file.is_none());
        assert!(iface.allowed_tools.is_none());
    }

    #[test]
    fn test_deserialize_minimal() {
        let toml_str = r#"
name = "test"
description = "A test skill"
"#;
        let iface: SkillInterface = toml::from_str(toml_str).expect("parse SKILL.toml");
        assert_eq!(iface.name, "test");
        assert!(iface.prompt_file.is_none());
        assert!(iface.prompt_inline.is_none());
        assert!(iface.allowed_tools.is_none());
    }

    #[test]
    fn test_serialize_roundtrip() {
        let iface = SkillInterface {
            name: "roundtrip".to_string(),
            description: "Roundtrip test".to_string(),
            prompt_file: None,
            prompt_inline: Some("Do things".to_string()),
            allowed_tools: Some(vec!["Bash".to_string()]),
        };
        let serialized = toml::to_string(&iface).expect("serialize");
        let deserialized: SkillInterface = toml::from_str(&serialized).expect("deserialize");
        assert_eq!(deserialized.name, "roundtrip");
        assert_eq!(deserialized.prompt_inline, Some("Do things".to_string()));
        assert_eq!(deserialized.allowed_tools, Some(vec!["Bash".to_string()]));
    }
}
