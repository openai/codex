//! XML wrapper functions for system reminders.
//!
//! This module provides utilities for wrapping content in XML tags
//! and extracting content from XML-wrapped strings.

use regex_lite::Regex;

use crate::types::XmlTag;

/// Wrap content with the specified XML tag.
///
/// If the tag is `XmlTag::None`, returns the content unchanged.
///
/// # Examples
///
/// ```
/// use cocode_system_reminder::{wrap_with_tag, XmlTag};
///
/// let content = "File changed: foo.rs";
/// let wrapped = wrap_with_tag(content, XmlTag::SystemReminder);
/// assert!(wrapped.starts_with("<system-reminder>"));
/// assert!(wrapped.ends_with("</system-reminder>"));
/// ```
pub fn wrap_with_tag(content: &str, tag: XmlTag) -> String {
    match tag.tag_name() {
        Some(tag_name) => {
            format!("<{tag_name}>\n{content}\n</{tag_name}>")
        }
        None => content.to_string(),
    }
}

/// Wrap content with the `<system-reminder>` tag.
///
/// Convenience function for the most common case.
///
/// # Examples
///
/// ```
/// use cocode_system_reminder::wrap_system_reminder;
///
/// let wrapped = wrap_system_reminder("Important context");
/// assert_eq!(wrapped, "<system-reminder>\nImportant context\n</system-reminder>");
/// ```
pub fn wrap_system_reminder(content: &str) -> String {
    wrap_with_tag(content, XmlTag::SystemReminder)
}

/// Extract content from a `<system-reminder>` tag.
///
/// Returns `None` if the string doesn't contain a valid system-reminder tag.
///
/// # Examples
///
/// ```
/// use cocode_system_reminder::extract_system_reminder;
///
/// let wrapped = "<system-reminder>\nHello world\n</system-reminder>";
/// let content = extract_system_reminder(wrapped);
/// assert_eq!(content, Some("Hello world"));
/// ```
pub fn extract_system_reminder(text: &str) -> Option<&str> {
    extract_tag_content(text, "system-reminder")
}

/// Extract content from any XML tag.
///
/// Uses a regex to find and extract the content between opening and closing tags.
fn extract_tag_content<'a>(text: &'a str, tag_name: &str) -> Option<&'a str> {
    // Pattern: <tag_name>\n?(content)\n?</tag_name>
    // Use lazy matching for content to handle nested tags correctly
    let pattern = format!(r"<{tag_name}>\n?([\s\S]*?)\n?</{tag_name}>");
    let re = Regex::new(&pattern).ok()?;

    re.captures(text)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str())
}

/// Check if text contains a system reminder.
pub fn has_system_reminder(text: &str) -> bool {
    text.contains("<system-reminder>") && text.contains("</system-reminder>")
}

/// Check if text contains a specific XML tag.
pub fn has_tag(text: &str, tag: XmlTag) -> bool {
    match tag.tag_name() {
        Some(name) => text.contains(&format!("<{name}>")) && text.contains(&format!("</{name}>")),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wrap_with_tag() {
        let content = "Hello world";

        let wrapped = wrap_with_tag(content, XmlTag::SystemReminder);
        assert_eq!(
            wrapped,
            "<system-reminder>\nHello world\n</system-reminder>"
        );

        let wrapped = wrap_with_tag(content, XmlTag::NewDiagnostics);
        assert_eq!(
            wrapped,
            "<new-diagnostics>\nHello world\n</new-diagnostics>"
        );

        let wrapped = wrap_with_tag(content, XmlTag::None);
        assert_eq!(wrapped, "Hello world");
    }

    #[test]
    fn test_wrap_system_reminder() {
        let content = "Important context";
        let wrapped = wrap_system_reminder(content);
        assert_eq!(
            wrapped,
            "<system-reminder>\nImportant context\n</system-reminder>"
        );
    }

    #[test]
    fn test_extract_system_reminder() {
        // Basic case
        let wrapped = "<system-reminder>\nHello world\n</system-reminder>";
        assert_eq!(extract_system_reminder(wrapped), Some("Hello world"));

        // Without newlines
        let wrapped = "<system-reminder>Hello world</system-reminder>";
        assert_eq!(extract_system_reminder(wrapped), Some("Hello world"));

        // Multi-line content
        let wrapped = "<system-reminder>\nLine 1\nLine 2\nLine 3\n</system-reminder>";
        assert_eq!(
            extract_system_reminder(wrapped),
            Some("Line 1\nLine 2\nLine 3")
        );

        // No match
        let text = "Just plain text";
        assert_eq!(extract_system_reminder(text), None);

        // Partial tag
        let text = "<system-reminder>unclosed";
        assert_eq!(extract_system_reminder(text), None);
    }

    #[test]
    fn test_roundtrip() {
        let original = "This is some important context\nwith multiple lines";
        let wrapped = wrap_system_reminder(original);
        let extracted = extract_system_reminder(&wrapped);
        assert_eq!(extracted, Some(original));
    }

    #[test]
    fn test_has_system_reminder() {
        assert!(has_system_reminder(
            "<system-reminder>content</system-reminder>"
        ));
        assert!(has_system_reminder(
            "prefix <system-reminder>content</system-reminder> suffix"
        ));
        assert!(!has_system_reminder("plain text"));
        assert!(!has_system_reminder("<system-reminder>unclosed"));
    }

    #[test]
    fn test_has_tag() {
        let text = "<new-diagnostics>content</new-diagnostics>";
        assert!(has_tag(text, XmlTag::NewDiagnostics));
        assert!(!has_tag(text, XmlTag::SystemReminder));
        assert!(!has_tag(text, XmlTag::None));
    }

    #[test]
    fn test_extract_other_tags() {
        let text = "<new-diagnostics>\nDiagnostic info\n</new-diagnostics>";
        let content = extract_tag_content(text, "new-diagnostics");
        assert_eq!(content, Some("Diagnostic info"));
    }
}
