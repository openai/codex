//! Performance tests for the context analyzer module
//! These tests ensure that token calculation and analysis functions perform efficiently

#[cfg(test)]
mod performance_tests {
    use codex_core::context_analyzer::{ContextBreakdown, analyze_context, estimate_tokens};
    use codex_protocol::models::{
        ContentItem, FunctionCallOutputPayload, LocalShellAction, LocalShellExecAction,
        ReasoningItemContent, ReasoningItemReasoningSummary, ResponseItem, WebSearchAction,
    };
    use std::time::Instant;

    /// Helper to measure execution time
    fn measure_time<F, R>(f: F) -> (R, std::time::Duration)
    where
        F: FnOnce() -> R,
    {
        let start = Instant::now();
        let result = f();
        let duration = start.elapsed();
        (result, duration)
    }

    #[test]
    fn test_performance_estimate_tokens_short_text() {
        let text = "Hello, world!";
        let (tokens, duration) = measure_time(|| estimate_tokens(text));

        assert!(tokens > 0);
        assert!(
            duration.as_micros() < 100,
            "Short text tokenization took {:?}, expected < 100µs",
            duration
        );
    }

    #[test]
    fn test_performance_estimate_tokens_medium_text() {
        let text = "The quick brown fox jumps over the lazy dog. ".repeat(10);
        let (tokens, duration) = measure_time(|| estimate_tokens(&text));

        assert!(tokens > 0);
        assert!(
            duration.as_micros() < 500,
            "Medium text tokenization took {:?}, expected < 500µs",
            duration
        );
    }

    #[test]
    fn test_performance_estimate_tokens_large_text() {
        let text = "Lorem ipsum dolor sit amet, consectetur adipiscing elit. ".repeat(100);
        let (tokens, duration) = measure_time(|| estimate_tokens(&text));

        assert!(tokens > 0);
        assert!(
            duration.as_millis() < 5,
            "Large text tokenization took {:?}, expected < 5ms",
            duration
        );
    }

    #[test]
    fn test_performance_analyze_empty_context() {
        let (breakdown, duration) = measure_time(|| analyze_context(None, &[], None));

        assert_eq!(breakdown.total(), 0);
        assert!(
            duration.as_micros() < 50,
            "Empty context analysis took {:?}, expected < 50µs",
            duration
        );
    }

    #[test]
    fn test_performance_analyze_small_conversation() {
        let history = vec![
            ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputText {
                    text: "Hello".to_string(),
                }],
            },
            ResponseItem::Message {
                id: None,
                role: "assistant".to_string(),
                content: vec![ContentItem::OutputText {
                    text: "Hi there!".to_string(),
                }],
            },
        ];

        let (breakdown, duration) = measure_time(|| analyze_context(None, &history, None));

        assert!(breakdown.conversation > 0);
        assert!(
            duration.as_micros() < 200,
            "Small conversation analysis took {:?}, expected < 200µs",
            duration
        );
    }

    #[test]
    fn test_performance_analyze_100_messages() {
        let mut history = Vec::new();
        for i in 0..100 {
            history.push(ResponseItem::Message {
                id: Some(format!("msg_{}", i)),
                role: if i % 2 == 0 { "user" } else { "assistant" }.to_string(),
                content: vec![ContentItem::InputText {
                    text: format!("This is message number {} in the conversation", i),
                }],
            });
        }

        let (breakdown, duration) = measure_time(|| analyze_context(None, &history, None));

        assert!(breakdown.conversation > 0);
        assert!(
            duration.as_millis() < 10,
            "100 messages analysis took {:?}, expected < 10ms",
            duration
        );
    }

    #[test]
    fn test_performance_analyze_1000_messages() {
        let mut history = Vec::new();
        for i in 0..1000 {
            history.push(ResponseItem::Message {
                id: Some(format!("msg_{}", i)),
                role: if i % 2 == 0 { "user" } else { "assistant" }.to_string(),
                content: vec![ContentItem::InputText {
                    text: format!("Message {}", i),
                }],
            });
        }

        let (breakdown, duration) = measure_time(|| analyze_context(None, &history, None));

        assert!(breakdown.conversation > 0);
        assert!(
            duration.as_millis() < 50,
            "1000 messages analysis took {:?}, expected < 50ms",
            duration
        );
    }

    #[test]
    fn test_performance_analyze_with_images() {
        let mut history = Vec::new();

        // Add messages with images
        for i in 0..50 {
            history.push(ResponseItem::Message {
                id: Some(format!("img_msg_{}", i)),
                role: "user".to_string(),
                content: vec![
                    ContentItem::InputText {
                        text: format!("Image {}", i),
                    },
                    ContentItem::InputImage {
                        image_url: if i % 2 == 0 {
                            "https://example.com/image.jpg".to_string()
                        } else {
                            "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAUA".to_string()
                        },
                    },
                ],
            });
        }

        let (breakdown, duration) = measure_time(|| analyze_context(None, &history, None));

        assert!(breakdown.conversation > 0);
        assert!(
            duration.as_millis() < 10,
            "50 messages with images analysis took {:?}, expected < 10ms",
            duration
        );
    }

    #[test]
    fn test_performance_analyze_with_function_calls() {
        let mut history = Vec::new();

        // Add function calls
        for i in 0..100 {
            history.push(ResponseItem::FunctionCall {
                id: Some(format!("call_{}", i)),
                name: format!("function_{}", i % 10),
                arguments: format!(r#"{{"param": "value_{}", "index": {}}}"#, i, i),
                call_id: format!("call_id_{}", i),
            });

            history.push(ResponseItem::FunctionCallOutput {
                call_id: format!("call_id_{}", i),
                output: FunctionCallOutputPayload {
                    content: format!("Result for call {}", i),
                    success: Some(true),
                },
            });
        }

        let (breakdown, duration) = measure_time(|| analyze_context(None, &history, None));

        assert!(breakdown.conversation > 0);
        assert!(
            duration.as_millis() < 20,
            "200 function call items analysis took {:?}, expected < 20ms",
            duration
        );
    }

    #[test]
    fn test_performance_analyze_complex_reasoning() {
        let mut history = Vec::new();

        for i in 0..50 {
            history.push(ResponseItem::Reasoning {
                id: Some(format!("reasoning_{}", i)),
                summary: vec![
                    ReasoningItemReasoningSummary::SummaryText {
                        text: format!("Step 1 of reasoning {}", i),
                    },
                    ReasoningItemReasoningSummary::SummaryText {
                        text: format!("Step 2 of reasoning {}", i),
                    },
                ],
                content: Some(vec![
                    ReasoningItemContent::ReasoningText {
                        text: format!("Detailed reasoning for item {}", i),
                    },
                    ReasoningItemContent::Text {
                        text: format!("Additional context for item {}", i),
                    },
                ]),
                encrypted_content: Some(format!("EncryptedContent{}", i)),
            });
        }

        let (breakdown, duration) = measure_time(|| analyze_context(None, &history, None));

        assert!(breakdown.conversation > 0);
        assert!(
            duration.as_millis() < 15,
            "50 reasoning items analysis took {:?}, expected < 15ms",
            duration
        );
    }

    #[test]
    fn test_performance_analyze_shell_calls() {
        let mut history = Vec::new();

        for i in 0..100 {
            history.push(ResponseItem::LocalShellCall {
                id: Some(format!("shell_{}", i)),
                action: if i % 3 == 0 {
                    LocalShellAction::Exec(LocalShellExecAction {
                        command: vec!["echo".to_string(), format!("Command {}", i)],
                        working_directory: Some(format!("/tmp/dir_{}", i)),
                        user: Some(format!("user_{}", i)),
                        environment: None,
                    })
                } else if i % 3 == 1 {
                    LocalShellAction::Run {
                        command: format!("echo 'Shell command {}'", i),
                    }
                } else {
                    LocalShellAction::Output {
                        stdout: format!("Output from command {}\nLine 2\nLine 3", i),
                        stderr: format!("Error output {}", i),
                    }
                },
            });
        }

        let (breakdown, duration) = measure_time(|| analyze_context(None, &history, None));

        assert!(breakdown.conversation > 0);
        assert!(
            duration.as_millis() < 20,
            "100 shell call items analysis took {:?}, expected < 20ms",
            duration
        );
    }

    #[test]
    fn test_performance_analyze_web_searches() {
        let mut history = Vec::new();

        for i in 0..100 {
            history.push(ResponseItem::WebSearchCall {
                id: Some(format!("search_{}", i)),
                action: if i % 2 == 0 {
                    WebSearchAction::Search {
                        query: format!("Search query number {} with some additional terms", i),
                    }
                } else {
                    WebSearchAction::Other
                },
            });
        }

        let (breakdown, duration) = measure_time(|| analyze_context(None, &history, None));

        assert!(breakdown.conversation > 0);
        assert!(
            duration.as_millis() < 10,
            "100 web search items analysis took {:?}, expected < 10ms",
            duration
        );
    }

    #[test]
    fn test_performance_analyze_full_context() {
        let system_prompt = "You are a helpful AI assistant specialized in multiple domains including programming, mathematics, science, and general knowledge. You should provide accurate, helpful, and concise responses.";

        let tools = r#"{
            "tools": [
                {"name": "search", "description": "Search the web for information", "parameters": {"query": "string"}},
                {"name": "calculate", "description": "Perform mathematical calculations", "parameters": {"expression": "string"}},
                {"name": "execute_code", "description": "Execute code snippets", "parameters": {"code": "string", "language": "string"}},
                {"name": "read_file", "description": "Read file contents", "parameters": {"path": "string"}},
                {"name": "write_file", "description": "Write content to a file", "parameters": {"path": "string", "content": "string"}}
            ]
        }"#;

        let mut history = Vec::new();

        // Add diverse content
        for i in 0..200 {
            match i % 5 {
                0 => history.push(ResponseItem::Message {
                    id: Some(format!("msg_{}", i)),
                    role: if i % 10 < 5 { "user" } else { "assistant" }.to_string(),
                    content: vec![ContentItem::InputText {
                        text: format!("Message {} with some content", i),
                    }],
                }),
                1 => history.push(ResponseItem::FunctionCall {
                    id: Some(format!("fc_{}", i)),
                    name: "search".to_string(),
                    arguments: format!(r#"{{"query": "Query {}}}"#, i),
                    call_id: format!("call_{}", i),
                }),
                2 => history.push(ResponseItem::FunctionCallOutput {
                    call_id: format!("call_{}", i - 1),
                    output: FunctionCallOutputPayload {
                        content: format!("Search results for query {}", i - 1),
                        success: Some(true),
                    },
                }),
                3 => history.push(ResponseItem::LocalShellCall {
                    id: Some(format!("shell_{}", i)),
                    action: LocalShellAction::Run {
                        command: format!("echo 'Command {}'", i),
                    },
                }),
                _ => history.push(ResponseItem::Message {
                    id: Some(format!("img_{}", i)),
                    role: "user".to_string(),
                    content: vec![ContentItem::InputImage {
                        image_url: "https://example.com/image.jpg".to_string(),
                    }],
                }),
            }
        }

        let (breakdown, duration) =
            measure_time(|| analyze_context(Some(system_prompt), &history, Some(tools)));

        assert!(breakdown.system_prompt > 0);
        assert!(breakdown.conversation > 0);
        assert!(breakdown.tools > 0);
        assert!(
            duration.as_millis() < 50,
            "Full context analysis with 200 items took {:?}, expected < 50ms",
            duration
        );
    }

    #[test]
    fn test_performance_context_breakdown_operations() {
        let mut breakdown = ContextBreakdown::new();
        breakdown.system_prompt = 1000;
        breakdown.conversation = 5000;
        breakdown.tools = 500;

        // Test total calculation performance
        let (total, duration) = measure_time(|| {
            let mut sum = 0;
            for _ in 0..10000 {
                sum += breakdown.total();
            }
            sum
        });

        assert!(total > 0);
        assert!(
            duration.as_micros() < 1000,
            "10000 total calculations took {:?}, expected < 1ms",
            duration
        );

        // Test cloning performance
        let (_, duration) = measure_time(|| {
            for _ in 0..1000 {
                let _ = breakdown.clone();
            }
        });

        assert!(
            duration.as_micros() < 500,
            "1000 clones took {:?}, expected < 500µs",
            duration
        );
    }

    #[test]
    fn test_performance_serialization() {
        let mut breakdown = ContextBreakdown::new();
        breakdown.system_prompt = 150;
        breakdown.conversation = 3500;
        breakdown.tools = 250;

        // Test serialization performance
        let (json, duration) = measure_time(|| serde_json::to_string(&breakdown).unwrap());

        assert!(!json.is_empty());
        assert!(
            duration.as_micros() < 100,
            "Serialization took {:?}, expected < 100µs",
            duration
        );

        // Test deserialization performance
        let (deserialized, duration) =
            measure_time(|| serde_json::from_str::<ContextBreakdown>(&json).unwrap());

        assert_eq!(deserialized.total(), breakdown.total());
        assert!(
            duration.as_micros() < 100,
            "Deserialization took {:?}, expected < 100µs",
            duration
        );
    }

    #[test]
    fn test_performance_concurrent_analysis() {
        use std::sync::Arc;
        use std::thread;

        let system_prompt = Arc::new("Test prompt".to_string());
        let tools = Arc::new(r#"{"tools": []}"#.to_string());
        let history = Arc::new(vec![ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "Test message".to_string(),
            }],
        }]);

        let (_, duration) = measure_time(|| {
            let mut handles = vec![];

            // Spawn 10 threads doing concurrent analysis
            for _ in 0..10 {
                let prompt = Arc::clone(&system_prompt);
                let tools_clone = Arc::clone(&tools);
                let history_clone = Arc::clone(&history);

                let handle = thread::spawn(move || {
                    analyze_context(Some(&prompt), &history_clone, Some(&tools_clone))
                });

                handles.push(handle);
            }

            // Wait for all threads
            for handle in handles {
                handle.join().unwrap();
            }
        });

        assert!(
            duration.as_millis() < 10,
            "10 concurrent analyses took {:?}, expected < 10ms",
            duration
        );
    }

    #[test]
    fn test_performance_worst_case_long_strings() {
        // Test with very long strings
        let long_string = "a".repeat(1_000_000); // 1 million characters

        let (tokens, duration) = measure_time(|| estimate_tokens(&long_string));

        assert!(tokens > 0);
        assert!(
            duration.as_millis() < 100,
            "1M character tokenization took {:?}, expected < 100ms",
            duration
        );
    }

    #[test]
    fn test_performance_worst_case_many_items() {
        let mut history = Vec::new();

        // Create 10,000 small messages
        for i in 0..10000 {
            history.push(ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputText {
                    text: "Hi".to_string(),
                }],
            });
        }

        let (breakdown, duration) = measure_time(|| analyze_context(None, &history, None));

        assert!(breakdown.conversation > 0);
        assert!(
            duration.as_millis() < 500,
            "10,000 messages analysis took {:?}, expected < 500ms",
            duration
        );
    }
}
