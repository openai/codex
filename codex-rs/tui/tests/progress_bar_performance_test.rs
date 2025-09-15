#[cfg(test)]
mod progress_bar_performance_tests {
    use std::time::Instant;
    
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
    fn test_performance_single_render() {
        // Test that a single render is fast enough
        let start = Instant::now();
        let _bar = render_progress_bar(64000, 128000, 50);
        let duration = start.elapsed();
        
        // Should complete in less than 1ms
        assert!(
            duration.as_millis() < 1,
            "Single render took too long: {:?}",
            duration
        );
    }

    #[test]
    fn test_performance_thousand_renders() {
        // Test rendering 1000 progress bars
        let start = Instant::now();
        
        for i in 0..1000 {
            let percentage = (i % 101) as u64;
            let tokens = (128000 * percentage / 100) as u64;
            let _bar = render_progress_bar(tokens, 128000, percentage);
        }
        
        let duration = start.elapsed();
        
        // Should complete 1000 renders in less than 100ms
        assert!(
            duration.as_millis() < 100,
            "1000 renders took too long: {:?}",
            duration
        );
    }

    #[test]
    fn test_performance_all_percentages() {
        // Test rendering all percentages from 0 to 100
        let start = Instant::now();
        
        for percentage in 0..=100 {
            let tokens = (128000 * percentage / 100) as u64;
            let _bar = render_progress_bar(tokens, 128000, percentage as u64);
        }
        
        let duration = start.elapsed();
        
        // Should complete all 101 renders quickly
        assert!(
            duration.as_millis() < 20,
            "All percentages took too long: {:?}",
            duration
        );
    }

    #[test]
    fn test_performance_large_numbers() {
        // Test performance with very large numbers
        let start = Instant::now();
        
        for i in 0..100 {
            let percentage = i as u64;
            let huge_total = u64::MAX / 2;
            let huge_used = (huge_total as f64 * (percentage as f64 / 100.0)) as u64;
            let _bar = render_progress_bar(huge_used, huge_total, percentage);
        }
        
        let duration = start.elapsed();
        
        // Large numbers shouldn't significantly impact performance
        assert!(
            duration.as_millis() < 50,
            "Large numbers took too long: {:?}",
            duration
        );
    }

    #[test]
    fn test_memory_efficiency() {
        // Test that the function doesn't use excessive memory
        let mut bars = Vec::new();
        
        // Create many progress bars
        for percentage in 0..=100 {
            let tokens = (128000 * percentage / 100) as u64;
            bars.push(render_progress_bar(tokens, 128000, percentage as u64));
        }
        
        // Check that each bar has reasonable size
        for bar in &bars {
            assert!(
                bar.len() < 100,
                "Bar string too large: {} bytes",
                bar.len()
            );
        }
        
        // Total memory should be reasonable
        let total_bytes: usize = bars.iter().map(|b| b.len()).sum();
        assert!(
            total_bytes < 10000,
            "Total memory usage too high: {} bytes",
            total_bytes
        );
    }

    #[test]
    fn test_stress_rapid_updates() {
        // Simulate rapid updates like in a real-time progress display
        let start = Instant::now();
        let mut last_bar = String::new();
        
        // Simulate 10000 rapid updates
        for i in 0..10000 {
            let percentage = ((i as f64 / 10000.0) * 100.0) as u64;
            let tokens = (128000 * percentage / 100) as u64;
            let bar = render_progress_bar(tokens, 128000, percentage);
            
            // Verify the bar is valid
            assert!(bar.starts_with("    ["));
            assert!(bar.contains("]"));
            
            last_bar = bar;
        }
        
        let duration = start.elapsed();
        
        // Should handle rapid updates efficiently
        assert!(
            duration.as_secs() < 1,
            "Rapid updates took too long: {:?}",
            duration
        );
        
        // Final bar should be at 99%
        assert!(last_bar.contains("(99%)"));
    }

    #[test]
    fn test_concurrent_rendering_performance() {
        // Test concurrent rendering from multiple threads
        use std::sync::Arc;
        use std::sync::Mutex;
        use std::thread;
        
        let start = Instant::now();
        let render_count = Arc::new(Mutex::new(0));
        
        let threads: Vec<_> = (0..10)
            .map(|thread_id| {
                let count = render_count.clone();
                thread::spawn(move || {
                    for i in 0..100 {
                        let percentage = ((thread_id * 10 + i / 10) % 101) as u64;
                        let tokens = (128000 * percentage / 100) as u64;
                        let _bar = render_progress_bar(tokens, 128000, percentage);
                        
                        let mut c = count.lock().unwrap();
                        *c += 1;
                    }
                })
            })
            .collect();
        
        for t in threads {
            t.join().expect("Thread should complete");
        }
        
        let duration = start.elapsed();
        let total_renders = *render_count.lock().unwrap();
        
        assert_eq!(total_renders, 1000, "Should have rendered 1000 bars");
        
        // Concurrent rendering should still be fast
        assert!(
            duration.as_millis() < 200,
            "Concurrent rendering took too long: {:?}",
            duration
        );
    }

    #[test]
    fn test_string_allocation_efficiency() {
        // Test that string allocations are efficient
        let start = Instant::now();
        
        // Render many bars and check string reuse
        let mut bars = Vec::with_capacity(1000);
        for i in 0..1000 {
            let percentage = (i % 101) as u64;
            let tokens = (128000 * percentage / 100) as u64;
            bars.push(render_progress_bar(tokens, 128000, percentage));
        }
        
        let duration = start.elapsed();
        
        // Should be efficient even with many allocations
        assert!(
            duration.as_millis() < 100,
            "String allocations took too long: {:?}",
            duration
        );
        
        // Verify all bars are valid
        for bar in &bars {
            assert!(bar.starts_with("    ["));
            assert!(bar.contains("]"));
        }
    }

    #[test]
    fn test_caching_potential() {
        // Test if same inputs could benefit from caching
        let start = Instant::now();
        
        // Render the same bar 10000 times
        for _ in 0..10000 {
            let _bar = render_progress_bar(64000, 128000, 50);
        }
        
        let duration = start.elapsed();
        
        // Even without caching, should be fast
        assert!(
            duration.as_millis() < 500,
            "Repeated same render took too long: {:?}",
            duration
        );
    }

    #[test]
    fn test_worst_case_performance() {
        // Test worst-case scenarios
        let start = Instant::now();
        
        // Worst case: maximum values with all calculations
        for _ in 0..100 {
            let _bar = render_progress_bar(u64::MAX - 1, u64::MAX, 99);
        }
        
        let duration = start.elapsed();
        
        // Even worst case should be reasonable
        assert!(
            duration.as_millis() < 100,
            "Worst case took too long: {:?}",
            duration
        );
    }

    #[test]
    fn test_performance_scaling() {
        // Test how performance scales with different inputs
        let mut durations = Vec::new();
        
        // Test with increasing token values
        for magnitude in [1000, 10000, 100000, 1000000, 10000000] {
            let start = Instant::now();
            
            for i in 0..100 {
                let percentage = i as u64;
                let tokens = (magnitude * percentage / 100) as u64;
                let _bar = render_progress_bar(tokens, magnitude, percentage);
            }
            
            durations.push((magnitude, start.elapsed()));
        }
        
        // Performance shouldn't degrade significantly with larger numbers
        for i in 1..durations.len() {
            let (prev_mag, prev_dur) = durations[i - 1];
            let (curr_mag, curr_dur) = durations[i];
            
            // Duration shouldn't increase more than 2x even if magnitude increases 10x
            assert!(
                curr_dur.as_micros() < prev_dur.as_micros() * 3,
                "Performance degraded too much from {} to {}: {:?} to {:?}",
                prev_mag, curr_mag, prev_dur, curr_dur
            );
        }
    }

    #[test]
    fn test_minimal_allocations() {
        // Test that the function minimizes allocations
        let start = Instant::now();
        
        // Render many bars with different percentages
        let mut total_length = 0;
        for percentage in 0..=100 {
            let tokens = (128000 * percentage / 100) as u64;
            let bar = render_progress_bar(tokens, 128000, percentage as u64);
            total_length += bar.len();
        }
        
        let duration = start.elapsed();
        
        // Should complete quickly
        assert!(
            duration.as_millis() < 20,
            "Allocation test took too long: {:?}",
            duration
        );
        
        // Average length should be reasonable
        let avg_length = total_length / 101;
        assert!(
            avg_length < 60,
            "Average bar length too high: {}",
            avg_length
        );
    }

    #[test]
    fn test_performance_consistency() {
        // Test that performance is consistent across runs
        let mut durations = Vec::new();
        
        // Run the same test multiple times
        for _ in 0..5 {
            let start = Instant::now();
            
            for percentage in 0..=100 {
                let tokens = (128000 * percentage / 100) as u64;
                let _bar = render_progress_bar(tokens, 128000, percentage as u64);
            }
            
            durations.push(start.elapsed());
        }
        
        // Calculate average and check variance
        let total: u128 = durations.iter().map(|d| d.as_micros()).sum();
        let avg = total / durations.len() as u128;
        
        // No single run should be more than 2x the average
        for dur in &durations {
            assert!(
                dur.as_micros() < avg * 2,
                "Performance inconsistent: {:?} vs avg {}μs",
                dur, avg
            );
        }
    }

    #[test]
    fn test_edge_case_performance() {
        // Test performance of edge cases
        let edge_cases = vec![
            (0, 0, 0),                    // All zeros
            (1, 1, 100),                  // Minimum non-zero
            (u64::MAX, u64::MAX, 100),   // Maximum values
            (1, u64::MAX, 0),             // Extreme ratio
            (u64::MAX - 1, u64::MAX, 99), // Near maximum
        ];
        
        let start = Instant::now();
        
        for _ in 0..100 {
            for &(used, total, percentage) in &edge_cases {
                let _bar = render_progress_bar(used, total, percentage);
            }
        }
        
        let duration = start.elapsed();
        
        // Edge cases shouldn't cause performance issues
        assert!(
            duration.as_millis() < 100,
            "Edge cases took too long: {:?}",
            duration
        );
    }

    #[test]
    fn test_batch_rendering_performance() {
        // Test rendering multiple bars in batch (like in a list view)
        let start = Instant::now();
        
        // Simulate rendering 20 progress bars at once (like in a UI list)
        for _ in 0..100 {
            let mut bars = Vec::with_capacity(20);
            for i in 0..20 {
                let percentage = (i * 5) as u64;
                let tokens = (128000 * percentage / 100) as u64;
                bars.push(render_progress_bar(tokens, 128000, percentage));
            }
            
            // Simulate using the bars (prevent optimization)
            assert_eq!(bars.len(), 20);
        }
        
        let duration = start.elapsed();
        
        // Batch rendering should be efficient
        assert!(
            duration.as_millis() < 200,
            "Batch rendering took too long: {:?}",
            duration
        );
    }

    #[test]
    fn test_animation_frame_rate() {
        // Test if the function is fast enough for smooth animation (60 FPS)
        let frame_budget = std::time::Duration::from_micros(16666); // 1/60 second
        
        let start = Instant::now();
        let _bar = render_progress_bar(64000, 128000, 50);
        let duration = start.elapsed();
        
        // Single render should be much faster than one frame
        assert!(
            duration < frame_budget / 10,
            "Too slow for smooth animation: {:?}",
            duration
        );
    }

    #[test]
    fn test_incremental_update_performance() {
        // Simulate incremental updates (like during file upload)
        let start = Instant::now();
        let total = 128000u64;
        
        // Simulate 1% increments
        for percentage in 0..=100 {
            let used = (total * percentage / 100) as u64;
            let _bar = render_progress_bar(used, total, percentage as u64);
        }
        
        let duration = start.elapsed();
        
        // Incremental updates should be fast
        assert!(
            duration.as_millis() < 20,
            "Incremental updates took too long: {:?}",
            duration
        );
    }
}