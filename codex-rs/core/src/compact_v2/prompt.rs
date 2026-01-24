//! Summarization prompt generation.
//!
//! Generates structured prompts for LLM-based conversation summarization.

/// System prompt for summarization LLM call.
///
/// Matches Claude Code's summarization system prompt.
pub const SUMMARIZATION_SYSTEM_PROMPT: &str =
    "You are a helpful AI assistant tasked with summarizing conversations.";

/// Template for the user prompt requesting summarization.
///
/// Instructs the LLM to produce a structured 9-section summary.
const SUMMARIZATION_PROMPT_TEMPLATE: &str = r#"Your task is to create a detailed summary of the conversation so far, paying close attention to the user's explicit requests and your previous actions.

This summary is critical - it will be used to maintain continuity in an ongoing conversation.
If previous summaries exist, merge them, adding any additional context from recent messages.

Wrap your analysis in <analysis> tags, reviewing each message chronologically:
- User's explicit requests and intents
- Your (Claude's) approach to addressing requests
- Key decisions, technical concepts, code patterns
- Specific details (file names, code snippets, function signatures)
- Errors encountered and fixes applied
- User feedback (especially corrections)

Then provide your summary in <summary> tags with these sections:

1. **Primary Request and Intent**
   What has the user explicitly asked for? Include specific, quoted phrases to capture intent. (This is the most important section)

2. **Key Technical Concepts**
   What technologies, frameworks, or concepts are being discussed or used?

3. **Files and Code Sections**
   What files have been examined, modified, or need attention? Include brief code snippets or signatures where relevant.

4. **Errors and Fixes**
   What problems have been encountered and what solutions were applied?

5. **Problem Solving**
   Document your troubleshooting approach: hypotheses, tests, and outcomes.

6. **All User Messages**
   Include all non-tool-result user messages. This is critical for maintaining user intent across context boundaries.

7. **Pending Tasks**
   What outstanding work items remain? List specifically.

8. **Current Work**
   What was being worked on immediately before this summary? Include specific details.

9. **Optional Next Step** (if applicable)
   What would be the logical next step to continue this conversation? Include verbatim quotes from the conversation to prevent task drift.

IMPORTANT:
- Be VERY THOROUGH - include specific file names, function names, code snippets
- Use verbatim quotes from the conversation to prevent task drift
- Preserve all technical details that would be needed to continue the work
- If there are multiple related tasks, clearly delineate them
"#;

/// Generate the summarization prompt.
///
/// # Arguments
///
/// * `custom_instructions` - Optional additional instructions from PreCompact hooks
///
/// # Returns
///
/// The complete prompt string to send to the LLM
pub fn generate_summarization_prompt(custom_instructions: Option<&str>) -> String {
    let mut prompt = SUMMARIZATION_PROMPT_TEMPLATE.to_string();

    if let Some(instructions) = custom_instructions {
        prompt.push_str("\n\nAdditional instructions:\n");
        prompt.push_str(instructions);
    }

    prompt
}

/// Generate a shorter summarization prompt for micro-compaction.
///
/// Used when only a brief summary is needed (e.g., for progress tracking).
#[allow(dead_code)] // Reserved for micro-compact brief summaries
pub fn generate_brief_prompt() -> &'static str {
    r#"Provide a brief 2-3 sentence summary of the key points from this conversation.
Focus on: current task, key decisions made, and any pending items."#
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_prompt_is_defined() {
        assert!(!SUMMARIZATION_SYSTEM_PROMPT.is_empty());
        assert!(SUMMARIZATION_SYSTEM_PROMPT.contains("summarizing conversations"));
    }

    #[test]
    fn generate_prompt_without_custom() {
        let prompt = generate_summarization_prompt(None);
        assert!(prompt.contains("Primary Request and Intent"));
        assert!(prompt.contains("<analysis>"));
        assert!(prompt.contains("<summary>"));
        // Check for 9 numbered sections (1-9)
        assert!(prompt.contains("1. **Primary Request"));
        assert!(prompt.contains("9. **Optional Next Step"));
        assert!(!prompt.contains("Additional instructions"));
    }

    #[test]
    fn generate_prompt_with_custom() {
        let custom = "Focus especially on error handling code.";
        let prompt = generate_summarization_prompt(Some(custom));
        assert!(prompt.contains("Primary Request and Intent"));
        assert!(prompt.contains("Additional instructions:"));
        assert!(prompt.contains(custom));
    }

    #[test]
    fn brief_prompt_is_concise() {
        let prompt = generate_brief_prompt();
        assert!(prompt.contains("brief"));
        assert!(prompt.len() < 300);
    }

    #[test]
    fn prompt_contains_all_sections() {
        let prompt = generate_summarization_prompt(None);
        assert!(prompt.contains("Primary Request and Intent"));
        assert!(prompt.contains("Key Technical Concepts"));
        assert!(prompt.contains("Files and Code Sections"));
        assert!(prompt.contains("Errors and Fixes"));
        assert!(prompt.contains("Problem Solving"));
        assert!(prompt.contains("All User Messages"));
        assert!(prompt.contains("Pending Tasks"));
        assert!(prompt.contains("Current Work"));
        assert!(prompt.contains("Optional Next Step"));
    }

    #[test]
    fn prompt_emphasizes_verbatim_quotes() {
        let prompt = generate_summarization_prompt(None);
        assert!(prompt.contains("verbatim quotes"));
        assert!(prompt.contains("prevent task drift"));
    }
}
