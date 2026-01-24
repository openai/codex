//! YAML frontmatter parsing for markdown files.
//!
//! Parses YAML frontmatter from SKILL.md, command files, and agent definitions.
//!
//! # Format
//!
//! ```markdown
//! ---
//! name: my-skill
//! description: A brief description
//! ---
//!
//! # Content below frontmatter
//! ...
//! ```

use serde::Deserialize;
use serde::Serialize;

/// Parsed frontmatter from a markdown file.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Frontmatter {
    /// Item name (overrides directory/file name).
    pub name: Option<String>,
    /// Short description.
    pub description: Option<String>,
    /// Long description or summary.
    pub summary: Option<String>,
    /// Tags/keywords.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Version.
    pub version: Option<String>,
    /// Author.
    pub author: Option<String>,
    /// Whether this is deprecated.
    #[serde(default)]
    pub deprecated: bool,
    /// Alternative item to use if deprecated.
    pub replaced_by: Option<String>,
    /// Aliases for this item.
    #[serde(default)]
    pub aliases: Vec<String>,
    /// Required capabilities/permissions.
    #[serde(default)]
    pub requires: Vec<String>,
    /// Additional custom fields.
    #[serde(flatten)]
    pub extra: std::collections::HashMap<String, serde_yaml::Value>,
}

/// Result of parsing a markdown file with frontmatter.
#[derive(Debug, Clone)]
pub struct ParsedMarkdown {
    /// Parsed frontmatter (if present).
    pub frontmatter: Option<Frontmatter>,
    /// Content after frontmatter.
    pub content: String,
}

/// Parse YAML frontmatter from markdown content.
///
/// Frontmatter must be at the beginning of the file, delimited by `---` lines.
///
/// # Examples
///
/// ```
/// use codex_plugin::frontmatter::parse_frontmatter;
///
/// let content = r#"---
/// name: my-skill
/// description: A helpful skill
/// ---
///
/// # My Skill
///
/// This skill does helpful things.
/// "#;
///
/// let parsed = parse_frontmatter(content);
/// assert!(parsed.frontmatter.is_some());
/// let fm = parsed.frontmatter.unwrap();
/// assert_eq!(fm.name, Some("my-skill".to_string()));
/// ```
pub fn parse_frontmatter(content: &str) -> ParsedMarkdown {
    let content = content.trim_start();

    // Check if content starts with frontmatter delimiter
    if !content.starts_with("---") {
        return ParsedMarkdown {
            frontmatter: None,
            content: content.to_string(),
        };
    }

    // Find the end of frontmatter
    let after_first = &content[3..];
    let end_pos = after_first
        .find("\n---")
        .or_else(|| after_first.find("\r\n---"));

    match end_pos {
        Some(pos) => {
            let yaml_content = &after_first[..pos];
            let rest_start = pos + 4; // Skip past "\n---"

            // Skip past the closing --- and any trailing newline
            let rest = after_first[rest_start..]
                .trim_start_matches('\n')
                .trim_start_matches('\r');

            match serde_yaml::from_str::<Frontmatter>(yaml_content) {
                Ok(fm) => ParsedMarkdown {
                    frontmatter: Some(fm),
                    content: rest.to_string(),
                },
                Err(_) => {
                    // Invalid YAML, treat as no frontmatter
                    ParsedMarkdown {
                        frontmatter: None,
                        content: content.to_string(),
                    }
                }
            }
        }
        None => {
            // No closing delimiter, treat as no frontmatter
            ParsedMarkdown {
                frontmatter: None,
                content: content.to_string(),
            }
        }
    }
}

/// Extract description from frontmatter or content.
///
/// Priority:
/// 1. Frontmatter description
/// 2. Frontmatter summary
/// 3. First non-heading, non-empty line from content
pub fn extract_description(parsed: &ParsedMarkdown) -> String {
    // Try frontmatter first
    if let Some(ref fm) = parsed.frontmatter {
        if let Some(ref desc) = fm.description {
            return desc.clone();
        }
        if let Some(ref summary) = fm.summary {
            return summary.clone();
        }
    }

    // Fall back to first content line
    parsed
        .content
        .lines()
        .find(|l| !l.starts_with('#') && !l.trim().is_empty())
        .unwrap_or("No description")
        .to_string()
}

/// Extract name from frontmatter or fallback.
pub fn extract_name(parsed: &ParsedMarkdown, fallback: &str) -> String {
    parsed
        .frontmatter
        .as_ref()
        .and_then(|fm| fm.name.clone())
        .unwrap_or_else(|| fallback.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_with_frontmatter() {
        let content = r#"---
name: my-skill
description: A helpful skill
tags:
  - utility
  - productivity
---

# My Skill

This skill does helpful things.
"#;

        let parsed = parse_frontmatter(content);
        assert!(parsed.frontmatter.is_some());

        let fm = parsed.frontmatter.unwrap();
        assert_eq!(fm.name, Some("my-skill".to_string()));
        assert_eq!(fm.description, Some("A helpful skill".to_string()));
        assert_eq!(fm.tags, vec!["utility", "productivity"]);

        assert!(parsed.content.starts_with("# My Skill"));
    }

    #[test]
    fn test_parse_without_frontmatter() {
        let content = "# My Skill\n\nThis is just regular markdown.";
        let parsed = parse_frontmatter(content);

        assert!(parsed.frontmatter.is_none());
        assert_eq!(parsed.content, content);
    }

    #[test]
    fn test_parse_empty_frontmatter() {
        let content = "---\n---\n\n# Content";
        let parsed = parse_frontmatter(content);

        assert!(parsed.frontmatter.is_some());
        let fm = parsed.frontmatter.unwrap();
        assert!(fm.name.is_none());
        assert!(parsed.content.starts_with("# Content"));
    }

    #[test]
    fn test_parse_invalid_yaml() {
        let content = "---\nname: [invalid yaml\n---\n\n# Content";
        let parsed = parse_frontmatter(content);

        // Invalid YAML should be treated as no frontmatter
        assert!(parsed.frontmatter.is_none());
    }

    #[test]
    fn test_parse_unclosed_frontmatter() {
        let content = "---\nname: test\n\n# Content without closing delimiter";
        let parsed = parse_frontmatter(content);

        assert!(parsed.frontmatter.is_none());
    }

    #[test]
    fn test_extract_description() {
        // From frontmatter
        let parsed = ParsedMarkdown {
            frontmatter: Some(Frontmatter {
                description: Some("Frontmatter description".to_string()),
                ..Default::default()
            }),
            content: "Some content here".to_string(),
        };
        assert_eq!(extract_description(&parsed), "Frontmatter description");

        // From summary when no description
        let parsed = ParsedMarkdown {
            frontmatter: Some(Frontmatter {
                summary: Some("Summary text".to_string()),
                ..Default::default()
            }),
            content: "Some content".to_string(),
        };
        assert_eq!(extract_description(&parsed), "Summary text");

        // From content when no frontmatter
        let parsed = ParsedMarkdown {
            frontmatter: None,
            content: "# Heading\n\nThis is the description.".to_string(),
        };
        assert_eq!(extract_description(&parsed), "This is the description.");
    }

    #[test]
    fn test_extract_name() {
        let parsed = ParsedMarkdown {
            frontmatter: Some(Frontmatter {
                name: Some("custom-name".to_string()),
                ..Default::default()
            }),
            content: String::new(),
        };
        assert_eq!(extract_name(&parsed, "fallback"), "custom-name");

        let parsed = ParsedMarkdown {
            frontmatter: None,
            content: String::new(),
        };
        assert_eq!(extract_name(&parsed, "fallback"), "fallback");
    }

    #[test]
    fn test_extra_fields() {
        let content = r#"---
name: test
custom_field: custom_value
another_field: 123
---

Content
"#;
        let parsed = parse_frontmatter(content);
        let fm = parsed.frontmatter.unwrap();

        assert!(fm.extra.contains_key("custom_field"));
        assert!(fm.extra.contains_key("another_field"));
    }

    #[test]
    fn test_deprecated_field() {
        let content = r#"---
name: old-skill
deprecated: true
replaced_by: new-skill
---

Old content
"#;
        let parsed = parse_frontmatter(content);
        let fm = parsed.frontmatter.unwrap();

        assert!(fm.deprecated);
        assert_eq!(fm.replaced_by, Some("new-skill".to_string()));
    }

    #[test]
    fn test_crlf_line_endings() {
        let content = "---\r\nname: test\r\n---\r\n\r\n# Content";
        let parsed = parse_frontmatter(content);

        assert!(parsed.frontmatter.is_some());
        let fm = parsed.frontmatter.unwrap();
        assert_eq!(fm.name, Some("test".to_string()));
    }
}
