//! Tool-related types for the agent system.
//!
//! These types define tool execution characteristics and results.

use std::path::PathBuf;

use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

use crate::loop_event::ToolResultContent;

/// Concurrency safety level for a tool.
///
/// Determines whether a tool can be executed concurrently with other tools.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConcurrencySafety {
    /// Tool is safe to run concurrently with other tools.
    #[default]
    Safe,
    /// Tool must run exclusively (cannot run with other tools).
    Unsafe,
}

impl ConcurrencySafety {
    /// Check if concurrent execution is safe.
    pub fn is_safe(&self) -> bool {
        matches!(self, ConcurrencySafety::Safe)
    }

    /// Get the safety level as a string.
    pub fn as_str(&self) -> &'static str {
        match self {
            ConcurrencySafety::Safe => "safe",
            ConcurrencySafety::Unsafe => "unsafe",
        }
    }
}

impl std::fmt::Display for ConcurrencySafety {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Output from a tool execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolOutput {
    /// The content of the output.
    pub content: ToolResultContent,
    /// Whether this output represents an error.
    #[serde(default)]
    pub is_error: bool,
    /// Context modifiers to apply after this tool execution.
    #[serde(default)]
    pub modifiers: Vec<ContextModifier>,
}

impl Default for ToolOutput {
    fn default() -> Self {
        Self {
            content: ToolResultContent::default(),
            is_error: false,
            modifiers: Vec::new(),
        }
    }
}

impl ToolOutput {
    /// Create a successful text output.
    pub fn text(content: impl Into<String>) -> Self {
        Self {
            content: ToolResultContent::Text(content.into()),
            is_error: false,
            modifiers: Vec::new(),
        }
    }

    /// Create an error output.
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            content: ToolResultContent::Text(message.into()),
            is_error: true,
            modifiers: Vec::new(),
        }
    }

    /// Create a structured output.
    pub fn structured(value: Value) -> Self {
        Self {
            content: ToolResultContent::Structured(value),
            is_error: false,
            modifiers: Vec::new(),
        }
    }

    /// Add a context modifier.
    pub fn with_modifier(mut self, modifier: ContextModifier) -> Self {
        self.modifiers.push(modifier);
        self
    }

    /// Add multiple context modifiers.
    pub fn with_modifiers(mut self, modifiers: impl IntoIterator<Item = ContextModifier>) -> Self {
        self.modifiers.extend(modifiers);
        self
    }

    /// Truncate text content to at most `max_chars`, preserving start and end.
    pub fn truncate_to(&mut self, max_chars: usize) {
        if let ToolResultContent::Text(ref text) = self.content {
            if text.len() > max_chars {
                let half = max_chars / 2;
                let start_end = text.floor_char_boundary(half);
                let suffix_start = text.ceil_char_boundary(text.len() - half);
                let start = &text[..start_end];
                let end = &text[suffix_start..];
                let omitted = text.len() - start_end - (text.len() - suffix_start);
                self.content = ToolResultContent::Text(format!(
                    "{start}\n\n... (output truncated, {omitted} characters omitted) ...\n\n{end}"
                ));
            }
        }
    }
}

/// A modifier that changes the conversation context after tool execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContextModifier {
    /// A file was read, record its content.
    FileRead {
        /// Path to the file.
        path: PathBuf,
        /// Content that was read.
        content: String,
    },
    /// A permission was granted for future operations.
    PermissionGranted {
        /// Tool that received permission.
        tool: String,
        /// Pattern for allowed operations.
        pattern: String,
    },
}

/// Result of validating tool input.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "kebab-case")]
pub enum ValidationResult {
    /// Input is valid.
    Valid,
    /// Input is invalid.
    Invalid {
        /// List of validation errors.
        errors: Vec<ValidationError>,
    },
}

impl ValidationResult {
    /// Check if validation passed.
    pub fn is_valid(&self) -> bool {
        matches!(self, ValidationResult::Valid)
    }

    /// Create a valid result.
    pub fn valid() -> Self {
        ValidationResult::Valid
    }

    /// Create an invalid result with errors.
    pub fn invalid(errors: impl IntoIterator<Item = ValidationError>) -> Self {
        ValidationResult::Invalid {
            errors: errors.into_iter().collect(),
        }
    }

    /// Create an invalid result with a single error.
    pub fn error(message: impl Into<String>) -> Self {
        ValidationResult::Invalid {
            errors: vec![ValidationError {
                message: message.into(),
                path: None,
            }],
        }
    }
}

/// A validation error.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationError {
    /// Error message.
    pub message: String,
    /// JSON path to the invalid field (if applicable).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

impl ValidationError {
    /// Create a new validation error.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            path: None,
        }
    }

    /// Create a validation error with a path.
    pub fn with_path(message: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            path: Some(path.into()),
        }
    }
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(path) = &self.path {
            write!(f, "{}: {}", path, self.message)
        } else {
            write!(f, "{}", self.message)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_concurrency_safety_default() {
        assert_eq!(ConcurrencySafety::default(), ConcurrencySafety::Safe);
        assert!(ConcurrencySafety::Safe.is_safe());
        assert!(!ConcurrencySafety::Unsafe.is_safe());
    }

    #[test]
    fn test_tool_output_constructors() {
        let text = ToolOutput::text("Hello");
        assert!(!text.is_error);
        assert!(text.modifiers.is_empty());

        let error = ToolOutput::error("Something went wrong");
        assert!(error.is_error);

        let structured = ToolOutput::structured(serde_json::json!({"key": "value"}));
        assert!(!structured.is_error);
    }

    #[test]
    fn test_tool_output_with_modifiers() {
        let output = ToolOutput::text("Read file")
            .with_modifier(ContextModifier::FileRead {
                path: PathBuf::from("/tmp/test.txt"),
                content: "file content".to_string(),
            })
            .with_modifier(ContextModifier::PermissionGranted {
                tool: "Read".to_string(),
                pattern: "/tmp/*".to_string(),
            });

        assert_eq!(output.modifiers.len(), 2);
    }

    #[test]
    fn test_validation_result() {
        assert!(ValidationResult::valid().is_valid());
        assert!(!ValidationResult::error("invalid").is_valid());

        let result = ValidationResult::invalid([
            ValidationError::new("field required"),
            ValidationError::with_path("must be positive", "count"),
        ]);

        if let ValidationResult::Invalid { errors } = result {
            assert_eq!(errors.len(), 2);
            assert_eq!(errors[1].path.as_deref(), Some("count"));
        } else {
            panic!("Expected Invalid result");
        }
    }

    #[test]
    fn test_validation_error_display() {
        let error = ValidationError::new("something wrong");
        assert_eq!(format!("{error}"), "something wrong");

        let error_with_path = ValidationError::with_path("must be positive", "count");
        assert_eq!(format!("{error_with_path}"), "count: must be positive");
    }

    #[test]
    fn test_serde_roundtrip() {
        let output = ToolOutput::text("test").with_modifier(ContextModifier::FileRead {
            path: PathBuf::from("/test"),
            content: "content".to_string(),
        });

        let json = serde_json::to_string(&output).unwrap();
        let parsed: ToolOutput = serde_json::from_str(&json).unwrap();
        assert!(!parsed.is_error);
        assert_eq!(parsed.modifiers.len(), 1);
    }

    #[test]
    fn test_truncate_to_no_op_when_within_limit() {
        let mut output = ToolOutput::text("short text");
        output.truncate_to(100);
        assert!(matches!(&output.content, ToolResultContent::Text(s) if s == "short text"));
    }

    #[test]
    fn test_truncate_to_truncates_long_text() {
        let long = "a".repeat(1000);
        let mut output = ToolOutput::text(long);
        output.truncate_to(100);
        if let ToolResultContent::Text(ref s) = output.content {
            assert!(s.len() < 1000);
            assert!(s.contains("output truncated"));
            assert!(s.contains("characters omitted"));
        } else {
            panic!("Expected Text content");
        }
    }

    #[test]
    fn test_truncate_to_preserves_start_and_end() {
        let text = format!("START{}END", "x".repeat(1000));
        let mut output = ToolOutput::text(text);
        output.truncate_to(100);
        if let ToolResultContent::Text(ref s) = output.content {
            assert!(s.starts_with("START"));
            assert!(s.ends_with("END"));
        } else {
            panic!("Expected Text content");
        }
    }

    #[test]
    fn test_truncate_to_ignores_structured() {
        let mut output = ToolOutput::structured(serde_json::json!({"key": "value"}));
        output.truncate_to(1); // Should not panic or change
        assert!(matches!(&output.content, ToolResultContent::Structured(_)));
    }

    #[test]
    fn test_truncate_to_utf8_safe() {
        // Use multibyte characters to verify UTF-8 safety
        let text = "你好世界".repeat(100); // 4 chars × 3 bytes each × 100 = 1200 bytes
        let mut output = ToolOutput::text(text);
        output.truncate_to(100);
        if let ToolResultContent::Text(ref s) = output.content {
            // Should not panic and result should be valid UTF-8
            assert!(s.is_char_boundary(0));
            assert!(s.contains("output truncated"));
        } else {
            panic!("Expected Text content");
        }
    }
}
