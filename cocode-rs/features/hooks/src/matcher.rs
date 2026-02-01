//! Hook matchers for filtering which invocations trigger a hook.
//!
//! Matchers inspect a string value (typically a tool name) to decide whether
//! the hook applies.

use serde::Deserialize;
use serde::Serialize;

use crate::error::HookError;
use crate::error::hook_error::*;

/// A matcher that determines whether a hook should fire for a given value.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HookMatcher {
    /// Matches an exact string value.
    Exact { value: String },

    /// Matches using a glob-style wildcard pattern.
    /// Supports `*` (any characters) and `?` (single character).
    Wildcard { pattern: String },

    /// Matches if any of the inner matchers match.
    Or { matchers: Vec<HookMatcher> },

    /// Matches using a regular expression.
    Regex { pattern: String },

    /// Matches everything.
    All,
}

impl HookMatcher {
    /// Returns `true` if the given value matches this matcher.
    ///
    /// For `Regex` patterns that fail to compile, returns `false` and logs a
    /// warning.
    pub fn matches(&self, value: &str) -> bool {
        match self {
            Self::Exact { value: expected } => value == expected,
            Self::Wildcard { pattern } => wildcard_matches(pattern, value),
            Self::Or { matchers } => matchers.iter().any(|m| m.matches(value)),
            Self::Regex { pattern } => match regex::Regex::new(pattern) {
                Ok(re) => re.is_match(value),
                Err(e) => {
                    tracing::warn!("Invalid regex pattern '{pattern}': {e}");
                    false
                }
            },
            Self::All => true,
        }
    }

    /// Validates this matcher, returning an error if it contains invalid
    /// patterns.
    pub fn validate(&self) -> Result<(), HookError> {
        match self {
            Self::Regex { pattern } => {
                regex::Regex::new(pattern).map_err(|e| {
                    InvalidMatcherSnafu {
                        message: format!("invalid regex '{pattern}': {e}"),
                    }
                    .build()
                })?;
                Ok(())
            }
            Self::Or { matchers } => {
                for m in matchers {
                    m.validate()?;
                }
                Ok(())
            }
            Self::Exact { .. } | Self::Wildcard { .. } | Self::All => Ok(()),
        }
    }
}

/// Simple glob-style wildcard matching.
///
/// `*` matches zero or more characters and `?` matches exactly one character.
fn wildcard_matches(pattern: &str, value: &str) -> bool {
    let pattern_chars: Vec<char> = pattern.chars().collect();
    let value_chars: Vec<char> = value.chars().collect();
    wildcard_recursive(&pattern_chars, &value_chars, 0, 0)
}

fn wildcard_recursive(pattern: &[char], value: &[char], pi: usize, vi: usize) -> bool {
    if pi == pattern.len() {
        return vi == value.len();
    }

    if pattern[pi] == '*' {
        // Try matching zero or more characters
        let mut v = vi;
        while v <= value.len() {
            if wildcard_recursive(pattern, value, pi + 1, v) {
                return true;
            }
            v += 1;
        }
        return false;
    }

    if vi < value.len() && (pattern[pi] == '?' || pattern[pi] == value[vi]) {
        return wildcard_recursive(pattern, value, pi + 1, vi + 1);
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_match() {
        let m = HookMatcher::Exact {
            value: "bash".to_string(),
        };
        assert!(m.matches("bash"));
        assert!(!m.matches("Bash"));
        assert!(!m.matches("bash_tool"));
    }

    #[test]
    fn test_wildcard_star() {
        let m = HookMatcher::Wildcard {
            pattern: "read_*".to_string(),
        };
        assert!(m.matches("read_file"));
        assert!(m.matches("read_dir"));
        assert!(m.matches("read_"));
        assert!(!m.matches("write_file"));
    }

    #[test]
    fn test_wildcard_question() {
        let m = HookMatcher::Wildcard {
            pattern: "tool_?".to_string(),
        };
        assert!(m.matches("tool_a"));
        assert!(m.matches("tool_1"));
        assert!(!m.matches("tool_ab"));
        assert!(!m.matches("tool_"));
    }

    #[test]
    fn test_wildcard_complex() {
        let m = HookMatcher::Wildcard {
            pattern: "*_file_*".to_string(),
        };
        assert!(m.matches("read_file_sync"));
        assert!(m.matches("write_file_async"));
        assert!(m.matches("_file_"));
        assert!(!m.matches("file"));
    }

    #[test]
    fn test_or_matcher() {
        let m = HookMatcher::Or {
            matchers: vec![
                HookMatcher::Exact {
                    value: "bash".to_string(),
                },
                HookMatcher::Exact {
                    value: "shell".to_string(),
                },
            ],
        };
        assert!(m.matches("bash"));
        assert!(m.matches("shell"));
        assert!(!m.matches("python"));
    }

    #[test]
    fn test_regex_match() {
        let m = HookMatcher::Regex {
            pattern: r"^(read|write)_\w+$".to_string(),
        };
        assert!(m.matches("read_file"));
        assert!(m.matches("write_data"));
        assert!(!m.matches("delete_file"));
        assert!(!m.matches("read file"));
    }

    #[test]
    fn test_regex_invalid_pattern() {
        let m = HookMatcher::Regex {
            pattern: r"[invalid".to_string(),
        };
        // Invalid regex should return false (not panic)
        assert!(!m.matches("anything"));
    }

    #[test]
    fn test_all_matcher() {
        let m = HookMatcher::All;
        assert!(m.matches("anything"));
        assert!(m.matches(""));
        assert!(m.matches("literally anything"));
    }

    #[test]
    fn test_validate_valid_regex() {
        let m = HookMatcher::Regex {
            pattern: r"^test$".to_string(),
        };
        assert!(m.validate().is_ok());
    }

    #[test]
    fn test_validate_invalid_regex() {
        let m = HookMatcher::Regex {
            pattern: r"[invalid".to_string(),
        };
        assert!(m.validate().is_err());
    }

    #[test]
    fn test_validate_or_with_invalid_regex() {
        let m = HookMatcher::Or {
            matchers: vec![
                HookMatcher::Exact {
                    value: "ok".to_string(),
                },
                HookMatcher::Regex {
                    pattern: r"[bad".to_string(),
                },
            ],
        };
        assert!(m.validate().is_err());
    }

    #[test]
    fn test_validate_non_regex() {
        let exact = HookMatcher::Exact {
            value: "test".to_string(),
        };
        assert!(exact.validate().is_ok());

        let wildcard = HookMatcher::Wildcard {
            pattern: "t*".to_string(),
        };
        assert!(wildcard.validate().is_ok());

        let all = HookMatcher::All;
        assert!(all.validate().is_ok());
    }

    #[test]
    fn test_serde_roundtrip() {
        let m = HookMatcher::Or {
            matchers: vec![
                HookMatcher::Exact {
                    value: "bash".to_string(),
                },
                HookMatcher::Wildcard {
                    pattern: "read_*".to_string(),
                },
            ],
        };
        let json = serde_json::to_string(&m).expect("serialize");
        let parsed: HookMatcher = serde_json::from_str(&json).expect("deserialize");
        assert!(parsed.matches("bash"));
        assert!(parsed.matches("read_file"));
        assert!(!parsed.matches("write_file"));
    }
}
