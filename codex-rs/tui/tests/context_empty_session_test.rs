//! Test for Context command with empty session (no conversation history)

use codex_core::config::{Config, ConfigOverrides, ConfigToml};
use codex_core::protocol::TokenUsage;
use codex_protocol::mcp_protocol::ConversationId;

/// Test that new_context_output handles empty session gracefully
#[test]
fn test_context_command_empty_session_no_crash() {
    // Create a test config
    let config = Config::load_from_base_config_with_overrides(
        ConfigToml::default(),
        ConfigOverrides::default(),
        std::env::temp_dir(),
    )
    .expect("Failed to create config");

    // Create empty token usage (simulating a new session with no history)
    let usage = TokenUsage::default();
    
    // No session ID for a brand new session
    let session_id: Option<ConversationId> = None;
    
    // This should not panic
    let cell = codex_tui::history_cell::new_context_output(&config, &usage, &session_id);
    
    // Verify the cell contains expected output
    let lines = cell.display_lines(80);
    
    // Check that we have some output
    assert!(!lines.is_empty(), "Context output should not be empty");
    
    // Check for key elements
    let output_text = lines
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");
    
    // Verify key sections are present
    assert!(output_text.contains("/context"), "Should show /context header");
    assert!(output_text.contains("Context Window Usage"), "Should show context window usage section");
    assert!(output_text.contains("Component Breakdown"), "Should show component breakdown");
    assert!(output_text.contains("Model"), "Should show model info");
    
    // Verify zero values are handled properly
    assert!(output_text.contains("0"), "Should show 0 tokens for empty session");
    assert!(output_text.contains("0%"), "Should show 0% usage");
    
    // Verify no crash occurs with empty conversation
    assert!(output_text.contains("Total:"), "Should show total usage");
    assert!(output_text.contains("Input:"), "Should show input tokens");
    assert!(output_text.contains("Output:"), "Should show output tokens");
}

/// Test that new_context_output shows warning at high usage
#[test]
fn test_context_command_high_usage_warning() {
    let config = Config::load_from_base_config_with_overrides(
        ConfigToml::default(),
        ConfigOverrides::default(),
        std::env::temp_dir(),
    )
    .expect("Failed to create config");

    // Create high token usage (> 70% of 128k context window)
    let mut usage = TokenUsage::default();
    usage.input_tokens = 100000; // About 78% of 128k
    
    let session_id: Option<ConversationId> = None;
    
    let cell = codex_tui::history_cell::new_context_output(&config, &usage, &session_id);
    let lines = cell.display_lines(80);
    
    let output_text = lines
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");
    
    // Should show warning for high usage
    assert!(output_text.contains("High Context Usage Warning") || output_text.contains("⚠"), 
            "Should show warning for high context usage");
    assert!(output_text.contains("/compact"), "Should suggest /compact command");
}

/// Test that new_context_output handles session with conversation history
#[test]
fn test_context_command_with_conversation_history() {
    let config = Config::load_from_base_config_with_overrides(
        ConfigToml::default(),
        ConfigOverrides::default(),
        std::env::temp_dir(),
    )
    .expect("Failed to create config");

    // Create token usage with actual values
    let mut usage = TokenUsage::default();
    usage.input_tokens = 5000;
    usage.output_tokens = 1500;
    usage.cached_input_tokens = 500;
    usage.reasoning_output_tokens = 200;
    
    let session_id = Some(ConversationId::new());
    
    let cell = codex_tui::history_cell::new_context_output(&config, &usage, &session_id);
    let lines = cell.display_lines(80);
    
    let output_text = lines
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");
    
    // Verify all token types are shown
    assert!(output_text.contains("5,000") || output_text.contains("5000"), 
            "Should show input tokens");
    assert!(output_text.contains("1,500") || output_text.contains("1500"), 
            "Should show output tokens");
    assert!(output_text.contains("Cached"), "Should show cached tokens when present");
    assert!(output_text.contains("Reasoning"), "Should show reasoning tokens when present");
    
    // Verify session ID is shown
    assert!(output_text.contains("Session") || output_text.contains("ID:"), 
            "Should show session info when available");
}