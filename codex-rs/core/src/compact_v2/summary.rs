//! Summary message formatting.
//!
//! Handles creation and formatting of summary messages after compaction.

use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use once_cell::sync::Lazy;
use regex::Regex;

/// Lazy-compiled regex for analysis tags.
static ANALYSIS_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"<analysis>([\s\S]*?)</analysis>").expect("invalid analysis regex"));

/// Lazy-compiled regex for summary tags.
static SUMMARY_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"<summary>([\s\S]*?)</summary>").expect("invalid summary regex"));

/// Lazy-compiled regex for collapsing multiple newlines.
static NEWLINES_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\n\n+").expect("invalid newlines regex"));

/// Prefix for V2 summary messages.
///
/// Used to identify summary messages from previous compactions.
pub const SUMMARY_PREFIX_V2: &str =
    "This session is being continued from a previous conversation that ran out of context.";

/// Format summary content with optional continue instruction.
///
/// Matches Claude Code's T91 / formatSummaryContent function.
pub fn format_summary_content(summary_text: &str, continue_without_asking: bool) -> String {
    let cleaned = cleanup_summary_tags(summary_text);
    let base = format!(
        "{}\nThe conversation is summarized below:\n{}",
        SUMMARY_PREFIX_V2, cleaned
    );

    if continue_without_asking {
        format!(
            "{}\nPlease continue the conversation from where we left it off without asking the user any further questions. Continue with the last task that you were asked to work on.",
            base
        )
    } else {
        base
    }
}

/// Clean up XML tags from LLM response.
///
/// Matches Claude Code's MD5 / cleanupSummaryTags function.
///
/// Transforms:
/// - `<analysis>...</analysis>` to `Analysis:\n...`
/// - `<summary>...</summary>` to `Summary:\n...`
/// - Collapses multiple newlines
pub fn cleanup_summary_tags(raw: &str) -> String {
    let mut result = raw.to_string();

    // Transform <analysis>...</analysis> to "Analysis:\n..."
    if let Some(caps) = ANALYSIS_RE.captures(&result) {
        if let Some(content) = caps.get(1) {
            let replacement = format!("Analysis:\n{}", content.as_str().trim());
            result = ANALYSIS_RE
                .replace(&result, replacement.as_str())
                .to_string();
        }
    }

    // Transform <summary>...</summary> to "Summary:\n..."
    if let Some(caps) = SUMMARY_RE.captures(&result) {
        if let Some(content) = caps.get(1) {
            let replacement = format!("Summary:\n{}", content.as_str().trim());
            result = SUMMARY_RE
                .replace(&result, replacement.as_str())
                .to_string();
        }
    }

    // Collapse multiple newlines
    result = NEWLINES_RE.replace_all(&result, "\n\n").to_string();

    result.trim().to_string()
}

/// Create summary message with proper metadata.
///
/// Creates a user message containing the formatted summary that will be
/// used as context for the model after compaction.
///
/// Matches Claude Code's summary message structure.
pub fn create_summary_message(summary_text: &str, continue_without_asking: bool) -> ResponseItem {
    ResponseItem::Message {
        id: Some("compact_summary".to_string()),
        role: "user".to_string(),
        content: vec![ContentItem::InputText {
            text: format_summary_content(summary_text, continue_without_asking),
        }],
    }
}

/// Extract summary text from LLM response.
///
/// Looks for content in `<summary>` tags or returns the full text if no tags.
#[allow(dead_code)] // Summary extraction utility
pub fn extract_summary_text(response: &str) -> String {
    if let Some(caps) = SUMMARY_RE.captures(response) {
        if let Some(content) = caps.get(1) {
            return content.as_str().trim().to_string();
        }
    }

    // No summary tags, return full response
    response.trim().to_string()
}

/// Minimum meaningful summary length (approximately 50 tokens at 4 chars/token).
const MIN_SUMMARY_LENGTH: usize = 200;

/// Check if summary is valid (not empty, not an error).
pub fn is_valid_summary(summary: &str) -> bool {
    !summary.is_empty()
        && !summary.starts_with("API_ERROR:")
        && !summary.starts_with("Error:")
        && summary.len() > MIN_SUMMARY_LENGTH
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn format_summary_basic() {
        let summary = "User asked about Rust. I explained ownership.";
        let formatted = format_summary_content(summary, false);

        assert!(formatted.starts_with(SUMMARY_PREFIX_V2));
        assert!(formatted.contains("The conversation is summarized below:"));
        assert!(formatted.contains(summary));
        assert!(!formatted.contains("Continue with the last task"));
    }

    #[test]
    fn format_summary_with_continue() {
        let summary = "User asked about Rust.";
        let formatted = format_summary_content(summary, true);

        assert!(formatted.contains("Continue with the last task"));
        assert!(formatted.contains("without asking the user any further questions"));
    }

    #[test]
    fn cleanup_analysis_tags() {
        let raw = "<analysis>This is the analysis content.</analysis>";
        let cleaned = cleanup_summary_tags(raw);
        assert_eq!(cleaned, "Analysis:\nThis is the analysis content.");
    }

    #[test]
    fn cleanup_summary_tags_test() {
        let raw = "<summary>This is the summary content.</summary>";
        let cleaned = cleanup_summary_tags(raw);
        assert_eq!(cleaned, "Summary:\nThis is the summary content.");
    }

    #[test]
    fn cleanup_both_tags() {
        let raw = "<analysis>Analysis here.</analysis>\n\n<summary>Summary here.</summary>";
        let cleaned = cleanup_summary_tags(raw);
        assert!(cleaned.contains("Analysis:\nAnalysis here."));
        assert!(cleaned.contains("Summary:\nSummary here."));
    }

    #[test]
    fn cleanup_collapses_newlines() {
        let raw = "Line 1\n\n\n\nLine 2";
        let cleaned = cleanup_summary_tags(raw);
        assert_eq!(cleaned, "Line 1\n\nLine 2");
    }

    #[test]
    fn create_summary_message_structure() {
        let summary = "Test summary";
        let msg = create_summary_message(summary, false);

        match msg {
            ResponseItem::Message { id, role, content } => {
                assert_eq!(id, Some("compact_summary".to_string()));
                assert_eq!(role, "user");
                assert_eq!(content.len(), 1);
                if let ContentItem::InputText { text } = &content[0] {
                    assert!(text.starts_with(SUMMARY_PREFIX_V2));
                } else {
                    panic!("expected InputText");
                }
            }
            _ => panic!("expected Message"),
        }
    }

    #[test]
    fn extract_summary_with_tags() {
        let response = "Some preamble.\n<summary>The actual summary content.</summary>\nPostamble.";
        let extracted = extract_summary_text(response);
        assert_eq!(extracted, "The actual summary content.");
    }

    #[test]
    fn extract_summary_without_tags() {
        let response = "Just plain text summary without tags.";
        let extracted = extract_summary_text(response);
        assert_eq!(extracted, response);
    }

    #[test]
    fn is_valid_summary_checks() {
        assert!(!is_valid_summary(""));
        assert!(!is_valid_summary("API_ERROR: rate limited"));
        assert!(!is_valid_summary("Error: something went wrong"));
        // Summaries must be > 200 chars to be meaningful
        assert!(!is_valid_summary(
            "This is a short summary that doesn't have enough content."
        ));
        // Valid summary with enough content (> 200 chars)
        let valid_summary = "This is a comprehensive summary of the conversation. The user was working on implementing a new feature for their Rust application. They asked about error handling patterns and we discussed using Result types with custom error enums. The implementation involved several files and we made good progress.";
        assert!(is_valid_summary(valid_summary));
        assert!(valid_summary.len() > MIN_SUMMARY_LENGTH);
    }
}
