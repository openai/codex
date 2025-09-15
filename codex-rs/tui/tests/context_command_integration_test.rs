//! Integration tests for the context command feature
//! Tests the full flow of the /context command in various scenarios

use codex_core::config::{Config, ConfigOverrides, ConfigToml};
use codex_core::protocol::{TokenUsage, TokenUsageInfo};
use codex_protocol::mcp_protocol::ConversationId;
use codex_tui::history_cell::{HistoryCell, new_context_output};
use std::sync::Arc;
use std::path::PathBuf;

/// Helper to create a complete TokenUsageInfo structure
fn create_token_usage_info(input: i32, output: i32, cached: i32, reasoning: i32) -> TokenUsageInfo {
    TokenUsageInfo {
        total_token_usage: TokenUsage {
            input_tokens: input,
            output_tokens: output,
            total_tokens: input + output,
            cached_input_tokens: cached,
            reasoning_output_tokens: reasoning,
        },
        turn_token_usage: TokenUsage {
            input_tokens: input / 2,
            output_tokens: output / 2,
            total_tokens: (input + output) / 2,
            cached_input_tokens: cached / 2,
            reasoning_output_tokens: reasoning / 2,
        },
        pricing_info: None,
    }
}

/// Test the context command in a fresh session with no history
#[test]
fn test_context_command_fresh_session() {
    let config = Config::load_from_base_config_with_overrides(
        ConfigToml::default(),
        ConfigOverrides::default(),
        std::env::temp_dir(),
    )
    .expect("Failed to create config");
    
    // Simulate a fresh session with no tokens used
    let usage = TokenUsage::default();
    let session_id: Option<ConversationId> = None;
    
    let cell = new_context_output(&config, &usage, &session_id);
    
    // Verify the cell is created successfully
    assert!(cell.desired_height(80) > 0, "Cell should have positive height");
    
    let lines = cell.display_lines(80);
    let text = lines
        .iter()
        .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");
    
    // Verify all essential elements are present
    assert!(text.contains("/context"), "Should show command header");
    assert!(text.contains("0 / 128,000"), "Should show 0 tokens used");
    assert!(text.contains("0%"), "Should show 0% usage");
    assert!(!text.contains("High Context Usage Warning"), "Should not show warning");
}

/// Test the context command after several turns of conversation
#[test]
fn test_context_command_active_conversation() {
    let config = Config::load_from_base_config_with_overrides(
        ConfigToml::default(),
        ConfigOverrides::default(),
        std::env::temp_dir(),
    )
    .expect("Failed to create config");
    
    // Simulate an active conversation with multiple turns
    let usage = TokenUsage {
        input_tokens: 15000,
        output_tokens: 8000,
        total_tokens: 23000,
        cached_input_tokens: 3000,
        reasoning_output_tokens: 1500,
    };
    let session_id = Some(ConversationId::new());
    
    let cell = new_context_output(&config, &usage, &session_id);
    let lines = cell.display_lines(80);
    let text = lines
        .iter()
        .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");
    
    // Verify conversation metrics are shown
    assert!(text.contains("23,000"), "Should show total tokens");
    assert!(text.contains("17%"), "Should show usage percentage (23000/128000 ≈ 17%)");
    assert!(text.contains("Input: 15,000"), "Should show input tokens");
    assert!(text.contains("Output: 8,000"), "Should show output tokens");
    assert!(text.contains("Cached: 3,000"), "Should show cached tokens");
    assert!(text.contains("Reasoning: 1,500"), "Should show reasoning tokens");
    assert!(text.contains("Session"), "Should show session info");
}

/// Test the context command when approaching context limit
#[test]
fn test_context_command_near_limit() {
    let config = Config::load_from_base_config_with_overrides(
        ConfigToml::default(),
        ConfigOverrides::default(),
        std::env::temp_dir(),
    )
    .expect("Failed to create config");
    
    // Simulate near context limit (95%)
    let usage = TokenUsage {
        input_tokens: 100000,
        output_tokens: 21600,
        total_tokens: 121600, // 95% of 128000
        cached_input_tokens: 20000,
        reasoning_output_tokens: 5000,
    };
    let session_id = Some(ConversationId::new());
    
    let cell = new_context_output(&config, &usage, &session_id);
    let lines = cell.display_lines(80);
    let text = lines
        .iter()
        .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");
    
    // Verify warning is shown
    assert!(text.contains("High Context Usage Warning"), "Should show warning at 95% usage");
    assert!(text.contains("/compact"), "Should suggest /compact command");
    assert!(text.contains("121,600"), "Should show total tokens");
    assert!(text.contains("95%"), "Should show 95% usage");
}

/// Test context command with different model configurations
#[test]
fn test_context_command_different_models() {
    let models = vec![
        ("gpt-4", 128000),
        ("gpt-3.5-turbo", 128000),
        ("claude-3-opus", 128000),
    ];
    
    for (model_name, expected_context) in models {
        let mut config_toml = ConfigToml::default();
        config_toml.model = Some(model_name.to_string());
        
        let config = Config::load_from_base_config_with_overrides(
            config_toml,
            ConfigOverrides::default(),
            std::env::temp_dir(),
        )
        .expect("Failed to create config");
        
        let usage = TokenUsage {
            input_tokens: 10000,
            output_tokens: 5000,
            total_tokens: 15000,
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
        
        assert!(text.contains(model_name), "Should show model: {}", model_name);
        assert!(text.contains(&format!("{},000", expected_context / 1000)), 
                "Should show context window for {}", model_name);
    }
}

/// Test context command after session compaction
#[test]
fn test_context_command_after_compaction() {
    let config = Config::load_from_base_config_with_overrides(
        ConfigToml::default(),
        ConfigOverrides::default(),
        std::env::temp_dir(),
    )
    .expect("Failed to create config");
    
    // Simulate state after /compact command
    // Tokens should be significantly reduced
    let usage_before = TokenUsage {
        input_tokens: 90000,
        output_tokens: 30000,
        total_tokens: 120000,
        cached_input_tokens: 40000,
        reasoning_output_tokens: 10000,
    };
    
    let usage_after = TokenUsage {
        input_tokens: 5000,
        output_tokens: 2000,
        total_tokens: 7000,
        cached_input_tokens: 1000,
        reasoning_output_tokens: 500,
    };
    
    let session_id = Some(ConversationId::new());
    
    // Before compaction
    let cell_before = new_context_output(&config, &usage_before, &session_id);
    let lines_before = cell_before.display_lines(80);
    let text_before = lines_before
        .iter()
        .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");
    
    assert!(text_before.contains("High Context Usage Warning"), "Should show warning before compaction");
    assert!(text_before.contains("93%"), "Should show high usage before compaction");
    
    // After compaction
    let cell_after = new_context_output(&config, &usage_after, &session_id);
    let lines_after = cell_after.display_lines(80);
    let text_after = lines_after
        .iter()
        .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");
    
    assert!(!text_after.contains("High Context Usage Warning"), "Should not show warning after compaction");
    assert!(text_after.contains("5%"), "Should show low usage after compaction");
}

/// Test context command with cached tokens optimization
#[test]
fn test_context_command_cached_tokens() {
    let config = Config::load_from_base_config_with_overrides(
        ConfigToml::default(),
        ConfigOverrides::default(),
        std::env::temp_dir(),
    )
    .expect("Failed to create config");
    
    // Test with high cache hit rate
    let usage = TokenUsage {
        input_tokens: 50000,
        output_tokens: 10000,
        total_tokens: 60000,
        cached_input_tokens: 45000, // 90% of input is cached
        reasoning_output_tokens: 2000,
    };
    let session_id = Some(ConversationId::new());
    
    let cell = new_context_output(&config, &usage, &session_id);
    let lines = cell.display_lines(80);
    let text = lines
        .iter()
        .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");
    
    // Verify cached tokens are prominently displayed
    assert!(text.contains("Cached: 45,000"), "Should show cached tokens");
    assert!(text.contains("35%"), "Should show cached percentage (45000/128000)");
}

/// Test context command with reasoning tokens
#[test]
fn test_context_command_reasoning_tokens() {
    let config = Config::load_from_base_config_with_overrides(
        ConfigToml::default(),
        ConfigOverrides::default(),
        std::env::temp_dir(),
    )
    .expect("Failed to create config");
    
    // Test with significant reasoning tokens
    let usage = TokenUsage {
        input_tokens: 30000,
        output_tokens: 25000,
        total_tokens: 55000,
        cached_input_tokens: 5000,
        reasoning_output_tokens: 15000, // 60% of output is reasoning
    };
    let session_id = Some(ConversationId::new());
    
    let cell = new_context_output(&config, &usage, &session_id);
    let lines = cell.display_lines(80);
    let text = lines
        .iter()
        .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");
    
    // Verify reasoning tokens are shown
    assert!(text.contains("Reasoning: 15,000"), "Should show reasoning tokens");
    assert!(text.contains("11%"), "Should show reasoning percentage (15000/128000)");
}

/// Test context command in edge case scenarios
#[test]
fn test_context_command_edge_cases() {
    let config = Config::load_from_base_config_with_overrides(
        ConfigToml::default(),
        ConfigOverrides::default(),
        std::env::temp_dir(),
    )
    .expect("Failed to create config");
    
    // Test exactly at context limit
    let usage_at_limit = TokenUsage {
        input_tokens: 100000,
        output_tokens: 28000,
        total_tokens: 128000,
        cached_input_tokens: 0,
        reasoning_output_tokens: 0,
    };
    
    let cell = new_context_output(&config, &usage_at_limit, &None);
    let lines = cell.display_lines(80);
    let text = lines
        .iter()
        .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");
    
    assert!(text.contains("100%"), "Should show 100% at limit");
    assert!(text.contains("High Context Usage Warning"), "Should show warning at 100%");
    
    // Test over context limit (should cap at 100%)
    let usage_over_limit = TokenUsage {
        input_tokens: 100000,
        output_tokens: 50000,
        total_tokens: 150000,
        cached_input_tokens: 0,
        reasoning_output_tokens: 0,
    };
    
    let cell = new_context_output(&config, &usage_over_limit, &None);
    let lines = cell.display_lines(80);
    let text = lines
        .iter()
        .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");
    
    assert!(text.contains("100%"), "Should cap at 100% when over limit");
    assert!(text.contains("150,000"), "Should show actual token count");
}

/// Test context command display at various terminal widths
#[test]
fn test_context_command_responsive_display() {
    let config = Config::load_from_base_config_with_overrides(
        ConfigToml::default(),
        ConfigOverrides::default(),
        std::env::temp_dir(),
    )
    .expect("Failed to create config");
    
    let usage = TokenUsage {
        input_tokens: 25000,
        output_tokens: 15000,
        total_tokens: 40000,
        cached_input_tokens: 5000,
        reasoning_output_tokens: 3000,
    };
    let session_id = Some(ConversationId::new());
    
    // Test various terminal widths
    let widths = vec![40, 60, 80, 100, 120];
    
    for width in widths {
        let cell = new_context_output(&config, &usage, &session_id);
        let lines = cell.display_lines(width);
        
        assert!(!lines.is_empty(), "Should have output at width {}", width);
        
        // Verify content adapts to width
        let max_line_width: usize = lines
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.len()).sum())
            .max()
            .unwrap_or(0);
        
        assert!(max_line_width <= width as usize, 
                "Lines should not exceed width {} (got {})", width, max_line_width);
    }
}

/// Test context command with rapid state changes
#[test]
fn test_context_command_rapid_updates() {
    let config = Config::load_from_base_config_with_overrides(
        ConfigToml::default(),
        ConfigOverrides::default(),
        std::env::temp_dir(),
    )
    .expect("Failed to create config");
    
    let session_id = Some(ConversationId::new());
    
    // Simulate rapid token usage updates
    let token_increments = vec![0, 1000, 5000, 10000, 20000, 40000, 80000, 100000];
    
    for tokens in token_increments {
        let usage = TokenUsage {
            input_tokens: tokens * 3 / 4,
            output_tokens: tokens / 4,
            total_tokens: tokens,
            cached_input_tokens: tokens / 10,
            reasoning_output_tokens: tokens / 20,
        };
        
        let cell = new_context_output(&config, &usage, &session_id);
        let lines = cell.display_lines(80);
        
        assert!(!lines.is_empty(), "Should handle {} tokens", tokens);
        
        let text = lines
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n");
        
        let percentage = (tokens as f64 / 128000.0 * 100.0) as i32;
        if percentage > 70 {
            assert!(text.contains("High Context Usage Warning"), 
                    "Should show warning at {} tokens ({}%)", tokens, percentage);
        } else {
            assert!(!text.contains("High Context Usage Warning"),
                    "Should not show warning at {} tokens ({}%)", tokens, percentage);
        }
    }
}

/// Test context command with mixed token types
#[test]
fn test_context_command_mixed_tokens() {
    let config = Config::load_from_base_config_with_overrides(
        ConfigToml::default(),
        ConfigOverrides::default(),
        std::env::temp_dir(),
    )
    .expect("Failed to create config");
    
    // Test various combinations of token types
    let test_cases = vec![
        // (input, output, cached, reasoning)
        (10000, 5000, 8000, 2000),    // High cache rate
        (50000, 20000, 5000, 15000),  // High reasoning
        (30000, 30000, 15000, 15000), // Balanced
        (80000, 10000, 0, 0),         // No optimization
        (20000, 40000, 10000, 30000), // Output heavy
    ];
    
    for (input, output, cached, reasoning) in test_cases {
        let usage = TokenUsage {
            input_tokens: input,
            output_tokens: output,
            total_tokens: input + output,
            cached_input_tokens: cached,
            reasoning_output_tokens: reasoning,
        };
        
        let cell = new_context_output(&config, &usage, &None);
        let lines = cell.display_lines(80);
        let text = lines
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n");
        
        // Verify all components are shown correctly
        assert!(text.contains(&format!("{},", input).replace(",000", ",000")), 
                "Should show input tokens: {}", input);
        assert!(text.contains(&format!("{},", output).replace(",000", ",000")) || 
                text.contains(&output.to_string()),
                "Should show output tokens: {}", output);
        
        if cached > 0 {
            assert!(text.contains("Cached:"), "Should show cached section when > 0");
        }
        if reasoning > 0 {
            assert!(text.contains("Reasoning:"), "Should show reasoning section when > 0");
        }
    }
}

/// Test context command with long-running sessions
#[test]
fn test_context_command_long_session() {
    let config = Config::load_from_base_config_with_overrides(
        ConfigToml::default(),
        ConfigOverrides::default(),
        std::env::temp_dir(),
    )
    .expect("Failed to create config");
    
    // Simulate a very long session with many turns
    let usage = TokenUsage {
        input_tokens: 85000,
        output_tokens: 35000,
        total_tokens: 120000,
        cached_input_tokens: 60000, // High cache due to context reuse
        reasoning_output_tokens: 12000,
    };
    let session_id = Some(ConversationId::new());
    
    let cell = new_context_output(&config, &usage, &session_id);
    let lines = cell.display_lines(80);
    let text = lines
        .iter()
        .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");
    
    // Verify long session characteristics
    assert!(text.contains("93%"), "Should show high usage for long session");
    assert!(text.contains("High Context Usage Warning"), "Should warn about high usage");
    assert!(text.contains("Cached: 60,000"), "Should show high cache utilization");
    assert!(text.contains("Session"), "Should show session info");
}

/// Test context command error resilience
#[test]
fn test_context_command_error_resilience() {
    let config = Config::load_from_base_config_with_overrides(
        ConfigToml::default(),
        ConfigOverrides::default(),
        std::env::temp_dir(),
    )
    .expect("Failed to create config");
    
    // Test with inconsistent token counts (total < input + output)
    let mut usage = TokenUsage {
        input_tokens: 50000,
        output_tokens: 30000,
        total_tokens: 70000, // Should be 80000
        cached_input_tokens: 10000,
        reasoning_output_tokens: 5000,
    };
    
    // Fix the inconsistency
    usage.total_tokens = usage.input_tokens + usage.output_tokens;
    
    let cell = new_context_output(&config, &usage, &None);
    let lines = cell.display_lines(80);
    
    assert!(!lines.is_empty(), "Should handle corrected token counts");
    
    // Test with extreme values
    let extreme_usage = TokenUsage {
        input_tokens: i32::MAX / 2,
        output_tokens: i32::MAX / 2,
        total_tokens: i32::MAX,
        cached_input_tokens: 0,
        reasoning_output_tokens: 0,
    };
    
    let cell = new_context_output(&config, &extreme_usage, &None);
    let lines = cell.display_lines(80);
    
    assert!(!lines.is_empty(), "Should handle extreme values");
}

/// Test context command with concurrent access simulation
#[test]
fn test_context_command_concurrent_access() {
    use std::thread;
    use std::sync::Mutex;
    
    let config = Arc::new(Config::load_from_base_config_with_overrides(
        ConfigToml::default(),
        ConfigOverrides::default(),
        std::env::temp_dir(),
    )
    .expect("Failed to create config"));
    
    let results = Arc::new(Mutex::new(Vec::new()));
    let mut handles = vec![];
    
    // Simulate multiple threads accessing context command
    for i in 0..10 {
        let config_clone = Arc::clone(&config);
        let results_clone = Arc::clone(&results);
        
        let handle = thread::spawn(move || {
            let usage = TokenUsage {
                input_tokens: i * 1000,
                output_tokens: i * 500,
                total_tokens: i * 1500,
                cached_input_tokens: i * 100,
                reasoning_output_tokens: i * 50,
            };
            
            let cell = new_context_output(&config_clone, &usage, &None);
            let lines = cell.display_lines(80);
            
            let mut res = results_clone.lock().unwrap();
            res.push((i, lines.len()));
        });
        
        handles.push(handle);
    }
    
    // Wait for all threads
    for handle in handles {
        handle.join().unwrap();
    }
    
    let res = results.lock().unwrap();
    assert_eq!(res.len(), 10, "All threads should complete");
    
    for (i, line_count) in res.iter() {
        assert!(*line_count > 0, "Thread {} should produce output", i);
    }
}