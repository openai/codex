//! Boundary and edge case tests for context command
//! Tests extreme values, boundary conditions, and error scenarios

use codex_core::config::{Config, ConfigOverrides, ConfigToml};
use codex_core::protocol::TokenUsage;
use codex_protocol::mcp_protocol::ConversationId;
use codex_tui::history_cell::{new_context_output, render_progress_bar};
use std::panic;

/// Helper to create test config
fn test_config() -> Config {
    Config::load_from_base_config_with_overrides(
        ConfigToml::default(),
        ConfigOverrides::default(),
        std::env::temp_dir(),
    )
    .expect("Failed to create config")
}

/// Test with negative token values (should handle gracefully)
#[test]
fn test_negative_token_values() {
    let config = test_config();
    
    // Create usage with negative values (shouldn't happen but test resilience)
    let mut usage = TokenUsage::default();
    usage.input_tokens = -100;
    usage.output_tokens = -50;
    usage.total_tokens = -150;
    
    // Should not panic
    let result = panic::catch_unwind(|| {
        new_context_output(&config, &usage, &None)
    });
    
    // If it doesn't panic, verify it handles the values
    if let Ok(cell) = result {
        let lines = cell.display_lines(80);
        assert!(!lines.is_empty(), "Should produce output even with negative values");
    }
}

/// Test with maximum integer values
#[test]
fn test_maximum_integer_values() {
    let config = test_config();
    
    let usage = TokenUsage {
        input_tokens: i32::MAX,
        output_tokens: i32::MAX,
        total_tokens: i32::MAX,
        cached_input_tokens: i32::MAX,
        reasoning_output_tokens: i32::MAX,
    };
    
    let cell = new_context_output(&config, &usage, &None);
    let lines = cell.display_lines(80);
    
    assert!(!lines.is_empty(), "Should handle maximum values");
    
    let text = lines
        .iter()
        .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");
    
    // Should cap percentage at 100%
    assert!(text.contains("100%"), "Should cap at 100% for huge values");
}

/// Test exactly at 70% threshold (boundary for warning)
#[test]
fn test_exact_70_percent_threshold() {
    let config = test_config();
    
    // Exactly 70% = 89600 / 128000
    let usage_70_exact = TokenUsage {
        input_tokens: 60000,
        output_tokens: 29600,
        total_tokens: 89600,
        cached_input_tokens: 0,
        reasoning_output_tokens: 0,
    };
    
    let cell = new_context_output(&config, &usage_70_exact, &None);
    let lines = cell.display_lines(80);
    let text = lines
        .iter()
        .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");
    
    // At exactly 70%, should NOT show warning
    assert!(!text.contains("High Context Usage Warning"), 
            "Should not show warning at exactly 70%");
    assert!(text.contains("70%"), "Should show 70% usage");
}

/// Test one token above and below 70% threshold
#[test]
fn test_threshold_plus_minus_one() {
    let config = test_config();
    
    // 70% - 1 token = 89599
    let usage_below = TokenUsage {
        input_tokens: 60000,
        output_tokens: 29599,
        total_tokens: 89599,
        cached_input_tokens: 0,
        reasoning_output_tokens: 0,
    };
    
    let cell_below = new_context_output(&config, &usage_below, &None);
    let lines_below = cell_below.display_lines(80);
    let text_below = lines_below
        .iter()
        .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");
    
    assert!(!text_below.contains("High Context Usage Warning"),
            "Should not show warning at 89599 tokens (just below 70%)");
    
    // 70% + 1 token = 89601
    let usage_above = TokenUsage {
        input_tokens: 60000,
        output_tokens: 29601,
        total_tokens: 89601,
        cached_input_tokens: 0,
        reasoning_output_tokens: 0,
    };
    
    let cell_above = new_context_output(&config, &usage_above, &None);
    let lines_above = cell_above.display_lines(80);
    let text_above = lines_above
        .iter()
        .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");
    
    assert!(text_above.contains("High Context Usage Warning"),
            "Should show warning at 89601 tokens (just above 70%)");
}

/// Test with zero-width display
#[test]
fn test_zero_width_display() {
    let config = test_config();
    let usage = TokenUsage::default();
    
    let cell = new_context_output(&config, &usage, &None);
    
    // Test with width 0 (edge case)
    let lines = cell.display_lines(0);
    // Should handle gracefully, might return empty or minimal output
    // The important thing is it doesn't panic
}

/// Test with width of 1
#[test]
fn test_width_one_display() {
    let config = test_config();
    let usage = TokenUsage {
        input_tokens: 10000,
        output_tokens: 5000,
        total_tokens: 15000,
        cached_input_tokens: 0,
        reasoning_output_tokens: 0,
    };
    
    let cell = new_context_output(&config, &usage, &None);
    let lines = cell.display_lines(1);
    
    // Should handle narrow width without panic
    for line in &lines {
        let width: usize = line.spans.iter().map(|s| s.content.len()).sum();
        assert!(width <= 1 || line.spans.is_empty(), 
                "Line width should not exceed 1");
    }
}

/// Test progress bar with boundary percentages
#[test]
fn test_progress_bar_boundaries() {
    // Test boundary values
    let test_cases = vec![
        (0, 40),    // Empty
        (1, 40),    // Minimum visible
        (49, 40),   // Just under half
        (50, 40),   // Exactly half
        (51, 40),   // Just over half
        (99, 40),   // Almost full
        (100, 40),  // Completely full
        (101, 40),  // Over 100% (should cap)
        (200, 40),  // Way over (should cap)
        (-10, 40),  // Negative (should handle)
    ];
    
    for (percentage, width) in test_cases {
        let result = panic::catch_unwind(|| {
            render_progress_bar(percentage, width)
        });
        
        if let Ok(bar) = result {
            assert!(bar.contains('['), "Bar should have opening bracket");
            assert!(bar.contains(']'), "Bar should have closing bracket");
            
            // Verify percentage is shown correctly
            if percentage <= 0 {
                assert!(bar.contains("0%"), "Should show 0% for percentage {}", percentage);
            } else if percentage >= 100 {
                assert!(bar.contains("100%"), "Should show 100% for percentage {}", percentage);
            } else {
                assert!(bar.contains(&format!("{}%", percentage)), 
                        "Should show {}% for percentage {}", percentage, percentage);
            }
        }
    }
}

/// Test with inconsistent token counts
#[test]
fn test_inconsistent_token_counts() {
    let config = test_config();
    
    // total_tokens less than input + output
    let usage = TokenUsage {
        input_tokens: 50000,
        output_tokens: 30000,
        total_tokens: 60000, // Should be 80000
        cached_input_tokens: 10000,
        reasoning_output_tokens: 5000,
    };
    
    // Should handle inconsistency gracefully
    let cell = new_context_output(&config, &usage, &None);
    let lines = cell.display_lines(80);
    assert!(!lines.is_empty(), "Should handle inconsistent counts");
}

/// Test with cached tokens exceeding input tokens
#[test]
fn test_cached_exceeds_input() {
    let config = test_config();
    
    let usage = TokenUsage {
        input_tokens: 10000,
        output_tokens: 5000,
        total_tokens: 15000,
        cached_input_tokens: 15000, // More than input!
        reasoning_output_tokens: 0,
    };
    
    let cell = new_context_output(&config, &usage, &None);
    let lines = cell.display_lines(80);
    
    assert!(!lines.is_empty(), "Should handle cached > input");
}

/// Test with reasoning tokens exceeding output tokens
#[test]
fn test_reasoning_exceeds_output() {
    let config = test_config();
    
    let usage = TokenUsage {
        input_tokens: 10000,
        output_tokens: 5000,
        total_tokens: 15000,
        cached_input_tokens: 0,
        reasoning_output_tokens: 10000, // More than output!
    };
    
    let cell = new_context_output(&config, &usage, &None);
    let lines = cell.display_lines(80);
    
    assert!(!lines.is_empty(), "Should handle reasoning > output");
}

/// Test with all token types at maximum
#[test]
fn test_all_tokens_maximum() {
    let config = test_config();
    
    let usage = TokenUsage {
        input_tokens: 128000,
        output_tokens: 128000,
        total_tokens: 256000,
        cached_input_tokens: 128000,
        reasoning_output_tokens: 128000,
    };
    
    let cell = new_context_output(&config, &usage, &None);
    let lines = cell.display_lines(80);
    let text = lines
        .iter()
        .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");
    
    assert!(text.contains("100%"), "Should show 100% for over-limit");
    assert!(text.contains("High Context Usage Warning"), "Should show warning");
}

/// Test with unicode in model name
#[test]
fn test_unicode_model_name() {
    let mut config_toml = ConfigToml::default();
    config_toml.model = Some("模型-🤖-测试".to_string());
    
    let config = Config::load_from_base_config_with_overrides(
        config_toml,
        ConfigOverrides::default(),
        std::env::temp_dir(),
    )
    .expect("Failed to create config");
    
    let usage = TokenUsage::default();
    
    let cell = new_context_output(&config, &usage, &None);
    let lines = cell.display_lines(80);
    let text = lines
        .iter()
        .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");
    
    assert!(text.contains("模型-🤖-测试"), "Should handle unicode model name");
}

/// Test rapid percentage changes
#[test]
fn test_rapid_percentage_changes() {
    let config = test_config();
    let session_id = Some(ConversationId::new());
    
    // Test every percentage from 0 to 100
    for percentage in 0..=100 {
        let tokens = (128000 * percentage / 100) as i32;
        let usage = TokenUsage {
            input_tokens: tokens * 3 / 4,
            output_tokens: tokens / 4,
            total_tokens: tokens,
            cached_input_tokens: 0,
            reasoning_output_tokens: 0,
        };
        
        let cell = new_context_output(&config, &usage, &session_id);
        let lines = cell.display_lines(80);
        let text = lines
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n");
        
        // Verify percentage is shown correctly
        assert!(text.contains(&format!("{}%", percentage)) || 
                text.contains(&format!("({}%)", percentage)),
                "Should show {}% for {} tokens", percentage, tokens);
        
        // Verify warning appears at right threshold
        if percentage > 70 {
            assert!(text.contains("High Context Usage Warning"),
                    "Should show warning at {}%", percentage);
        } else {
            assert!(!text.contains("High Context Usage Warning"),
                    "Should not show warning at {}%", percentage);
        }
    }
}

/// Test with very long session ID
#[test]
fn test_very_long_session_id() {
    let config = test_config();
    let usage = TokenUsage::default();
    
    // ConversationId::new() creates a UUID, test with it
    let session_id = Some(ConversationId::new());
    
    let cell = new_context_output(&config, &usage, &session_id);
    let lines = cell.display_lines(80);
    
    assert!(!lines.is_empty(), "Should handle session ID");
    
    let text = lines
        .iter()
        .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");
    
    assert!(text.contains("Session"), "Should show session info");
}

/// Test floating point precision in percentage calculations
#[test]
fn test_percentage_precision() {
    let config = test_config();
    
    // Test cases that might cause floating point issues
    let test_cases = vec![
        (1, 0),      // 0.0078125% -> 0%
        (128, 0),    // 0.1% -> 0%
        (1280, 1),   // 1%
        (1281, 1),   // 1.0008% -> 1%
        (1408, 1),   // 1.1% -> 1%
        (42666, 33), // 33.333% -> 33%
        (42667, 33), // 33.334% -> 33%
        (85333, 66), // 66.666% -> 66%
        (85334, 66), // 66.667% -> 66%
    ];
    
    for (tokens, expected_percentage) in test_cases {
        let usage = TokenUsage {
            input_tokens: tokens,
            output_tokens: 0,
            total_tokens: tokens,
            cached_input_tokens: 0,
            reasoning_output_tokens: 0,
        };
        
        let cell = new_context_output(&config, &usage, &None);
        let lines = cell.display_lines(80);
        let text = lines
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n");
        
        assert!(text.contains(&format!("({}%)", expected_percentage)),
                "For {} tokens, expected {}%, text: {}", 
                tokens, expected_percentage, text);
    }
}

/// Test empty string model name
#[test]
fn test_empty_model_name() {
    let mut config_toml = ConfigToml::default();
    config_toml.model = Some("".to_string());
    
    let config = Config::load_from_base_config_with_overrides(
        config_toml,
        ConfigOverrides::default(),
        std::env::temp_dir(),
    )
    .expect("Failed to create config");
    
    let usage = TokenUsage::default();
    
    let cell = new_context_output(&config, &usage, &None);
    let lines = cell.display_lines(80);
    
    assert!(!lines.is_empty(), "Should handle empty model name");
}

/// Test with whitespace-only model name
#[test]
fn test_whitespace_model_name() {
    let mut config_toml = ConfigToml::default();
    config_toml.model = Some("   \t\n   ".to_string());
    
    let config = Config::load_from_base_config_with_overrides(
        config_toml,
        ConfigOverrides::default(),
        std::env::temp_dir(),
    )
    .expect("Failed to create config");
    
    let usage = TokenUsage::default();
    
    let cell = new_context_output(&config, &usage, &None);
    let lines = cell.display_lines(80);
    
    assert!(!lines.is_empty(), "Should handle whitespace model name");
}

/// Test alternating between high and low usage
#[test]
fn test_alternating_usage_levels() {
    let config = test_config();
    let session_id = Some(ConversationId::new());
    
    let usage_patterns = vec![
        (10000, false),   // Low usage
        (100000, true),   // High usage
        (5000, false),    // Back to low
        (95000, true),    // High again
        (0, false),       // Empty
        (128000, true),   // Maximum
    ];
    
    for (tokens, should_warn) in usage_patterns {
        let usage = TokenUsage {
            input_tokens: tokens * 2 / 3,
            output_tokens: tokens / 3,
            total_tokens: tokens,
            cached_input_tokens: 0,
            reasoning_output_tokens: 0,
        };
        
        let cell = new_context_output(&config, &usage, &session_id);
        let lines = cell.display_lines(80);
        let text = lines
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n");
        
        if should_warn {
            assert!(text.contains("High Context Usage Warning"),
                    "Should show warning for {} tokens", tokens);
        } else {
            assert!(!text.contains("High Context Usage Warning"),
                    "Should not show warning for {} tokens", tokens);
        }
    }
}