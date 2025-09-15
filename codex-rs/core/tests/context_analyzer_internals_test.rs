//! Unit tests for internal functions of the context analyzer module
//! These tests focus on edge cases and boundary conditions for helper functions

#[cfg(test)]
mod test_helpers {
    use codex_core::context_analyzer::{estimate_tokens, ContextBreakdown};
    use codex_protocol::models::{
        ContentItem, FunctionCallOutputPayload, LocalShellAction, LocalShellExecAction,
        ReasoningItemContent, ReasoningItemReasoningSummary, ResponseItem, WebSearchAction,
    };

    /// Helper function to create a test message
    pub fn create_test_message(role: &str, text: &str) -> ResponseItem {
        ResponseItem::Message {
            id: None,
            role: role.to_string(),
            content: vec![ContentItem::InputText {
                text: text.to_string(),
            }],
        }
    }

    /// Helper function to create a test function call
    pub fn create_test_function_call(name: &str, args: &str) -> ResponseItem {
        ResponseItem::FunctionCall {
            id: None,
            name: name.to_string(),
            arguments: args.to_string(),
            call_id: format!("call_{}", name),
        }
    }

    /// Helper to verify token count is within expected range
    pub fn assert_token_range(text: &str, min: usize, max: usize) {
        let tokens = estimate_tokens(text);
        assert!(
            tokens >= min && tokens <= max,
            "Token count {} for '{}' not in range [{}, {}]",
            tokens,
            text,
            min,
            max
        );
    }
}

#[cfg(test)]
mod token_estimation_edge_cases {
    use super::test_helpers::*;
    use codex_core::context_analyzer::estimate_tokens;

    #[test]
    fn test_estimate_tokens_with_control_characters() {
        // Test various control characters
        let test_cases = vec![
            ("\n\r\t", 0),                    // Only control chars -> 0 tokens
            ("Hello\nWorld", 2),              // Newline in middle
            ("Tab\tSeparated\tValues", 3),    // Tabs
            ("Carriage\rReturn", 2),          // Carriage return
            ("\x00\x01\x02", 0),              // Null and other control chars
        ];

        for (text, expected_min) in test_cases {
            let tokens = estimate_tokens(text);
            assert!(
                tokens >= expected_min,
                "Text '{}' (escaped) got {} tokens, expected at least {}",
                text.escape_default(),
                tokens,
                expected_min
            );
        }
    }

    #[test]
    fn test_estimate_tokens_mathematical_expressions() {
        let expressions = vec![
            ("2+2=4", 1, 3),
            ("x^2 + y^2 = z^2", 3, 8),
            ("∫f(x)dx", 2, 5),
            ("∑(i=1 to n) i^2", 3, 8),
            ("√(a²+b²)", 2, 5),
        ];

        for (expr, min, max) in expressions {
            assert_token_range(expr, min, max);
        }
    }

    #[test]
    fn test_estimate_tokens_programming_syntax() {
        let code_snippets = vec![
            ("let x = 5;", 3, 6),
            ("fn foo() {}", 3, 6),
            ("async/await", 2, 4),
            ("Option<Vec<String>>", 3, 6),
            ("impl<T: Clone>", 3, 6),
            ("#[derive(Debug)]", 2, 5),
        ];

        for (code, min, max) in code_snippets {
            assert_token_range(code, min, max);
        }
    }

    #[test]
    fn test_estimate_tokens_urls_and_paths() {
        let urls_paths = vec![
            ("https://www.example.com", 3, 8),
            ("http://localhost:8080/api/v1", 5, 12),
            ("/usr/local/bin/rustc", 4, 10),
            ("C:\\Windows\\System32", 3, 8),
            ("file:///home/user/document.pdf", 5, 12),
        ];

        for (url, min, max) in urls_paths {
            assert_token_range(url, min, max);
        }
    }

    #[test]
    fn test_estimate_tokens_email_addresses() {
        let emails = vec![
            ("user@example.com", 2, 5),
            ("john.doe+filter@company.co.uk", 4, 10),
            ("admin@localhost", 2, 5),
        ];

        for (email, min, max) in emails {
            assert_token_range(email, min, max);
        }
    }

    #[test]
    fn test_estimate_tokens_repeated_characters() {
        // Test how the estimator handles repeated characters
        assert_token_range(&"a".repeat(100), 20, 30);
        assert_token_range(&"ab".repeat(50), 20, 30);
        assert_token_range(&"test ".repeat(20), 20, 30);
    }

    #[test]
    fn test_estimate_tokens_mixed_scripts() {
        let mixed_scripts = vec![
            ("Hello世界", 2, 5),
            ("Привет мир", 2, 5),
            ("مرحبا بالعالم", 2, 6),
            ("हैलो वर्ल्ड", 2, 6),
            ("😀🚀🌟", 1, 4),
        ];

        for (text, min, max) in mixed_scripts {
            assert_token_range(text, min, max);
        }
    }
}

#[cfg(test)]
mod response_item_token_calculation {
    use super::test_helpers::*;
    use codex_core::context_analyzer::analyze_context;
    use codex_protocol::models::*;
    use std::collections::HashMap;

    #[test]
    fn test_message_with_multiple_content_items() {
        let history = vec![ResponseItem::Message {
            id: Some("multi".to_string()),
            role: "assistant".to_string(),
            content: vec![
                ContentItem::OutputText {
                    text: "Here's the answer:".to_string(),
                },
                ContentItem::OutputText {
                    text: "Part 1: Introduction".to_string(),
                },
                ContentItem::OutputText {
                    text: "Part 2: Details".to_string(),
                },
                ContentItem::OutputText {
                    text: "Part 3: Conclusion".to_string(),
                },
            ],
        }];

        let breakdown = analyze_context(None, &history, None);
        assert!(breakdown.conversation > 10); // Should count all text parts
    }

    #[test]
    fn test_function_call_with_complex_json() {
        let complex_json = r#"{
            "query": "search term",
            "filters": {
                "date_range": {"start": "2024-01-01", "end": "2024-12-31"},
                "categories": ["tech", "science", "news"],
                "limit": 100
            },
            "options": {
                "sort": "relevance",
                "highlight": true
            }
        }"#;

        let history = vec![ResponseItem::FunctionCall {
            id: Some("complex".to_string()),
            name: "advanced_search".to_string(),
            arguments: complex_json.to_string(),
            call_id: "call_complex_search".to_string(),
        }];

        let breakdown = analyze_context(None, &history, None);
        assert!(breakdown.conversation > 20); // Complex JSON should have many tokens
    }

    #[test]
    fn test_local_shell_exec_with_environment() {
        let mut env_vars = HashMap::new();
        env_vars.insert("PATH".to_string(), "/usr/local/bin:/usr/bin:/bin".to_string());
        env_vars.insert("HOME".to_string(), "/home/user".to_string());
        env_vars.insert("RUST_BACKTRACE".to_string(), "1".to_string());

        let history = vec![ResponseItem::LocalShellCall {
            id: Some("exec_env".to_string()),
            action: LocalShellAction::Exec(LocalShellExecAction {
                command: vec!["cargo".to_string(), "build".to_string(), "--release".to_string()],
                working_directory: Some("/home/user/project".to_string()),
                user: Some("developer".to_string()),
                environment: Some(env_vars),
            }),
        }];

        let breakdown = analyze_context(None, &history, None);
        assert!(breakdown.conversation > 5); // Should count command, directory, and user
    }

    #[test]
    fn test_reasoning_with_mixed_content_types() {
        let history = vec![ResponseItem::Reasoning {
            id: Some("reasoning".to_string()),
            summary: vec![
                ReasoningItemReasoningSummary::SummaryText {
                    text: "Step 1: Analyze".to_string(),
                },
                ReasoningItemReasoningSummary::SummaryText {
                    text: "Step 2: Process".to_string(),
                },
                ReasoningItemReasoningSummary::SummaryText {
                    text: "Step 3: Conclude".to_string(),
                },
            ],
            content: Some(vec![
                ReasoningItemContent::ReasoningText {
                    text: "Detailed reasoning about the problem".to_string(),
                },
                ReasoningItemContent::Text {
                    text: "Additional context and information".to_string(),
                },
                ReasoningItemContent::ReasoningText {
                    text: "Final thoughts and conclusions".to_string(),
                },
            ]),
            encrypted_content: Some("U29tZSBlbmNyeXB0ZWQgY29udGVudCBoZXJl".to_string()),
        }];

        let breakdown = analyze_context(None, &history, None);
        assert!(breakdown.conversation > 15); // Should count all summary, content, and encrypted
    }

    #[test]
    fn test_web_search_with_long_query() {
        let long_query = "How to implement a distributed consensus algorithm like Raft or Paxos in Rust with proper error handling and network partition tolerance";
        
        let history = vec![ResponseItem::WebSearchCall {
            id: Some("search".to_string()),
            action: WebSearchAction::Search {
                query: long_query.to_string(),
            },
        }];

        let breakdown = analyze_context(None, &history, None);
        assert!(breakdown.conversation > 10); // Long query should have many tokens
    }

    #[test]
    fn test_custom_tool_with_nested_json() {
        let nested_json = r#"{
            "action": "analyze",
            "target": {
                "type": "repository",
                "url": "https://github.com/example/repo",
                "branch": "main",
                "commit": "abc123def456"
            },
            "analysis": {
                "security": true,
                "performance": true,
                "dependencies": {
                    "check_outdated": true,
                    "check_vulnerabilities": true
                }
            }
        }"#;

        let history = vec![
            ResponseItem::CustomToolCall {
                id: Some("custom".to_string()),
                name: "code_analyzer".to_string(),
                input: nested_json.to_string(),
                call_id: "tool_analyze_123".to_string(),
            },
            ResponseItem::CustomToolCallOutput {
                call_id: "tool_analyze_123".to_string(),
                output: "Analysis complete: 3 security issues, 5 performance improvements suggested".to_string(),
            },
        ];

        let breakdown = analyze_context(None, &history, None);
        assert!(breakdown.conversation > 30); // Complex input and output
    }

    #[test]
    fn test_mixed_image_types_in_conversation() {
        let history = vec![
            ResponseItem::Message {
                id: Some("images".to_string()),
                role: "user".to_string(),
                content: vec![
                    ContentItem::InputText {
                        text: "Compare these images:".to_string(),
                    },
                    ContentItem::InputImage {
                        image_url: "https://example.com/image1.jpg".to_string(), // 85 tokens
                    },
                    ContentItem::InputImage {
                        image_url: "data:image/png;base64,iVBORw0KGgoAAAAN...".to_string(), // 170 tokens
                    },
                    ContentItem::InputImage {
                        image_url: "https://example.com/image2.png".to_string(), // 85 tokens
                    },
                ],
            },
        ];

        let breakdown = analyze_context(None, &history, None);
        // Should be at least 340 tokens from images alone (85 + 170 + 85)
        assert!(breakdown.conversation >= 340);
    }

    #[test]
    fn test_empty_and_whitespace_only_content() {
        let history = vec![
            ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![
                    ContentItem::InputText {
                        text: "".to_string(), // Empty
                    },
                    ContentItem::InputText {
                        text: "   ".to_string(), // Only spaces
                    },
                    ContentItem::InputText {
                        text: "\t\n\r".to_string(), // Only whitespace
                    },
                ],
            },
        ];

        let breakdown = analyze_context(None, &history, None);
        // Should still count the role
        assert!(breakdown.conversation > 0);
    }
}

#[cfg(test)]
mod context_breakdown_scenarios {
    use super::test_helpers::*;
    use codex_core::context_analyzer::{analyze_context, ContextBreakdown};
    use codex_protocol::models::*;

    #[test]
    fn test_incremental_context_building() {
        let system_prompt = "You are a helpful assistant.";
        let tools = r#"{"tools": [{"name": "search"}]}"#;
        
        // Start with just system prompt
        let breakdown1 = analyze_context(Some(system_prompt), &[], None);
        assert!(breakdown1.system_prompt > 0);
        assert_eq!(breakdown1.conversation, 0);
        assert_eq!(breakdown1.tools, 0);
        
        // Add tools
        let breakdown2 = analyze_context(Some(system_prompt), &[], Some(tools));
        assert_eq!(breakdown2.system_prompt, breakdown1.system_prompt);
        assert!(breakdown2.tools > 0);
        
        // Add conversation
        let history = vec![create_test_message("user", "Hello")];
        let breakdown3 = analyze_context(Some(system_prompt), &history, Some(tools));
        assert_eq!(breakdown3.system_prompt, breakdown1.system_prompt);
        assert_eq!(breakdown3.tools, breakdown2.tools);
        assert!(breakdown3.conversation > 0);
    }

    #[test]
    fn test_conversation_growth_tracking() {
        let mut history = Vec::new();
        let mut previous_tokens = 0;
        
        // Add messages and verify token count grows
        for i in 0..10 {
            history.push(create_test_message(
                if i % 2 == 0 { "user" } else { "assistant" },
                &format!("Message number {}", i),
            ));
            
            let breakdown = analyze_context(None, &history, None);
            assert!(
                breakdown.conversation > previous_tokens,
                "Conversation tokens should grow with each message"
            );
            previous_tokens = breakdown.conversation;
        }
    }

    #[test]
    fn test_tool_definition_variations() {
        let test_cases = vec![
            (r#"{}"#, 1),                                    // Empty JSON
            (r#"{"tools": []}"#, 2),                        // Empty tools array
            (r#"{"tools": [{"name": "t1"}]}"#, 5),         // Single tool
            (r#"{"tools": [{"name": "t1"}, {"name": "t2"}, {"name": "t3"}]}"#, 10), // Multiple tools
        ];
        
        for (tools_def, min_tokens) in test_cases {
            let breakdown = analyze_context(None, &[], Some(tools_def));
            assert!(
                breakdown.tools >= min_tokens,
                "Tools '{}' should have at least {} tokens",
                tools_def,
                min_tokens
            );
        }
    }

    #[test]
    fn test_system_prompt_variations() {
        let prompts = vec![
            ("", 0),
            (".", 1),
            ("You are helpful.", 3),
            ("You are a helpful AI assistant specialized in Rust programming.", 10),
            (&"Be helpful. ".repeat(100), 200), // Repeated prompt
        ];
        
        for (prompt, min_tokens) in prompts {
            let breakdown = analyze_context(Some(prompt), &[], None);
            assert!(
                breakdown.system_prompt >= min_tokens,
                "Prompt '{}...' should have at least {} tokens",
                &prompt.chars().take(20).collect::<String>(),
                min_tokens
            );
        }
    }

    #[test]
    fn test_breakdown_serialization_roundtrip() {
        let mut original = ContextBreakdown::new();
        original.system_prompt = 123;
        original.conversation = 456;
        original.tools = 789;
        
        // Serialize to JSON
        let json = serde_json::to_string(&original).unwrap();
        
        // Deserialize back
        let deserialized: ContextBreakdown = serde_json::from_str(&json).unwrap();
        
        // Verify equality
        assert_eq!(original.system_prompt, deserialized.system_prompt);
        assert_eq!(original.conversation, deserialized.conversation);
        assert_eq!(original.tools, deserialized.tools);
        assert_eq!(original.total(), deserialized.total());
    }

    #[test]
    fn test_breakdown_with_extreme_values() {
        let mut breakdown = ContextBreakdown::new();
        
        // Test with maximum usize values (should not overflow)
        breakdown.system_prompt = usize::MAX / 3;
        breakdown.conversation = usize::MAX / 3;
        breakdown.tools = usize::MAX / 3;
        
        // This should not panic or overflow
        let total = breakdown.total();
        assert!(total > 0);
    }
}

#[cfg(test)]
mod stress_tests {
    use super::test_helpers::*;
    use codex_core::context_analyzer::analyze_context;
    use codex_protocol::models::*;

    #[test]
    fn test_very_large_conversation_history() {
        let mut history = Vec::new();
        
        // Create a conversation with 10,000 messages
        for i in 0..10000 {
            history.push(create_test_message(
                if i % 2 == 0 { "user" } else { "assistant" },
                &format!("Message {}", i),
            ));
        }
        
        let start = std::time::Instant::now();
        let breakdown = analyze_context(None, &history, None);
        let duration = start.elapsed();
        
        // Should handle large history efficiently
        assert!(duration.as_secs() < 1, "Should process 10k messages in < 1 second");
        assert!(breakdown.conversation > 0);
    }

    #[test]
    fn test_deeply_nested_function_calls() {
        let mut history = Vec::new();
        
        // Create deeply nested JSON arguments
        let mut json = r#"{"level": 0"#.to_string();
        for i in 1..100 {
            json.push_str(&format!(r#", "nested_{}": {{"level": {}"#, i, i));
        }
        for _ in 0..100 {
            json.push_str("}");
        }
        json.push('}');
        
        history.push(ResponseItem::FunctionCall {
            id: None,
            name: "deep_function".to_string(),
            arguments: json,
            call_id: "call_deep".to_string(),
        });
        
        let breakdown = analyze_context(None, &history, None);
        assert!(breakdown.conversation > 100); // Deep nesting should count many tokens
    }

    #[test]
    fn test_unicode_stress() {
        // Test with various Unicode ranges
        let unicode_samples = vec![
            "𝕳𝖊𝖑𝖑𝖔 𝖂𝖔𝖗𝖑𝖉", // Mathematical bold
            "🏴󐐀󐐁󐐂󐐃󐐄", // Flags and other symbols
            "𐌀𐌁𐌂𐌃𐌄𐌅", // Gothic letters
            "ℵℶℷℸ", // Mathematical symbols
            "⺀⺁⺂⺃⺄⺅", // CJK radicals
        ];
        
        let mut history = Vec::new();
        for sample in unicode_samples {
            history.push(create_test_message("user", sample));
        }
        
        let breakdown = analyze_context(None, &history, None);
        assert!(breakdown.conversation > 0);
    }

    #[test]
    fn test_memory_efficiency() {
        // Test that we don't have memory leaks with repeated calls
        for _ in 0..1000 {
            let history = vec![
                create_test_message("user", "Test message"),
                create_test_function_call("test", r#"{"key": "value"}"#),
            ];
            
            let _ = analyze_context(
                Some("System prompt"),
                &history,
                Some(r#"{"tools": []}"#),
            );
        }
        // This test passes if it doesn't run out of memory
    }
}

#[cfg(test)]
mod regression_tests {
    use super::test_helpers::*;
    use codex_core::context_analyzer::{analyze_context, estimate_tokens};
    use codex_protocol::models::*;

    #[test]
    fn test_issue_empty_encrypted_content() {
        // Regression test for handling empty encrypted content
        let history = vec![ResponseItem::Reasoning {
            id: None,
            summary: vec![],
            content: None,
            encrypted_content: Some("".to_string()), // Empty encrypted content
        }];
        
        let breakdown = analyze_context(None, &history, None);
        assert_eq!(breakdown.conversation, 0); // Should handle empty encrypted content
    }

    #[test]
    fn test_issue_malformed_base64_image() {
        // Regression test for malformed base64 image URLs
        let history = vec![ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![
                ContentItem::InputImage {
                    image_url: "data:image/png;base64,INVALID!!!".to_string(),
                },
            ],
        }];
        
        let breakdown = analyze_context(None, &history, None);
        assert_eq!(breakdown.conversation, 170); // Should still count as base64 image
    }

    #[test]
    fn test_issue_shell_output_with_ansi_codes() {
        // Regression test for shell output with ANSI escape codes
        let ansi_output = "\x1b[32mSUCCESS\x1b[0m: Tests passed\n\x1b[31mERROR\x1b[0m: 0 failures";
        
        let history = vec![ResponseItem::LocalShellCall {
            id: None,
            action: LocalShellAction::Output {
                stdout: ansi_output.to_string(),
                stderr: "".to_string(),
            },
        }];
        
        let breakdown = analyze_context(None, &history, None);
        assert!(breakdown.conversation > 0); // Should handle ANSI codes
    }

    #[test]
    fn test_issue_zero_length_tokens() {
        // Regression test for edge case with zero-length token estimation
        assert_eq!(estimate_tokens(""), 0);
        assert_eq!(estimate_tokens("   "), 0);
        assert_eq!(estimate_tokens("\n\n\n"), 0);
    }

    #[test]
    fn test_issue_integer_overflow_protection() {
        // Regression test for potential integer overflow
        let huge_text = "a".repeat(usize::MAX / 10);
        let tokens = estimate_tokens(&huge_text);
        assert!(tokens > 0); // Should not panic or overflow
    }
}