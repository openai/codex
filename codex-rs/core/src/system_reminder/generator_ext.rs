//! Extended parsing utilities for @mention support.
//!
//! Provides parsing for user prompt @mentions:
//! - File mentions: @file.txt, @"path with spaces", @file.txt#L10-20
//! - Agent mentions: @agent-search, @agent-edit

use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;

// ============================================
// Regex Patterns
// ============================================

/// Quoted file mentions: @"path with spaces"
static QUOTED_FILE_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?:^|\s)@"([^"]+)""#).expect("valid quoted file regex"));

/// Unquoted file mentions: @filename or @path/to/file
static UNQUOTED_FILE_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?:^|\s)@([^\s"@]+)"#).expect("valid unquoted file regex"));

/// Agent mentions: @agent-type
static AGENT_MENTION_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?:^|\s)@(agent-[\w:.@-]+)").expect("valid agent mention regex"));

/// Line range pattern: filename#L10 or filename#L10-20
static LINE_RANGE_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^([^#]+)(?:#L(\d+)(?:-(\d+))?)?$").expect("valid line range regex"));

// ============================================
// Types
// ============================================

/// A file mention parsed from user prompt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileMention {
    /// Raw path string from the mention.
    pub raw_path: String,
    /// Line start (1-indexed, if specified).
    pub line_start: Option<i32>,
    /// Line end (1-indexed, if specified).
    pub line_end: Option<i32>,
    /// Whether the path was quoted.
    pub is_quoted: bool,
}

impl FileMention {
    /// Resolve the file mention to an absolute path.
    pub fn resolve(&self, cwd: &Path) -> PathBuf {
        let path = Path::new(&self.raw_path);
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            cwd.join(path)
        }
    }

    /// Check if this mention has a line range.
    pub fn has_line_range(&self) -> bool {
        self.line_start.is_some()
    }
}

/// An agent mention parsed from user prompt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentMention {
    /// Agent type (e.g., "search", "edit").
    pub agent_type: String,
}

/// Result of parsing all @mentions from user prompt.
#[derive(Debug, Default)]
pub struct ParsedMentions {
    /// File mentions (@file, @"path", @file#L10-20).
    pub files: Vec<FileMention>,
    /// Agent mentions (@agent-type).
    pub agents: Vec<AgentMention>,
}

// ============================================
// Parsing Functions
// ============================================

/// Parse all @mentions from user prompt.
///
/// Extracts file mentions and agent mentions from the user's message.
/// Deduplicates mentions and filters out agent mentions from file mentions.
pub fn parse_mentions(user_prompt: &str) -> ParsedMentions {
    let mut result = ParsedMentions::default();

    // First, extract agent mentions (they start with @agent-)
    let agent_mentions = parse_agent_mentions(user_prompt);
    let agent_strings: HashSet<String> = agent_mentions
        .iter()
        .map(|a| format!("agent-{}", a.agent_type))
        .collect();
    result.agents = agent_mentions;

    // Then extract file mentions, filtering out agent mentions
    result.files = parse_file_mentions(user_prompt)
        .into_iter()
        .filter(|f| !agent_strings.contains(&f.raw_path))
        .collect();

    result
}

/// Parse file mentions from user prompt.
///
/// Supports:
/// - @file.txt
/// - @"path with spaces"
/// - @path/to/file
/// - @file.txt#L10
/// - @file.txt#L10-20
pub fn parse_file_mentions(user_prompt: &str) -> Vec<FileMention> {
    let mut mentions = Vec::new();
    let mut seen = HashSet::new();

    // Parse quoted mentions first (higher priority)
    for cap in QUOTED_FILE_REGEX.captures_iter(user_prompt) {
        if let Some(path_match) = cap.get(1) {
            let raw = path_match.as_str().to_string();
            if seen.insert(raw.clone()) {
                let (path, line_start, line_end) = parse_line_range(&raw);
                mentions.push(FileMention {
                    raw_path: path,
                    line_start,
                    line_end,
                    is_quoted: true,
                });
            }
        }
    }

    // Parse unquoted mentions
    for cap in UNQUOTED_FILE_REGEX.captures_iter(user_prompt) {
        if let Some(path_match) = cap.get(1) {
            let raw = path_match.as_str().to_string();
            // Skip if already seen (quoted version) or if it's an agent mention
            if raw.starts_with("agent-") {
                continue;
            }
            if seen.insert(raw.clone()) {
                let (path, line_start, line_end) = parse_line_range(&raw);
                mentions.push(FileMention {
                    raw_path: path,
                    line_start,
                    line_end,
                    is_quoted: false,
                });
            }
        }
    }

    mentions
}

/// Parse agent mentions from user prompt.
///
/// Supports: @agent-search, @agent-edit, @agent-custom
pub fn parse_agent_mentions(user_prompt: &str) -> Vec<AgentMention> {
    let mut mentions = Vec::new();
    let mut seen = HashSet::new();

    for cap in AGENT_MENTION_REGEX.captures_iter(user_prompt) {
        if let Some(agent_match) = cap.get(1) {
            let full_type = agent_match.as_str();
            // Extract agent type (strip "agent-" prefix)
            let agent_type = full_type
                .strip_prefix("agent-")
                .unwrap_or(full_type)
                .to_string();

            if seen.insert(agent_type.clone()) {
                mentions.push(AgentMention { agent_type });
            }
        }
    }

    mentions
}

/// Parse line range from file path.
///
/// Input: "file.txt#L10-20" -> ("file.txt", Some(10), Some(20))
/// Input: "file.txt#L10" -> ("file.txt", Some(10), Some(10))
/// Input: "file.txt" -> ("file.txt", None, None)
fn parse_line_range(input: &str) -> (String, Option<i32>, Option<i32>) {
    if let Some(caps) = LINE_RANGE_REGEX.captures(input) {
        let path = caps
            .get(1)
            .map(|m| m.as_str().to_string())
            .unwrap_or_default();
        let line_start = caps.get(2).and_then(|m| m.as_str().parse::<i32>().ok());
        let line_end = caps
            .get(3)
            .and_then(|m| m.as_str().parse::<i32>().ok())
            .or(line_start); // If only start specified, end = start

        (path, line_start, line_end)
    } else {
        (input.to_string(), None, None)
    }
}

// ============================================
// Tests
// ============================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_unquoted_file_mentions() {
        let mentions = parse_file_mentions("Check @file.txt and @src/main.rs");
        assert_eq!(mentions.len(), 2);
        assert_eq!(mentions[0].raw_path, "file.txt");
        assert_eq!(mentions[1].raw_path, "src/main.rs");
        assert!(!mentions[0].is_quoted);
    }

    #[test]
    fn test_parse_quoted_file_mentions() {
        let mentions = parse_file_mentions(r#"Check @"path with spaces/file.txt""#);
        assert_eq!(mentions.len(), 1);
        assert_eq!(mentions[0].raw_path, "path with spaces/file.txt");
        assert!(mentions[0].is_quoted);
    }

    #[test]
    fn test_parse_file_with_line_range() {
        let mentions = parse_file_mentions("Check @file.txt#L10-20");
        assert_eq!(mentions.len(), 1);
        assert_eq!(mentions[0].raw_path, "file.txt");
        assert_eq!(mentions[0].line_start, Some(10));
        assert_eq!(mentions[0].line_end, Some(20));
    }

    #[test]
    fn test_parse_file_with_single_line() {
        let mentions = parse_file_mentions("Check @file.txt#L42");
        assert_eq!(mentions.len(), 1);
        assert_eq!(mentions[0].raw_path, "file.txt");
        assert_eq!(mentions[0].line_start, Some(42));
        assert_eq!(mentions[0].line_end, Some(42));
    }

    #[test]
    fn test_parse_agent_mentions() {
        let mentions = parse_agent_mentions("Use @agent-search to find files");
        assert_eq!(mentions.len(), 1);
        assert_eq!(mentions[0].agent_type, "search");
    }

    #[test]
    fn test_parse_multiple_agent_mentions() {
        let mentions = parse_agent_mentions("Use @agent-search and @agent-edit");
        assert_eq!(mentions.len(), 2);
        assert_eq!(mentions[0].agent_type, "search");
        assert_eq!(mentions[1].agent_type, "edit");
    }

    #[test]
    fn test_parse_mentions_combined() {
        let result = parse_mentions("Check @file.txt and use @agent-search");
        assert_eq!(result.files.len(), 1);
        assert_eq!(result.files[0].raw_path, "file.txt");
        assert_eq!(result.agents.len(), 1);
        assert_eq!(result.agents[0].agent_type, "search");
    }

    #[test]
    fn test_parse_mentions_deduplication() {
        let result = parse_mentions("Check @file.txt and @file.txt again");
        assert_eq!(result.files.len(), 1);
    }

    #[test]
    fn test_file_mention_resolve() {
        let mention = FileMention {
            raw_path: "src/main.rs".to_string(),
            line_start: None,
            line_end: None,
            is_quoted: false,
        };
        let resolved = mention.resolve(Path::new("/project"));
        assert_eq!(resolved, PathBuf::from("/project/src/main.rs"));
    }

    #[test]
    fn test_file_mention_resolve_absolute() {
        let mention = FileMention {
            raw_path: "/absolute/path.rs".to_string(),
            line_start: None,
            line_end: None,
            is_quoted: false,
        };
        let resolved = mention.resolve(Path::new("/project"));
        assert_eq!(resolved, PathBuf::from("/absolute/path.rs"));
    }

    #[test]
    fn test_parse_line_range() {
        assert_eq!(
            parse_line_range("file.txt#L10-20"),
            ("file.txt".to_string(), Some(10), Some(20))
        );
        assert_eq!(
            parse_line_range("file.txt#L42"),
            ("file.txt".to_string(), Some(42), Some(42))
        );
        assert_eq!(
            parse_line_range("file.txt"),
            ("file.txt".to_string(), None, None)
        );
    }

    #[test]
    fn test_agent_mentions_not_in_file_mentions() {
        let result = parse_mentions("@agent-search @file.txt");
        assert_eq!(result.files.len(), 1);
        assert_eq!(result.files[0].raw_path, "file.txt");
        assert_eq!(result.agents.len(), 1);
        assert_eq!(result.agents[0].agent_type, "search");
    }
}
