//! Context analyzer module for token calculation and breakdown

use codex_protocol::models::{
    ContentItem, LocalShellAction, ReasoningItemContent, ReasoningItemReasoningSummary,
    ResponseItem, WebSearchAction,
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

/// Model family for token estimation adjustments
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ModelFamily {
    Claude,
    GPT4,
    GPT35,
    Unknown,
}

/// Get model family from model name
pub fn get_model_family(model: &str) -> ModelFamily {
    if model.starts_with("claude-") {
        ModelFamily::Claude
    } else if model.starts_with("gpt-4") {
        ModelFamily::GPT4
    } else if model.starts_with("gpt-3.5") {
        ModelFamily::GPT35
    } else {
        ModelFamily::Unknown
    }
}

/// Estimates the number of tokens in a given text string
/// This estimation varies by model family for better accuracy
pub fn estimate_tokens(text: &str) -> usize {
    estimate_tokens_for_model(text, ModelFamily::Unknown)
}

/// Estimates tokens with model-specific adjustments
pub fn estimate_tokens_for_model(text: &str, model_family: ModelFamily) -> usize {
    if text.is_empty() {
        return 0;
    }

    let char_count = text.chars().count();
    let word_count = text.split_whitespace().count();

    // Model-specific token ratios based on empirical observations
    let (chars_per_token, words_per_token) = match model_family {
        ModelFamily::Claude => (3.8, 0.72), // Claude tokenizer is more efficient
        ModelFamily::GPT4 => (3.5, 0.7),    // Standard GPT-4 tokenization
        ModelFamily::GPT35 => (3.3, 0.68),  // GPT-3.5 less efficient
        ModelFamily::Unknown => (3.5, 0.7), // Conservative default
    };

    // Calculate estimates
    let char_estimate = (char_count as f64 / chars_per_token) as usize;
    let word_estimate = (word_count as f64 / words_per_token) as usize;

    // Special handling for code vs natural language
    let has_code_indicators = text.contains('{')
        || text.contains('}')
        || text.contains("function")
        || text.contains("def")
        || text.contains("```");

    if has_code_indicators {
        // Code typically has more tokens per character
        let code_multiplier = match model_family {
            ModelFamily::Claude => 1.15,
            ModelFamily::GPT4 => 1.2,
            ModelFamily::GPT35 => 1.25,
            ModelFamily::Unknown => 1.2,
        };
        ((char_estimate + word_estimate) as f64 / 2.0 * code_multiplier) as usize
    } else {
        // Natural language - use weighted average
        (char_estimate * 2 + word_estimate) / 3
    }
}

/// Analyzes conversation context and returns token breakdown
pub fn analyze_context(
    system_prompt: Option<&str>,
    conversation_history: &[ResponseItem],
    tools_definition: Option<&str>,
) -> ContextBreakdown {
    analyze_context_with_model(
        system_prompt,
        conversation_history,
        tools_definition,
        ModelFamily::Unknown,
    )
}

/// Analyzes conversation context with model-specific token estimation
pub fn analyze_context_with_model(
    system_prompt: Option<&str>,
    conversation_history: &[ResponseItem],
    tools_definition: Option<&str>,
    model_family: ModelFamily,
) -> ContextBreakdown {
    let mut breakdown = ContextBreakdown::new();

    // Calculate system prompt tokens
    if let Some(prompt) = system_prompt {
        breakdown.system_prompt = estimate_tokens_for_model(prompt, model_family);
    }

    // Calculate conversation tokens
    breakdown.conversation =
        calculate_conversation_tokens_with_model(conversation_history, model_family);

    // Calculate tools tokens
    if let Some(tools) = tools_definition {
        breakdown.tools = estimate_tokens_for_model(tools, model_family);
    }

    breakdown
}

/// Calculates the total token count for conversation history with model-specific estimation
fn calculate_conversation_tokens_with_model(
    history: &[ResponseItem],
    model_family: ModelFamily,
) -> usize {
    let mut total_tokens = 0;

    for item in history {
        total_tokens += calculate_response_item_tokens_with_model(item, model_family);
    }

    total_tokens
}

/// Calculates tokens for a single ResponseItem with model-specific estimation
fn calculate_response_item_tokens_with_model(
    item: &ResponseItem,
    model_family: ModelFamily,
) -> usize {
    match item {
        ResponseItem::Message { role, content, .. } => {
            let mut tokens = estimate_tokens_for_model(role, model_family);
            for content_item in content {
                tokens += calculate_content_item_tokens_with_model(content_item, model_family);
            }
            tokens
        }
        ResponseItem::Reasoning {
            summary,
            content,
            encrypted_content,
            ..
        } => {
            let mut tokens = 0;

            // Count tokens in summary
            for summary_item in summary {
                match summary_item {
                    ReasoningItemReasoningSummary::SummaryText { text } => {
                        tokens += estimate_tokens_for_model(text, model_family);
                    }
                }
            }

            // Count tokens in content if present
            if let Some(content_items) = content {
                for content_item in content_items {
                    match content_item {
                        ReasoningItemContent::ReasoningText { text }
                        | ReasoningItemContent::Text { text } => {
                            tokens += estimate_tokens_for_model(text, model_family);
                        }
                    }
                }
            }

            // Count tokens in encrypted content if present
            if let Some(encrypted) = encrypted_content {
                // Encrypted content is base64, so we estimate based on decoded size
                // Base64 increases size by ~33%, so we scale down
                tokens += (encrypted.len() * 3 / 4) / 4;
            }

            tokens
        }
        ResponseItem::FunctionCall {
            name,
            arguments,
            call_id,
            ..
        } => {
            estimate_tokens_for_model(name, model_family)
                + estimate_tokens_for_model(arguments, model_family)
                + estimate_tokens_for_model(call_id, model_family)
        }
        ResponseItem::FunctionCallOutput { call_id, output } => {
            // The FunctionCallOutputPayload contains content and optional success flag
            estimate_tokens_for_model(call_id, model_family)
                + estimate_tokens_for_model(&output.content, model_family)
        }
        ResponseItem::CustomToolCall {
            name,
            input,
            call_id,
            ..
        } => {
            estimate_tokens_for_model(name, model_family)
                + estimate_tokens_for_model(input, model_family)
                + estimate_tokens_for_model(call_id, model_family)
        }
        ResponseItem::CustomToolCallOutput { call_id, output } => {
            estimate_tokens_for_model(call_id, model_family)
                + estimate_tokens_for_model(output, model_family)
        }
        ResponseItem::LocalShellCall {
            action, call_id, ..
        } => {
            // Estimate based on the action
            let mut tokens = 0;

            // Count tokens for call_id if present
            if let Some(cid) = call_id {
                tokens += estimate_tokens_for_model(cid, model_family);
            }

            match action {
                LocalShellAction::Exec(exec_action) => {
                    // Count tokens for command
                    for cmd_part in &exec_action.command {
                        tokens += estimate_tokens_for_model(cmd_part, model_family);
                    }
                    // Add some tokens for other fields if present
                    if let Some(wd) = &exec_action.working_directory {
                        tokens += estimate_tokens_for_model(wd, model_family);
                    }
                    if let Some(user) = &exec_action.user {
                        tokens += estimate_tokens_for_model(user, model_family);
                    }
                    tokens
                }
            }
        }
        ResponseItem::WebSearchCall { action, .. } => {
            match action {
                WebSearchAction::Search { query } => estimate_tokens_for_model(query, model_family),
                WebSearchAction::Other => 10, // Small fixed estimate
            }
        }
        ResponseItem::Other => 0,
    }
}

/// Calculates tokens for a ContentItem with model-specific estimation
fn calculate_content_item_tokens_with_model(
    item: &ContentItem,
    model_family: ModelFamily,
) -> usize {
    match item {
        ContentItem::InputText { text } | ContentItem::OutputText { text } => {
            estimate_tokens_for_model(text, model_family)
        }
        ContentItem::InputImage { image_url } => {
            // Images typically use a fixed token count in vision models
            // Token count varies by model
            let base_tokens = match model_family {
                ModelFamily::Claude => 65, // Claude uses fewer tokens for images
                ModelFamily::GPT4 => 85,   // GPT-4 vision standard
                ModelFamily::GPT35 => 85,  // GPT-3.5 doesn't support images but use same estimate
                ModelFamily::Unknown => 85,
            };

            if image_url.starts_with("data:") {
                // Base64 encoded image - double the token count
                base_tokens * 2
            } else {
                // URL reference - standard token count
                base_tokens
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
        let history = vec![ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "Hello".to_string(),
            }],
        }];

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

        // Test with different model families
        let claude_url = calculate_content_item_tokens_with_model(&url_image, ModelFamily::Claude);
        let claude_base64 =
            calculate_content_item_tokens_with_model(&base64_image, ModelFamily::Claude);
        assert_eq!(claude_url, 65);
        assert_eq!(claude_base64, 130);

        let gpt4_url = calculate_content_item_tokens_with_model(&url_image, ModelFamily::GPT4);
        let gpt4_base64 =
            calculate_content_item_tokens_with_model(&base64_image, ModelFamily::GPT4);
        assert_eq!(gpt4_url, 85);
        assert_eq!(gpt4_base64, 170);
    }

    #[test]
    fn test_model_family_detection() {
        assert_eq!(get_model_family("claude-3-5-sonnet"), ModelFamily::Claude);
        assert_eq!(get_model_family("claude-3-opus"), ModelFamily::Claude);
        assert_eq!(get_model_family("gpt-4o"), ModelFamily::GPT4);
        assert_eq!(get_model_family("gpt-4-turbo"), ModelFamily::GPT4);
        assert_eq!(get_model_family("gpt-3.5-turbo"), ModelFamily::GPT35);
        assert_eq!(get_model_family("unknown-model"), ModelFamily::Unknown);
    }

    #[test]
    fn test_model_specific_token_estimation() {
        let text = "This is a test sentence with multiple words.";
        let code = "function test() { return 42; }";

        // Claude should estimate fewer tokens
        let claude_text = estimate_tokens_for_model(text, ModelFamily::Claude);
        let gpt4_text = estimate_tokens_for_model(text, ModelFamily::GPT4);
        assert!(claude_text <= gpt4_text);

        // Code should have more tokens than plain text of similar length
        let claude_code = estimate_tokens_for_model(code, ModelFamily::Claude);
        let claude_text_similar =
            estimate_tokens_for_model("This is approximately the same length", ModelFamily::Claude);
        assert!(claude_code >= claude_text_similar);
    }

    #[test]
    fn test_empty_text_with_model_families() {
        // Test empty text returns 0 for all model families
        assert_eq!(estimate_tokens_for_model("", ModelFamily::Claude), 0);
        assert_eq!(estimate_tokens_for_model("", ModelFamily::GPT4), 0);
        assert_eq!(estimate_tokens_for_model("", ModelFamily::GPT35), 0);
        assert_eq!(estimate_tokens_for_model("", ModelFamily::Unknown), 0);
    }

    #[test]
    fn test_code_detection_various_indicators() {
        // Test different code indicators
        let code_with_braces = "const obj = { key: 'value' };";
        let code_with_function = "function getName() { return name; }";
        let code_with_def = "def calculate(x): return x * 2";
        let code_with_markdown = "```python\nprint('hello')\n```";

        // All should be detected as code and have multiplier applied
        let tokens_braces = estimate_tokens_for_model(code_with_braces, ModelFamily::GPT4);
        let tokens_plain =
            estimate_tokens_for_model("const obj equals key value semicolon", ModelFamily::GPT4);
        assert!(tokens_braces > tokens_plain);

        assert!(estimate_tokens_for_model(code_with_function, ModelFamily::Claude) > 0);
        assert!(estimate_tokens_for_model(code_with_def, ModelFamily::GPT35) > 0);
        assert!(estimate_tokens_for_model(code_with_markdown, ModelFamily::Unknown) > 0);
    }

    #[test]
    fn test_reasoning_response_item() {
        use codex_protocol::models::{ReasoningItemContent, ReasoningItemReasoningSummary};

        // Test with summary only
        let reasoning_summary = ResponseItem::Reasoning {
            id: "reason_1".to_string(),
            summary: vec![ReasoningItemReasoningSummary::SummaryText {
                text: "Analyzing the request".to_string(),
            }],
            content: None,
            encrypted_content: None,
        };

        let tokens =
            calculate_response_item_tokens_with_model(&reasoning_summary, ModelFamily::Claude);
        assert!(tokens > 0);

        // Test with content
        let reasoning_with_content = ResponseItem::Reasoning {
            id: "reason_2".to_string(),
            summary: vec![ReasoningItemReasoningSummary::SummaryText {
                text: "Summary".to_string(),
            }],
            content: Some(vec![
                ReasoningItemContent::ReasoningText {
                    text: "Detailed reasoning here".to_string(),
                },
                ReasoningItemContent::Text {
                    text: "More text".to_string(),
                },
            ]),
            encrypted_content: None,
        };

        let tokens_content =
            calculate_response_item_tokens_with_model(&reasoning_with_content, ModelFamily::GPT4);
        assert!(tokens_content > tokens);

        // Test with encrypted content
        let reasoning_encrypted = ResponseItem::Reasoning {
            id: "reason_3".to_string(),
            summary: vec![],
            content: None,
            encrypted_content: Some("aGVsbG8gd29ybGQ=".to_string()), // base64
        };

        let tokens_encrypted =
            calculate_response_item_tokens_with_model(&reasoning_encrypted, ModelFamily::Unknown);
        assert!(tokens_encrypted > 0);
    }

    #[test]
    fn test_function_call_response_items() {
        use codex_protocol::models::FunctionCallOutputPayload;

        // Test FunctionCall
        let function_call = ResponseItem::FunctionCall {
            id: None,
            name: "search_web".to_string(),
            arguments: r#"{"query": "rust programming"}"#.to_string(),
            call_id: "call_123".to_string(),
        };

        let tokens = calculate_response_item_tokens_with_model(&function_call, ModelFamily::Claude);
        assert!(tokens > 0);

        // Test FunctionCallOutput
        let function_output = ResponseItem::FunctionCallOutput {
            call_id: "call_123".to_string(),
            output: FunctionCallOutputPayload {
                content: "Search results: Rust is a systems programming language".to_string(),
                success: Some(true),
            },
        };

        let output_tokens =
            calculate_response_item_tokens_with_model(&function_output, ModelFamily::GPT4);
        assert!(output_tokens > 0);
    }

    #[test]
    fn test_custom_tool_call_response_items() {
        // Test CustomToolCall
        let tool_call = ResponseItem::CustomToolCall {
            id: None,
            status: Some("completed".to_string()),
            name: "calculator".to_string(),
            input: r#"{"operation": "add", "a": 5, "b": 3}"#.to_string(),
            call_id: "tool_456".to_string(),
        };

        let tokens = calculate_response_item_tokens_with_model(&tool_call, ModelFamily::GPT35);
        assert!(tokens > 0);

        // Test CustomToolCallOutput
        let tool_output = ResponseItem::CustomToolCallOutput {
            call_id: "tool_456".to_string(),
            output: "8".to_string(),
        };

        let output_tokens =
            calculate_response_item_tokens_with_model(&tool_output, ModelFamily::Unknown);
        assert!(output_tokens > 0);
    }

    #[test]
    fn test_local_shell_call_response_item() {
        use codex_protocol::models::{LocalShellAction, LocalShellExecAction, LocalShellStatus};

        // Test with minimal exec action
        let shell_call_minimal = ResponseItem::LocalShellCall {
            id: None,
            call_id: Some("shell_1".to_string()),
            status: LocalShellStatus::Completed,
            action: LocalShellAction::Exec(LocalShellExecAction {
                command: vec!["ls".to_string(), "-la".to_string()],
                working_directory: None,
                user: None,
                timeout_ms: None,
                env: None,
            }),
        };

        let tokens_minimal =
            calculate_response_item_tokens_with_model(&shell_call_minimal, ModelFamily::Claude);
        assert!(tokens_minimal > 0);

        // Test with all fields
        let shell_call_full = ResponseItem::LocalShellCall {
            id: None,
            call_id: Some("shell_2".to_string()),
            status: LocalShellStatus::InProgress,
            action: LocalShellAction::Exec(LocalShellExecAction {
                command: vec!["echo".to_string(), "hello world".to_string()],
                working_directory: Some("/home/user/projects".to_string()),
                user: Some("developer".to_string()),
                timeout_ms: Some(5000),
                env: Some(std::collections::HashMap::new()),
            }),
        };

        let tokens_full =
            calculate_response_item_tokens_with_model(&shell_call_full, ModelFamily::GPT4);
        assert!(tokens_full > tokens_minimal);

        // Test with empty command
        let shell_call_empty = ResponseItem::LocalShellCall {
            id: None,
            call_id: None,
            status: LocalShellStatus::Incomplete,
            action: LocalShellAction::Exec(LocalShellExecAction {
                command: vec![],
                working_directory: None,
                user: None,
                timeout_ms: None,
                env: None,
            }),
        };

        let tokens_empty =
            calculate_response_item_tokens_with_model(&shell_call_empty, ModelFamily::Unknown);
        assert_eq!(tokens_empty, 0);
    }

    #[test]
    fn test_web_search_call_response_item() {
        use codex_protocol::models::WebSearchAction;

        // Test Search action
        let search_call = ResponseItem::WebSearchCall {
            id: None,
            status: Some("completed".to_string()),
            action: WebSearchAction::Search {
                query: "Rust async programming tutorial".to_string(),
            },
        };

        let tokens_search =
            calculate_response_item_tokens_with_model(&search_call, ModelFamily::Claude);
        assert!(tokens_search > 0);

        // Test Other action
        let other_call = ResponseItem::WebSearchCall {
            id: None,
            status: None,
            action: WebSearchAction::Other,
        };

        let tokens_other =
            calculate_response_item_tokens_with_model(&other_call, ModelFamily::GPT35);
        assert_eq!(tokens_other, 10); // Fixed value for Other
    }

    #[test]
    fn test_other_response_item() {
        let other = ResponseItem::Other;
        assert_eq!(
            calculate_response_item_tokens_with_model(&other, ModelFamily::Claude),
            0
        );
        assert_eq!(
            calculate_response_item_tokens_with_model(&other, ModelFamily::GPT4),
            0
        );
    }

    #[test]
    fn test_message_with_empty_content() {
        let empty_message = ResponseItem::Message {
            id: Some("msg_123".to_string()),
            role: "assistant".to_string(),
            content: vec![],
        };

        let tokens = calculate_response_item_tokens_with_model(&empty_message, ModelFamily::Claude);
        assert!(tokens > 0); // Should count role tokens
    }

    #[test]
    fn test_message_with_mixed_content() {
        let mixed_message = ResponseItem::Message {
            id: Some("msg_456".to_string()),
            role: "user".to_string(),
            content: vec![
                ContentItem::InputText {
                    text: "Here is an image:".to_string(),
                },
                ContentItem::InputImage {
                    image_url: "https://example.com/diagram.png".to_string(),
                },
                ContentItem::InputText {
                    text: "What do you see?".to_string(),
                },
            ],
        };

        let tokens = calculate_response_item_tokens_with_model(&mixed_message, ModelFamily::GPT4);
        assert!(tokens > 90); // Text + image tokens (85 for image + text)
    }

    #[test]
    fn test_analyze_context_with_model_directly() {
        let prompt = "Be concise.";
        let history = vec![ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "Hi".to_string(),
            }],
        }];
        let tools = r#"{"tool": "test"}"#;

        // Test with different model families
        let breakdown_claude =
            analyze_context_with_model(Some(prompt), &history, Some(tools), ModelFamily::Claude);
        let breakdown_gpt4 =
            analyze_context_with_model(Some(prompt), &history, Some(tools), ModelFamily::GPT4);

        // Different models should produce different token counts
        assert!(breakdown_claude.total() > 0);
        assert!(breakdown_gpt4.total() > 0);
        // Claude is more efficient, so should have fewer tokens
        assert!(breakdown_claude.total() <= breakdown_gpt4.total());
    }

    #[test]
    fn test_complex_conversation_with_all_response_types() {
        use codex_protocol::models::*;

        let complex_history = vec![
            ResponseItem::Message {
                id: Some("1".to_string()),
                role: "user".to_string(),
                content: vec![ContentItem::InputText {
                    text: "Analyze this".to_string(),
                }],
            },
            ResponseItem::Reasoning {
                id: "2".to_string(),
                summary: vec![ReasoningItemReasoningSummary::SummaryText {
                    text: "Thinking...".to_string(),
                }],
                content: Some(vec![ReasoningItemContent::ReasoningText {
                    text: "Complex analysis".to_string(),
                }]),
                encrypted_content: Some("encrypted_data_here".to_string()),
            },
            ResponseItem::FunctionCall {
                id: Some("3".to_string()),
                name: "analyze_data".to_string(),
                arguments: "{}".to_string(),
                call_id: "call_1".to_string(),
            },
            ResponseItem::FunctionCallOutput {
                call_id: "call_1".to_string(),
                output: FunctionCallOutputPayload {
                    content: "Analysis complete".to_string(),
                    success: Some(true),
                },
            },
            ResponseItem::LocalShellCall {
                id: Some("5".to_string()),
                call_id: Some("shell_call".to_string()),
                status: LocalShellStatus::Completed,
                action: LocalShellAction::Exec(LocalShellExecAction {
                    command: vec!["echo".to_string(), "done".to_string()],
                    working_directory: Some("/tmp".to_string()),
                    user: None,
                    timeout_ms: None,
                    env: None,
                }),
            },
            ResponseItem::WebSearchCall {
                id: Some("6".to_string()),
                status: Some("completed".to_string()),
                action: WebSearchAction::Other,
            },
            ResponseItem::Other,
        ];

        let breakdown = analyze_context_with_model(
            Some("System prompt"),
            &complex_history,
            Some("tools"),
            ModelFamily::Claude,
        );

        assert!(breakdown.conversation > 0);
        assert!(breakdown.total() > 20); // With efficient Claude tokenizer and short messages
    }

    #[test]
    fn test_image_tokens_all_model_families() {
        let url_image = ContentItem::InputImage {
            image_url: "https://example.com/test.jpg".to_string(),
        };
        let base64_image = ContentItem::InputImage {
            image_url: "data:image/png;base64,iVBORw0KGgo...".to_string(),
        };

        // Test all model families
        for (model, expected_url, expected_base64) in [
            (ModelFamily::Claude, 65, 130),
            (ModelFamily::GPT4, 85, 170),
            (ModelFamily::GPT35, 85, 170),
            (ModelFamily::Unknown, 85, 170),
        ] {
            assert_eq!(
                calculate_content_item_tokens_with_model(&url_image, model),
                expected_url
            );
            assert_eq!(
                calculate_content_item_tokens_with_model(&base64_image, model),
                expected_base64
            );
        }
    }

    #[test]
    fn test_reasoning_with_empty_summary() {
        use codex_protocol::models::ReasoningItemContent;

        let reasoning = ResponseItem::Reasoning {
            id: "empty_summary".to_string(),
            summary: vec![],
            content: Some(vec![ReasoningItemContent::Text {
                text: "Some reasoning content".to_string(),
            }]),
            encrypted_content: None,
        };

        let tokens = calculate_response_item_tokens_with_model(&reasoning, ModelFamily::Claude);
        assert!(tokens > 0); // Should still count content tokens
    }

    #[test]
    fn test_edge_cases_and_special_characters() {
        // Test with Unicode and special characters
        let unicode_text = "Hello 世界 🌍 Привет мир";
        let tokens_unicode = estimate_tokens_for_model(unicode_text, ModelFamily::Claude);
        assert!(tokens_unicode > 0);

        // Test very long text
        let long_text = "Lorem ipsum ".repeat(1000);
        let tokens_long = estimate_tokens_for_model(&long_text, ModelFamily::GPT4);
        assert!(tokens_long > 1000); // Should have many tokens

        // Test text with only whitespace
        let whitespace = "   \t\n\r   ";
        let tokens_ws = estimate_tokens_for_model(whitespace, ModelFamily::Unknown);
        assert!(tokens_ws > 0);
    }

    #[test]
    fn test_all_code_indicators() {
        // Test each code indicator separately
        let codes = vec![
            ("{ key: value }", ModelFamily::Claude),
            ("function test() { }", ModelFamily::GPT4),
            ("def calculate():", ModelFamily::GPT35),
            ("```rust\nfn main() {}\n```", ModelFamily::Unknown),
        ];

        for (code, model) in codes {
            let tokens = estimate_tokens_for_model(code, model);
            assert!(tokens > 0);

            // Code should have more tokens than similar length plain text
            let plain = "a".repeat(code.len());
            let plain_tokens = estimate_tokens_for_model(&plain, model);
            assert!(tokens >= plain_tokens);
        }
    }

    #[test]
    fn test_custom_tool_call_without_status() {
        let tool_call = ResponseItem::CustomToolCall {
            id: None,
            status: None,
            name: "tool".to_string(),
            input: "{}".to_string(),
            call_id: "id".to_string(),
        };

        let tokens = calculate_response_item_tokens_with_model(&tool_call, ModelFamily::Claude);
        assert!(tokens > 0);
    }

    #[test]
    fn test_web_search_all_variations() {
        use codex_protocol::models::WebSearchAction;

        // Test with empty query
        let empty_search = ResponseItem::WebSearchCall {
            id: Some("search1".to_string()),
            status: None,
            action: WebSearchAction::Search {
                query: "".to_string(),
            },
        };

        let tokens = calculate_response_item_tokens_with_model(&empty_search, ModelFamily::Claude);
        assert_eq!(tokens, 0); // Empty query should be 0

        // Test with long query
        let long_search = ResponseItem::WebSearchCall {
            id: None,
            status: Some("in_progress".to_string()),
            action: WebSearchAction::Search {
                query: "How to implement a distributed consensus algorithm using Raft in Rust with proper error handling and testing strategies for production systems".to_string(),
            },
        };

        let long_tokens =
            calculate_response_item_tokens_with_model(&long_search, ModelFamily::GPT4);
        assert!(long_tokens > 20);
    }

    #[test]
    fn test_gpt35_specific_paths() {
        // Test GPT-3.5 specific code multiplier
        let code = "async function fetchData() { return await fetch(url); }";
        let tokens_gpt35 = estimate_tokens_for_model(code, ModelFamily::GPT35);
        let tokens_gpt4 = estimate_tokens_for_model(code, ModelFamily::GPT4);

        // GPT-3.5 should have different estimation due to different multipliers
        assert!(tokens_gpt35 > 0);
        assert!(tokens_gpt4 > 0);
    }

    #[test]
    fn test_function_call_output_without_success() {
        use codex_protocol::models::FunctionCallOutputPayload;

        let output = ResponseItem::FunctionCallOutput {
            call_id: "test".to_string(),
            output: FunctionCallOutputPayload {
                content: "Result".to_string(),
                success: None,
            },
        };

        let tokens = calculate_response_item_tokens_with_model(&output, ModelFamily::Claude);
        assert!(tokens > 0);
    }

    #[test]
    fn test_local_shell_with_only_call_id() {
        use codex_protocol::models::{LocalShellAction, LocalShellExecAction, LocalShellStatus};

        let shell_call = ResponseItem::LocalShellCall {
            id: Some("id".to_string()),
            call_id: Some("call_only".to_string()),
            status: LocalShellStatus::Completed,
            action: LocalShellAction::Exec(LocalShellExecAction {
                command: vec![],
                working_directory: None,
                user: None,
                timeout_ms: None,
                env: None,
            }),
        };

        let tokens = calculate_response_item_tokens_with_model(&shell_call, ModelFamily::GPT4);
        assert!(tokens > 0); // Should count call_id tokens
    }

    #[test]
    fn test_message_with_id() {
        let message = ResponseItem::Message {
            id: Some("message_with_very_long_id_that_should_count_tokens".to_string()),
            role: "system".to_string(),
            content: vec![],
        };

        let tokens = calculate_response_item_tokens_with_model(&message, ModelFamily::Claude);
        assert!(tokens > 0); // Should count role tokens
    }

    #[test]
    fn test_output_text_content_item() {
        let output_text = ContentItem::OutputText {
            text: "This is an output from the model.".to_string(),
        };

        let tokens = calculate_content_item_tokens_with_model(&output_text, ModelFamily::GPT4);
        assert!(tokens > 0);

        // OutputText should be calculated the same as InputText for same content
        let input_text = ContentItem::InputText {
            text: "This is an output from the model.".to_string(),
        };

        let input_tokens = calculate_content_item_tokens_with_model(&input_text, ModelFamily::GPT4);
        assert_eq!(tokens, input_tokens);
    }

    #[test]
    fn test_context_breakdown_new() {
        let breakdown1 = ContextBreakdown::new();
        let breakdown2 = ContextBreakdown::default();

        // new() should return the same as default()
        assert_eq!(breakdown1.system_prompt, breakdown2.system_prompt);
        assert_eq!(breakdown1.conversation, breakdown2.conversation);
        assert_eq!(breakdown1.tools, breakdown2.tools);
        assert_eq!(breakdown1.total(), 0);
    }

    #[test]
    fn test_get_model_family_edge_cases() {
        // Test models that don't match any pattern
        assert_eq!(get_model_family("mistral-7b"), ModelFamily::Unknown);
        assert_eq!(get_model_family("llama2-70b"), ModelFamily::Unknown);
        assert_eq!(get_model_family(""), ModelFamily::Unknown);
        assert_eq!(get_model_family("gpt-"), ModelFamily::Unknown);
        assert_eq!(get_model_family("claude"), ModelFamily::Unknown); // Missing dash
    }

    #[test]
    fn test_estimate_tokens_wrapper() {
        // Test the public wrapper function
        let text = "Hello world";
        let wrapper_result = estimate_tokens(text);
        let direct_result = estimate_tokens_for_model(text, ModelFamily::Unknown);

        assert_eq!(wrapper_result, direct_result);
    }

    #[test]
    fn test_analyze_context_wrapper() {
        // Test the public wrapper function
        let prompt = "test";
        let history = vec![ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "hi".to_string(),
            }],
        }];
        let tools = "tools";

        let wrapper_result = analyze_context(Some(prompt), &history, Some(tools));
        let direct_result =
            analyze_context_with_model(Some(prompt), &history, Some(tools), ModelFamily::Unknown);

        assert_eq!(wrapper_result.total(), direct_result.total());
    }

    #[test]
    fn test_encrypted_content_calculation() {
        // Test exact calculation for encrypted content
        let encrypted_16_chars = "1234567890123456"; // 16 chars
        let reasoning = ResponseItem::Reasoning {
            id: "test".to_string(),
            summary: vec![],
            content: None,
            encrypted_content: Some(encrypted_16_chars.to_string()),
        };

        let tokens = calculate_response_item_tokens_with_model(&reasoning, ModelFamily::Claude);
        // Formula: (16 * 3 / 4) / 4 = 12 / 4 = 3
        assert_eq!(tokens, 3);

        // Test with longer encrypted content
        let encrypted_100_chars = "a".repeat(100);
        let reasoning2 = ResponseItem::Reasoning {
            id: "test2".to_string(),
            summary: vec![],
            content: None,
            encrypted_content: Some(encrypted_100_chars),
        };

        let tokens2 = calculate_response_item_tokens_with_model(&reasoning2, ModelFamily::GPT4);
        // Formula: (100 * 3 / 4) / 4 = 75 / 4 = 18
        assert_eq!(tokens2, 18);
    }

    #[test]
    fn test_local_shell_call_without_call_id() {
        use codex_protocol::models::{LocalShellAction, LocalShellExecAction, LocalShellStatus};

        // Test with call_id = None
        let shell_call = ResponseItem::LocalShellCall {
            id: None,
            call_id: None,
            status: LocalShellStatus::InProgress,
            action: LocalShellAction::Exec(LocalShellExecAction {
                command: vec!["pwd".to_string()],
                working_directory: None,
                user: None,
                timeout_ms: Some(1000),
                env: None,
            }),
        };

        let tokens = calculate_response_item_tokens_with_model(&shell_call, ModelFamily::Claude);
        // "pwd" is very short and may result in 0 tokens with Claude's ratios
        assert_eq!(tokens, 0); // No call_id and very short command
    }

    #[test]
    fn test_local_shell_with_env_variables() {
        use codex_protocol::models::{LocalShellAction, LocalShellExecAction, LocalShellStatus};
        use std::collections::HashMap;

        let mut env_vars = HashMap::new();
        env_vars.insert("PATH".to_string(), "/usr/bin:/bin".to_string());
        env_vars.insert("HOME".to_string(), "/home/user".to_string());

        let shell_call = ResponseItem::LocalShellCall {
            id: Some("shell123".to_string()),
            call_id: Some("call123".to_string()),
            status: LocalShellStatus::Completed,
            action: LocalShellAction::Exec(LocalShellExecAction {
                command: vec!["env".to_string()],
                working_directory: None,
                user: None,
                timeout_ms: None,
                env: Some(env_vars),
            }),
        };

        let tokens = calculate_response_item_tokens_with_model(&shell_call, ModelFamily::GPT4);
        // "env" (3 chars) + "call123" (7 chars) = 10 chars, 2 words
        // Should get some tokens from both command and call_id
        assert!(tokens > 0); // Should count command + call_id
    }

    #[test]
    fn test_function_call_with_id() {
        let func_call = ResponseItem::FunctionCall {
            id: Some("func_id_123".to_string()),
            name: "get_data".to_string(),
            arguments: "{}".to_string(),
            call_id: "call_789".to_string(),
        };

        let tokens = calculate_response_item_tokens_with_model(&func_call, ModelFamily::Claude);
        assert!(tokens > 0);
    }

    #[test]
    fn test_custom_tool_call_with_id() {
        let tool_call = ResponseItem::CustomToolCall {
            id: Some("tool_id_456".to_string()),
            status: Some("running".to_string()),
            name: "analyzer".to_string(),
            input: "{\"data\": true}".to_string(),
            call_id: "call_000".to_string(),
        };

        let tokens = calculate_response_item_tokens_with_model(&tool_call, ModelFamily::GPT35);
        assert!(tokens > 0);
    }

    #[test]
    fn test_web_search_various_statuses() {
        use codex_protocol::models::WebSearchAction;

        // Test different status values
        let statuses = vec!["pending", "running", "failed", "completed"];

        for status in statuses {
            let search = ResponseItem::WebSearchCall {
                id: Some("search".to_string()),
                status: Some(status.to_string()),
                action: WebSearchAction::Search {
                    query: "test query".to_string(),
                },
            };

            let tokens = calculate_response_item_tokens_with_model(&search, ModelFamily::Unknown);
            assert!(tokens > 0);
        }
    }

    #[test]
    fn test_natural_language_formula() {
        // Test specific natural language calculation
        let text = "This is natural language without any code";
        let tokens = estimate_tokens_for_model(text, ModelFamily::Claude);

        let char_count = text.chars().count();
        let word_count = text.split_whitespace().count();
        let expected_chars = (char_count as f64 / 3.8) as usize;
        let expected_words = (word_count as f64 / 0.72) as usize;
        let expected = (expected_chars * 2 + expected_words) / 3;

        assert_eq!(tokens, expected);
    }

    #[test]
    fn test_code_with_only_closing_brace() {
        // Test code detection with only closing brace
        let text = "end of function }";
        let tokens = estimate_tokens_for_model(text, ModelFamily::GPT4);

        // Should detect as code due to }
        let plain_text = "end of function x";
        let plain_tokens = estimate_tokens_for_model(plain_text, ModelFamily::GPT4);

        assert!(tokens >= plain_tokens);
    }

    #[test]
    fn test_data_url_non_base64() {
        // Test data URL that's not base64
        let data_url_image = ContentItem::InputImage {
            image_url: "data:image/svg+xml,<svg>...</svg>".to_string(),
        };

        let tokens = calculate_content_item_tokens_with_model(&data_url_image, ModelFamily::Claude);
        assert_eq!(tokens, 130); // Should still double for data: URLs
    }

    #[test]
    fn test_gpt35_code_multiplier() {
        // Test GPT-3.5 specific code multiplier
        let code = "class Test { constructor() { this.value = 42; } }";
        let tokens = estimate_tokens_for_model(code, ModelFamily::GPT35);

        let char_count = code.chars().count();
        let word_count = code.split_whitespace().count();
        let char_estimate = (char_count as f64 / 3.3) as usize;
        let word_estimate = (word_count as f64 / 0.68) as usize;
        let base = (char_estimate + word_estimate) as f64 / 2.0;
        let expected = (base * 1.25) as usize; // GPT35 code multiplier is 1.25

        assert_eq!(tokens, expected);
    }

    #[test]
    fn test_all_local_shell_statuses() {
        use codex_protocol::models::{LocalShellAction, LocalShellExecAction, LocalShellStatus};

        let statuses = vec![
            LocalShellStatus::Completed,
            LocalShellStatus::InProgress,
            LocalShellStatus::Incomplete,
        ];

        for status in statuses {
            let shell_call = ResponseItem::LocalShellCall {
                id: None,
                call_id: None,
                status,
                action: LocalShellAction::Exec(LocalShellExecAction {
                    command: vec!["test".to_string()],
                    working_directory: None,
                    user: None,
                    timeout_ms: None,
                    env: None,
                }),
            };

            let tokens =
                calculate_response_item_tokens_with_model(&shell_call, ModelFamily::Claude);
            assert!(tokens > 0);
        }
    }
}
