//! Context analyzer module for token calculation and breakdown

use codex_protocol::models::{
    ContentItem, FunctionCallOutputPayload, LocalShellAction, ReasoningItemContent,
    ReasoningItemReasoningSummary, ResponseItem, WebSearchAction,
};
use serde::{Deserialize, Serialize};

/// Breakdown of token counts for different parts of a conversation context
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContextBreakdown {
    /// Token count for the system prompt
    pub system_prompt: usize,
    /// Token count for conversation history
    pub conversation: usize,
    /// Token count for tools/functions
    pub tools: usize,
}

impl ContextBreakdown {
    /// Creates a new empty ContextBreakdown
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the total token count across all categories
    pub fn total(&self) -> usize {
        self.system_prompt + self.conversation + self.tools
    }
}

/// Estimates the number of tokens in a given text string
/// This is a simplified estimation - actual tokenization varies by model
/// As a rough heuristic: ~4 characters per token for English text
pub fn estimate_tokens(text: &str) -> usize {
    // More accurate estimation based on common patterns
    // Average English word is ~5 characters, average token is ~0.75 words
    let char_count = text.chars().count();
    let word_count = text.split_whitespace().count();
    
    // Use a weighted average of character and word-based estimates
    // Character-based: ~4 chars per token
    // Word-based: ~0.75 words per token (or 1.33 tokens per word)
    let char_estimate = char_count / 4;
    let word_estimate = (word_count as f64 * 1.33) as usize;
    
    // Return the average of both estimates for better accuracy
    (char_estimate + word_estimate) / 2
}

/// Analyzes conversation context and returns token breakdown
pub fn analyze_context(
    system_prompt: Option<&str>,
    conversation_history: &[ResponseItem],
    tools_definition: Option<&str>,
) -> ContextBreakdown {
    let mut breakdown = ContextBreakdown::new();
    
    // Calculate system prompt tokens
    if let Some(prompt) = system_prompt {
        breakdown.system_prompt = estimate_tokens(prompt);
    }
    
    // Calculate conversation tokens
    breakdown.conversation = calculate_conversation_tokens(conversation_history);
    
    // Calculate tools tokens
    if let Some(tools) = tools_definition {
        breakdown.tools = estimate_tokens(tools);
    }
    
    breakdown
}

/// Calculates the total token count for conversation history
fn calculate_conversation_tokens(history: &[ResponseItem]) -> usize {
    let mut total_tokens = 0;
    
    for item in history {
        total_tokens += calculate_response_item_tokens(item);
    }
    
    total_tokens
}

/// Calculates tokens for a single ResponseItem
fn calculate_response_item_tokens(item: &ResponseItem) -> usize {
    match item {
        ResponseItem::Message { role, content, .. } => {
            let mut tokens = estimate_tokens(role);
            for content_item in content {
                tokens += calculate_content_item_tokens(content_item);
            }
            tokens
        }
        ResponseItem::Reasoning { summary, content, encrypted_content, .. } => {
            let mut tokens = 0;
            
            // Count tokens in summary
            for summary_item in summary {
                match summary_item {
                    ReasoningItemReasoningSummary::SummaryText { text } => {
                        tokens += estimate_tokens(text);
                    }
                }
            }
            
            // Count tokens in content if present
            if let Some(ref content_items) = content {
                for content_item in content_items {
                    match content_item {
                        ReasoningItemContent::ReasoningText { text } | 
                        ReasoningItemContent::Text { text } => {
                            tokens += estimate_tokens(text);
                        }
                    }
                }
            }
            
            // Count tokens in encrypted content if present
            if let Some(ref encrypted) = encrypted_content {
                // Encrypted content is base64, so we estimate based on decoded size
                // Base64 increases size by ~33%, so we scale down
                tokens += (encrypted.len() * 3 / 4) / 4;
            }
            
            tokens
        }
        ResponseItem::FunctionCall { name, arguments, call_id, .. } => {
            estimate_tokens(name) + estimate_tokens(arguments) + estimate_tokens(call_id)
        }
        ResponseItem::FunctionCallOutput { call_id, output } => {
            // The FunctionCallOutputPayload contains content and optional success flag
            estimate_tokens(call_id) + estimate_tokens(&output.content)
        }
        ResponseItem::CustomToolCall { name, input, call_id, .. } => {
            estimate_tokens(name) + estimate_tokens(input) + estimate_tokens(call_id)
        }
        ResponseItem::CustomToolCallOutput { call_id, output } => {
            estimate_tokens(call_id) + estimate_tokens(output)
        }
        ResponseItem::LocalShellCall { action, .. } => {
            // Estimate based on the action
            match action {
                LocalShellAction::Exec(exec_action) => {
                    let mut tokens = 0;
                    // Count tokens for command
                    for cmd_part in &exec_action.command {
                        tokens += estimate_tokens(cmd_part);
                    }
                    // Add some tokens for other fields if present
                    if let Some(ref wd) = exec_action.working_directory {
                        tokens += estimate_tokens(wd);
                    }
                    if let Some(ref user) = exec_action.user {
                        tokens += estimate_tokens(user);
                    }
                    tokens
                }
                LocalShellAction::Run { command } => {
                    estimate_tokens(command)
                }
                LocalShellAction::Output { stdout, stderr } => {
                    estimate_tokens(stdout) + estimate_tokens(stderr)
                }
            }
        }
        ResponseItem::WebSearchCall { action, .. } => {
            match action {
                WebSearchAction::Search { query } => estimate_tokens(query),
                WebSearchAction::Other => 10, // Small fixed estimate
            }
        }
        ResponseItem::Other => 0,
    }
}

/// Calculates tokens for a ContentItem
fn calculate_content_item_tokens(item: &ContentItem) -> usize {
    match item {
        ContentItem::InputText { text } | ContentItem::OutputText { text } => {
            estimate_tokens(text)
        }
        ContentItem::InputImage { image_url } => {
            // Images typically use a fixed token count in vision models
            // Using a conservative estimate of 85 tokens per image
            // (actual varies by model and image size)
            if image_url.starts_with("data:") {
                // Base64 encoded image - larger token count
                170
            } else {
                // URL reference - smaller token count
                85
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_protocol::models::{ContentItem, ResponseItem};

    #[test]
    fn test_estimate_tokens() {
        // Test basic token estimation
        assert!(estimate_tokens("Hello world") > 0);
        assert!(estimate_tokens("") == 0);
        
        // Longer text should have more tokens
        let short_text = "Hello";
        let long_text = "Hello world, this is a longer piece of text for testing";
        assert!(estimate_tokens(long_text) > estimate_tokens(short_text));
    }

    #[test]
    fn test_context_breakdown_total() {
        let mut breakdown = ContextBreakdown::new();
        breakdown.system_prompt = 100;
        breakdown.conversation = 200;
        breakdown.tools = 50;
        
        assert_eq!(breakdown.total(), 350);
    }

    #[test]
    fn test_analyze_context_empty() {
        let breakdown = analyze_context(None, &[], None);
        assert_eq!(breakdown.total(), 0);
    }

    #[test]
    fn test_analyze_context_with_system_prompt() {
        let prompt = "You are a helpful assistant that answers questions concisely.";
        let breakdown = analyze_context(Some(prompt), &[], None);
        
        assert!(breakdown.system_prompt > 0);
        assert_eq!(breakdown.conversation, 0);
        assert_eq!(breakdown.tools, 0);
    }

    #[test]
    fn test_analyze_context_with_conversation() {
        let history = vec![
            ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputText {
                    text: "What is the capital of France?".to_string(),
                }],
            },
            ResponseItem::Message {
                id: None,
                role: "assistant".to_string(),
                content: vec![ContentItem::OutputText {
                    text: "The capital of France is Paris.".to_string(),
                }],
            },
        ];
        
        let breakdown = analyze_context(None, &history, None);
        
        assert_eq!(breakdown.system_prompt, 0);
        assert!(breakdown.conversation > 0);
        assert_eq!(breakdown.tools, 0);
    }

    #[test]
    fn test_analyze_context_full() {
        let prompt = "You are a helpful assistant.";
        let tools = r#"{"tools": [{"name": "search", "description": "Search the web"}]}"#;
        let history = vec![
            ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputText {
                    text: "Hello".to_string(),
                }],
            },
        ];
        
        let breakdown = analyze_context(Some(prompt), &history, Some(tools));
        
        assert!(breakdown.system_prompt > 0);
        assert!(breakdown.conversation > 0);
        assert!(breakdown.tools > 0);
        assert!(breakdown.total() > 0);
    }

    #[test]
    fn test_content_item_image_tokens() {
        let url_image = ContentItem::InputImage {
            image_url: "https://example.com/image.jpg".to_string(),
        };
        let base64_image = ContentItem::InputImage {
            image_url: "data:image/jpeg;base64,/9j/4AAQ...".to_string(),
        };
        
        let url_tokens = calculate_content_item_tokens(&url_image);
        let base64_tokens = calculate_content_item_tokens(&base64_image);
        
        assert_eq!(url_tokens, 85);
        assert_eq!(base64_tokens, 170);
    }
}