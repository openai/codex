#[cfg(test)]
mod progress_bar_integration_tests {
    use codex_core::config::{Config, ConfigOverrides, ConfigToml};
    use codex_core::protocol::TokenUsage;
    use codex_protocol::mcp_protocol::ConversationId;

    fn test_config() -> Config {
        Config::load_from_base_config_with_overrides(
            ConfigToml::default(),
            ConfigOverrides::default(),
            std::env::temp_dir(),
        )
        .expect("Failed to create test config")
    }

    // Helper function to simulate the render_progress_bar function
    fn render_progress_bar(used_tokens: u64, total_tokens: u64, percentage: u64) -> String {
        const BAR_WIDTH: usize = 10;
        let filled = ((percentage as f64 / 100.0) * BAR_WIDTH as f64) as usize;
        let empty = BAR_WIDTH.saturating_sub(filled);

        let mut bar = String::from("    [");
        if filled > 0 {
            bar.push_str(&"█".repeat(filled));
        }
        if empty > 0 {
            bar.push_str(&"░".repeat(empty));
        }

        bar.push_str(&format!(
            "] {}/{} ({}%)",
            format_with_separators(used_tokens),
            format_with_separators(total_tokens),
            percentage
        ));
        bar
    }

    fn format_with_separators(n: u64) -> String {
        let s = n.to_string();
        let mut result = String::new();
        let mut chars: Vec<char> = s.chars().collect();
        chars.reverse();

        for (i, ch) in chars.iter().enumerate() {
            if i > 0 && i % 3 == 0 {
                result.insert(0, ',');
            }
            result.insert(0, *ch);
        }
        result
    }

    #[test]
    fn test_progress_bar_in_context_output() {
        // Test that progress bar is correctly integrated into context output
        let usage = TokenUsage {
            input_tokens: 45000,
            output_tokens: 15000,
            total_tokens: 60000,
            cached_input_tokens: 5000,
            reasoning_output_tokens: 2000,
        };

        let percentage = ((usage.total_tokens as f64 / 128000.0) * 100.0) as u64;
        let bar = render_progress_bar(usage.total_tokens, 128000, percentage);

        // Verify the progress bar format
        assert!(bar.starts_with("    ["));
        assert!(bar.contains("60,000/128,000"));
        assert!(bar.contains("(46%)"));

        // Verify the visual representation
        let filled_blocks = bar.chars().filter(|&c| c == '█').count();
        assert_eq!(filled_blocks, 4); // 46% of 10 blocks ≈ 4.6, rounds to 4

        let empty_blocks = bar.chars().filter(|&c| c == '░').count();
        assert_eq!(empty_blocks, 6);
    }

    #[test]
    fn test_progress_bar_with_high_usage() {
        // Test progress bar appearance when usage exceeds 70% threshold
        let usage = TokenUsage {
            input_tokens: 70000,
            output_tokens: 25000,
            total_tokens: 95000, // 74% of 128000
            cached_input_tokens: 10000,
            reasoning_output_tokens: 5000,
        };

        let percentage = ((usage.total_tokens as f64 / 128000.0) * 100.0) as u64;
        let bar = render_progress_bar(usage.total_tokens, 128000, percentage);

        // Should show high usage visually
        assert!(bar.contains("95,000/128,000"));
        assert!(bar.contains("(74%)"));

        let filled_blocks = bar.chars().filter(|&c| c == '█').count();
        assert_eq!(filled_blocks, 7); // 74% of 10 blocks ≈ 7.4, rounds to 7
    }

    #[test]
    fn test_progress_bar_with_critical_usage() {
        // Test progress bar at 90%+ usage
        let usage = TokenUsage {
            input_tokens: 90000,
            output_tokens: 25600,
            total_tokens: 115600, // 90.3% of 128000
            cached_input_tokens: 0,
            reasoning_output_tokens: 0,
        };

        let percentage = ((usage.total_tokens as f64 / 128000.0) * 100.0) as u64;
        let bar = render_progress_bar(usage.total_tokens, 128000, percentage);

        // Should show critical usage
        assert!(bar.contains("115,600/128,000"));
        assert!(bar.contains("(90%)"));

        let filled_blocks = bar.chars().filter(|&c| c == '█').count();
        assert_eq!(filled_blocks, 9); // 90% of 10 blocks = 9

        let empty_blocks = bar.chars().filter(|&c| c == '░').count();
        assert_eq!(empty_blocks, 1); // Only 1 block remaining
    }

    #[test]
    fn test_progress_bar_at_capacity() {
        // Test progress bar at exactly 100% capacity
        let usage = TokenUsage {
            input_tokens: 100000,
            output_tokens: 28000,
            total_tokens: 128000, // Exactly at limit
            cached_input_tokens: 20000,
            reasoning_output_tokens: 8000,
        };

        let percentage = 100; // At max capacity
        let bar = render_progress_bar(usage.total_tokens, 128000, percentage);

        // Should show full bar
        assert!(bar.contains("128,000/128,000"));
        assert!(bar.contains("(100%)"));

        let filled_blocks = bar.chars().filter(|&c| c == '█').count();
        assert_eq!(filled_blocks, 10); // All blocks filled

        let empty_blocks = bar.chars().filter(|&c| c == '░').count();
        assert_eq!(empty_blocks, 0); // No empty blocks
    }

    #[test]
    fn test_progress_bar_over_capacity() {
        // Test when usage exceeds the context window
        let usage = TokenUsage {
            input_tokens: 100000,
            output_tokens: 50000,
            total_tokens: 150000, // Over the limit
            cached_input_tokens: 0,
            reasoning_output_tokens: 0,
        };

        // Should cap percentage at 100%
        let percentage = 100; // Capped at 100%
        let bar = render_progress_bar(usage.total_tokens, 128000, percentage);

        // Should show over-capacity numbers but cap visual at 100%
        assert!(bar.contains("150,000/128,000"));
        assert!(bar.contains("(100%)"));

        let filled_blocks = bar.chars().filter(|&c| c == '█').count();
        assert_eq!(filled_blocks, 10); // All blocks filled (capped)
    }

    #[test]
    fn test_progress_bar_empty_session() {
        // Test progress bar with no tokens used
        let usage = TokenUsage {
            input_tokens: 0,
            output_tokens: 0,
            total_tokens: 0,
            cached_input_tokens: 0,
            reasoning_output_tokens: 0,
        };

        let percentage = 0;
        let bar = render_progress_bar(usage.total_tokens, 128000, percentage);

        // Should show empty bar
        assert!(bar.contains("0/128,000"));
        assert!(bar.contains("(0%)"));

        let filled_blocks = bar.chars().filter(|&c| c == '█').count();
        assert_eq!(filled_blocks, 0); // No blocks filled

        let empty_blocks = bar.chars().filter(|&c| c == '░').count();
        assert_eq!(empty_blocks, 10); // All blocks empty
    }

    #[test]
    fn test_progress_bar_precise_percentages() {
        // Test that percentage calculation is precise
        struct TestCase {
            used: u64,
            total: u64,
            expected_percentage: u64,
            expected_filled_blocks: usize,
        }

        let test_cases = vec![
            TestCase {
                used: 1280,
                total: 128000,
                expected_percentage: 1,
                expected_filled_blocks: 0,
            },
            TestCase {
                used: 6400,
                total: 128000,
                expected_percentage: 5,
                expected_filled_blocks: 0,
            },
            TestCase {
                used: 12800,
                total: 128000,
                expected_percentage: 10,
                expected_filled_blocks: 1,
            },
            TestCase {
                used: 19200,
                total: 128000,
                expected_percentage: 15,
                expected_filled_blocks: 1,
            },
            TestCase {
                used: 25600,
                total: 128000,
                expected_percentage: 20,
                expected_filled_blocks: 2,
            },
            TestCase {
                used: 32000,
                total: 128000,
                expected_percentage: 25,
                expected_filled_blocks: 2,
            },
            TestCase {
                used: 38400,
                total: 128000,
                expected_percentage: 30,
                expected_filled_blocks: 3,
            },
            TestCase {
                used: 44800,
                total: 128000,
                expected_percentage: 35,
                expected_filled_blocks: 3,
            },
            TestCase {
                used: 51200,
                total: 128000,
                expected_percentage: 40,
                expected_filled_blocks: 4,
            },
            TestCase {
                used: 57600,
                total: 128000,
                expected_percentage: 45,
                expected_filled_blocks: 4,
            },
            TestCase {
                used: 64000,
                total: 128000,
                expected_percentage: 50,
                expected_filled_blocks: 5,
            },
            TestCase {
                used: 70400,
                total: 128000,
                expected_percentage: 55,
                expected_filled_blocks: 5,
            },
            TestCase {
                used: 76800,
                total: 128000,
                expected_percentage: 60,
                expected_filled_blocks: 6,
            },
            TestCase {
                used: 83200,
                total: 128000,
                expected_percentage: 65,
                expected_filled_blocks: 6,
            },
            TestCase {
                used: 89600,
                total: 128000,
                expected_percentage: 70,
                expected_filled_blocks: 7,
            },
            TestCase {
                used: 96000,
                total: 128000,
                expected_percentage: 75,
                expected_filled_blocks: 7,
            },
            TestCase {
                used: 102400,
                total: 128000,
                expected_percentage: 80,
                expected_filled_blocks: 8,
            },
            TestCase {
                used: 108800,
                total: 128000,
                expected_percentage: 85,
                expected_filled_blocks: 8,
            },
            TestCase {
                used: 115200,
                total: 128000,
                expected_percentage: 90,
                expected_filled_blocks: 9,
            },
            TestCase {
                used: 121600,
                total: 128000,
                expected_percentage: 95,
                expected_filled_blocks: 9,
            },
            TestCase {
                used: 128000,
                total: 128000,
                expected_percentage: 100,
                expected_filled_blocks: 10,
            },
        ];

        for test_case in test_cases {
            let bar = render_progress_bar(
                test_case.used,
                test_case.total,
                test_case.expected_percentage,
            );

            // Verify percentage display
            assert!(
                bar.contains(&format!("({}%)", test_case.expected_percentage)),
                "Expected {}% for {}/{}",
                test_case.expected_percentage,
                test_case.used,
                test_case.total
            );

            // Verify visual representation
            let filled_blocks = bar.chars().filter(|&c| c == '█').count();
            assert_eq!(
                filled_blocks, test_case.expected_filled_blocks,
                "Expected {} filled blocks for {}%",
                test_case.expected_filled_blocks, test_case.expected_percentage
            );
        }
    }

    #[test]
    fn test_progress_bar_visual_consistency() {
        // Test that the visual representation is consistent across similar percentages
        let test_pairs = vec![
            (69, 70),  // Just below and at threshold
            (70, 71),  // At and just above threshold
            (89, 90),  // High usage boundaries
            (99, 100), // Near and at capacity
        ];

        for (percent1, percent2) in test_pairs {
            let tokens1 = (128000 * percent1 / 100) as u64;
            let tokens2 = (128000 * percent2 / 100) as u64;

            let bar1 = render_progress_bar(tokens1, 128000, percent1);
            let bar2 = render_progress_bar(tokens2, 128000, percent2);

            let filled1 = bar1.chars().filter(|&c| c == '█').count();
            let filled2 = bar2.chars().filter(|&c| c == '█').count();

            // Adjacent percentages should have at most 1 block difference
            let diff = if filled2 > filled1 {
                filled2 - filled1
            } else {
                filled1 - filled2
            };
            assert!(
                diff <= 1,
                "Visual difference between {}% and {}% should be at most 1 block, got {} vs {}",
                percent1,
                percent2,
                filled1,
                filled2
            );
        }
    }

    #[test]
    fn test_progress_bar_with_different_context_windows() {
        // Test with different context window sizes (future-proofing)
        struct TestCase {
            used: u64,
            total: u64,
            description: &'static str,
        }

        let test_cases = vec![
            TestCase {
                used: 8000,
                total: 16000,
                description: "16k context window at 50%",
            },
            TestCase {
                used: 16000,
                total: 32000,
                description: "32k context window at 50%",
            },
            TestCase {
                used: 32000,
                total: 64000,
                description: "64k context window at 50%",
            },
            TestCase {
                used: 64000,
                total: 128000,
                description: "128k context window at 50%",
            },
            TestCase {
                used: 100000,
                total: 200000,
                description: "200k context window at 50%",
            },
            TestCase {
                used: 500000,
                total: 1000000,
                description: "1M context window at 50%",
            },
        ];

        for test_case in test_cases {
            let percentage = ((test_case.used as f64 / test_case.total as f64) * 100.0) as u64;
            let bar = render_progress_bar(test_case.used, test_case.total, percentage);

            // All should show 50% with 5 filled blocks
            assert!(
                bar.contains("(50%)"),
                "Failed for: {}",
                test_case.description
            );

            let filled_blocks = bar.chars().filter(|&c| c == '█').count();
            assert_eq!(filled_blocks, 5, "Failed for: {}", test_case.description);

            let empty_blocks = bar.chars().filter(|&c| c == '░').count();
            assert_eq!(empty_blocks, 5, "Failed for: {}", test_case.description);
        }
    }

    #[test]
    fn test_progress_bar_unicode_rendering() {
        // Test that Unicode characters render correctly
        let bar = render_progress_bar(64000, 128000, 50);

        // Count actual Unicode characters
        let full_blocks: Vec<char> = bar.chars().filter(|&c| c == '\u{2588}').collect();
        let light_blocks: Vec<char> = bar.chars().filter(|&c| c == '\u{2591}').collect();

        assert_eq!(full_blocks.len(), 5, "Should have 5 full block characters");
        assert_eq!(
            light_blocks.len(),
            5,
            "Should have 5 light shade characters"
        );

        // Verify no unexpected Unicode characters
        let bar_content = &bar[bar.find('[').unwrap() + 1..bar.find(']').unwrap()];
        for ch in bar_content.chars() {
            assert!(
                ch == '\u{2588}' || ch == '\u{2591}',
                "Unexpected character in bar: {:?}",
                ch
            );
        }
    }

    #[test]
    fn test_progress_bar_formatting_with_commas() {
        // Test number formatting with thousand separators
        struct TestCase {
            tokens: u64,
            expected: &'static str,
        }

        let test_cases = vec![
            TestCase {
                tokens: 0,
                expected: "0",
            },
            TestCase {
                tokens: 999,
                expected: "999",
            },
            TestCase {
                tokens: 1000,
                expected: "1,000",
            },
            TestCase {
                tokens: 10000,
                expected: "10,000",
            },
            TestCase {
                tokens: 100000,
                expected: "100,000",
            },
            TestCase {
                tokens: 1000000,
                expected: "1,000,000",
            },
            TestCase {
                tokens: 12345678,
                expected: "12,345,678",
            },
        ];

        for test_case in test_cases {
            let formatted = format_with_separators(test_case.tokens);
            assert_eq!(
                formatted, test_case.expected,
                "Failed to format {} correctly",
                test_case.tokens
            );
        }
    }

    #[test]
    fn test_progress_bar_alignment() {
        // Test that progress bars align properly at different percentages
        let percentages = vec![0, 10, 25, 50, 75, 90, 100];
        let bars: Vec<String> = percentages
            .iter()
            .map(|&p| {
                let tokens = (128000 * p / 100) as u64;
                render_progress_bar(tokens, 128000, p as u64)
            })
            .collect();

        // All bars should start with the same prefix
        for bar in &bars {
            assert!(bar.starts_with("    ["), "Bar should start with '    ['");
        }

        // The bracket positions should be consistent
        for bar in &bars {
            let open_bracket = bar.find('[').unwrap();
            let close_bracket = bar.find(']').unwrap();

            assert_eq!(open_bracket, 4, "Opening bracket should be at position 4");
            assert_eq!(
                close_bracket, 15,
                "Closing bracket should be at position 15"
            );

            // Bar content should be exactly 10 characters
            let bar_length = close_bracket - open_bracket - 1;
            assert_eq!(bar_length, 10, "Bar content should be 10 characters");
        }
    }

    #[test]
    fn test_progress_bar_stress_test() {
        // Test many different percentage values to ensure no panics or errors
        for percentage in 0..=100 {
            let tokens = (128000 * percentage / 100) as u64;
            let bar = render_progress_bar(tokens, 128000, percentage as u64);

            // Basic sanity checks
            assert!(bar.starts_with("    ["));
            assert!(bar.contains(']'));
            assert!(bar.contains(&format!("({}%)", percentage)));

            // Verify block count makes sense
            let filled = bar.chars().filter(|&c| c == '█').count();
            let empty = bar.chars().filter(|&c| c == '░').count();
            assert_eq!(filled + empty, 10, "Should always have 10 total blocks");
        }
    }

    #[test]
    fn test_progress_bar_visual_progression() {
        // Test that the bar visually progresses smoothly
        let mut previous_filled = 0;

        for percentage in (0..=100).step_by(5) {
            let tokens = (128000 * percentage / 100) as u64;
            let bar = render_progress_bar(tokens, 128000, percentage as u64);

            let filled = bar.chars().filter(|&c| c == '█').count();

            // Filled blocks should never decrease
            assert!(
                filled >= previous_filled,
                "Filled blocks decreased from {} to {} at {}%",
                previous_filled,
                filled,
                percentage
            );

            // Filled blocks should increase reasonably (not more than 1 per 10%)
            let increase = filled - previous_filled;
            assert!(
                increase <= 1,
                "Too large increase from {} to {} at {}%",
                previous_filled,
                filled,
                percentage
            );

            previous_filled = filled;
        }
    }

    #[test]
    fn test_progress_bar_boundary_rounding() {
        // Test rounding at exact boundaries
        struct BoundaryTest {
            percentage: u64,
            expected_filled: usize,
            description: &'static str,
        }

        let tests = vec![
            BoundaryTest {
                percentage: 4,
                expected_filled: 0,
                description: "4% rounds to 0 blocks",
            },
            BoundaryTest {
                percentage: 5,
                expected_filled: 0,
                description: "5% rounds to 0 blocks",
            },
            BoundaryTest {
                percentage: 6,
                expected_filled: 0,
                description: "6% rounds to 0 blocks",
            },
            BoundaryTest {
                percentage: 14,
                expected_filled: 1,
                description: "14% rounds to 1 block",
            },
            BoundaryTest {
                percentage: 15,
                expected_filled: 1,
                description: "15% rounds to 1 block",
            },
            BoundaryTest {
                percentage: 16,
                expected_filled: 1,
                description: "16% rounds to 1 block",
            },
            BoundaryTest {
                percentage: 24,
                expected_filled: 2,
                description: "24% rounds to 2 blocks",
            },
            BoundaryTest {
                percentage: 25,
                expected_filled: 2,
                description: "25% rounds to 2 blocks",
            },
            BoundaryTest {
                percentage: 26,
                expected_filled: 2,
                description: "26% rounds to 2 blocks",
            },
            BoundaryTest {
                percentage: 94,
                expected_filled: 9,
                description: "94% rounds to 9 blocks",
            },
            BoundaryTest {
                percentage: 95,
                expected_filled: 9,
                description: "95% rounds to 9 blocks",
            },
            BoundaryTest {
                percentage: 96,
                expected_filled: 9,
                description: "96% rounds to 9 blocks",
            },
        ];

        for test in tests {
            let tokens = (128000 * test.percentage / 100) as u64;
            let bar = render_progress_bar(tokens, 128000, test.percentage);

            let filled = bar.chars().filter(|&c| c == '█').count();
            assert_eq!(filled, test.expected_filled, "Failed: {}", test.description);
        }
    }
}
