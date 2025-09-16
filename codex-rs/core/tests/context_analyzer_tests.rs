//! Comprehensive tests for the context analyzer module

use codex_core::context_analyzer::{ContextBreakdown, analyze_context, estimate_tokens};
use codex_protocol::models::{
    ContentItem, FunctionCallOutputPayload, LocalShellAction, LocalShellExecAction,
    ReasoningItemContent, ReasoningItemReasoningSummary, ResponseItem, WebSearchAction,
};

#[cfg(test)]
mod context_breakdown_tests {
    use super::*;

    #[test]
    fn test_context_breakdown_new() {
        let breakdown = ContextBreakdown::new();
        assert_eq!(breakdown.system_prompt, 0);
        assert_eq!(breakdown.conversation, 0);
        assert_eq!(breakdown.tools, 0);
        assert_eq!(breakdown.total(), 0);
    }

    #[test]
    fn test_context_breakdown_default() {
        let breakdown = ContextBreakdown::default();
        assert_eq!(breakdown.system_prompt, 0);
        assert_eq!(breakdown.conversation, 0);
        assert_eq!(breakdown.tools, 0);
    }

    #[test]
    fn test_context_breakdown_total_calculation() {
        let mut breakdown = ContextBreakdown::new();

        // Test with various combinations
        breakdown.system_prompt = 100;
        assert_eq!(breakdown.total(), 100);

        breakdown.conversation = 200;
        assert_eq!(breakdown.total(), 300);

        breakdown.tools = 50;
        assert_eq!(breakdown.total(), 350);

        // Test with zero values
        breakdown.system_prompt = 0;
        breakdown.conversation = 0;
        breakdown.tools = 0;
        assert_eq!(breakdown.total(), 0);

        // Test with large values
        breakdown.system_prompt = 10000;
        breakdown.conversation = 20000;
        breakdown.tools = 5000;
        assert_eq!(breakdown.total(), 35000);
    }

    #[test]
    fn test_context_breakdown_clone() {
        let mut original = ContextBreakdown::new();
        original.system_prompt = 100;
        original.conversation = 200;
        original.tools = 50;

        let cloned = original.clone();
        assert_eq!(cloned.system_prompt, original.system_prompt);
        assert_eq!(cloned.conversation, original.conversation);
        assert_eq!(cloned.tools, original.tools);
        assert_eq!(cloned.total(), original.total());
    }

    #[test]
    fn test_context_breakdown_serialization() {
        let mut breakdown = ContextBreakdown::new();
        breakdown.system_prompt = 100;
        breakdown.conversation = 200;
        breakdown.tools = 50;

        // Test serialization
        let serialized = serde_json::to_string(&breakdown).unwrap();
        assert!(serialized.contains("\"system_prompt\":100"));
        assert!(serialized.contains("\"conversation\":200"));
        assert!(serialized.contains("\"tools\":50"));

        // Test deserialization
        let deserialized: ContextBreakdown = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.system_prompt, breakdown.system_prompt);
        assert_eq!(deserialized.conversation, breakdown.conversation);
        assert_eq!(deserialized.tools, breakdown.tools);
    }
}

#[cfg(test)]
mod estimate_tokens_tests {
    use super::*;

    #[test]
    fn test_estimate_tokens_empty_string() {
        assert_eq!(estimate_tokens(""), 0);
    }

    #[test]
    fn test_estimate_tokens_single_word() {
        let tokens = estimate_tokens("Hello");
        assert!(tokens > 0);
        assert!(tokens <= 2); // Single word should be 1-2 tokens
    }

    #[test]
    fn test_estimate_tokens_short_sentence() {
        let tokens = estimate_tokens("Hello world");
        assert!(tokens >= 2);
        assert!(tokens <= 4);
    }

    #[test]
    fn test_estimate_tokens_long_text() {
        let long_text = "The quick brown fox jumps over the lazy dog. This is a longer piece of text that should result in more tokens being estimated. We need to ensure that the token estimation scales appropriately with text length.";
        let tokens = estimate_tokens(long_text);
        assert!(tokens > 20);
        assert!(tokens < 100);
    }

    #[test]
    fn test_estimate_tokens_punctuation() {
        let text_with_punct = "Hello, world! How are you? I'm fine.";
        let tokens = estimate_tokens(text_with_punct);
        assert!(tokens > 5);
        assert!(tokens < 20);
    }

    #[test]
    fn test_estimate_tokens_numbers() {
        let text_with_numbers = "The year 2024 has 365 days and 12 months.";
        let tokens = estimate_tokens(text_with_numbers);
        assert!(tokens > 5);
        assert!(tokens < 20);
    }

    #[test]
    fn test_estimate_tokens_special_characters() {
        let text_with_special =
            "Email: test@example.com, URL: https://www.example.com/path?query=value#anchor";
        let tokens = estimate_tokens(text_with_special);
        assert!(tokens > 10);
    }

    #[test]
    fn test_estimate_tokens_unicode() {
        let unicode_text = "Hello 世界 🌍 émojis and ñoñ-ASCII çharacters";
        let tokens = estimate_tokens(unicode_text);
        assert!(tokens > 0);
    }

    #[test]
    fn test_estimate_tokens_whitespace_only() {
        assert_eq!(estimate_tokens("   "), 0); // Only spaces
        assert_eq!(estimate_tokens("\t\t"), 0); // Only tabs
        assert_eq!(estimate_tokens("\n\n"), 0); // Only newlines
        assert_eq!(estimate_tokens(" \t \n "), 0); // Mixed whitespace
    }

    #[test]
    fn test_estimate_tokens_code_snippet() {
        let code = "fn main() { println!(\"Hello, world!\"); }";
        let tokens = estimate_tokens(code);
        assert!(tokens > 5);
        assert!(tokens < 20);
    }

    #[test]
    fn test_estimate_tokens_json() {
        let json = r#"{"name": "test", "value": 42, "active": true}"#;
        let tokens = estimate_tokens(json);
        assert!(tokens > 5);
        assert!(tokens < 25);
    }

    #[test]
    fn test_estimate_tokens_consistency() {
        let text = "This is a test sentence for token estimation.";
        let tokens1 = estimate_tokens(text);
        let tokens2 = estimate_tokens(text);
        assert_eq!(tokens1, tokens2); // Should be deterministic
    }

    #[test]
    fn test_estimate_tokens_scaling() {
        let short_text = "Hello";
        let medium_text = "Hello world, this is a test";
        let long_text = "Hello world, this is a test. Let me add some more words to make this text even longer for testing purposes.";

        let short_tokens = estimate_tokens(short_text);
        let medium_tokens = estimate_tokens(medium_text);
        let long_tokens = estimate_tokens(long_text);

        assert!(short_tokens < medium_tokens);
        assert!(medium_tokens < long_tokens);
    }
}

#[cfg(test)]
mod analyze_context_tests {
    use super::*;

    #[test]
    fn test_analyze_context_all_none() {
        let breakdown = analyze_context(None, &[], None);
        assert_eq!(breakdown.system_prompt, 0);
        assert_eq!(breakdown.conversation, 0);
        assert_eq!(breakdown.tools, 0);
        assert_eq!(breakdown.total(), 0);
    }

    #[test]
    fn test_analyze_context_only_system_prompt() {
        let prompt = "You are a helpful AI assistant.";
        let breakdown = analyze_context(Some(prompt), &[], None);

        assert!(breakdown.system_prompt > 0);
        assert_eq!(breakdown.conversation, 0);
        assert_eq!(breakdown.tools, 0);
    }

    #[test]
    fn test_analyze_context_only_tools() {
        let tools = r#"{
            "tools": [
                {"name": "search", "description": "Search the web for information"},
                {"name": "calculate", "description": "Perform mathematical calculations"}
            ]
        }"#;
        let breakdown = analyze_context(None, &[], Some(tools));

        assert_eq!(breakdown.system_prompt, 0);
        assert_eq!(breakdown.conversation, 0);
        assert!(breakdown.tools > 0);
    }

    #[test]
    fn test_analyze_context_simple_conversation() {
        let history = vec![
            ResponseItem::Message {
                id: Some("msg1".to_string()),
                role: "user".to_string(),
                content: vec![ContentItem::InputText {
                    text: "What is 2 + 2?".to_string(),
                }],
            },
            ResponseItem::Message {
                id: Some("msg2".to_string()),
                role: "assistant".to_string(),
                content: vec![ContentItem::OutputText {
                    text: "2 + 2 equals 4.".to_string(),
                }],
            },
        ];

        let breakdown = analyze_context(None, &history, None);

        assert_eq!(breakdown.system_prompt, 0);
        assert!(breakdown.conversation > 0);
        assert_eq!(breakdown.tools, 0);
    }

    #[test]
    fn test_analyze_context_with_images() {
        let history = vec![
            ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![
                    ContentItem::InputText {
                        text: "What's in this image?".to_string(),
                    },
                    ContentItem::InputImage {
                        image_url: "https://example.com/image.jpg".to_string(),
                    },
                ],
            },
            ResponseItem::Message {
                id: None,
                role: "assistant".to_string(),
                content: vec![ContentItem::OutputText {
                    text: "I can see a cat in the image.".to_string(),
                }],
            },
        ];

        let breakdown = analyze_context(None, &history, None);

        assert!(breakdown.conversation > 0);
        // Should include tokens for the image (85 tokens for URL)
        assert!(breakdown.conversation > 90);
    }

    #[test]
    fn test_analyze_context_with_base64_image() {
        let history = vec![ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputImage {
                image_url: "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAUA".to_string(),
            }],
        }];

        let breakdown = analyze_context(None, &history, None);

        // Base64 images should count as 170 tokens
        assert!(breakdown.conversation >= 170);
    }

    #[test]
    fn test_analyze_context_with_function_calls() {
        let history = vec![
            ResponseItem::FunctionCall {
                id: None,
                name: "get_weather".to_string(),
                arguments: r#"{"location": "New York", "units": "celsius"}"#.to_string(),
                call_id: "call_123".to_string(),
            },
            ResponseItem::FunctionCallOutput {
                call_id: "call_123".to_string(),
                output: FunctionCallOutputPayload {
                    content: "Temperature: 22°C, Sunny".to_string(),
                    success: Some(true),
                },
            },
        ];

        let breakdown = analyze_context(None, &history, None);

        assert!(breakdown.conversation > 0);
    }

    #[test]
    fn test_analyze_context_with_custom_tool_calls() {
        let history = vec![
            ResponseItem::CustomToolCall {
                id: None,
                name: "calculator".to_string(),
                input: r#"{"operation": "add", "a": 5, "b": 3}"#.to_string(),
                call_id: "tool_456".to_string(),
            },
            ResponseItem::CustomToolCallOutput {
                call_id: "tool_456".to_string(),
                output: "Result: 8".to_string(),
            },
        ];

        let breakdown = analyze_context(None, &history, None);

        assert!(breakdown.conversation > 0);
    }

    #[test]
    fn test_analyze_context_with_reasoning() {
        let history = vec![ResponseItem::Reasoning {
            id: None,
            summary: vec![ReasoningItemReasoningSummary::SummaryText {
                text: "Analyzing the problem step by step".to_string(),
            }],
            content: Some(vec![
                ReasoningItemContent::ReasoningText {
                    text: "First, let's break down the components".to_string(),
                },
                ReasoningItemContent::Text {
                    text: "Additional reasoning details".to_string(),
                },
            ]),
            encrypted_content: None,
        }];

        let breakdown = analyze_context(None, &history, None);

        assert!(breakdown.conversation > 0);
    }

    #[test]
    fn test_analyze_context_with_encrypted_reasoning() {
        let history = vec![ResponseItem::Reasoning {
            id: None,
            summary: vec![],
            content: None,
            encrypted_content: Some("SGVsbG8gV29ybGQhIFRoaXMgaXMgYSB0ZXN0".to_string()),
        }];

        let breakdown = analyze_context(None, &history, None);

        assert!(breakdown.conversation > 0);
    }

    #[test]
    fn test_analyze_context_with_local_shell_exec() {
        let history = vec![ResponseItem::LocalShellCall {
            id: None,
            action: LocalShellAction::Exec(LocalShellExecAction {
                command: vec!["ls".to_string(), "-la".to_string()],
                working_directory: Some("/home/user".to_string()),
                user: Some("testuser".to_string()),
                environment: None,
            }),
        }];

        let breakdown = analyze_context(None, &history, None);

        assert!(breakdown.conversation > 0);
    }

    #[test]
    fn test_analyze_context_with_local_shell_run() {
        let history = vec![ResponseItem::LocalShellCall {
            id: None,
            action: LocalShellAction::Run {
                command: "echo 'Hello, World!'".to_string(),
            },
        }];

        let breakdown = analyze_context(None, &history, None);

        assert!(breakdown.conversation > 0);
    }

    #[test]
    fn test_analyze_context_with_local_shell_output() {
        let history = vec![ResponseItem::LocalShellCall {
            id: None,
            action: LocalShellAction::Output {
                stdout: "Command executed successfully\nOutput line 2".to_string(),
                stderr: "Warning: deprecated option used".to_string(),
            },
        }];

        let breakdown = analyze_context(None, &history, None);

        assert!(breakdown.conversation > 0);
    }

    #[test]
    fn test_analyze_context_with_web_search() {
        let history = vec![ResponseItem::WebSearchCall {
            id: None,
            action: WebSearchAction::Search {
                query: "Rust programming language best practices".to_string(),
            },
        }];

        let breakdown = analyze_context(None, &history, None);

        assert!(breakdown.conversation > 0);
    }

    #[test]
    fn test_analyze_context_with_web_search_other() {
        let history = vec![ResponseItem::WebSearchCall {
            id: None,
            action: WebSearchAction::Other,
        }];

        let breakdown = analyze_context(None, &history, None);

        // Should use the fixed estimate of 10 tokens
        assert_eq!(breakdown.conversation, 10);
    }

    #[test]
    fn test_analyze_context_with_other_response_item() {
        let history = vec![ResponseItem::Other];

        let breakdown = analyze_context(None, &history, None);

        // ResponseItem::Other should contribute 0 tokens
        assert_eq!(breakdown.conversation, 0);
    }

    #[test]
    fn test_analyze_context_complex_scenario() {
        let system_prompt = "You are an AI assistant specialized in Rust programming. You provide helpful, accurate, and concise answers.";

        let tools = r#"{
            "tools": [
                {
                    "name": "code_search",
                    "description": "Search for code snippets in the repository",
                    "parameters": {
                        "query": "string",
                        "language": "string"
                    }
                },
                {
                    "name": "execute_code",
                    "description": "Execute code snippets safely",
                    "parameters": {
                        "code": "string",
                        "language": "string"
                    }
                }
            ]
        }"#;

        let history = vec![
            ResponseItem::Message {
                id: Some("msg1".to_string()),
                role: "user".to_string(),
                content: vec![
                    ContentItem::InputText {
                        text: "Can you help me write a function to calculate fibonacci numbers?".to_string(),
                    },
                ],
            },
            ResponseItem::Message {
                id: Some("msg2".to_string()),
                role: "assistant".to_string(),
                content: vec![
                    ContentItem::OutputText {
                        text: "I'll help you write a fibonacci function in Rust.".to_string(),
                    },
                ],
            },
            ResponseItem::FunctionCall {
                id: Some("fc1".to_string()),
                name: "code_search".to_string(),
                arguments: r#"{"query": "fibonacci", "language": "rust"}"#.to_string(),
                call_id: "call_fib_search".to_string(),
            },
            ResponseItem::FunctionCallOutput {
                call_id: "call_fib_search".to_string(),
                output: FunctionCallOutputPayload {
                    content: "Found 3 relevant examples".to_string(),
                    success: Some(true),
                },
            },
            ResponseItem::Message {
                id: Some("msg3".to_string()),
                role: "assistant".to_string(),
                content: vec![
                    ContentItem::OutputText {
                        text: "Here's an efficient recursive implementation with memoization:".to_string(),
                    },
                    ContentItem::OutputText {
                        text: "```rust\nfn fibonacci(n: u32) -> u64 {\n    match n {\n        0 => 0,\n        1 => 1,\n        _ => fibonacci(n - 1) + fibonacci(n - 2),\n    }\n}\n```".to_string(),
                    },
                ],
            },
        ];

        let breakdown = analyze_context(Some(system_prompt), &history, Some(tools));

        assert!(breakdown.system_prompt > 0);
        assert!(breakdown.conversation > 0);
        assert!(breakdown.tools > 0);
        assert!(breakdown.total() > breakdown.system_prompt);
        assert!(breakdown.total() > breakdown.conversation);
        assert!(breakdown.total() > breakdown.tools);
    }

    #[test]
    fn test_analyze_context_empty_content_arrays() {
        let history = vec![
            ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![], // Empty content
            },
            ResponseItem::Reasoning {
                id: None,
                summary: vec![],       // Empty summary
                content: Some(vec![]), // Empty content
                encrypted_content: None,
            },
        ];

        let breakdown = analyze_context(None, &history, None);

        // Should still count the role tokens
        assert!(breakdown.conversation > 0);
    }

    #[test]
    fn test_analyze_context_very_long_inputs() {
        let long_prompt = "a".repeat(10000); // 10,000 character prompt
        let long_tools = "b".repeat(5000); // 5,000 character tools definition

        let history = vec![ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "c".repeat(20000), // 20,000 character message
            }],
        }];

        let breakdown = analyze_context(Some(&long_prompt), &history, Some(&long_tools));

        assert!(breakdown.system_prompt > 1000);
        assert!(breakdown.conversation > 2000);
        assert!(breakdown.tools > 500);
        assert!(breakdown.total() > 3500);
    }

    #[test]
    fn test_analyze_context_mixed_content_types() {
        let history = vec![
            ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![
                    ContentItem::InputText {
                        text: "Analyze this image and code:".to_string(),
                    },
                    ContentItem::InputImage {
                        image_url: "https://example.com/diagram.png".to_string(),
                    },
                    ContentItem::InputText {
                        text: "fn main() { println!(\"Hello\"); }".to_string(),
                    },
                ],
            },
            ResponseItem::FunctionCall {
                id: None,
                name: "analyze_code".to_string(),
                arguments: "{}".to_string(),
                call_id: "call_analyze".to_string(),
            },
            ResponseItem::LocalShellCall {
                id: None,
                action: LocalShellAction::Run {
                    command: "rustc --version".to_string(),
                },
            },
            ResponseItem::WebSearchCall {
                id: None,
                action: WebSearchAction::Search {
                    query: "Rust best practices".to_string(),
                },
            },
        ];

        let breakdown = analyze_context(None, &history, None);

        // Should account for text, image (85 tokens), function call, shell call, and web search
        assert!(breakdown.conversation > 100);
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[test]
    fn test_real_world_conversation_flow() {
        // Simulate a real conversation flow
        let system_prompt = "You are a helpful coding assistant.";
        let tools = r#"{"tools": [{"name": "execute", "description": "Execute code"}]}"#;

        let mut history = Vec::new();

        // User asks a question
        history.push(ResponseItem::Message {
            id: Some("1".to_string()),
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "Write a hello world program".to_string(),
            }],
        });

        // Assistant responds
        history.push(ResponseItem::Message {
            id: Some("2".to_string()),
            role: "assistant".to_string(),
            content: vec![ContentItem::OutputText {
                text: "Here's a hello world program in Rust:".to_string(),
            }],
        });

        // Assistant calls a function
        history.push(ResponseItem::FunctionCall {
            id: Some("3".to_string()),
            name: "execute".to_string(),
            arguments: r#"{"code": "fn main() { println!(\"Hello, World!\"); }"}"#.to_string(),
            call_id: "exec_1".to_string(),
        });

        // Function returns output
        history.push(ResponseItem::FunctionCallOutput {
            call_id: "exec_1".to_string(),
            output: FunctionCallOutputPayload {
                content: "Hello, World!".to_string(),
                success: Some(true),
            },
        });

        let breakdown = analyze_context(Some(system_prompt), &history, Some(tools));

        // Verify all components contribute to the total
        assert!(breakdown.system_prompt > 0);
        assert!(breakdown.conversation > 0);
        assert!(breakdown.tools > 0);

        // The total should be the sum of all parts
        assert_eq!(
            breakdown.total(),
            breakdown.system_prompt + breakdown.conversation + breakdown.tools
        );
    }

    #[test]
    fn test_token_estimation_accuracy_boundaries() {
        // Test that our estimation is within reasonable bounds
        // Real tokenizers typically produce ~1.3-1.5 tokens per word for English

        let test_cases = vec![
            ("Hello", 1, 2),                                // 1 word -> 1-2 tokens
            ("Hello world", 2, 4),                          // 2 words -> 2-4 tokens
            ("The quick brown fox", 4, 8),                  // 4 words -> 4-8 tokens
            ("This is a sentence with seven words", 7, 14), // 7 words -> 7-14 tokens
        ];

        for (text, min_tokens, max_tokens) in test_cases {
            let estimated = estimate_tokens(text);
            assert!(
                estimated >= min_tokens && estimated <= max_tokens,
                "Text '{}' estimated as {} tokens, expected {}-{}",
                text,
                estimated,
                min_tokens,
                max_tokens
            );
        }
    }

    #[test]
    fn test_performance_large_conversation() {
        // Create a large conversation history
        let mut history = Vec::new();

        for i in 0..1000 {
            history.push(ResponseItem::Message {
                id: Some(format!("msg_{}", i)),
                role: if i % 2 == 0 { "user" } else { "assistant" }.to_string(),
                content: vec![ContentItem::InputText {
                    text: format!("This is message number {}", i),
                }],
            });
        }

        let start = std::time::Instant::now();
        let breakdown = analyze_context(None, &history, None);
        let duration = start.elapsed();

        // Should complete in reasonable time (< 100ms for 1000 messages)
        assert!(duration.as_millis() < 100);
        assert!(breakdown.conversation > 0);
    }
}

#[cfg(test)]
mod edge_case_tests {
    use super::*;

    #[test]
    fn test_null_characters_in_text() {
        let text_with_null = "Hello\0World";
        let tokens = estimate_tokens(text_with_null);
        assert!(tokens > 0);
    }

    #[test]
    fn test_very_long_single_word() {
        let long_word = "a".repeat(1000); // 1000-character word
        let tokens = estimate_tokens(&long_word);
        assert!(tokens > 100); // Should recognize this as many tokens
    }

    #[test]
    fn test_repeated_whitespace() {
        let text = "Hello     World     Test"; // Multiple spaces
        let tokens = estimate_tokens(text);
        assert!(tokens > 0);
        assert!(tokens < 10); // Should handle multiple spaces correctly
    }

    #[test]
    fn test_mixed_languages() {
        let mixed = "Hello 世界 Bonjour мир";
        let tokens = estimate_tokens(mixed);
        assert!(tokens > 0);
    }

    #[test]
    fn test_malformed_json_in_tools() {
        let malformed_tools = "{ invalid json }";
        let breakdown = analyze_context(None, &[], Some(malformed_tools));
        assert!(breakdown.tools > 0); // Should still count tokens even if JSON is invalid
    }

    #[test]
    fn test_empty_function_arguments() {
        let history = vec![ResponseItem::FunctionCall {
            id: None,
            name: "test_function".to_string(),
            arguments: "".to_string(), // Empty arguments
            call_id: "call_empty".to_string(),
        }];

        let breakdown = analyze_context(None, &history, None);
        assert!(breakdown.conversation > 0); // Should still count name and call_id
    }

    #[test]
    fn test_shell_action_with_empty_fields() {
        let history = vec![ResponseItem::LocalShellCall {
            id: None,
            action: LocalShellAction::Exec(LocalShellExecAction {
                command: vec![], // Empty command
                working_directory: None,
                user: None,
                environment: None,
            }),
        }];

        let breakdown = analyze_context(None, &history, None);
        assert_eq!(breakdown.conversation, 0); // Empty command should result in 0 tokens
    }

    #[test]
    fn test_reasoning_with_all_none_fields() {
        let history = vec![ResponseItem::Reasoning {
            id: None,
            summary: vec![],
            content: None,
            encrypted_content: None,
        }];

        let breakdown = analyze_context(None, &history, None);
        assert_eq!(breakdown.conversation, 0); // All empty should be 0
    }

    #[test]
    fn test_concurrent_access() {
        use std::sync::Arc;
        use std::thread;

        let prompt = Arc::new("Test prompt".to_string());
        let tools = Arc::new("Test tools".to_string());

        let mut handles = vec![];

        for _ in 0..10 {
            let prompt_clone = Arc::clone(&prompt);
            let tools_clone = Arc::clone(&tools);

            let handle = thread::spawn(move || {
                let breakdown = analyze_context(Some(&prompt_clone), &[], Some(&tools_clone));
                breakdown.total()
            });

            handles.push(handle);
        }

        let results: Vec<usize> = handles.into_iter().map(|h| h.join().unwrap()).collect();

        // All threads should get the same result
        let first_result = results[0];
        assert!(results.iter().all(|&r| r == first_result));
    }
}
