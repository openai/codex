//! Output style configuration types.
//!
//! Defines the output style system for customizing how Codex communicates.
//! Matches Claude Code v2.0.59 output style system.

use serde::Deserialize;
use serde::Serialize;

/// Output styles directory name.
pub const OUTPUT_STYLES_DIR: &str = "output-styles";

/// Default style name constant.
pub const DEFAULT_STYLE_NAME: &str = "default";

/// Source of an output style definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputStyleSource {
    /// Built-in style (Default, Explanatory, Learning).
    BuiltIn,
    /// User settings (~/.codex/output-styles/).
    UserSettings,
    /// Project settings (.codex/output-styles/).
    ProjectSettings,
}

/// An output style definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputStyle {
    /// Unique style name (case-insensitive matching).
    pub name: String,
    /// User-facing description for menus.
    pub description: String,
    /// The prompt to inject (None for "default" style).
    pub prompt: Option<String>,
    /// Whether to preserve default coding tool instructions.
    pub keep_coding_instructions: bool,
    /// Source of this style definition.
    pub source: OutputStyleSource,
}

impl OutputStyle {
    /// Create a new output style.
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        prompt: Option<String>,
        keep_coding_instructions: bool,
        source: OutputStyleSource,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            prompt,
            keep_coding_instructions,
            source,
        }
    }

    /// Check if this is the default (no-op) style.
    pub fn is_default(&self) -> bool {
        self.name.to_lowercase() == DEFAULT_STYLE_NAME
    }
}

// ============================================
// Tests
// ============================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_output_style_new() {
        let style = OutputStyle::new(
            "Test",
            "Test description",
            Some("Test prompt".to_string()),
            true,
            OutputStyleSource::BuiltIn,
        );

        assert_eq!(style.name, "Test");
        assert_eq!(style.description, "Test description");
        assert_eq!(style.prompt, Some("Test prompt".to_string()));
        assert!(style.keep_coding_instructions);
        assert_eq!(style.source, OutputStyleSource::BuiltIn);
    }

    #[test]
    fn test_is_default() {
        let default_style = OutputStyle::new(
            "default",
            "Default style",
            None,
            false,
            OutputStyleSource::BuiltIn,
        );
        assert!(default_style.is_default());

        let default_upper = OutputStyle::new(
            "Default",
            "Default style",
            None,
            false,
            OutputStyleSource::BuiltIn,
        );
        assert!(default_upper.is_default());

        let explanatory = OutputStyle::new(
            "Explanatory",
            "Explanatory style",
            Some("prompt".to_string()),
            true,
            OutputStyleSource::BuiltIn,
        );
        assert!(!explanatory.is_default());
    }

    #[test]
    fn test_output_style_source_serde() {
        let sources = vec![
            OutputStyleSource::BuiltIn,
            OutputStyleSource::UserSettings,
            OutputStyleSource::ProjectSettings,
        ];

        for source in sources {
            let json = serde_json::to_string(&source).unwrap();
            let deserialized: OutputStyleSource = serde_json::from_str(&json).unwrap();
            assert_eq!(source, deserialized);
        }
    }
}
