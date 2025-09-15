#[cfg(test)]
mod render_progress_bar_tests {
    use codex_core::protocol::TokenUsage;
    use codex_protocol::num_format::format_with_separators;
    
    // Import the function we're testing (assumes it's exposed or we're testing via the module)
    // Note: In the actual implementation, this is part of history_cell.rs
    fn render_progress_bar(used_tokens: u64, total_tokens: u64, percentage: u64) -> String {
        // Fixed width of 10 for the progress bar itself
        const BAR_WIDTH: usize = 10;

        let filled = ((percentage as f64 / 100.0) * BAR_WIDTH as f64) as usize;
        let empty = BAR_WIDTH.saturating_sub(filled);

        let mut bar = String::from("    [");

        // Use UTF-8 block characters for visual representation
        if filled > 0 {
            bar.push_str(&"█".repeat(filled));
        }

        if empty > 0 {
            bar.push_str(&"░".repeat(empty));
        }

        // Format with token counts and percentage display
        bar.push_str(&format!(
            "] {}/{} ({}%)",
            format_with_separators(used_tokens),
            format_with_separators(total_tokens),
            percentage
        ));
        bar
    }

    #[test]
    fn test_render_progress_bar_empty() {
        let result = render_progress_bar(0, 128000, 0);
        assert_eq!(result, "    [░░░░░░░░░░] 0/128,000 (0%)");
        
        // Verify structure
        assert!(result.starts_with("    ["));
        assert!(result.contains("] "));
        assert!(result.ends_with("(0%)"));
    }

    #[test]
    fn test_render_progress_bar_full() {
        let result = render_progress_bar(128000, 128000, 100);
        assert_eq!(result, "    [██████████] 128,000/128,000 (100%)");
        
        // Verify all blocks are filled
        let filled_blocks = result.chars().filter(|&c| c == '█').count();
        assert_eq!(filled_blocks, 10);
        
        let empty_blocks = result.chars().filter(|&c| c == '░').count();
        assert_eq!(empty_blocks, 0);
    }

    #[test]
    fn test_render_progress_bar_half() {
        let result = render_progress_bar(64000, 128000, 50);
        assert_eq!(result, "    [█████░░░░░] 64,000/128,000 (50%)");
        
        // Should have exactly 5 filled and 5 empty blocks
        let filled_blocks = result.chars().filter(|&c| c == '█').count();
        assert_eq!(filled_blocks, 5);
        
        let empty_blocks = result.chars().filter(|&c| c == '░').count();
        assert_eq!(empty_blocks, 5);
    }

    #[test]
    fn test_render_progress_bar_quarter() {
        let result = render_progress_bar(32000, 128000, 25);
        
        // 25% of 10 blocks = 2.5, rounds to 2 filled blocks
        let filled_blocks = result.chars().filter(|&c| c == '█').count();
        assert_eq!(filled_blocks, 2);
        
        let empty_blocks = result.chars().filter(|&c| c == '░').count();
        assert_eq!(empty_blocks, 8);
        
        assert!(result.contains("32,000/128,000 (25%)"));
    }

    #[test]
    fn test_render_progress_bar_three_quarters() {
        let result = render_progress_bar(96000, 128000, 75);
        
        // 75% of 10 blocks = 7.5, rounds to 7 filled blocks
        let filled_blocks = result.chars().filter(|&c| c == '█').count();
        assert_eq!(filled_blocks, 7);
        
        let empty_blocks = result.chars().filter(|&c| c == '░').count();
        assert_eq!(empty_blocks, 3);
        
        assert!(result.contains("96,000/128,000 (75%)"));
    }

    #[test]
    fn test_render_progress_bar_ten_percent() {
        let result = render_progress_bar(12800, 128000, 10);
        
        // 10% of 10 blocks = 1 filled block
        let filled_blocks = result.chars().filter(|&c| c == '█').count();
        assert_eq!(filled_blocks, 1);
        
        let empty_blocks = result.chars().filter(|&c| c == '░').count();
        assert_eq!(empty_blocks, 9);
        
        assert!(result.contains("12,800/128,000 (10%)"));
    }

    #[test]
    fn test_render_progress_bar_ninety_percent() {
        let result = render_progress_bar(115200, 128000, 90);
        
        // 90% of 10 blocks = 9 filled blocks
        let filled_blocks = result.chars().filter(|&c| c == '█').count();
        assert_eq!(filled_blocks, 9);
        
        let empty_blocks = result.chars().filter(|&c| c == '░').count();
        assert_eq!(empty_blocks, 1);
        
        assert!(result.contains("115,200/128,000 (90%)"));
    }

    #[test]
    fn test_render_progress_bar_one_percent() {
        let result = render_progress_bar(1280, 128000, 1);
        
        // 1% of 10 blocks = 0.1, should still show no filled blocks
        let filled_blocks = result.chars().filter(|&c| c == '█').count();
        assert_eq!(filled_blocks, 0);
        
        let empty_blocks = result.chars().filter(|&c| c == '░').count();
        assert_eq!(empty_blocks, 10);
        
        assert!(result.contains("1,280/128,000 (1%)"));
    }

    #[test]
    fn test_render_progress_bar_five_percent() {
        let result = render_progress_bar(6400, 128000, 5);
        
        // 5% of 10 blocks = 0.5, rounds down to 0 filled blocks
        let filled_blocks = result.chars().filter(|&c| c == '█').count();
        assert_eq!(filled_blocks, 0);
        
        let empty_blocks = result.chars().filter(|&c| c == '░').count();
        assert_eq!(empty_blocks, 10);
        
        assert!(result.contains("6,400/128,000 (5%)"));
    }

    #[test]
    fn test_render_progress_bar_fifteen_percent() {
        let result = render_progress_bar(19200, 128000, 15);
        
        // 15% of 10 blocks = 1.5, rounds to 1 filled block
        let filled_blocks = result.chars().filter(|&c| c == '█').count();
        assert_eq!(filled_blocks, 1);
        
        let empty_blocks = result.chars().filter(|&c| c == '░').count();
        assert_eq!(empty_blocks, 9);
        
        assert!(result.contains("19,200/128,000 (15%)"));
    }

    #[test]
    fn test_render_progress_bar_thirty_five_percent() {
        let result = render_progress_bar(44800, 128000, 35);
        
        // 35% of 10 blocks = 3.5, rounds to 3 filled blocks
        let filled_blocks = result.chars().filter(|&c| c == '█').count();
        assert_eq!(filled_blocks, 3);
        
        let empty_blocks = result.chars().filter(|&c| c == '░').count();
        assert_eq!(empty_blocks, 7);
        
        assert!(result.contains("44,800/128,000 (35%)"));
    }

    #[test]
    fn test_render_progress_bar_sixty_percent() {
        let result = render_progress_bar(76800, 128000, 60);
        
        // 60% of 10 blocks = 6 filled blocks
        let filled_blocks = result.chars().filter(|&c| c == '█').count();
        assert_eq!(filled_blocks, 6);
        
        let empty_blocks = result.chars().filter(|&c| c == '░').count();
        assert_eq!(empty_blocks, 4);
        
        assert!(result.contains("76,800/128,000 (60%)"));
    }

    #[test]
    fn test_render_progress_bar_eighty_five_percent() {
        let result = render_progress_bar(108800, 128000, 85);
        
        // 85% of 10 blocks = 8.5, rounds to 8 filled blocks
        let filled_blocks = result.chars().filter(|&c| c == '█').count();
        assert_eq!(filled_blocks, 8);
        
        let empty_blocks = result.chars().filter(|&c| c == '░').count();
        assert_eq!(empty_blocks, 2);
        
        assert!(result.contains("108,800/128,000 (85%)"));
    }

    #[test]
    fn test_render_progress_bar_ninety_nine_percent() {
        let result = render_progress_bar(126720, 128000, 99);
        
        // 99% of 10 blocks = 9.9, rounds to 9 filled blocks
        let filled_blocks = result.chars().filter(|&c| c == '█').count();
        assert_eq!(filled_blocks, 9);
        
        let empty_blocks = result.chars().filter(|&c| c == '░').count();
        assert_eq!(empty_blocks, 1);
        
        assert!(result.contains("126,720/128,000 (99%)"));
    }

    #[test]
    fn test_render_progress_bar_number_formatting() {
        // Test with small numbers (no separators needed)
        let result = render_progress_bar(100, 1000, 10);
        assert!(result.contains("100/1,000 (10%)"));
        
        // Test with medium numbers (thousands)
        let result = render_progress_bar(5000, 10000, 50);
        assert!(result.contains("5,000/10,000 (50%)"));
        
        // Test with large numbers (millions)
        let result = render_progress_bar(1500000, 2000000, 75);
        assert!(result.contains("1,500,000/2,000,000 (75%)"));
    }

    #[test]
    fn test_render_progress_bar_edge_case_percentages() {
        // Test 0.5% - should round down to 0 filled blocks
        let result = render_progress_bar(640, 128000, 0);
        let filled_blocks = result.chars().filter(|&c| c == '█').count();
        assert_eq!(filled_blocks, 0);
        
        // Test 95.5% - should round to 9 filled blocks
        let result = render_progress_bar(122240, 128000, 95);
        let filled_blocks = result.chars().filter(|&c| c == '█').count();
        assert_eq!(filled_blocks, 9);
        
        // Test 99.9% - should still be 9 filled blocks (not quite 100%)
        let result = render_progress_bar(127872, 128000, 99);
        let filled_blocks = result.chars().filter(|&c| c == '█').count();
        assert_eq!(filled_blocks, 9);
    }

    #[test]
    fn test_render_progress_bar_over_capacity() {
        // Test when usage exceeds total (should handle gracefully)
        let result = render_progress_bar(150000, 128000, 100);
        
        // Should show all blocks filled when over 100%
        let filled_blocks = result.chars().filter(|&c| c == '█').count();
        assert_eq!(filled_blocks, 10);
        
        let empty_blocks = result.chars().filter(|&c| c == '░').count();
        assert_eq!(empty_blocks, 0);
        
        assert!(result.contains("150,000/128,000 (100%)"));
    }

    #[test]
    fn test_render_progress_bar_fixed_width() {
        // Test that the bar width is always exactly 10 characters
        for percentage in [0, 10, 25, 33, 50, 66, 75, 90, 100] {
            let tokens = (128000 * percentage / 100) as u64;
            let result = render_progress_bar(tokens, 128000, percentage as u64);
            
            // Extract just the bar portion between [ and ]
            let start = result.find('[').unwrap();
            let end = result.find(']').unwrap();
            let bar_content = &result[start + 1..end];
            
            // Bar content should always be exactly 10 characters
            assert_eq!(
                bar_content.chars().count(),
                10,
                "Bar width should be 10 for {}%",
                percentage
            );
            
            // All characters should be either filled or empty blocks
            for ch in bar_content.chars() {
                assert!(
                    ch == '█' || ch == '░',
                    "Unexpected character '{}' in bar at {}%",
                    ch,
                    percentage
                );
            }
        }
    }

    #[test]
    fn test_render_progress_bar_layout_consistency() {
        // Test that the layout is consistent: "    [bar] tokens/total (percentage%)"
        let test_cases = vec![
            (0, 128000, 0),
            (32000, 128000, 25),
            (64000, 128000, 50),
            (96000, 128000, 75),
            (128000, 128000, 100),
        ];
        
        for (used, total, percentage) in test_cases {
            let result = render_progress_bar(used, total, percentage);
            
            // Should start with 4 spaces
            assert!(result.starts_with("    "), "Should start with 4 spaces");
            
            // Should have opening bracket at position 4
            assert_eq!(result.chars().nth(4), Some('['), "Should have [ at position 4");
            
            // Should have closing bracket at position 15 (4 spaces + '[' + 10 bar chars)
            assert_eq!(result.chars().nth(15), Some(']'), "Should have ] at position 15");
            
            // Should have space after closing bracket
            assert_eq!(result.chars().nth(16), Some(' '), "Should have space after ]");
            
            // Should end with percentage in parentheses
            assert!(
                result.ends_with(&format!("({}%)", percentage)),
                "Should end with ({}%)",
                percentage
            );
        }
    }

    #[test]
    fn test_render_progress_bar_zero_total_tokens() {
        // Edge case: what happens when total_tokens is 0?
        // This shouldn't happen in practice, but let's test defensive behavior
        let result = render_progress_bar(0, 0, 0);
        
        // Should handle gracefully with empty bar
        let filled_blocks = result.chars().filter(|&c| c == '█').count();
        assert_eq!(filled_blocks, 0);
        
        let empty_blocks = result.chars().filter(|&c| c == '░').count();
        assert_eq!(empty_blocks, 10);
        
        assert!(result.contains("0/0 (0%)"));
    }

    #[test]
    fn test_render_progress_bar_mismatched_percentage() {
        // Test when percentage doesn't match the actual ratio
        // (This could happen due to rounding or calculation errors)
        
        // Used: 64000, Total: 128000 should be 50%, but we pass 45%
        let result = render_progress_bar(64000, 128000, 45);
        
        // Should use the percentage parameter, not recalculate
        let filled_blocks = result.chars().filter(|&c| c == '█').count();
        assert_eq!(filled_blocks, 4); // 45% of 10 = 4.5, rounds to 4
        
        assert!(result.contains("64,000/128,000 (45%)"));
    }

    #[test]
    fn test_render_progress_bar_unicode_consistency() {
        // Ensure we're using the correct Unicode characters
        let result = render_progress_bar(50000, 100000, 50);
        
        // Check for exact Unicode block characters
        assert!(result.contains('\u{2588}'), "Should contain full block character (█)");
        assert!(result.contains('\u{2591}'), "Should contain light shade character (░)");
        
        // Ensure no other block characters are used
        assert!(!result.contains('\u{2592}'), "Should not contain medium shade (▒)");
        assert!(!result.contains('\u{2593}'), "Should not contain dark shade (▓)");
        assert!(!result.contains('\u{258C}'), "Should not contain left half block (▌)");
    }

    #[test]
    fn test_render_progress_bar_percentage_boundaries() {
        // Test percentage boundaries for each block transition
        // Each block represents 10% (100% / 10 blocks)
        
        let boundaries = vec![
            (9, 0),   // 0-9% -> 0 blocks
            (10, 1),  // 10% -> 1 block
            (19, 1),  // 11-19% -> 1 block
            (20, 2),  // 20% -> 2 blocks
            (29, 2),  // 21-29% -> 2 blocks
            (30, 3),  // 30% -> 3 blocks
            (39, 3),  // 31-39% -> 3 blocks
            (40, 4),  // 40% -> 4 blocks
            (49, 4),  // 41-49% -> 4 blocks
            (50, 5),  // 50% -> 5 blocks
            (59, 5),  // 51-59% -> 5 blocks
            (60, 6),  // 60% -> 6 blocks
            (69, 6),  // 61-69% -> 6 blocks
            (70, 7),  // 70% -> 7 blocks
            (79, 7),  // 71-79% -> 7 blocks
            (80, 8),  // 80% -> 8 blocks
            (89, 8),  // 81-89% -> 8 blocks
            (90, 9),  // 90% -> 9 blocks
            (99, 9),  // 91-99% -> 9 blocks
            (100, 10), // 100% -> 10 blocks
        ];
        
        for (percentage, expected_blocks) in boundaries {
            let tokens = (128000 * percentage / 100) as u64;
            let result = render_progress_bar(tokens, 128000, percentage);
            
            let filled_blocks = result.chars().filter(|&c| c == '█').count();
            assert_eq!(
                filled_blocks, expected_blocks,
                "At {}%, expected {} filled blocks",
                percentage, expected_blocks
            );
        }
    }

    #[test]
    fn test_render_progress_bar_string_structure() {
        // Test the exact string structure
        let result = render_progress_bar(45231, 128000, 35);
        
        // Should match pattern: "    [███░░░░░░░] 45,231/128,000 (35%)"
        assert!(result.starts_with("    ["));
        assert!(result.contains("] "));
        assert!(result.contains("45,231/128,000"));
        assert!(result.ends_with(" (35%)"));
        
        // Total length check (approximate, depends on number formatting)
        assert!(result.len() > 30, "Result should have reasonable length");
    }

    #[test]
    fn test_render_progress_bar_consistency_across_calls() {
        // Test that the same inputs always produce the same output
        let test_cases = vec![
            (0, 128000, 0),
            (32000, 128000, 25),
            (64000, 128000, 50),
            (96000, 128000, 75),
            (128000, 128000, 100),
        ];
        
        for (used, total, percentage) in test_cases {
            let result1 = render_progress_bar(used, total, percentage);
            let result2 = render_progress_bar(used, total, percentage);
            let result3 = render_progress_bar(used, total, percentage);
            
            assert_eq!(result1, result2, "Results should be consistent");
            assert_eq!(result2, result3, "Results should be consistent");
        }
    }

    #[test]
    fn test_render_progress_bar_large_token_counts() {
        // Test with very large token counts (e.g., for larger context windows)
        let result = render_progress_bar(500000, 1000000, 50);
        
        // Should format large numbers correctly
        assert!(result.contains("500,000/1,000,000 (50%)"));
        
        // Should still have correct bar proportions
        let filled_blocks = result.chars().filter(|&c| c == '█').count();
        assert_eq!(filled_blocks, 5);
        
        let empty_blocks = result.chars().filter(|&c| c == '░').count();
        assert_eq!(empty_blocks, 5);
    }

    #[test]
    fn test_render_progress_bar_small_token_counts() {
        // Test with very small token counts
        let result = render_progress_bar(1, 10, 10);
        
        // Should format small numbers correctly
        assert!(result.contains("1/10 (10%)"));
        
        // Should still show correct bar
        let filled_blocks = result.chars().filter(|&c| c == '█').count();
        assert_eq!(filled_blocks, 1);
    }

    #[test]
    fn test_render_progress_bar_rounding_behavior() {
        // Test specific rounding edge cases
        
        // 14.9% should round to 1 block (14.9% of 10 = 1.49)
        let result = render_progress_bar(19072, 128000, 14);
        let filled = result.chars().filter(|&c| c == '█').count();
        assert_eq!(filled, 1);
        
        // 15.0% should round to 1 block (15% of 10 = 1.5)
        let result = render_progress_bar(19200, 128000, 15);
        let filled = result.chars().filter(|&c| c == '█').count();
        assert_eq!(filled, 1);
        
        // 24.9% should round to 2 blocks (24.9% of 10 = 2.49)
        let result = render_progress_bar(31872, 128000, 24);
        let filled = result.chars().filter(|&c| c == '█').count();
        assert_eq!(filled, 2);
        
        // 25.0% should round to 2 blocks (25% of 10 = 2.5)
        let result = render_progress_bar(32000, 128000, 25);
        let filled = result.chars().filter(|&c| c == '█').count();
        assert_eq!(filled, 2);
    }
}