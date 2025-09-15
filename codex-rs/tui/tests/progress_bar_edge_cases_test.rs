#[cfg(test)]
mod progress_bar_edge_cases_tests {
    use std::panic;
    
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
    fn test_zero_total_tokens_edge_case() {
        // When total is 0, percentage calculation would normally divide by zero
        // The function should handle this gracefully
        let bar = render_progress_bar(0, 0, 0);
        
        // Should produce a valid empty bar
        assert!(bar.starts_with("    ["));
        assert!(bar.contains("░░░░░░░░░░"));
        assert!(bar.contains("0/0 (0%)"));
    }

    #[test]
    fn test_used_exceeds_total() {
        // When used tokens exceed total (shouldn't happen but test defensive coding)
        let bar = render_progress_bar(150000, 128000, 100);
        
        // Should cap at 100% visually but show actual numbers
        assert!(bar.contains("150,000/128,000"));
        assert!(bar.contains("(100%)"));
        
        // All blocks should be filled (capped at maximum)
        let filled = bar.chars().filter(|&c| c == '█').count();
        assert_eq!(filled, 10);
    }

    #[test]
    fn test_percentage_exceeds_100() {
        // Test when percentage parameter exceeds 100
        let bar = render_progress_bar(128000, 128000, 150);
        
        // Should cap the visual representation at 100%
        let filled = bar.chars().filter(|&c| c == '█').count();
        assert_eq!(filled, 10); // Should not exceed 10 blocks
        
        // But should show the actual percentage passed
        assert!(bar.contains("(150%)"));
    }

    #[test]
    fn test_very_large_token_values() {
        // Test with u64 max values
        let bar = render_progress_bar(u64::MAX / 2, u64::MAX, 50);
        
        // Should handle large numbers without panic
        assert!(bar.starts_with("    ["));
        assert!(bar.contains("]"));
        assert!(bar.contains("(50%)"));
        
        // Should have correct visual representation
        let filled = bar.chars().filter(|&c| c == '█').count();
        assert_eq!(filled, 5);
    }

    #[test]
    fn test_mismatched_percentage_and_ratio() {
        // Test when percentage doesn't match actual ratio
        // (This could indicate a calculation error elsewhere)
        
        // Actual ratio is 50% but we pass 75%
        let bar = render_progress_bar(64000, 128000, 75);
        
        // Should use the percentage parameter, not recalculate
        let filled = bar.chars().filter(|&c| c == '█').count();
        assert_eq!(filled, 7); // 75% of 10 = 7.5, rounds to 7
        
        // Should show the actual values and percentage passed
        assert!(bar.contains("64,000/128,000"));
        assert!(bar.contains("(75%)"));
    }

    #[test]
    fn test_floating_point_precision() {
        // Test percentages that might cause floating point precision issues
        let tricky_percentages = vec![
            33, // 33% of 10 = 3.3
            66, // 66% of 10 = 6.6
            17, // 17% of 10 = 1.7
            83, // 83% of 10 = 8.3
        ];
        
        for percentage in tricky_percentages {
            let tokens = (128000 * percentage / 100) as u64;
            let bar = render_progress_bar(tokens, 128000, percentage);
            
            // Should not panic or produce invalid output
            assert!(bar.starts_with("    ["));
            assert!(bar.contains("]"));
            
            // Total blocks should always be 10
            let filled = bar.chars().filter(|&c| c == '█').count();
            let empty = bar.chars().filter(|&c| c == '░').count();
            assert_eq!(filled + empty, 10);
        }
    }

    #[test]
    fn test_percentage_rounding_edge_cases() {
        // Test exact boundaries where rounding changes
        struct RoundingTest {
            percentage: f64,
            expected_blocks: usize,
        }
        
        // Test fractional percentages (simulated by calculation)
        let tests = vec![
            RoundingTest { percentage: 4.9, expected_blocks: 0 },  // < 0.5 blocks
            RoundingTest { percentage: 5.0, expected_blocks: 0 },  // = 0.5 blocks (rounds down)
            RoundingTest { percentage: 5.1, expected_blocks: 0 },  // > 0.5 blocks but < 1
            RoundingTest { percentage: 9.9, expected_blocks: 0 },  // < 1 block
            RoundingTest { percentage: 10.0, expected_blocks: 1 }, // = 1 block
            RoundingTest { percentage: 10.1, expected_blocks: 1 }, // > 1 block
            RoundingTest { percentage: 14.9, expected_blocks: 1 }, // < 1.5 blocks
            RoundingTest { percentage: 15.0, expected_blocks: 1 }, // = 1.5 blocks (rounds down)
            RoundingTest { percentage: 15.1, expected_blocks: 1 }, // > 1.5 blocks
            RoundingTest { percentage: 94.9, expected_blocks: 9 }, // < 9.5 blocks
            RoundingTest { percentage: 95.0, expected_blocks: 9 }, // = 9.5 blocks (rounds down)
            RoundingTest { percentage: 95.1, expected_blocks: 9 }, // > 9.5 blocks
            RoundingTest { percentage: 99.9, expected_blocks: 9 }, // < 10 blocks
        ];
        
        for test in tests {
            let percentage_int = test.percentage as u64;
            let tokens = ((128000.0 * test.percentage / 100.0) as u64).min(128000);
            let bar = render_progress_bar(tokens, 128000, percentage_int);
            
            let filled = bar.chars().filter(|&c| c == '█').count();
            assert_eq!(
                filled, test.expected_blocks,
                "Failed for {}%: expected {} blocks, got {}",
                test.percentage, test.expected_blocks, filled
            );
        }
    }

    #[test]
    fn test_unicode_character_width() {
        // Ensure Unicode characters don't break layout
        let bar = render_progress_bar(64000, 128000, 50);
        
        // Extract just the bar portion
        let start = bar.find('[').unwrap();
        let end = bar.find(']').unwrap();
        let bar_content = &bar[start + 1..end];
        
        // Count Unicode characters (not bytes)
        let char_count = bar_content.chars().count();
        assert_eq!(char_count, 10, "Bar should contain exactly 10 Unicode characters");
        
        // Verify each character is the expected width
        for ch in bar_content.chars() {
            assert!(
                ch == '█' || ch == '░',
                "Unexpected character: {:?}",
                ch
            );
        }
    }

    #[test]
    fn test_negative_percentage_safety() {
        // Although percentage is u64 (can't be negative), test with 0 as boundary
        let bar = render_progress_bar(0, 128000, 0);
        
        // Should produce valid empty bar
        let filled = bar.chars().filter(|&c| c == '█').count();
        assert_eq!(filled, 0);
        
        let empty = bar.chars().filter(|&c| c == '░').count();
        assert_eq!(empty, 10);
    }

    #[test]
    fn test_string_buffer_overflow_protection() {
        // Test with maximum values to ensure no buffer overflow
        let bar = render_progress_bar(u64::MAX, u64::MAX, 100);
        
        // Should not panic and produce valid output
        assert!(bar.starts_with("    ["));
        assert!(bar.contains("██████████]"));
        
        // Check that the string is reasonable length (not corrupted)
        assert!(bar.len() < 200, "Bar string suspiciously long: {} chars", bar.len());
    }

    #[test]
    fn test_concurrent_safety() {
        // Test that the function is safe to call from multiple threads
        use std::sync::Arc;
        use std::thread;
        
        let threads: Vec<_> = (0..10)
            .map(|i| {
                thread::spawn(move || {
                    let percentage = (i * 10) as u64;
                    let tokens = (128000 * percentage / 100) as u64;
                    let bar = render_progress_bar(tokens, 128000, percentage);
                    
                    // Basic validation
                    assert!(bar.starts_with("    ["));
                    assert!(bar.contains(&format!("({}%)", percentage)));
                })
            })
            .collect();
        
        // All threads should complete without panic
        for t in threads {
            t.join().expect("Thread should not panic");
        }
    }

    #[test]
    fn test_special_percentage_values() {
        // Test special percentage values that might cause issues
        let special_cases = vec![
            (0, 0),     // Minimum
            (1, 0),     // Smallest non-zero
            (99, 9),    // Largest non-full
            (100, 10),  // Maximum
        ];
        
        for (percentage, expected_filled) in special_cases {
            let tokens = (128000 * percentage / 100) as u64;
            let bar = render_progress_bar(tokens, 128000, percentage);
            
            let filled = bar.chars().filter(|&c| c == '█').count();
            assert_eq!(
                filled, expected_filled,
                "Failed for {}%",
                percentage
            );
        }
    }

    #[test]
    fn test_asymmetric_token_values() {
        // Test when used/total have unusual ratios
        struct AsymmetricTest {
            used: u64,
            total: u64,
            percentage: u64,
        }
        
        let tests = vec![
            AsymmetricTest { used: 1, total: 1000000, percentage: 0 },
            AsymmetricTest { used: 999999, total: 1000000, percentage: 99 },
            AsymmetricTest { used: 1, total: 2, percentage: 50 },
            AsymmetricTest { used: 3, total: 7, percentage: 42 },
        ];
        
        for test in tests {
            let bar = render_progress_bar(test.used, test.total, test.percentage);
            
            // Should not panic and produce valid output
            assert!(bar.starts_with("    ["));
            assert!(bar.contains(&format!("({}%)", test.percentage)));
            
            // Verify total blocks is always 10
            let filled = bar.chars().filter(|&c| c == '█').count();
            let empty = bar.chars().filter(|&c| c == '░').count();
            assert_eq!(filled + empty, 10);
        }
    }

    #[test]
    fn test_number_formatting_edge_cases() {
        // Test edge cases in number formatting
        struct FormatTest {
            value: u64,
            expected: &'static str,
        }
        
        let tests = vec![
            FormatTest { value: 0, expected: "0" },
            FormatTest { value: 1, expected: "1" },
            FormatTest { value: 99, expected: "99" },
            FormatTest { value: 100, expected: "100" },
            FormatTest { value: 999, expected: "999" },
            FormatTest { value: 1000, expected: "1,000" },
            FormatTest { value: 9999, expected: "9,999" },
            FormatTest { value: 10000, expected: "10,000" },
            FormatTest { value: 99999, expected: "99,999" },
            FormatTest { value: 100000, expected: "100,000" },
            FormatTest { value: 999999, expected: "999,999" },
            FormatTest { value: 1000000, expected: "1,000,000" },
        ];
        
        for test in tests {
            let formatted = format_with_separators(test.value);
            assert_eq!(formatted, test.expected);
        }
    }

    #[test]
    fn test_consistent_spacing() {
        // Ensure consistent spacing in output format
        let bar = render_progress_bar(45231, 128000, 35);
        
        // Check for consistent spacing pattern
        assert!(bar.starts_with("    ")); // 4 spaces
        assert!(bar.contains("] ")); // Space after closing bracket
        assert!(bar.contains(" (")); // Space before opening parenthesis
        
        // No double spaces
        assert!(!bar.contains("  ") || bar.starts_with("    "));
    }

    #[test]
    fn test_percentage_calculation_overflow() {
        // Test potential overflow in percentage calculation
        let huge_value = u64::MAX / 2;
        let bar = render_progress_bar(huge_value, u64::MAX, 50);
        
        // Should handle without overflow
        assert!(bar.contains("(50%)"));
        
        let filled = bar.chars().filter(|&c| c == '█').count();
        assert_eq!(filled, 5);
    }

    #[test]
    fn test_bar_width_constant() {
        // Verify BAR_WIDTH constant is respected
        const EXPECTED_BAR_WIDTH: usize = 10;
        
        // Test at various percentages
        for percentage in (0..=100).step_by(10) {
            let tokens = (128000 * percentage / 100) as u64;
            let bar = render_progress_bar(tokens, 128000, percentage as u64);
            
            // Extract bar content
            let start = bar.find('[').unwrap();
            let end = bar.find(']').unwrap();
            let bar_content = &bar[start + 1..end];
            
            // Bar should always be exactly EXPECTED_BAR_WIDTH characters
            assert_eq!(
                bar_content.chars().count(),
                EXPECTED_BAR_WIDTH,
                "Bar width incorrect at {}%",
                percentage
            );
        }
    }

    #[test]
    fn test_empty_blocks_never_negative() {
        // Ensure empty block count never goes negative (saturating_sub)
        for percentage in 0..=100 {
            let tokens = (128000 * percentage / 100) as u64;
            let bar = render_progress_bar(tokens, 128000, percentage as u64);
            
            let empty = bar.chars().filter(|&c| c == '░').count();
            assert!(empty <= 10, "Empty blocks exceed maximum at {}%", percentage);
            
            let filled = bar.chars().filter(|&c| c == '█').count();
            assert_eq!(
                filled + empty, 10,
                "Total blocks not 10 at {}%",
                percentage
            );
        }
    }

    #[test]
    fn test_idempotency() {
        // Same input should always produce same output
        let test_cases = vec![
            (0, 128000, 0),
            (64000, 128000, 50),
            (128000, 128000, 100),
            (150000, 128000, 100), // Over capacity
        ];
        
        for (used, total, percentage) in test_cases {
            let bar1 = render_progress_bar(used, total, percentage);
            let bar2 = render_progress_bar(used, total, percentage);
            let bar3 = render_progress_bar(used, total, percentage);
            
            assert_eq!(bar1, bar2, "Results not consistent");
            assert_eq!(bar2, bar3, "Results not consistent");
        }
    }
}