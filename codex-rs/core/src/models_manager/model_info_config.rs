//! Configurable model info definitions that can be loaded from TOML.
//!
//! This module allows users to define custom model info in `~/.codex/model_info.toml`
//! instead of relying solely on code-defined model info.

use codex_protocol::openai_models::ReasoningEffort;
use serde::Deserialize;
use serde::Serialize;
use std::path::Path;

/// Configurable model info definition.
///
/// Users can define custom model info in `~/.codex/model_info.toml`:
///
/// ```toml
/// [deepseek-r1]
/// display_name = "DeepSeek R1"
/// context_window = 64000
/// supports_reasoning_summaries = true
/// default_reasoning_effort = "high"
/// base_instructions = "You are DeepSeek R1..."
///
/// [qwen-coder]
/// context_window = 131072
/// base_instructions_file = "instructions/qwen-coder.md"
/// ```
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Default)]
pub struct ModelInfoConfig {
    /// Display name for the model.
    #[serde(default)]
    pub display_name: Option<String>,

    /// Context window size in tokens.
    #[serde(default)]
    pub context_window: Option<i64>,

    /// Token threshold for auto-compaction of conversation history.
    #[serde(default)]
    pub auto_compact_token_limit: Option<i64>,

    /// Whether this model supports reasoning summaries.
    #[serde(default)]
    pub supports_reasoning_summaries: bool,

    /// Default reasoning effort for this model.
    #[serde(default)]
    pub default_reasoning_effort: Option<ReasoningEffort>,

    /// Whether this model supports parallel tool calls.
    #[serde(default)]
    pub supports_parallel_tool_calls: bool,

    /// Base system instructions (inline).
    #[serde(default)]
    pub base_instructions: Option<String>,

    /// Path to base system instructions file (relative to ~/.codex/).
    /// Takes precedence over `base_instructions` if both are set.
    #[serde(default)]
    pub base_instructions_file: Option<String>,
}

impl ModelInfoConfig {
    /// Resolve base_instructions from inline string or file.
    ///
    /// Priority: inline `base_instructions` > `base_instructions_file`
    pub fn resolve_base_instructions(&self, codex_home: &Path) -> Option<String> {
        // Prefer inline instructions
        if let Some(instructions) = &self.base_instructions {
            let trimmed = instructions.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }

        // Fall back to file
        if let Some(file_path) = &self.base_instructions_file {
            let full_path = codex_home.join(file_path);
            match std::fs::read_to_string(&full_path) {
                Ok(content) => {
                    let trimmed = content.trim();
                    if !trimmed.is_empty() {
                        return Some(trimmed.to_string());
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to read base_instructions_file '{}': {}",
                        full_path.display(),
                        e
                    );
                }
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_inline_instructions_take_precedence() {
        let config = ModelInfoConfig {
            base_instructions: Some("Inline instructions".to_string()),
            base_instructions_file: Some("nonexistent.md".to_string()),
            ..Default::default()
        };

        let codex_home = tempdir().unwrap();
        let result = config.resolve_base_instructions(codex_home.path());

        assert_eq!(result, Some("Inline instructions".to_string()));
    }

    #[test]
    fn test_file_instructions_fallback() {
        let codex_home = tempdir().unwrap();
        let instructions_dir = codex_home.path().join("instructions");
        std::fs::create_dir_all(&instructions_dir).unwrap();
        std::fs::write(instructions_dir.join("test.md"), "  File instructions  \n").unwrap();

        let config = ModelInfoConfig {
            base_instructions: None,
            base_instructions_file: Some("instructions/test.md".to_string()),
            ..Default::default()
        };

        let result = config.resolve_base_instructions(codex_home.path());

        assert_eq!(result, Some("File instructions".to_string()));
    }

    #[test]
    fn test_empty_inline_falls_back_to_file() {
        let codex_home = tempdir().unwrap();
        std::fs::write(codex_home.path().join("test.md"), "File content").unwrap();

        let config = ModelInfoConfig {
            base_instructions: Some("   ".to_string()), // whitespace only
            base_instructions_file: Some("test.md".to_string()),
            ..Default::default()
        };

        let result = config.resolve_base_instructions(codex_home.path());

        assert_eq!(result, Some("File content".to_string()));
    }

    #[test]
    fn test_deserialize_from_toml() {
        let toml_str = r#"
            display_name = "DeepSeek R1"
            context_window = 64000
            supports_reasoning_summaries = true
            default_reasoning_effort = "high"
            base_instructions = "You are DeepSeek R1"
        "#;

        let config: ModelInfoConfig = toml::from_str(toml_str).unwrap();

        assert_eq!(config.display_name, Some("DeepSeek R1".to_string()));
        assert_eq!(config.context_window, Some(64000));
        assert!(config.supports_reasoning_summaries);
        assert_eq!(config.default_reasoning_effort, Some(ReasoningEffort::High));
        assert_eq!(
            config.base_instructions,
            Some("You are DeepSeek R1".to_string())
        );
    }
}
