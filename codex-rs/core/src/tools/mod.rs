pub mod context;
pub mod events;
pub(crate) mod handlers;
pub mod orchestrator;
pub mod parallel;
pub mod registry;
pub mod router;
pub mod runtimes;
pub mod sandboxing;
pub mod spec;

use crate::exec::ExecToolCallOutput;
use crate::truncate::TruncationPolicy;
use crate::truncate::formatted_truncate_text;
use crate::truncate::truncate_text;
pub use router::ToolRouter;
use serde::Serialize;

// Telemetry preview limits: keep log events smaller than model budgets.
pub(crate) const TELEMETRY_PREVIEW_MAX_BYTES: usize = 2 * 1024; // 2 KiB
pub(crate) const TELEMETRY_PREVIEW_MAX_LINES: usize = 64; // lines
pub(crate) const TELEMETRY_PREVIEW_TRUNCATION_NOTICE: &str =
    "[... telemetry preview truncated ...]";

/// Format the combined exec output for sending back to the model.
/// Includes exit code and duration metadata; truncates large bodies safely.
pub fn format_exec_output_for_model_structured(
    exec_output: &ExecToolCallOutput,
    truncation_policy: TruncationPolicy,
) -> String {
    let ExecToolCallOutput {
        exit_code,
        duration,
        ..
    } = exec_output;

    #[derive(Serialize)]
    struct ExecMetadata {
        exit_code: i32,
        duration_seconds: f32,
    }

    #[derive(Serialize)]
    struct ExecOutput<'a> {
        output: &'a str,
        metadata: ExecMetadata,
    }

    // round to 1 decimal place
    let duration_seconds = ((duration.as_secs_f32()) * 10.0).round() / 10.0;

    let formatted_output = format_exec_output_str(exec_output, truncation_policy);

    let payload = ExecOutput {
        output: &formatted_output,
        metadata: ExecMetadata {
            exit_code: *exit_code,
            duration_seconds,
        },
    };

    #[expect(clippy::expect_used)]
    serde_json::to_string(&payload).expect("serialize ExecOutput")
}

pub fn format_exec_output_for_model_freeform(
    exec_output: &ExecToolCallOutput,
    truncation_policy: TruncationPolicy,
) -> String {
    // round to 1 decimal place
    let duration_seconds = ((exec_output.duration.as_secs_f32()) * 10.0).round() / 10.0;

    let content = build_content_with_timeout(exec_output);

    let total_lines = content.lines().count();

    let formatted_output = truncate_text(&content, truncation_policy);

    let mut sections = Vec::new();

    sections.push(format!("Exit code: {}", exec_output.exit_code));
    sections.push(format!("Wall time: {duration_seconds} seconds"));
    if total_lines != formatted_output.lines().count() {
        sections.push(format!("Total output lines: {total_lines}"));
    }

    sections.push("Output:".to_string());
    sections.push(formatted_output);

    sections.join("\n")
}

pub fn format_exec_output_str(
    exec_output: &ExecToolCallOutput,
    truncation_policy: TruncationPolicy,
) -> String {
    let content = build_content_with_timeout(exec_output);

    // Apply error-priority formatting: extract and prioritize error information
    // This ensures critical error messages from compilation failures, test failures,
    // etc. are preserved even when output needs to be truncated.
    format_exec_output_with_error_priority(&content, truncation_policy)
}

/// Format exec output with priority given to error information.
/// Extracts error lines and ensures they are preserved during truncation,
/// then truncates remaining content within the available budget.
fn format_exec_output_with_error_priority(
    content: &str,
    policy: TruncationPolicy,
) -> String {
    // Get full output without truncation to analyze error patterns
    let full_output = content;

    // Extract error lines from the output
    let errors = extract_errors(full_output);

    // If errors are found, prioritize them in the output
    if !errors.is_empty() {
        let error_text = errors.join("\n");
        let error_tokens = crate::truncate::approx_token_count(&error_text);
        let budget = policy.token_budget();

        // Calculate remaining budget after reserving space for errors
        let remaining_budget = budget.saturating_sub(error_tokens);

        if remaining_budget > 0 {
            // We have budget for both errors and some other content
            // Remove error lines from original content and truncate the rest
            let content_without_errors = remove_error_lines(full_output, &errors);
            let truncated_rest = if !content_without_errors.trim().is_empty() {
                crate::truncate::truncate_text(
                    &content_without_errors,
                    match policy {
                        TruncationPolicy::Bytes(_) => {
                            TruncationPolicy::Bytes(policy.byte_budget().saturating_sub(
                                crate::truncate::approx_bytes_for_tokens(error_tokens),
                            ))
                        }
                        TruncationPolicy::Tokens(_) => TruncationPolicy::Tokens(remaining_budget),
                    },
                )
            } else {
                String::new()
            };

            if truncated_rest.is_empty() {
                // Only errors fit, return just errors
                crate::truncate::truncate_text(&error_text, policy)
            } else {
                // Combine errors and truncated rest
                format!("{}\n\n--- Other Output ---\n{}", error_text, truncated_rest)
            }
        } else {
            // Error information itself exceeds budget, truncate errors but keep them
            crate::truncate::truncate_text(&error_text, policy)
        }
    } else {
        // No errors found, use standard truncation
        crate::truncate::formatted_truncate_text(content, policy)
    }
}

/// Extract error lines from output content.
/// Identifies lines containing error patterns (error, failed, exception, etc.)
/// and returns them as a vector of strings.
fn extract_errors(content: &str) -> Vec<String> {
    content
        .lines()
        .filter(|line| contains_error_patterns(line))
        .map(|line| line.to_string())
        .collect()
}

/// Remove error lines from content, returning the remaining text.
fn remove_error_lines(content: &str, error_lines: &[String]) -> String {
    let error_set: std::collections::HashSet<&str> =
        error_lines.iter().map(|s| s.as_str()).collect();
    content
        .lines()
        .filter(|line| !error_set.contains(line))
        .collect::<Vec<&str>>()
        .join("\n")
}

/// Check if a line contains error patterns.
/// Uses case-insensitive matching for common error keywords.
fn contains_error_patterns(line: &str) -> bool {
    let lower = line.to_lowercase();
    lower.contains("error")
        || lower.contains("failed")
        || lower.contains("exception")
        || lower.contains("fatal")
        || lower.contains("panic")
        || lower.contains("abort")
        || lower.contains("warning:")
        || lower.contains("error:")
        || lower.contains("failure")
}

/// Extracts exec output content and prepends a timeout message if the command timed out.
fn build_content_with_timeout(exec_output: &ExecToolCallOutput) -> String {
    if exec_output.timed_out {
        format!(
            "command timed out after {} milliseconds\n{}",
            exec_output.duration.as_millis(),
            exec_output.aggregated_output.text
        )
    } else {
        exec_output.aggregated_output.text.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exec::ExecToolCallOutput;
    use crate::exec::StreamOutput;
    use std::time::Duration;

    fn make_exec_output(text: &str) -> ExecToolCallOutput {
        ExecToolCallOutput {
            exit_code: 0,
            stdout: StreamOutput::new(String::new()),
            stderr: StreamOutput::new(String::new()),
            aggregated_output: StreamOutput::new(text.to_string()),
            duration: Duration::from_millis(100),
            timed_out: false,
        }
    }

    #[test]
    fn format_exec_output_str_preserves_error_lines() {
        // Test that error lines are extracted and preserved
        let content = "line 1\nline 2\nerror: compilation failed\nline 3\nline 4";
        let exec_output = make_exec_output(content);
        let result = format_exec_output_str(&exec_output, TruncationPolicy::Tokens(10));

        // Error line should be preserved
        assert!(result.contains("error: compilation failed"), "Error line should be preserved");
    }

    #[test]
    fn format_exec_output_str_prioritizes_errors_over_regular_output() {
        // Test that errors are prioritized when budget is limited
        let content = "normal output line 1\nnormal output line 2\nerror: test failed\nnormal output line 3";
        let exec_output = make_exec_output(content);
        let result = format_exec_output_str(&exec_output, TruncationPolicy::Tokens(5));

        // Error should be present even with limited budget
        assert!(result.contains("error: test failed"), "Error should be preserved even with limited budget");
    }

    #[test]
    fn format_exec_output_str_handles_multiple_errors() {
        // Test that multiple error lines are all preserved
        let content = "start\nerror: first error\nmiddle\nerror: second error\nend";
        let exec_output = make_exec_output(content);
        let result = format_exec_output_str(&exec_output, TruncationPolicy::Tokens(15));

        // Both errors should be preserved
        assert!(result.contains("error: first error"), "First error should be preserved");
        assert!(result.contains("error: second error"), "Second error should be preserved");
    }

    #[test]
    fn format_exec_output_str_detects_case_insensitive_errors() {
        // Test case-insensitive error detection
        let content = "Some output\nERROR: Something went wrong\nMore output";
        let exec_output = make_exec_output(content);
        let result = format_exec_output_str(&exec_output, TruncationPolicy::Tokens(10));

        assert!(result.contains("ERROR: Something went wrong"), "Uppercase ERROR should be detected");
    }

    #[test]
    fn format_exec_output_str_handles_no_errors() {
        // Test that regular output is handled correctly when no errors present
        let content = "normal output line 1\nnormal output line 2\nnormal output line 3";
        let exec_output = make_exec_output(content);
        let result = format_exec_output_str(&exec_output, TruncationPolicy::Tokens(20));

        // Should contain the output (may be truncated if over budget)
        assert!(!result.is_empty());
    }

    #[test]
    fn format_exec_output_str_detects_failed_keyword() {
        // Test detection of "failed" keyword
        let content = "operation failed\nmore output";
        let exec_output = make_exec_output(content);
        let result = format_exec_output_str(&exec_output, TruncationPolicy::Tokens(10));

        assert!(result.contains("failed"), "Failed keyword should be detected");
    }

    #[test]
    fn format_exec_output_str_detects_exception_keyword() {
        // Test detection of "exception" keyword
        let content = "exception occurred\nmore output";
        let exec_output = make_exec_output(content);
        let result = format_exec_output_str(&exec_output, TruncationPolicy::Tokens(10));

        assert!(result.contains("exception"), "Exception keyword should be detected");
    }

    #[test]
    fn format_exec_output_str_separates_errors_from_other_output() {
        // Test that errors are separated from other output when both fit
        let content = "normal line 1\nerror: something wrong\nnormal line 2";
        let exec_output = make_exec_output(content);
        let result = format_exec_output_str(&exec_output, TruncationPolicy::Tokens(20));

        // Should contain separator or error section
        assert!(result.contains("error: something wrong"), "Error should be present");
    }
}
