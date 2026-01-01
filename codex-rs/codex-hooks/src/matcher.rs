//! Pattern matching for hook matchers.
//!
//! Aligned with Claude Code's `matchPattern` (tk3) function.

use regex::Regex;
use tracing::debug;

/// Match a pattern against a value.
///
/// Pattern types supported:
/// - Empty or "*" → match all
/// - Simple alphanumeric (no special chars) → exact match
/// - Pipe-separated (e.g., "Write|Edit|Bash") → match any
/// - Otherwise → regex pattern
///
/// # Examples
///
/// ```
/// use codex_hooks::matches_pattern;
///
/// // Wildcard matches everything
/// assert!(matches_pattern("*", "anything"));
/// assert!(matches_pattern("", "anything"));
///
/// // Exact match
/// assert!(matches_pattern("Write", "Write"));
/// assert!(!matches_pattern("Write", "Read"));
///
/// // Pipe-separated alternatives
/// assert!(matches_pattern("Write|Edit|Bash", "Edit"));
/// assert!(!matches_pattern("Write|Edit|Bash", "Read"));
///
/// // Regex patterns
/// assert!(matches_pattern("^Bash.*", "BashOutput"));
/// assert!(matches_pattern("^(Read|Write)$", "Read"));
/// ```
pub fn matches_pattern(pattern: &str, value: &str) -> bool {
    let pattern = pattern.trim();

    // Empty pattern or "*" matches everything
    if pattern.is_empty() || pattern == "*" {
        return true;
    }

    // Check if pattern is simple alphanumeric (with underscores and pipes)
    if is_simple_pattern(pattern) {
        // Pipe-separated list
        if pattern.contains('|') {
            return pattern.split('|').map(str::trim).any(|p| p == value);
        }
        // Exact match
        return pattern == value;
    }

    // Treat as regex
    match Regex::new(pattern) {
        Ok(re) => re.is_match(value),
        Err(e) => {
            debug!("Invalid regex pattern in hook matcher: {pattern}: {e}");
            false
        }
    }
}

/// Check if a pattern is a simple alphanumeric pattern (with underscores, pipes, and spaces).
///
/// Simple patterns can use exact matching or pipe-separated alternatives
/// without regex overhead.
fn is_simple_pattern(pattern: &str) -> bool {
    pattern
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '|' || c == ' ')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wildcard_patterns() {
        assert!(matches_pattern("*", "anything"));
        assert!(matches_pattern("", "anything"));
        assert!(matches_pattern("  *  ", "anything"));
        assert!(matches_pattern("  ", "anything"));
    }

    #[test]
    fn test_exact_match() {
        assert!(matches_pattern("Write", "Write"));
        assert!(matches_pattern("Bash", "Bash"));
        assert!(!matches_pattern("Write", "Read"));
        assert!(!matches_pattern("Write", "write")); // Case sensitive
    }

    #[test]
    fn test_pipe_separated() {
        assert!(matches_pattern("Write|Edit|Bash", "Write"));
        assert!(matches_pattern("Write|Edit|Bash", "Edit"));
        assert!(matches_pattern("Write|Edit|Bash", "Bash"));
        assert!(!matches_pattern("Write|Edit|Bash", "Read"));
        assert!(!matches_pattern("Write|Edit|Bash", "WriteEdit"));
    }

    #[test]
    fn test_pipe_separated_with_spaces() {
        assert!(matches_pattern("Write | Edit | Bash", "Edit"));
    }

    #[test]
    fn test_regex_patterns() {
        // Starts with pattern
        assert!(matches_pattern("^Bash.*", "Bash"));
        assert!(matches_pattern("^Bash.*", "BashOutput"));
        assert!(!matches_pattern("^Bash.*", "NotBash"));

        // Exact match with anchors
        assert!(matches_pattern("^(Read|Write)$", "Read"));
        assert!(matches_pattern("^(Read|Write)$", "Write"));
        assert!(!matches_pattern("^(Read|Write)$", "ReadWrite"));

        // Partial match with regex
        assert!(matches_pattern(".*Tool.*", "MyToolName"));
        // Note: Simple alphanumeric patterns use exact match, not regex
        assert!(!matches_pattern("Tool", "MyToolName")); // Exact match fails
        assert!(matches_pattern("Tool", "Tool")); // Exact match succeeds
    }

    #[test]
    fn test_invalid_regex() {
        // Invalid regex should return false, not panic
        assert!(!matches_pattern("[invalid(regex", "anything"));
        assert!(!matches_pattern("(unclosed", "anything"));
    }

    #[test]
    fn test_is_simple_pattern() {
        assert!(is_simple_pattern("Write"));
        assert!(is_simple_pattern("Write|Edit"));
        assert!(is_simple_pattern("tool_name"));
        assert!(is_simple_pattern("Tool123"));
        assert!(is_simple_pattern("Write | Edit")); // Spaces allowed

        assert!(!is_simple_pattern("^Write"));
        assert!(!is_simple_pattern("Write.*"));
        assert!(!is_simple_pattern("(Read|Write)"));
        assert!(!is_simple_pattern("Write$"));
    }

    #[test]
    fn test_real_world_patterns() {
        // Common tool matchers
        assert!(matches_pattern("Bash", "Bash"));
        assert!(matches_pattern("Write|Edit|Bash", "Bash"));
        assert!(matches_pattern("^(Read|Write|Edit|Glob|Grep)$", "Read"));

        // Source matchers for SessionStart
        assert!(matches_pattern("cli", "cli"));
        assert!(matches_pattern("cli|ide|api", "ide"));

        // Trigger matchers for PreCompact
        assert!(matches_pattern("auto|manual", "auto"));
    }
}
