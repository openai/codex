#[cfg(test)]
mod context_output_tests {
    use crate::history_cell::{new_context_output, render_progress_bar, HistoryCell, PlainHistoryCell};
    use codex_core::config::{Config, ConfigOverrides, ConfigToml};
    use codex_core::protocol::TokenUsage;
    use codex_protocol::mcp_protocol::ConversationId;
    use ratatui::prelude::*;
    use std::path::PathBuf;

    fn test_config() -> Config {
        Config::load_from_base_config_with_overrides(
            ConfigToml::default(),
            ConfigOverrides::default(),
            std::env::temp_dir(),
        )
        .expect("Failed to create test config")
    }

    fn render_lines(lines: &[Line<'static>]) -> Vec<String> {
        lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect()
    }

    #[test]
    fn test_new_context_output_basic() {
        let config = test_config();
        let usage = TokenUsage {
            input_tokens: 50000,
            output_tokens: 10000,
            total_tokens: 60000,
            cached_input_tokens: 5000,
            reasoning_output_tokens: 2000,
        };
        let session_id = Some(ConversationId::new());

        let cell = new_context_output(&config, &usage, &session_id);
        let lines = cell.display_lines(80);
        let rendered = render_lines(&lines);

        // Check that essential elements are present
        assert!(rendered[0].contains("/context"));
        assert!(rendered.iter().any(|l| l.contains("Context Window Usage")));
        assert!(rendered.iter().any(|l| l.contains("60,000"))); // Total tokens
        assert!(rendered.iter().any(|l| l.contains("128,000"))); // Context window
        assert!(rendered.iter().any(|l| l.contains("46%"))); // Percentage (60000/128000)
        assert!(rendered.iter().any(|l| l.contains("Component Breakdown")));
        assert!(rendered.iter().any(|l| l.contains("Input:"))); 
        assert!(rendered.iter().any(|l| l.contains("50,000"))); // Input tokens
        assert!(rendered.iter().any(|l| l.contains("Output:")));
        assert!(rendered.iter().any(|l| l.contains("10,000"))); // Output tokens
        assert!(rendered.iter().any(|l| l.contains("Reasoning:")));
        assert!(rendered.iter().any(|l| l.contains("2,000"))); // Reasoning tokens
    }

    #[test]
    fn test_new_context_output_no_session_id() {
        let config = test_config();
        let usage = TokenUsage {
            input_tokens: 10000,
            output_tokens: 5000,
            total_tokens: 15000,
            cached_input_tokens: 0,
            reasoning_output_tokens: 0,
        };
        let session_id = None;

        let cell = new_context_output(&config, &usage, &session_id);
        let lines = cell.display_lines(80);
        let rendered = render_lines(&lines);

        // Should not have Session section when no session_id
        assert!(!rendered.iter().any(|l| l.contains("Session")));
        assert!(rendered.iter().any(|l| l.contains("15,000"))); // Total tokens
        assert!(rendered.iter().any(|l| l.contains("11%"))); // Percentage (15000/128000)
    }

    #[test]
    fn test_new_context_output_zero_tokens() {
        let config = test_config();
        let usage = TokenUsage {
            input_tokens: 0,
            output_tokens: 0,
            total_tokens: 0,
            cached_input_tokens: 0,
            reasoning_output_tokens: 0,
        };
        let session_id = Some(ConversationId::new());

        let cell = new_context_output(&config, &usage, &session_id);
        let lines = cell.display_lines(80);
        let rendered = render_lines(&lines);

        assert!(rendered.iter().any(|l| l.contains("0%")));
        assert!(rendered.iter().any(|l| l.contains("0 / 128,000")));
    }

    #[test]
    fn test_new_context_output_at_max_capacity() {
        let config = test_config();
        let usage = TokenUsage {
            input_tokens: 100000,
            output_tokens: 28000,
            total_tokens: 128000,
            cached_input_tokens: 10000,
            reasoning_output_tokens: 5000,
        };
        let session_id = Some(ConversationId::new());

        let cell = new_context_output(&config, &usage, &session_id);
        let lines = cell.display_lines(80);
        let rendered = render_lines(&lines);

        assert!(rendered.iter().any(|l| l.contains("100%"))); // At max capacity
        assert!(rendered.iter().any(|l| l.contains("128,000 / 128,000")));
    }

    #[test]
    fn test_new_context_output_over_capacity() {
        let config = test_config();
        let usage = TokenUsage {
            input_tokens: 100000,
            output_tokens: 50000,
            total_tokens: 150000, // Over the 128k limit
            cached_input_tokens: 0,
            reasoning_output_tokens: 0,
        };
        let session_id = None;

        let cell = new_context_output(&config, &usage, &session_id);
        let lines = cell.display_lines(80);
        let rendered = render_lines(&lines);

        // Should cap at 100%
        assert!(rendered.iter().any(|l| l.contains("100%")));
        assert!(rendered.iter().any(|l| l.contains("150,000 / 128,000")));
    }

    #[test]
    fn test_new_context_output_with_cached_tokens() {
        let config = test_config();
        let usage = TokenUsage {
            input_tokens: 30000,
            output_tokens: 10000,
            total_tokens: 40000,
            cached_input_tokens: 15000, // Half of input is cached
            reasoning_output_tokens: 0,
        };
        let session_id = Some(ConversationId::new());

        let cell = new_context_output(&config, &usage, &session_id);
        let lines = cell.display_lines(80);
        let rendered = render_lines(&lines);

        // Should show cached tokens
        assert!(rendered.iter().any(|l| l.contains("Cached:")));
        assert!(rendered.iter().any(|l| l.contains("15,000")));
        assert!(rendered.iter().any(|l| l.contains("11%"))); // 15000/128000 for cached
    }

    #[test]
    fn test_new_context_output_no_reasoning_tokens() {
        let config = test_config();
        let usage = TokenUsage {
            input_tokens: 10000,
            output_tokens: 5000,
            total_tokens: 15000,
            cached_input_tokens: 0,
            reasoning_output_tokens: 0, // No reasoning tokens
        };
        let session_id = None;

        let cell = new_context_output(&config, &usage, &session_id);
        let lines = cell.display_lines(80);
        let rendered = render_lines(&lines);

        // Should not show Reasoning line when reasoning_output_tokens is 0
        assert!(!rendered.iter().any(|l| l.contains("Reasoning:")));
    }

    #[test]
    fn test_render_progress_bar_empty() {
        let bar = render_progress_bar(0, 40);
        assert_eq!(bar, "    [░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░] 0%");
    }

    #[test]
    fn test_render_progress_bar_full() {
        let bar = render_progress_bar(100, 40);
        assert_eq!(bar, "    [███████████████████████████████████████████] 100%");
    }

    #[test]
    fn test_render_progress_bar_half() {
        let bar = render_progress_bar(50, 40);
        assert!(bar.contains("] 50%"));
        // Should have roughly half filled, half empty
        let filled_count = bar.chars().filter(|&c| c == '█' || c == '▌').count();
        assert!(filled_count >= 19 && filled_count <= 21); // Allow for rounding
    }

    #[test]
    fn test_render_progress_bar_quarter() {
        let bar = render_progress_bar(25, 40);
        assert!(bar.contains("] 25%"));
        let filled_count = bar.chars().filter(|&c| c == '█' || c == '▌').count();
        assert!(filled_count >= 9 && filled_count <= 11); // 25% of 40 is 10
    }

    #[test]
    fn test_render_progress_bar_three_quarters() {
        let bar = render_progress_bar(75, 40);
        assert!(bar.contains("] 75%"));
        let filled_count = bar.chars().filter(|&c| c == '█' || c == '▌').count();
        assert!(filled_count >= 29 && filled_count <= 31); // 75% of 40 is 30
    }

    #[test]
    fn test_render_progress_bar_small_width() {
        let bar = render_progress_bar(50, 10);
        assert!(bar.contains("] 50%"));
        assert_eq!(bar.len(), 19); // "    [" + 10 chars + "] 50%"
    }

    #[test]
    fn test_render_progress_bar_large_width() {
        let bar = render_progress_bar(33, 100);
        assert!(bar.contains("] 33%"));
        let filled_count = bar.chars().filter(|&c| c == '█' || c == '▌').count();
        assert!(filled_count >= 32 && filled_count <= 34); // 33% of 100
    }

    #[test]
    fn test_render_progress_bar_one_percent() {
        let bar = render_progress_bar(1, 40);
        assert!(bar.contains("] 1%"));
        // Should have at least one partial block
        assert!(bar.contains('▌'));
    }

    #[test]
    fn test_render_progress_bar_ninety_nine_percent() {
        let bar = render_progress_bar(99, 40);
        assert!(bar.contains("] 99%"));
        // Should be almost full
        let filled_count = bar.chars().filter(|&c| c == '█' || c == '▌').count();
        assert!(filled_count >= 39); // Almost all filled
    }

    #[test]
    fn test_render_progress_bar_exact_percentages() {
        // Test exact percentage calculations
        let bar_10 = render_progress_bar(10, 40);
        assert!(bar_10.contains("] 10%"));
        
        let bar_20 = render_progress_bar(20, 40);
        assert!(bar_20.contains("] 20%"));
        
        let bar_80 = render_progress_bar(80, 40);
        assert!(bar_80.contains("] 80%"));
        
        let bar_90 = render_progress_bar(90, 40);
        assert!(bar_90.contains("] 90%"));
    }

    #[test]
    fn test_context_output_display_width_variations() {
        let config = test_config();
        let usage = TokenUsage {
            input_tokens: 50000,
            output_tokens: 25000,
            total_tokens: 75000,
            cached_input_tokens: 10000,
            reasoning_output_tokens: 5000,
        };
        let session_id = Some(ConversationId::new());

        let cell = new_context_output(&config, &usage, &session_id);
        
        // Test with different widths
        let lines_narrow = cell.display_lines(40);
        let lines_medium = cell.display_lines(80);
        let lines_wide = cell.display_lines(120);
        
        // All should contain the same key information
        for lines in [lines_narrow, lines_medium, lines_wide] {
            let rendered = render_lines(&lines);
            assert!(rendered.iter().any(|l| l.contains("75,000")));
            assert!(rendered.iter().any(|l| l.contains("58%"))); // 75000/128000
        }
    }

    #[test]
    fn test_context_output_model_info() {
        let mut config = test_config();
        config.model = "test-model-3.5".to_string();
        
        let usage = TokenUsage {
            input_tokens: 10000,
            output_tokens: 5000,
            total_tokens: 15000,
            cached_input_tokens: 0,
            reasoning_output_tokens: 0,
        };
        let session_id = None;

        let cell = new_context_output(&config, &usage, &session_id);
        let lines = cell.display_lines(80);
        let rendered = render_lines(&lines);

        // Should show model information
        assert!(rendered.iter().any(|l| l.contains("Model")));
        assert!(rendered.iter().any(|l| l.contains("test-model-3.5")));
        assert!(rendered.iter().any(|l| l.contains("Context Window:")));
        assert!(rendered.iter().any(|l| l.contains("128,000 tokens")));
    }

    #[test]
    fn test_progress_bar_edge_cases() {
        // Test with width of 1
        let bar = render_progress_bar(50, 1);
        assert!(bar.contains("] 50%"));
        
        // Test with width of 2
        let bar = render_progress_bar(50, 2);
        assert!(bar.contains("] 50%"));
        
        // Test percentage beyond 100 (should cap at 100)
        let bar = render_progress_bar(150, 40);
        assert_eq!(bar, render_progress_bar(100, 40));
    }

    #[test]
    fn test_context_output_formatting_consistency() {
        let config = test_config();
        let usage = TokenUsage {
            input_tokens: 12345,
            output_tokens: 6789,
            total_tokens: 19134,
            cached_input_tokens: 1234,
            reasoning_output_tokens: 567,
        };
        let session_id = Some(ConversationId::new());

        let cell = new_context_output(&config, &usage, &session_id);
        let lines = cell.display_lines(80);
        let rendered = render_lines(&lines);

        // Check number formatting with separators
        assert!(rendered.iter().any(|l| l.contains("12,345"))); // Input tokens
        assert!(rendered.iter().any(|l| l.contains("6,789")));  // Output tokens
        assert!(rendered.iter().any(|l| l.contains("19,134"))); // Total tokens
        assert!(rendered.iter().any(|l| l.contains("1,234")));  // Cached tokens
        assert!(rendered.iter().any(|l| l.contains("567")));    // Reasoning tokens
    }

    #[test]
    fn test_context_output_percentage_calculations() {
        let config = test_config();
        
        // Test various percentage scenarios
        let test_cases = vec![
            (1000, 1),     // 0.78% -> rounds to 0%
            (1280, 1),     // 1% exactly
            (6400, 5),     // 5% exactly
            (12800, 10),   // 10% exactly
            (32000, 25),   // 25% exactly
            (64000, 50),   // 50% exactly
            (96000, 75),   // 75% exactly
            (128000, 100), // 100% exactly
        ];
        
        for (total_tokens, expected_percentage) in test_cases {
            let usage = TokenUsage {
                input_tokens: total_tokens / 2,
                output_tokens: total_tokens / 2,
                total_tokens,
                cached_input_tokens: 0,
                reasoning_output_tokens: 0,
            };
            
            let cell = new_context_output(&config, &usage, &None);
            let lines = cell.display_lines(80);
            let rendered = render_lines(&lines);
            
            assert!(
                rendered.iter().any(|l| l.contains(&format!("({}%)", expected_percentage))),
                "Failed for total_tokens={}, expected {}%",
                total_tokens,
                expected_percentage
            );
        }
    }

    #[test]
    fn test_plain_history_cell_trait_implementation() {
        let config = test_config();
        let usage = TokenUsage {
            input_tokens: 1000,
            output_tokens: 500,
            total_tokens: 1500,
            cached_input_tokens: 0,
            reasoning_output_tokens: 0,
        };
        
        let cell = new_context_output(&config, &usage, &None);
        
        // Test HistoryCell trait methods
        let display_lines = cell.display_lines(80);
        assert!(!display_lines.is_empty());
        
        let transcript_lines = cell.transcript_lines();
        assert!(!transcript_lines.is_empty());
        
        let height = cell.desired_height(80);
        assert!(height > 0);
        
        assert!(!cell.is_stream_continuation());
    }
}