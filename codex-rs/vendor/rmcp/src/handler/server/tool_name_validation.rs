//! Tool name validation utilities according to SEP: Specify Format for Tool Names
//!
//! Tool names SHOULD be between 1 and 128 characters in length (inclusive).
//! Tool names are case-sensitive.
//! Allowed characters: uppercase and lowercase ASCII letters (A-Z, a-z), digits
//! (0-9), underscore (_), dash (-), and dot (.).
//! Tool names SHOULD NOT contain spaces, commas, or other special characters.

use std::collections::HashSet;

/// Result of tool name validation containing validation status and warnings.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ToolNameValidationResult {
    /// Whether the tool name is valid according to the specification
    is_valid: bool,
    /// Array of warning messages about non-conforming aspects of the tool name
    warnings: Vec<String>,
}

impl ToolNameValidationResult {
    /// Create a new validation result
    fn new(is_valid: bool, warnings: Vec<String>) -> Self {
        Self { is_valid, warnings }
    }
}

/// Validates a tool name according to the SEP specification.
fn validate_tool_name(name: &str) -> ToolNameValidationResult {
    let mut warnings = Vec::new();

    // Check length
    if name.is_empty() {
        return ToolNameValidationResult::new(false, vec!["Tool name cannot be empty".to_string()]);
    }

    if name.len() > 128 {
        return ToolNameValidationResult::new(
            false,
            vec![format!(
                "Tool name exceeds maximum length of 128 characters (current: {})",
                name.len()
            )],
        );
    }

    // Check for specific problematic patterns (these are warnings, not validation failures)
    if name.contains(' ') {
        warnings.push("Tool name contains spaces, which may cause parsing issues".to_string());
    }

    if name.contains(',') {
        warnings.push("Tool name contains commas, which may cause parsing issues".to_string());
    }

    // Check for potentially confusing patterns (leading/trailing dashes, dots, slashes)
    if name.starts_with('-') || name.ends_with('-') {
        warnings.push(
            "Tool name starts or ends with a dash, which may cause parsing issues in some contexts"
                .to_string(),
        );
    }

    if name.starts_with('.') || name.ends_with('.') {
        warnings.push(
            "Tool name starts or ends with a dot, which may cause parsing issues in some contexts"
                .to_string(),
        );
    }

    // Check for invalid characters
    let mut invalid_chars = HashSet::new();
    let valid_chars: HashSet<char> =
        "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789._-"
            .chars()
            .collect();

    for ch in name.chars() {
        if !valid_chars.contains(&ch) {
            invalid_chars.insert(ch);
        }
    }

    if !invalid_chars.is_empty() {
        let invalid_chars_list: Vec<String> =
            invalid_chars.iter().map(|c| format!("\"{}\"", c)).collect();
        warnings.push(format!(
            "Tool name contains invalid characters: {}",
            invalid_chars_list.join(", ")
        ));
        warnings.push(
            "Allowed characters are: A-Z, a-z, 0-9, underscore (_), dash (-), and dot (.)"
                .to_string(),
        );

        return ToolNameValidationResult::new(false, warnings);
    }

    // Verify the pattern matches (double check with character-by-character validation)
    // We've already validated characters above, just need to verify length is within bounds
    if name.is_empty() || name.len() > 128 {
        return ToolNameValidationResult::new(
            false,
            vec!["Tool name length must be between 1 and 128 characters".to_string()],
        );
    }

    ToolNameValidationResult::new(true, warnings)
}

/// Issues warnings for non-conforming tool names.
fn issue_tool_name_warning(name: &str, warnings: &[String]) {
    tracing::warn!("Tool name validation warning for \"{}\":", name);
    for warning in warnings {
        tracing::warn!("  - {}", warning);
    }
    tracing::warn!("Tool registration will proceed, but this may cause compatibility issues.");
    tracing::warn!("Consider updating the tool name to conform to the MCP tool naming standard.");
    tracing::warn!(
        "See SEP: Specify Format for Tool Names (https://github.com/modelcontextprotocol/modelcontextprotocol/issues/986) for more details."
    );
}

/// Validates a tool name and issues warnings for non-conforming names.
pub fn validate_and_warn_tool_name(name: &str) -> bool {
    let result = validate_tool_name(name);

    if !result.warnings.is_empty() {
        issue_tool_name_warning(name, &result.warnings);
    }

    result.is_valid
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_tool_names() {
        let max_length_name = "a".repeat(128);
        let valid_names = vec![
            "my_tool",
            "MyTool",
            "my-tool",
            "my.tool",
            "tool123",
            "a",
            max_length_name.as_str(), // Maximum length
        ];

        for name in valid_names {
            let result = validate_tool_name(name);
            assert!(result.is_valid, "Tool name '{}' should be valid", name);
        }
    }

    #[test]
    fn test_empty_tool_name() {
        let result = validate_tool_name("");
        assert!(!result.is_valid);
        assert!(
            result
                .warnings
                .contains(&"Tool name cannot be empty".to_string())
        );
    }

    #[test]
    fn test_too_long_tool_name() {
        let name = "a".repeat(129);
        let result = validate_tool_name(&name);
        assert!(!result.is_valid);
        assert!(result.warnings[0].contains("exceeds maximum length"));
    }

    #[test]
    fn test_tool_name_with_spaces() {
        let result = validate_tool_name("my tool");
        assert!(!result.is_valid);
        assert!(
            result
                .warnings
                .iter()
                .any(|w| w.contains("contains spaces"))
        );
    }

    #[test]
    fn test_tool_name_with_commas() {
        let result = validate_tool_name("my,tool");
        assert!(!result.is_valid);
        assert!(
            result
                .warnings
                .iter()
                .any(|w| w.contains("contains commas"))
        );
    }

    #[test]
    fn test_tool_name_starting_with_dash() {
        let result = validate_tool_name("-tool");
        assert!(result.is_valid); // Still valid, but has warning
        assert!(
            result
                .warnings
                .iter()
                .any(|w| w.contains("starts or ends with a dash"))
        );
    }

    #[test]
    fn test_tool_name_ending_with_dot() {
        let result = validate_tool_name("tool.");
        assert!(result.is_valid); // Still valid, but has warning
        assert!(
            result
                .warnings
                .iter()
                .any(|w| w.contains("starts or ends with a dot"))
        );
    }

    #[test]
    fn test_tool_name_with_invalid_characters() {
        let result = validate_tool_name("my@tool");
        assert!(!result.is_valid);
        assert!(
            result
                .warnings
                .iter()
                .any(|w| w.contains("contains invalid characters"))
        );
    }

    #[test]
    fn test_tool_name_all_special_characters_allowed() {
        let valid_chars = vec!['_', '-', '.'];
        for ch in valid_chars {
            let name = format!("tool{}", ch);
            let result = validate_tool_name(&name);
            assert!(
                result.is_valid,
                "Tool name with character '{}' should be valid",
                ch
            );
        }
    }

    #[test]
    fn test_minimum_length() {
        let result = validate_tool_name("a");
        assert!(result.is_valid);
    }

    #[test]
    fn test_maximum_length() {
        let name = "a".repeat(128);
        let result = validate_tool_name(&name);
        assert!(result.is_valid);
    }
}
