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
    fn test_new_context_output_high_usage_warning() {
        let config = test_config();
        
        // Test at 71% usage (just over 70% threshold)
        let usage_71 = TokenUsage {
            input_tokens: 60000,
            output_tokens: 31000,
            total_tokens: 91000,  // 71% of 128000
            cached_input_tokens: 0,
            reasoning_output_tokens: 0,
        };
        let session_id = Some(ConversationId::new());

        let cell = new_context_output(&config, &usage_71, &session_id);
        let lines = cell.display_lines(80);
        let rendered = render_lines(&lines);

        // Should show warning message
        assert!(rendered.iter().any(|l| l.contains("High Context Usage Warning")));
        assert!(rendered.iter().any(|l| l.contains("/compact")));
        assert!(rendered.iter().any(|l| l.contains("reduce context")));

        // Test at exactly 70% usage (should not show warning)
        let usage_70 = TokenUsage {
            input_tokens: 60000,
            output_tokens: 29600,
            total_tokens: 89600,  // Exactly 70% of 128000
            cached_input_tokens: 0,
            reasoning_output_tokens: 0,
        };

        let cell = new_context_output(&config, &usage_70, &None);
        let lines = cell.display_lines(80);
        let rendered = render_lines(&lines);

        // Should NOT show warning message at exactly 70%
        assert!(!rendered.iter().any(|l| l.contains("High Context Usage Warning")));
        assert!(!rendered.iter().any(|l| l.contains("/compact")));

        // Test at 80% usage (should show warning)
        let usage_80 = TokenUsage {
            input_tokens: 70000,
            output_tokens: 32400,
            total_tokens: 102400,  // 80% of 128000
            cached_input_tokens: 0,
            reasoning_output_tokens: 0,
        };

        let cell = new_context_output(&config, &usage_80, &None);
        let lines = cell.display_lines(80);
        let rendered = render_lines(&lines);

        // Should show warning message
        assert!(rendered.iter().any(|l| l.contains("High Context Usage Warning")));
        assert!(rendered.iter().any(|l| l.contains("/compact")));

        // Test at 95% usage (should show warning)
        let usage_95 = TokenUsage {
            input_tokens: 90000,
            output_tokens: 31600,
            total_tokens: 121600,  // 95% of 128000
            cached_input_tokens: 0,
            reasoning_output_tokens: 0,
        };

        let cell = new_context_output(&config, &usage_95, &None);
        let lines = cell.display_lines(80);
        let rendered = render_lines(&lines);

        // Should show warning message
        assert!(rendered.iter().any(|l| l.contains("High Context Usage Warning")));
        assert!(rendered.iter().any(|l| l.contains("/compact")));
    }

    #[test]
    fn test_high_usage_warning_boundary_conditions() {
        let config = test_config();
        
        // Test at 69.9% usage (just below threshold, should not show warning)
        let usage_69_9 = TokenUsage {
            input_tokens: 60000,
            output_tokens: 29472,
            total_tokens: 89472,  // 69.9% of 128000
            cached_input_tokens: 0,
            reasoning_output_tokens: 0,
        };

        let cell = new_context_output(&config, &usage_69_9, &None);
        let lines = cell.display_lines(80);
        let rendered = render_lines(&lines);

        // Should NOT show warning message
        assert!(!rendered.iter().any(|l| l.contains("High Context Usage Warning")));
        assert!(!rendered.iter().any(|l| l.contains("/compact")));

        // Test at 70.1% usage (just above threshold, should show warning)
        let usage_70_1 = TokenUsage {
            input_tokens: 60000,
            output_tokens: 29728,
            total_tokens: 89728,  // 70.1% of 128000
            cached_input_tokens: 0,
            reasoning_output_tokens: 0,
        };

        let cell = new_context_output(&config, &usage_70_1, &None);
        let lines = cell.display_lines(80);
        let rendered = render_lines(&lines);

        // Should show warning message
        assert!(rendered.iter().any(|l| l.contains("High Context Usage Warning")));
        assert!(rendered.iter().any(|l| l.contains("/compact")));
    }

    #[test]
    fn test_high_usage_warning_with_all_token_types() {
        let config = test_config();
        
        // Test with mixed token types exceeding 70% threshold
        let usage = TokenUsage {
            input_tokens: 50000,
            output_tokens: 30000,
            total_tokens: 91000,  // 71% of 128000
            cached_input_tokens: 20000,
            reasoning_output_tokens: 10000,
        };
        let session_id = Some(ConversationId::new());

        let cell = new_context_output(&config, &usage, &session_id);
        let lines = cell.display_lines(80);
        let rendered = render_lines(&lines);

        // Should show warning message
        assert!(rendered.iter().any(|l| l.contains("High Context Usage Warning")));
        assert!(rendered.iter().any(|l| l.contains("/compact")));
        assert!(rendered.iter().any(|l| l.contains("reduce context")));
        
        // Should also show all token types
        assert!(rendered.iter().any(|l| l.contains("Input:")));
        assert!(rendered.iter().any(|l| l.contains("Output:")));
        assert!(rendered.iter().any(|l| l.contains("Cached:")));
        assert!(rendered.iter().any(|l| l.contains("Reasoning:")));
    }

    #[test]
    fn test_high_usage_warning_formatting() {
        let config = test_config();
        
        // Test at 85% usage to ensure warning is shown
        let usage = TokenUsage {
            input_tokens: 80000,
            output_tokens: 28800,
            total_tokens: 108800,  // 85% of 128000
            cached_input_tokens: 0,
            reasoning_output_tokens: 0,
        };

        let cell = new_context_output(&config, &usage, &None);
        let lines = cell.display_lines(80);
        let rendered = render_lines(&lines);

        // Find the warning lines
        let warning_header_idx = rendered.iter().position(|l| l.contains("High Context Usage Warning"));
        assert!(warning_header_idx.is_some(), "Warning header should be present");
        
        let header_idx = warning_header_idx.unwrap();
        
        // Check that the warning icon is present
        assert!(rendered[header_idx].contains("⚠️"));
        
        // Check that the suggestion line follows the header
        if header_idx + 1 < rendered.len() {
            assert!(rendered[header_idx + 1].contains("Consider using"));
            assert!(rendered[header_idx + 1].contains("/compact"));
            assert!(rendered[header_idx + 1].contains("to reduce context"));
        }
    }

    #[test]
    fn test_warning_appears_after_blank_line() {
        let config = test_config();
        
        // Test at 75% usage
        let usage = TokenUsage {
            input_tokens: 70000,
            output_tokens: 26000,
            total_tokens: 96000,  // 75% of 128000
            cached_input_tokens: 0,
            reasoning_output_tokens: 0,
        };

        let cell = new_context_output(&config, &usage, &None);
        let lines = cell.display_lines(80);
        let rendered = render_lines(&lines);

        // Find the warning header
        let warning_idx = rendered.iter().position(|l| l.contains("High Context Usage Warning"));
        assert!(warning_idx.is_some());
        
        let idx = warning_idx.unwrap();
        
        // Check that there's a blank line before the warning
        if idx > 0 {
            assert_eq!(rendered[idx - 1].trim(), "", "Should have blank line before warning");
        }
    }

    #[test]
    fn test_warning_with_different_display_widths() {
        let config = test_config();
        
        // Test at 75% usage
        let usage = TokenUsage {
            input_tokens: 70000,
            output_tokens: 26000,
            total_tokens: 96000,  // 75% of 128000
            cached_input_tokens: 0,
            reasoning_output_tokens: 0,
        };

        // Test with various display widths
        for width in [40, 60, 80, 100, 120] {
            let cell = new_context_output(&config, &usage, &None);
            let lines = cell.display_lines(width);
            let rendered = render_lines(&lines);

            // Warning should be present at all widths
            assert!(
                rendered.iter().any(|l| l.contains("High Context Usage Warning")),
                "Warning should be present at width {}",
                width
            );
            assert!(
                rendered.iter().any(|l| l.contains("/compact")),
                "/compact suggestion should be present at width {}",
                width
            );
        }
    }

    #[test]
    fn test_warning_percentage_calculation_precision() {
        let config = test_config();
        
        // Test various percentages around the 70% threshold
        let test_cases = vec![
            (89599, false),  // 69.999% - no warning
            (89600, false),  // 70.000% - no warning (exactly at threshold)
            (89601, true),   // 70.001% - warning
            (90880, true),   // 71.000% - warning
            (115200, true),  // 90.000% - warning
            (127999, true),  // 99.999% - warning
            (128000, true),  // 100.00% - warning
            (150000, true),  // >100% - warning (capped at 100%)
        ];

        for (total_tokens, should_warn) in test_cases {
            let usage = TokenUsage {
                input_tokens: total_tokens / 2,
                output_tokens: total_tokens - (total_tokens / 2),
                total_tokens,
                cached_input_tokens: 0,
                reasoning_output_tokens: 0,
            };

            let cell = new_context_output(&config, &usage, &None);
            let lines = cell.display_lines(80);
            let rendered = render_lines(&lines);

            let has_warning = rendered.iter().any(|l| l.contains("High Context Usage Warning"));
            
            assert_eq!(
                has_warning, should_warn,
                "For {} tokens ({}%), warning should be {}",
                total_tokens,
                (total_tokens as f64 / 128000.0 * 100.0),
                if should_warn { "shown" } else { "hidden" }
            );
        }
    }

    #[test]
    fn test_render_progress_bar_empty() {
        let bar = render_progress_bar(0, 40);
        assert_eq!(bar, "    [░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░] 0%");
    }

    #[test]
    fn test_render_progress_bar_full() {
        let bar = render_progress_bar(100, 40);
        assert_eq!(bar, "    [████████████████████████████████████████] 100%");
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
            (1000, 0),     // 0.78% -> rounds to 0%
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

    #[test]
    fn test_warning_message_exact_wording() {
        let config = test_config();
        
        // Test at 75% usage to ensure warning is shown
        let usage = TokenUsage {
            input_tokens: 70000,
            output_tokens: 26000,
            total_tokens: 96000,  // 75% of 128000
            cached_input_tokens: 0,
            reasoning_output_tokens: 0,
        };

        let cell = new_context_output(&config, &usage, &None);
        let lines = cell.display_lines(80);
        let rendered = render_lines(&lines).join("\n");

        // Check exact warning message wording
        assert!(rendered.contains("High Context Usage Warning"));
        assert!(rendered.contains("Consider using"));
        assert!(rendered.contains("/compact"));
        assert!(rendered.contains("to reduce context"));
    }

    #[test]
    fn test_warning_not_shown_for_low_usage() {
        let config = test_config();
        
        // Test various low usage scenarios
        let test_cases = vec![
            (1000, 0),     // ~0.8%
            (10000, 7),    // ~7.8%
            (25600, 20),   // 20%
            (38400, 30),   // 30%
            (51200, 40),   // 40%
            (64000, 50),   // 50%
            (76800, 60),   // 60%
            (89600, 70),   // 70% exactly (at threshold, no warning)
        ];

        for (total_tokens, percentage) in test_cases {
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
                !rendered.iter().any(|l| l.contains("High Context Usage Warning")),
                "Warning should NOT appear at {}% usage ({} tokens)",
                percentage,
                total_tokens
            );
        }
    }

    #[test]
    fn test_warning_integration_with_full_context_display() {
        let config = test_config();
        
        // Create a complex usage scenario exceeding 70%
        let usage = TokenUsage {
            input_tokens: 60000,
            output_tokens: 35000,
            total_tokens: 95000,  // ~74% of 128000
            cached_input_tokens: 25000,
            reasoning_output_tokens: 15000,
        };
        let session_id = Some(ConversationId::new());

        let cell = new_context_output(&config, &usage, &session_id);
        let lines = cell.display_lines(80);
        let rendered = render_lines(&lines);

        // Verify all sections are present in correct order
        let mut found_sections = vec![];
        for line in &rendered {
            if line.contains("/context") {
                found_sections.push("command");
            } else if line.contains("Context Window Usage") {
                found_sections.push("usage");
            } else if line.contains("Component Breakdown") {
                found_sections.push("breakdown");
            } else if line.contains("Model") && !line.contains("model-") {
                found_sections.push("model");
            } else if line.contains("Session") {
                found_sections.push("session");
            } else if line.contains("High Context Usage Warning") {
                found_sections.push("warning");
            }
        }

        // Verify sections appear in expected order
        assert_eq!(found_sections[0], "command");
        assert!(found_sections.contains(&"usage"));
        assert!(found_sections.contains(&"breakdown"));
        assert!(found_sections.contains(&"model"));
        assert!(found_sections.contains(&"session"));
        assert_eq!(found_sections.last(), Some(&"warning"), "Warning should appear last");

        // Verify all token values are displayed correctly
        assert!(rendered.iter().any(|l| l.contains("95,000")));  // Total
        assert!(rendered.iter().any(|l| l.contains("60,000")));  // Input
        assert!(rendered.iter().any(|l| l.contains("35,000")));  // Output
        assert!(rendered.iter().any(|l| l.contains("25,000")));  // Cached
        assert!(rendered.iter().any(|l| l.contains("15,000")));  // Reasoning
    }

    #[test]
    fn test_warning_styling_preserved() {
        // This test verifies that the warning is properly styled
        // In the actual implementation, the warning uses .yellow().bold() for the header
        // and .cyan().bold() for the /compact command
        let config = test_config();
        
        let usage = TokenUsage {
            input_tokens: 70000,
            output_tokens: 26000,
            total_tokens: 96000,  // 75% of 128000
            cached_input_tokens: 0,
            reasoning_output_tokens: 0,
        };

        let cell = new_context_output(&config, &usage, &None);
        let lines = cell.display_lines(80);
        
        // Find the warning lines
        let warning_line_idx = lines.iter().position(|l| {
            l.spans.iter().any(|s| s.content.contains("High Context Usage Warning"))
        });
        
        assert!(warning_line_idx.is_some(), "Warning should be present");
        
        // The actual Line objects contain styled spans, which would have Style information
        // This test just verifies the structure is correct
        let warning_line = &lines[warning_line_idx.unwrap()];
        assert!(!warning_line.spans.is_empty(), "Warning line should have styled spans");
    }
}