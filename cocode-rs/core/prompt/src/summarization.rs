//! Summarization prompts for context compaction.
//!
//! Provides prompt construction for full compaction and micro-compaction,
//! and parsing of structured summary responses.

use crate::templates;

/// Parsed summary response with extracted sections.
#[derive(Debug, Clone, Default)]
pub struct ParsedSummary {
    /// The main summary content.
    pub summary: String,
    /// Optional analysis section.
    pub analysis: Option<String>,
}

/// Build a summarization prompt for full context compaction.
///
/// Returns `(system_prompt, user_prompt)` for the summarization request.
pub fn build_summarization_prompt(
    conversation_summary: &str,
    custom_instructions: Option<&str>,
) -> (String, String) {
    let mut system = templates::SUMMARIZATION.to_string();

    if let Some(instructions) = custom_instructions {
        system.push_str("\n\n## Additional Instructions\n\n");
        system.push_str(instructions);
    }

    let user = format!(
        "Please summarize the following conversation:\n\n---\n\n{conversation_summary}\n\n---\n\nProvide your summary using the required section format."
    );

    (system, user)
}

/// Build a brief summarization prompt for micro-compaction.
///
/// Returns `(system_prompt, user_prompt)` for a shorter summary.
pub fn build_brief_summary_prompt(conversation_text: &str) -> (String, String) {
    let system = "You are a conversation summarizer. Provide a brief, actionable summary \
                  of the conversation so far. Focus on: what was done, what files were \
                  changed, and what remains to be done. Be concise (2-4 sentences)."
        .to_string();

    let user = format!("Briefly summarize this conversation:\n\n---\n\n{conversation_text}\n\n---");

    (system, user)
}

/// Parse a summary response, extracting `<summary>` and `<analysis>` tags.
pub fn parse_summary_response(response: &str) -> ParsedSummary {
    let summary = extract_tag(response, "summary").unwrap_or_else(|| response.to_string());
    let analysis = extract_tag(response, "analysis");

    ParsedSummary { summary, analysis }
}

/// Extract content between `<tag>` and `</tag>`.
fn extract_tag(text: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");

    let start = text.find(&open)?;
    let end = text.find(&close)?;

    if end <= start {
        return None;
    }

    let content = &text[start + open.len()..end];
    Some(content.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_summarization_prompt() {
        let (system, user) = build_summarization_prompt("User asked to fix a bug", None);

        assert!(!system.is_empty());
        assert!(user.contains("fix a bug"));
        assert!(system.contains("Conversation Summarization"));
    }

    #[test]
    fn test_build_summarization_prompt_with_instructions() {
        let (system, _user) =
            build_summarization_prompt("conversation text", Some("Focus on Rust code"));

        assert!(system.contains("Focus on Rust code"));
        assert!(system.contains("Additional Instructions"));
    }

    #[test]
    fn test_build_brief_summary_prompt() {
        let (system, user) = build_brief_summary_prompt("some conversation");

        assert!(system.contains("brief"));
        assert!(user.contains("some conversation"));
    }

    #[test]
    fn test_parse_summary_response_with_tags() {
        let response = r#"
Here is the summary:

<summary>
The user asked to implement two new crates for context management.
Files were created in core/context/ and core/prompt/.
</summary>

<analysis>
The conversation was productive. All tasks were completed.
</analysis>
"#;

        let parsed = parse_summary_response(response);
        assert!(parsed.summary.contains("two new crates"));
        assert!(parsed.analysis.is_some());
        assert!(parsed.analysis.as_deref().unwrap().contains("productive"));
    }

    #[test]
    fn test_parse_summary_response_no_tags() {
        let response = "This is a plain summary without any tags.";
        let parsed = parse_summary_response(response);
        assert_eq!(parsed.summary, response);
        assert!(parsed.analysis.is_none());
    }

    #[test]
    fn test_parse_summary_response_partial_tags() {
        let response = "<summary>Only summary here</summary>";
        let parsed = parse_summary_response(response);
        assert_eq!(parsed.summary, "Only summary here");
        assert!(parsed.analysis.is_none());
    }

    #[test]
    fn test_extract_tag() {
        assert_eq!(
            extract_tag("<foo>bar</foo>", "foo"),
            Some("bar".to_string())
        );
        assert_eq!(extract_tag("no tags here", "foo"), None);
        assert_eq!(
            extract_tag("<foo>  spaced  </foo>", "foo"),
            Some("spaced".to_string())
        );
    }
}
