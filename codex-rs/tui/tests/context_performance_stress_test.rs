//! Performance and stress tests for context command
//! Tests performance under load, memory usage, and concurrent access

use codex_core::config::{Config, ConfigOverrides, ConfigToml};
use codex_core::protocol::TokenUsage;
use codex_protocol::mcp_protocol::ConversationId;
use codex_tui::history_cell::{HistoryCell, new_context_output, render_progress_bar};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

/// Helper to create test config
fn test_config() -> Config {
    Config::load_from_base_config_with_overrides(
        ConfigToml::default(),
        ConfigOverrides::default(),
        std::env::temp_dir(),
    )
    .expect("Failed to create config")
}

/// Benchmark context output generation
#[test]
fn test_context_output_performance() {
    let config = test_config();
    let iterations = 1000;

    let usage = TokenUsage {
        input_tokens: 50000,
        output_tokens: 25000,
        total_tokens: 75000,
        cached_input_tokens: 10000,
        reasoning_output_tokens: 5000,
    };
    let session_id = Some(ConversationId::new());

    let start = Instant::now();

    for _ in 0..iterations {
        let cell = new_context_output(&config, &usage, &session_id);
        let _ = cell.display_lines(80);
    }

    let elapsed = start.elapsed();
    let avg_time = elapsed / iterations;

    println!("Average time per context output: {:?}", avg_time);

    // Performance assertion - should be fast
    assert!(
        avg_time < Duration::from_millis(10),
        "Context output should generate in less than 10ms on average, got {:?}",
        avg_time
    );
}

/// Test progress bar rendering performance
#[test]
fn test_progress_bar_performance() {
    let iterations = 10000;
    let start = Instant::now();

    for i in 0..iterations {
        let percentage = (i % 101) as i32;
        let _ = render_progress_bar(percentage, 40);
    }

    let elapsed = start.elapsed();
    let avg_time = elapsed / iterations;

    println!("Average time per progress bar: {:?}", avg_time);

    assert!(
        avg_time < Duration::from_micros(100),
        "Progress bar should render in less than 100μs on average, got {:?}",
        avg_time
    );
}

/// Stress test with rapid updates
#[test]
fn test_rapid_context_updates() {
    let config = test_config();
    let session_id = Some(ConversationId::new());

    // Simulate rapid token updates over time
    let updates = 500;
    let start = Instant::now();

    for i in 0..updates {
        let tokens = (i * 256) % 128000;
        let usage = TokenUsage {
            input_tokens: tokens * 3 / 4,
            output_tokens: tokens / 4,
            total_tokens: tokens,
            cached_input_tokens: tokens / 10,
            reasoning_output_tokens: tokens / 20,
        };

        let cell = new_context_output(&config, &usage, &session_id);
        let lines = cell.display_lines(80);

        // Verify output is valid
        assert!(!lines.is_empty(), "Update {} should produce output", i);
    }

    let elapsed = start.elapsed();

    println!("Time for {} rapid updates: {:?}", updates, elapsed);

    assert!(
        elapsed < Duration::from_secs(2),
        "Should handle {} updates in less than 2 seconds, took {:?}",
        updates,
        elapsed
    );
}

/// Test concurrent access from multiple threads
#[test]
fn test_concurrent_context_access() {
    let config = Arc::new(test_config());
    let num_threads = 20;
    let operations_per_thread = 100;

    let start = Instant::now();
    let mut handles = vec![];

    for thread_id in 0..num_threads {
        let config_clone = Arc::clone(&config);

        let handle = thread::spawn(move || {
            let mut results = Vec::new();

            for op in 0..operations_per_thread {
                let tokens = (thread_id * 1000 + op * 100) % 128000;
                let usage = TokenUsage {
                    input_tokens: tokens,
                    output_tokens: tokens / 2,
                    total_tokens: tokens * 3 / 2,
                    cached_input_tokens: tokens / 4,
                    reasoning_output_tokens: tokens / 8,
                };

                let cell = new_context_output(&config_clone, &usage, &None);
                let lines = cell.display_lines(80);
                results.push(lines.len());
            }

            results
        });

        handles.push(handle);
    }

    // Collect all results
    let mut total_operations = 0;
    for handle in handles {
        let results = handle.join().expect("Thread should complete");
        total_operations += results.len();

        // Verify all operations produced valid output
        for line_count in results {
            assert!(line_count > 0, "Each operation should produce output");
        }
    }

    let elapsed = start.elapsed();

    println!(
        "Time for {} concurrent operations: {:?}",
        total_operations, elapsed
    );

    assert_eq!(
        total_operations,
        num_threads * operations_per_thread,
        "All operations should complete"
    );

    assert!(
        elapsed < Duration::from_secs(5),
        "Concurrent operations should complete within 5 seconds, took {:?}",
        elapsed
    );
}

/// Memory stress test - create many context outputs
#[test]
fn test_memory_stress() {
    let config = test_config();
    let num_instances = 1000;

    let mut cells = Vec::new();
    let start = Instant::now();

    for i in 0..num_instances {
        let usage = TokenUsage {
            input_tokens: i * 100,
            output_tokens: i * 50,
            total_tokens: i * 150,
            cached_input_tokens: i * 10,
            reasoning_output_tokens: i * 5,
        };

        let cell = new_context_output(&config, &usage, &None);
        cells.push(cell);
    }

    let creation_time = start.elapsed();

    // Now render all of them
    let render_start = Instant::now();
    for cell in &cells {
        let _ = cell.display_lines(80);
    }
    let render_time = render_start.elapsed();

    println!("Created {} instances in {:?}", num_instances, creation_time);
    println!("Rendered {} instances in {:?}", num_instances, render_time);

    assert!(
        creation_time < Duration::from_secs(2),
        "Should create {} instances quickly",
        num_instances
    );
    assert!(
        render_time < Duration::from_secs(3),
        "Should render {} instances quickly",
        num_instances
    );
}

/// Test with various display widths performance
#[test]
fn test_various_widths_performance() {
    let config = test_config();
    let usage = TokenUsage {
        input_tokens: 50000,
        output_tokens: 25000,
        total_tokens: 75000,
        cached_input_tokens: 10000,
        reasoning_output_tokens: 5000,
    };

    let widths: Vec<u16> = (20..=200).step_by(10).collect();
    let iterations_per_width = 100;

    let start = Instant::now();

    for width in &widths {
        for _ in 0..iterations_per_width {
            let cell = new_context_output(&config, &usage, &None);
            let _ = cell.display_lines(*width);
        }
    }

    let elapsed = start.elapsed();
    let total_operations = widths.len() * iterations_per_width;
    let avg_time = elapsed / total_operations as u32;

    println!("Average time per render at various widths: {:?}", avg_time);

    assert!(
        avg_time < Duration::from_millis(5),
        "Should handle different widths efficiently, avg {:?}",
        avg_time
    );
}

/// Test cache efficiency with repeated identical calls
#[test]
fn test_repeated_identical_calls() {
    let config = test_config();
    let usage = TokenUsage {
        input_tokens: 30000,
        output_tokens: 15000,
        total_tokens: 45000,
        cached_input_tokens: 5000,
        reasoning_output_tokens: 2000,
    };
    let session_id = Some(ConversationId::new());

    let iterations = 1000;

    // First pass - cold
    let cold_start = Instant::now();
    for _ in 0..iterations {
        let cell = new_context_output(&config, &usage, &session_id);
        let _ = cell.display_lines(80);
    }
    let cold_time = cold_start.elapsed();

    // Second pass - potentially optimized
    let warm_start = Instant::now();
    for _ in 0..iterations {
        let cell = new_context_output(&config, &usage, &session_id);
        let _ = cell.display_lines(80);
    }
    let warm_time = warm_start.elapsed();

    println!("Cold time: {:?}, Warm time: {:?}", cold_time, warm_time);

    // Both should be fast
    assert!(
        cold_time < Duration::from_secs(1),
        "Cold calls should be fast"
    );
    assert!(
        warm_time < Duration::from_secs(1),
        "Warm calls should be fast"
    );
}

/// Test worst-case scenario performance
#[test]
fn test_worst_case_performance() {
    let config = test_config();

    // Worst case: maximum values, all token types present
    let usage = TokenUsage {
        input_tokens: i32::MAX / 2,
        output_tokens: i32::MAX / 2,
        total_tokens: i32::MAX,
        cached_input_tokens: i32::MAX / 4,
        reasoning_output_tokens: i32::MAX / 4,
    };
    let session_id = Some(ConversationId::new());

    let iterations = 100;
    let start = Instant::now();

    for _ in 0..iterations {
        let cell = new_context_output(&config, &usage, &session_id);
        let _ = cell.display_lines(u16::MAX); // Maximum width too
    }

    let elapsed = start.elapsed();
    let avg_time = elapsed / iterations;

    println!("Worst-case average time: {:?}", avg_time);

    assert!(
        avg_time < Duration::from_millis(50),
        "Even worst case should be reasonably fast, got {:?}",
        avg_time
    );
}

/// Test performance with increasing token counts
#[test]
fn test_scaling_performance() {
    let config = test_config();
    let token_counts = vec![100, 1000, 10000, 50000, 100000, 128000, 200000];

    let mut times = Vec::new();

    for &tokens in &token_counts {
        let usage = TokenUsage {
            input_tokens: tokens * 2 / 3,
            output_tokens: tokens / 3,
            total_tokens: tokens,
            cached_input_tokens: tokens / 10,
            reasoning_output_tokens: tokens / 20,
        };

        let iterations = 100;
        let start = Instant::now();

        for _ in 0..iterations {
            let cell = new_context_output(&config, &usage, &None);
            let _ = cell.display_lines(80);
        }

        let elapsed = start.elapsed();
        let avg_time = elapsed / iterations;
        times.push((tokens, avg_time));
    }

    // Print scaling results
    for (tokens, time) in &times {
        println!("Tokens: {}, Avg time: {:?}", tokens, time);
    }

    // Performance should not degrade significantly with token count
    let first_time = times[0].1;
    let last_time = times[times.len() - 1].1;

    assert!(
        last_time < first_time * 10,
        "Performance should not degrade more than 10x with token count"
    );
}

/// Test thread safety with shared state
#[test]
fn test_thread_safety_shared_state() {
    let config = Arc::new(test_config());
    let shared_counter = Arc::new(Mutex::new(0));
    let num_threads = 10;
    let ops_per_thread = 50;

    let mut handles = vec![];

    for thread_id in 0..num_threads {
        let config_clone = Arc::clone(&config);
        let counter_clone = Arc::clone(&shared_counter);

        let handle = thread::spawn(move || {
            for _ in 0..ops_per_thread {
                let tokens = {
                    let mut counter = counter_clone.lock().unwrap();
                    *counter += 1000;
                    *counter
                };

                let usage = TokenUsage {
                    input_tokens: tokens,
                    output_tokens: tokens / 2,
                    total_tokens: tokens * 3 / 2,
                    cached_input_tokens: 0,
                    reasoning_output_tokens: 0,
                };

                let cell = new_context_output(&config_clone, &usage, &None);
                let lines = cell.display_lines(80);

                assert!(
                    !lines.is_empty(),
                    "Thread {} should produce output",
                    thread_id
                );
            }
        });

        handles.push(handle);
    }

    // Wait for all threads
    for handle in handles {
        handle.join().expect("Thread should complete");
    }

    let final_count = *shared_counter.lock().unwrap();
    assert_eq!(
        final_count,
        num_threads * ops_per_thread * 1000,
        "Counter should reflect all operations"
    );
}

/// Stress test HistoryCell trait methods
#[test]
fn test_history_cell_methods_performance() {
    let config = test_config();
    let usage = TokenUsage {
        input_tokens: 50000,
        output_tokens: 25000,
        total_tokens: 75000,
        cached_input_tokens: 10000,
        reasoning_output_tokens: 5000,
    };

    let cell = new_context_output(&config, &usage, &None);
    let iterations = 1000;

    // Test display_lines performance
    let display_start = Instant::now();
    for _ in 0..iterations {
        let _ = cell.display_lines(80);
    }
    let display_time = display_start.elapsed();

    // Test transcript_lines performance
    let transcript_start = Instant::now();
    for _ in 0..iterations {
        let _ = cell.transcript_lines();
    }
    let transcript_time = transcript_start.elapsed();

    // Test desired_height performance
    let height_start = Instant::now();
    for _ in 0..iterations {
        let _ = cell.desired_height(80);
    }
    let height_time = height_start.elapsed();

    // Test is_stream_continuation performance
    let stream_start = Instant::now();
    for _ in 0..iterations {
        let _ = cell.is_stream_continuation();
    }
    let stream_time = stream_start.elapsed();

    println!("Method performance ({} iterations):", iterations);
    println!("  display_lines: {:?}", display_time);
    println!("  transcript_lines: {:?}", transcript_time);
    println!("  desired_height: {:?}", height_time);
    println!("  is_stream_continuation: {:?}", stream_time);

    // All methods should be fast
    assert!(display_time < Duration::from_secs(1));
    assert!(transcript_time < Duration::from_secs(1));
    assert!(height_time < Duration::from_millis(100));
    assert!(stream_time < Duration::from_millis(10));
}

/// Test performance with random data patterns
#[test]
fn test_random_data_patterns() {
    use rand::Rng;
    let mut rng = rand::thread_rng();

    let config = test_config();
    let iterations = 500;
    let start = Instant::now();

    for _ in 0..iterations {
        let input = rng.gen_range(0..100000);
        let output = rng.gen_range(0..50000);
        let cached = rng.gen_range(0..input.max(1));
        let reasoning = rng.gen_range(0..output.max(1));

        let usage = TokenUsage {
            input_tokens: input,
            output_tokens: output,
            total_tokens: input + output,
            cached_input_tokens: cached,
            reasoning_output_tokens: reasoning,
        };

        let session_id = if rng.gen_bool(0.5) {
            Some(ConversationId::new())
        } else {
            None
        };

        let cell = new_context_output(&config, &usage, &session_id);
        let width = rng.gen_range(20..200);
        let _ = cell.display_lines(width);
    }

    let elapsed = start.elapsed();
    let avg_time = elapsed / iterations;

    println!("Average time with random data: {:?}", avg_time);

    assert!(
        avg_time < Duration::from_millis(10),
        "Should handle random patterns efficiently"
    );
}

/// Benchmark number formatting performance
#[test]
fn test_number_formatting_performance() {
    let config = test_config();
    let iterations = 1000;

    // Test with various number magnitudes
    let test_values = vec![
        0,
        1,
        10,
        100,
        1000,
        10000,
        100000,
        1000000,
        10000000,
        i32::MAX,
    ];

    let start = Instant::now();

    for &value in &test_values {
        for _ in 0..iterations {
            let usage = TokenUsage {
                input_tokens: value,
                output_tokens: value / 2,
                total_tokens: value * 3 / 2,
                cached_input_tokens: value / 4,
                reasoning_output_tokens: value / 8,
            };

            let cell = new_context_output(&config, &usage, &None);
            let _ = cell.display_lines(80);
        }
    }

    let elapsed = start.elapsed();
    let total_ops = test_values.len() * iterations;
    let avg_time = elapsed / total_ops as u32;

    println!("Average time for number formatting: {:?}", avg_time);

    assert!(
        avg_time < Duration::from_millis(5),
        "Number formatting should be fast"
    );
}
