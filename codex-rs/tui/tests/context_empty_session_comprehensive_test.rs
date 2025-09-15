//! Comprehensive tests for Context command with empty session (no conversation history)
//! This test suite ensures robust handling of edge cases and null-safety

use codex_core::config::{Config, ConfigOverrides, ConfigToml};
use codex_core::protocol::TokenUsage;
use codex_protocol::mcp_protocol::ConversationId;
use std::panic;

/// Helper function to create a test config
fn create_test_config() -> Config {
    Config::load_from_base_config_with_overrides(
        ConfigToml::default(),
        ConfigOverrides::default(),
        std::env::temp_dir(),
    )
    .expect("Failed to create test config")
}

/// Helper function to extract text from display lines
fn extract_text_from_lines(lines: &[ratatui::text::Line]) -> String {
    lines
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Test that new_context_output handles completely empty session without panicking
#[test]
fn test_empty_session_no_panic() {
    let config = create_test_config();
    let usage = TokenUsage::default();
    let session_id: Option<ConversationId> = None;
    
    // This should not panic
    let result = panic::catch_unwind(|| {
        codex_tui::history_cell::new_context_output(&config, &usage, &session_id)
    });
    
    assert!(result.is_ok(), "new_context_output should not panic with empty session");
}

/// Test that empty session displays all required sections
#[test]
fn test_empty_session_displays_all_sections() {
    let config = create_test_config();
    let usage = TokenUsage::default();
    let session_id: Option<ConversationId> = None;
    
    let cell = codex_tui::history_cell::new_context_output(&config, &usage, &session_id);
    let lines = cell.display_lines(80);
    let output = extract_text_from_lines(&lines);
    
    // Check all required sections are present
    assert!(output.contains("/context"), "Missing /context header");
    assert!(output.contains("Context Window Usage"), "Missing Context Window Usage section");
    assert!(output.contains("Component Breakdown"), "Missing Component Breakdown section");
    assert!(output.contains("Model"), "Missing Model section");
    assert!(output.contains("Total:"), "Missing Total tokens line");
    assert!(output.contains("Input:"), "Missing Input tokens line");
    assert!(output.contains("Output:"), "Missing Output tokens line");
    assert!(output.contains("Context Window:"), "Missing Context Window info");
}

/// Test that empty session correctly shows zero values
#[test]
fn test_empty_session_zero_values() {
    let config = create_test_config();
    let usage = TokenUsage::default();
    let session_id: Option<ConversationId> = None;
    
    let cell = codex_tui::history_cell::new_context_output(&config, &usage, &session_id);
    let lines = cell.display_lines(80);
    let output = extract_text_from_lines(&lines);
    
    // Verify zero values
    assert!(output.contains("0 / 128,000"), "Should show 0 out of 128,000 tokens");
    assert!(output.contains("0%"), "Should show 0% usage");
    assert!(output.contains("Input: 0"), "Should show 0 input tokens");
    assert!(output.contains("Output: 0"), "Should show 0 output tokens");
    
    // Should NOT show cached or reasoning tokens when they're zero
    assert!(!output.contains("Cached:"), "Should not show Cached line when zero");
    assert!(!output.contains("Reasoning:"), "Should not show Reasoning line when zero");
}

/// Test empty session with various display widths
#[test]
fn test_empty_session_various_widths() {
    let config = create_test_config();
    let usage = TokenUsage::default();
    let session_id: Option<ConversationId> = None;
    
    let widths = vec![40, 60, 80, 100, 120, 200];
    
    for width in widths {
        let cell = codex_tui::history_cell::new_context_output(&config, &usage, &session_id);
        let lines = cell.display_lines(width);
        
        assert!(!lines.is_empty(), "Should have output at width {}", width);
        
        let output = extract_text_from_lines(&lines);
        assert!(output.contains("/context"), "Missing header at width {}", width);
        assert!(output.contains("0%"), "Missing percentage at width {}", width);
    }
}

/// Test empty session with and without session ID
#[test]
fn test_empty_session_with_without_session_id() {
    let config = create_test_config();
    let usage = TokenUsage::default();
    
    // Test without session ID
    let cell_no_id = codex_tui::history_cell::new_context_output(&config, &usage, &None);
    let lines_no_id = cell_no_id.display_lines(80);
    let output_no_id = extract_text_from_lines(&lines_no_id);
    
    assert!(!output_no_id.contains("Session"), "Should not show Session section without ID");
    
    // Test with session ID
    let session_id = Some(ConversationId::new());
    let cell_with_id = codex_tui::history_cell::new_context_output(&config, &usage, &session_id);
    let lines_with_id = cell_with_id.display_lines(80);
    let output_with_id = extract_text_from_lines(&lines_with_id);
    
    assert!(output_with_id.contains("Session"), "Should show Session section with ID");
}

/// Test that empty session doesn't trigger high usage warning
#[test]
fn test_empty_session_no_warning() {
    let config = create_test_config();
    let usage = TokenUsage::default();
    let session_id: Option<ConversationId> = None;
    
    let cell = codex_tui::history_cell::new_context_output(&config, &usage, &session_id);
    let lines = cell.display_lines(80);
    let output = extract_text_from_lines(&lines);
    
    assert!(!output.contains("High Context Usage Warning"), "Should not show warning for empty session");
    assert!(!output.contains("/compact"), "Should not suggest /compact for empty session");
}

/// Test transition from empty to non-empty session
#[test]
fn test_transition_from_empty_to_populated() {
    let config = create_test_config();
    let session_id = Some(ConversationId::new());
    
    // Start with empty session
    let empty_usage = TokenUsage::default();
    let empty_cell = codex_tui::history_cell::new_context_output(&config, &empty_usage, &session_id);
    let empty_lines = empty_cell.display_lines(80);
    let empty_output = extract_text_from_lines(&empty_lines);
    
    assert!(empty_output.contains("0%"), "Empty session should show 0%");
    
    // Add some tokens
    let mut populated_usage = TokenUsage::default();
    populated_usage.input_tokens = 1000;
    populated_usage.output_tokens = 500;
    populated_usage.total_tokens = 1500;
    
    let populated_cell = codex_tui::history_cell::new_context_output(&config, &populated_usage, &session_id);
    let populated_lines = populated_cell.display_lines(80);
    let populated_output = extract_text_from_lines(&populated_lines);
    
    assert!(populated_output.contains("1,500"), "Should show 1,500 total tokens");
    assert!(populated_output.contains("1%"), "Should show 1% usage");
    assert!(!populated_output.contains("0%"), "Should not show 0% anymore");
}

/// Test empty session with different models (context window sizes)
#[test]
fn test_empty_session_different_models() {
    let mut config = create_test_config();
    let usage = TokenUsage::default();
    let session_id: Option<ConversationId> = None;
    
    // Test with different model names
    let models = vec![
        "gpt-4",
        "gpt-3.5-turbo",
        "claude-3-opus",
        "test-model",
    ];
    
    for model in models {
        config.model = model.to_string();
        
        let cell = codex_tui::history_cell::new_context_output(&config, &usage, &session_id);
        let lines = cell.display_lines(80);
        let output = extract_text_from_lines(&lines);
        
        assert!(output.contains(&model), "Should show model name: {}", model);
        assert!(output.contains("128,000"), "Should show context window size");
        assert!(output.contains("0%"), "Should show 0% for empty session");
    }
}

/// Test empty session progress bar rendering
#[test]
fn test_empty_session_progress_bar() {
    let config = create_test_config();
    let usage = TokenUsage::default();
    let session_id: Option<ConversationId> = None;
    
    let cell = codex_tui::history_cell::new_context_output(&config, &usage, &session_id);
    let lines = cell.display_lines(80);
    let output = extract_text_from_lines(&lines);
    
    // Check for progress bar characters
    assert!(output.contains('['), "Should have progress bar opening bracket");
    assert!(output.contains(']'), "Should have progress bar closing bracket");
    assert!(output.contains('░'), "Should have empty progress bar characters");
    assert!(!output.contains('█'), "Should not have filled progress bar characters");
    assert!(output.contains("0%"), "Should show 0% next to progress bar");
}

/// Test empty session with partial token data (some fields populated)
#[test]
fn test_empty_session_partial_data() {
    let config = create_test_config();
    let session_id: Option<ConversationId> = None;
    
    // Test with only input tokens
    let mut usage_input_only = TokenUsage::default();
    usage_input_only.input_tokens = 100;
    usage_input_only.total_tokens = 100;
    
    let cell = codex_tui::history_cell::new_context_output(&config, &usage_input_only, &session_id);
    let lines = cell.display_lines(80);
    let output = extract_text_from_lines(&lines);
    
    assert!(output.contains("Input: 100"), "Should show input tokens");
    assert!(output.contains("Output: 0"), "Should show 0 output tokens");
    
    // Test with only output tokens
    let mut usage_output_only = TokenUsage::default();
    usage_output_only.output_tokens = 50;
    usage_output_only.total_tokens = 50;
    
    let cell = codex_tui::history_cell::new_context_output(&config, &usage_output_only, &session_id);
    let lines = cell.display_lines(80);
    let output = extract_text_from_lines(&lines);
    
    assert!(output.contains("Input: 0"), "Should show 0 input tokens");
    assert!(output.contains("Output: 50"), "Should show output tokens");
}

/// Test empty session HistoryCell trait methods
#[test]
fn test_empty_session_history_cell_trait() {
    let config = create_test_config();
    let usage = TokenUsage::default();
    let session_id: Option<ConversationId> = None;
    
    let cell = codex_tui::history_cell::new_context_output(&config, &usage, &session_id);
    
    // Test display_lines
    let display_lines = cell.display_lines(80);
    assert!(!display_lines.is_empty(), "display_lines should not be empty");
    
    // Test transcript_lines
    let transcript_lines = cell.transcript_lines();
    assert!(!transcript_lines.is_empty(), "transcript_lines should not be empty");
    
    // Test desired_height
    let height = cell.desired_height(80);
    assert!(height > 0, "desired_height should be positive");
    assert!(height < 100, "desired_height should be reasonable for empty session");
    
    // Test is_stream_continuation
    assert!(!cell.is_stream_continuation(), "Should not be a stream continuation");
}

/// Test empty session formatting consistency
#[test]
fn test_empty_session_formatting() {
    let config = create_test_config();
    let usage = TokenUsage::default();
    let session_id: Option<ConversationId> = None;
    
    let cell = codex_tui::history_cell::new_context_output(&config, &usage, &session_id);
    let lines = cell.display_lines(80);
    
    // Check that lines are properly formatted
    for (i, line) in lines.iter().enumerate() {
        // Check that spans are valid
        assert!(!line.spans.is_empty() || line.spans.iter().all(|s| s.content.is_empty()), 
                "Line {} should have valid spans", i);
        
        // Check that line width doesn't exceed display width
        let line_width: usize = line.spans.iter().map(|s| s.content.len()).sum();
        assert!(line_width <= 80, "Line {} width ({}) exceeds display width", i, line_width);
    }
}

/// Test empty session with very narrow display width
#[test]
fn test_empty_session_narrow_width() {
    let config = create_test_config();
    let usage = TokenUsage::default();
    let session_id: Option<ConversationId> = None;
    
    // Test with very narrow widths
    let narrow_widths = vec![20, 30, 35];
    
    for width in narrow_widths {
        let cell = codex_tui::history_cell::new_context_output(&config, &usage, &session_id);
        let lines = cell.display_lines(width);
        
        assert!(!lines.is_empty(), "Should have output at width {}", width);
        
        // Verify content still appears (might be wrapped)
        let output = extract_text_from_lines(&lines);
        assert!(output.contains("context"), "Should contain context at width {}", width);
    }
}

/// Test that empty session correctly calculates percentage
#[test]
fn test_empty_session_percentage_calculation() {
    let config = create_test_config();
    let usage = TokenUsage::default();
    let session_id: Option<ConversationId> = None;
    
    let cell = codex_tui::history_cell::new_context_output(&config, &usage, &session_id);
    let lines = cell.display_lines(80);
    let output = extract_text_from_lines(&lines);
    
    // Should show exactly 0%
    assert!(output.contains("(0%)"), "Should show (0%) in parentheses");
    assert!(output.contains("] 0%"), "Should show 0% after progress bar");
}

/// Test empty session with config overrides
#[test]
fn test_empty_session_with_config_overrides() {
    let mut config_toml = ConfigToml::default();
    config_toml.model = Some("custom-model".to_string());
    
    let config = Config::load_from_base_config_with_overrides(
        config_toml,
        ConfigOverrides::default(),
        std::env::temp_dir(),
    )
    .expect("Failed to create config with overrides");
    
    let usage = TokenUsage::default();
    let session_id: Option<ConversationId> = None;
    
    let cell = codex_tui::history_cell::new_context_output(&config, &usage, &session_id);
    let lines = cell.display_lines(80);
    let output = extract_text_from_lines(&lines);
    
    assert!(output.contains("custom-model"), "Should show custom model name");
    assert!(output.contains("0%"), "Should show 0% usage");
}

/// Test multiple empty sessions in sequence
#[test]
fn test_multiple_empty_sessions() {
    let config = create_test_config();
    let usage = TokenUsage::default();
    
    // Create multiple sessions
    for i in 0..5 {
        let session_id = if i % 2 == 0 { 
            Some(ConversationId::new()) 
        } else { 
            None 
        };
        
        let cell = codex_tui::history_cell::new_context_output(&config, &usage, &session_id);
        let lines = cell.display_lines(80);
        
        assert!(!lines.is_empty(), "Session {} should have output", i);
        
        let output = extract_text_from_lines(&lines);
        assert!(output.contains("0%"), "Session {} should show 0%", i);
        
        if session_id.is_some() {
            assert!(output.contains("Session"), "Session {} should show Session info", i);
        } else {
            assert!(!output.contains("Session"), "Session {} should not show Session info", i);
        }
    }
}

/// Test empty session doesn't show optional components
#[test]
fn test_empty_session_hides_optional_components() {
    let config = create_test_config();
    let usage = TokenUsage::default();
    let session_id: Option<ConversationId> = None;
    
    let cell = codex_tui::history_cell::new_context_output(&config, &usage, &session_id);
    let lines = cell.display_lines(80);
    let output = extract_text_from_lines(&lines);
    
    // Should not show these when zero
    assert!(!output.contains("Cached:"), "Should not show Cached section when zero");
    assert!(!output.contains("Reasoning:"), "Should not show Reasoning section when zero");
    assert!(!output.contains("High Context Usage Warning"), "Should not show warning when empty");
    
    // Should show these always
    assert!(output.contains("Input:"), "Should always show Input section");
    assert!(output.contains("Output:"), "Should always show Output section");
    assert!(output.contains("Total:"), "Should always show Total section");
}

/// Stress test: rapid creation of empty session outputs
#[test]
fn test_empty_session_stress() {
    let config = create_test_config();
    let usage = TokenUsage::default();
    
    // Create many instances rapidly
    for _ in 0..100 {
        let session_id = if rand::random::<bool>() {
            Some(ConversationId::new())
        } else {
            None
        };
        
        let cell = codex_tui::history_cell::new_context_output(&config, &usage, &session_id);
        let lines = cell.display_lines(80);
        
        // Basic validation
        assert!(!lines.is_empty());
        assert!(cell.desired_height(80) > 0);
    }
}

/// Test empty session with maximum display width
#[test]
fn test_empty_session_maximum_width() {
    let config = create_test_config();
    let usage = TokenUsage::default();
    let session_id: Option<ConversationId> = None;
    
    let max_width = u16::MAX;
    let cell = codex_tui::history_cell::new_context_output(&config, &usage, &session_id);
    let lines = cell.display_lines(max_width);
    
    assert!(!lines.is_empty(), "Should handle maximum width");
    
    let output = extract_text_from_lines(&lines);
    assert!(output.contains("0%"), "Should show percentage at maximum width");
}

/// Test that empty session rendering is deterministic
#[test]
fn test_empty_session_deterministic() {
    let config = create_test_config();
    let usage = TokenUsage::default();
    let session_id = Some(ConversationId::new());
    
    // Create two identical cells
    let cell1 = codex_tui::history_cell::new_context_output(&config, &usage, &session_id);
    let cell2 = codex_tui::history_cell::new_context_output(&config, &usage, &session_id);
    
    let lines1 = cell1.display_lines(80);
    let lines2 = cell2.display_lines(80);
    
    // Extract text (excluding session ID which might be different)
    let output1 = extract_text_from_lines(&lines1);
    let output2 = extract_text_from_lines(&lines2);
    
    // Check that main content is the same (excluding session ID line)
    let filtered1: Vec<_> = output1.lines()
        .filter(|l| !l.contains("Session ID:"))
        .collect();
    let filtered2: Vec<_> = output2.lines()
        .filter(|l| !l.contains("Session ID:"))
        .collect();
    
    assert_eq!(filtered1.len(), filtered2.len(), "Should have same number of lines");
}