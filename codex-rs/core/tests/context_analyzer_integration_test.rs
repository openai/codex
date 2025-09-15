//! Integration tests for the context analyzer module
//! These tests verify that the module is properly exported and can be used from external code

#[test]
fn test_module_exports() {
    // Verify that all public types and functions are accessible
    use codex_core::context_analyzer::{analyze_context, estimate_tokens, ContextBreakdown};
    
    // Test that we can create a ContextBreakdown
    let breakdown = ContextBreakdown::new();
    assert_eq!(breakdown.total(), 0);
    
    // Test that we can use estimate_tokens
    let tokens = estimate_tokens("Hello, world!");
    assert!(tokens > 0);
    
    // Test that we can use analyze_context
    let result = analyze_context(None, &[], None);
    assert_eq!(result.total(), 0);
}

#[test]
fn test_public_api_completeness() {
    // This test ensures all expected public APIs are available
    use codex_core::context_analyzer::ContextBreakdown;
    
    // Create instance using new()
    let mut breakdown = ContextBreakdown::new();
    
    // Access all public fields
    breakdown.system_prompt = 100;
    breakdown.conversation = 200;
    breakdown.tools = 50;
    
    // Use public methods
    let total = breakdown.total();
    assert_eq!(total, 350);
    
    // Test Clone trait
    let cloned = breakdown.clone();
    assert_eq!(cloned.total(), breakdown.total());
    
    // Test Debug trait
    let debug_str = format!("{:?}", breakdown);
    assert!(debug_str.contains("ContextBreakdown"));
}

#[test]
fn test_serialization_deserialization() {
    use codex_core::context_analyzer::ContextBreakdown;
    use serde_json;
    
    let mut breakdown = ContextBreakdown::new();
    breakdown.system_prompt = 123;
    breakdown.conversation = 456;
    breakdown.tools = 789;
    
    // Serialize
    let json = serde_json::to_string(&breakdown).expect("Serialization should work");
    assert!(json.contains("system_prompt"));
    assert!(json.contains("conversation"));
    assert!(json.contains("tools"));
    
    // Deserialize
    let deserialized: ContextBreakdown = 
        serde_json::from_str(&json).expect("Deserialization should work");
    
    assert_eq!(deserialized.system_prompt, breakdown.system_prompt);
    assert_eq!(deserialized.conversation, breakdown.conversation);
    assert_eq!(deserialized.tools, breakdown.tools);
}

#[test]
fn test_real_world_usage_scenario() {
    use codex_core::context_analyzer::{analyze_context, ContextBreakdown};
    use codex_protocol::models::{ContentItem, ResponseItem};
    
    // Simulate a real-world usage scenario
    let system_prompt = "You are a helpful assistant.";
    let tools_definition = r#"{"tools": [{"name": "search", "description": "Search the web"}]}"#;
    
    let conversation = vec![
        ResponseItem::Message {
            id: Some("1".to_string()),
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "What is the weather like today?".to_string(),
            }],
        },
        ResponseItem::Message {
            id: Some("2".to_string()),
            role: "assistant".to_string(),
            content: vec![ContentItem::OutputText {
                text: "I'll help you check the weather. Let me search for current conditions.".to_string(),
            }],
        },
    ];
    
    let breakdown = analyze_context(
        Some(system_prompt),
        &conversation,
        Some(tools_definition),
    );
    
    // Verify all components are counted
    assert!(breakdown.system_prompt > 0, "System prompt should have tokens");
    assert!(breakdown.conversation > 0, "Conversation should have tokens");
    assert!(breakdown.tools > 0, "Tools should have tokens");
    
    // Verify total is sum of parts
    assert_eq!(
        breakdown.total(),
        breakdown.system_prompt + breakdown.conversation + breakdown.tools
    );
    
    // Use the breakdown in a typical way
    let total_tokens = breakdown.total();
    let context_limit = 4096;
    
    if total_tokens > context_limit {
        println!("Warning: Context exceeds limit ({} > {})", total_tokens, context_limit);
    } else {
        println!("Context usage: {}/{} tokens", total_tokens, context_limit);
    }
}

#[test]
fn test_thread_safety() {
    use codex_core::context_analyzer::{analyze_context, estimate_tokens, ContextBreakdown};
    use std::sync::{Arc, Mutex};
    use std::thread;
    
    // Test that ContextBreakdown can be shared safely
    let shared_breakdown = Arc::new(Mutex::new(ContextBreakdown::new()));
    
    let mut handles = vec![];
    
    for i in 0..10 {
        let breakdown_clone = Arc::clone(&shared_breakdown);
        
        let handle = thread::spawn(move || {
            let mut breakdown = breakdown_clone.lock().unwrap();
            
            // Modify different fields from different threads
            match i % 3 {
                0 => breakdown.system_prompt += 10,
                1 => breakdown.conversation += 20,
                _ => breakdown.tools += 5,
            }
        });
        
        handles.push(handle);
    }
    
    // Wait for all threads to complete
    for handle in handles {
        handle.join().unwrap();
    }
    
    let final_breakdown = shared_breakdown.lock().unwrap();
    assert!(final_breakdown.total() > 0);
}

#[test]
fn test_estimate_tokens_consistency() {
    use codex_core::context_analyzer::estimate_tokens;
    
    // Test that the same input always produces the same output
    let text = "This is a test sentence for consistency checking.";
    
    let mut results = Vec::new();
    for _ in 0..100 {
        results.push(estimate_tokens(text));
    }
    
    // All results should be identical
    let first = results[0];
    assert!(
        results.iter().all(|&r| r == first),
        "estimate_tokens should be deterministic"
    );
}

#[test]
fn test_analyze_context_with_all_response_types() {
    use codex_core::context_analyzer::analyze_context;
    use codex_protocol::models::{
        ContentItem, FunctionCallOutputPayload, LocalShellAction,
        ReasoningItemContent, ReasoningItemReasoningSummary, ResponseItem,
        WebSearchAction, CustomToolCall, CustomToolCallOutput,
    };
    
    // Create a history with all possible ResponseItem variants
    let history = vec![
        // Message
        ResponseItem::Message {
            id: Some("msg".to_string()),
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "Test message".to_string(),
            }],
        },
        // Reasoning
        ResponseItem::Reasoning {
            id: Some("reason".to_string()),
            summary: vec![ReasoningItemReasoningSummary::SummaryText {
                text: "Reasoning summary".to_string(),
            }],
            content: Some(vec![ReasoningItemContent::Text {
                text: "Reasoning content".to_string(),
            }]),
            encrypted_content: None,
        },
        // FunctionCall
        ResponseItem::FunctionCall {
            id: Some("func".to_string()),
            name: "test_function".to_string(),
            arguments: "{}".to_string(),
            call_id: "call_123".to_string(),
        },
        // FunctionCallOutput
        ResponseItem::FunctionCallOutput {
            call_id: "call_123".to_string(),
            output: FunctionCallOutputPayload {
                content: "Function output".to_string(),
                success: Some(true),
            },
        },
        // CustomToolCall
        ResponseItem::CustomToolCall {
            id: Some("tool".to_string()),
            name: "custom_tool".to_string(),
            input: "{}".to_string(),
            call_id: "tool_456".to_string(),
        },
        // CustomToolCallOutput
        ResponseItem::CustomToolCallOutput {
            call_id: "tool_456".to_string(),
            output: "Tool output".to_string(),
        },
        // LocalShellCall with Run action
        ResponseItem::LocalShellCall {
            id: Some("shell".to_string()),
            action: LocalShellAction::Run {
                command: "echo test".to_string(),
            },
        },
        // WebSearchCall
        ResponseItem::WebSearchCall {
            id: Some("search".to_string()),
            action: WebSearchAction::Search {
                query: "test query".to_string(),
            },
        },
        // Other
        ResponseItem::Other,
    ];
    
    let breakdown = analyze_context(None, &history, None);
    
    // Should handle all types without panicking
    assert!(breakdown.conversation >= 0);
}

#[test]
fn test_context_breakdown_default_trait() {
    use codex_core::context_analyzer::ContextBreakdown;
    
    // Test that Default trait is implemented
    let breakdown: ContextBreakdown = Default::default();
    assert_eq!(breakdown.system_prompt, 0);
    assert_eq!(breakdown.conversation, 0);
    assert_eq!(breakdown.tools, 0);
    assert_eq!(breakdown.total(), 0);
}

#[test]
fn test_error_handling_resilience() {
    use codex_core::context_analyzer::{analyze_context, estimate_tokens};
    use codex_protocol::models::{ContentItem, ResponseItem};
    
    // Test with potentially problematic inputs
    
    // Empty string
    let tokens = estimate_tokens("");
    assert_eq!(tokens, 0);
    
    // Very long string (shouldn't panic or overflow)
    let long_string = "a".repeat(10_000_000);
    let tokens = estimate_tokens(&long_string);
    assert!(tokens > 0);
    
    // History with empty content
    let history = vec![
        ResponseItem::Message {
            id: None,
            role: "".to_string(), // Empty role
            content: vec![], // Empty content
        },
    ];
    
    let breakdown = analyze_context(None, &history, None);
    assert!(breakdown.conversation >= 0); // Should handle gracefully
    
    // Malformed tools JSON (should still count tokens)
    let malformed_tools = "{ this is not valid JSON }";
    let breakdown = analyze_context(None, &[], Some(malformed_tools));
    assert!(breakdown.tools > 0);
}

#[test]
fn test_cross_crate_usage() {
    // This test simulates usage from another crate
    // It ensures the public API is stable and usable
    
    // Import only what's publicly available
    use codex_core::context_analyzer;
    
    // Use the module's public functions
    let tokens = context_analyzer::estimate_tokens("Test text");
    assert!(tokens > 0);
    
    // Create and use ContextBreakdown
    let mut breakdown = context_analyzer::ContextBreakdown::new();
    breakdown.system_prompt = 50;
    breakdown.conversation = 100;
    breakdown.tools = 25;
    
    let total = breakdown.total();
    assert_eq!(total, 175);
    
    // Use analyze_context
    let result = context_analyzer::analyze_context(
        Some("System prompt"),
        &[],
        Some("Tools definition"),
    );
    
    assert!(result.system_prompt > 0);
    assert!(result.tools > 0);
}