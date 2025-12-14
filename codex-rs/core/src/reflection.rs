//! Reflection layer for task verification.
//!
//! This module implements a "judge" that verifies if the agent completed a task
//! correctly. After each turn, if reflection is enabled, it:
//! 1. Collects the initial task, recent tool calls, and final result
//! 2. Sends this context to a judge model for evaluation
//! 3. Returns a verdict indicating if the task was completed or needs more work

use crate::client::ModelClient;
use crate::client_common::Prompt;
use crate::client_common::ResponseEvent;
use crate::error::Result;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::{debug, info, warn};

/// Maximum number of recent tool calls to include in reflection context.
const MAX_TOOL_CALLS: usize = 10;

/// JSON Schema for the reflection verdict output.
/// This ensures the judge model returns structured, parseable JSON.
fn verdict_json_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "properties": {
            "completed": {
                "type": "boolean",
                "description": "Whether the task was completed successfully"
            },
            "confidence": {
                "type": "number",
                "description": "Confidence score from 0.0 to 1.0"
            },
            "reasoning": {
                "type": "string",
                "description": "Brief explanation of the verdict"
            },
            "feedback": {
                "type": ["string", "null"],
                "description": "If not completed: specific instructions for what still needs to be done. If completed: null"
            }
        },
        "required": ["completed", "confidence", "reasoning", "feedback"],
        "additionalProperties": false
    })
}

/// Heuristically detect if tool output indicates an error.
///
/// This function uses more sophisticated pattern matching to reduce false positives
/// from phrases like "error handling", "no errors", etc.
fn output_indicates_error(output: &str) -> bool {
    let output_lower = output.to_lowercase();

    // Patterns that strongly indicate an actual error
    let error_indicators = [
        "error:",           // "Error: something failed"
        "failed:",          // "Failed: reason"
        "failure:",         // "Failure: reason"
        "exception:",       // "Exception: ..."
        "panic:",           // Rust panic
        "traceback",        // Python traceback
        "fatal error",      // Fatal error
        "cannot ",          // "cannot find", "cannot open"
        "could not ",       // "could not connect"
        "permission denied",
        "access denied",
        "not found",        // "file not found", "command not found"
        "no such file",     // Unix errors
        "does not exist",
        "is not recognized", // Windows command errors
        "syntax error",
        "compilation error",
        "build failed",
        "test failed",
        "assertion failed",
        "segmentation fault",
        "stack overflow",
        "out of memory",
        "timeout",
        "timed out",
        "connection refused",
        "connection reset",
        "exit code 1",      // Non-zero exit codes
        "exit status 1",
        "exited with",      // "exited with code 1"
        "returned error",
        "threw an error",
    ];

    // Patterns that suggest false positives - output is actually OK
    let false_positive_indicators = [
        "no error",
        "no errors",
        "0 errors",
        "without error",
        "error handling",
        "error handler",
        "error message",
        "error-free",
        "errorfree",
        "on error",    // "on error do something"
        "if error",    // "if error then"
        "handle error",
        "catch error",
        "log error",
        "print error",
    ];

    // If output contains a false positive indicator, be more conservative
    for fp in &false_positive_indicators {
        if output_lower.contains(fp) {
            // Still check for strong error indicators (these override false positives)
            let strong_indicators = [
                "error:",
                "failed:",
                "failure:",
                "exception:",
                "panic:",
                "traceback",
                "fatal error",
                "segmentation fault",
                "stack overflow",
            ];
            for indicator in &strong_indicators {
                if output_lower.contains(indicator) {
                    return true;
                }
            }
            return false;
        }
    }

    // Check for error indicators
    for indicator in &error_indicators {
        if output_lower.contains(indicator) {
            return true;
        }
    }

    false
}

/// Result of a reflection evaluation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReflectionVerdict {
    /// Whether the task was completed successfully.
    pub completed: bool,
    /// Confidence level (0.0 to 1.0).
    pub confidence: f32,
    /// Feedback for the agent if task is incomplete.
    pub feedback: Option<String>,
    /// Reasoning behind the verdict.
    pub reasoning: String,
}

/// Context collected for reflection evaluation.
#[derive(Debug, Clone)]
pub struct ReflectionContext {
    /// The original user task/prompt.
    pub initial_task: String,
    /// Recent tool calls with their results.
    pub tool_calls: Vec<ToolCallSummary>,
    /// The agent's final message/response.
    pub final_response: Option<String>,
    /// Current reflection attempt number.
    pub attempt: u32,
    /// Maximum number of reflection attempts (from config).
    pub max_attempts: u32,
}

/// Summary of a tool call for reflection context.
#[derive(Debug, Clone, Serialize)]
pub struct ToolCallSummary {
    pub tool_name: String,
    pub arguments: String,
    pub result: String,
    pub success: bool,
}

impl ReflectionContext {
    /// Create a new reflection context from conversation items.
    pub fn from_conversation(
        initial_task: String,
        items: &[ResponseItem],
        attempt: u32,
        max_attempts: u32,
    ) -> Self {
        let mut tool_calls: Vec<ToolCallSummary> = Vec::new();
        let mut final_response = None;

        // Iterate forward through items to properly pair FunctionCall with FunctionCallOutput
        for item in items.iter() {
            match item {
                ResponseItem::FunctionCall {
                    name,
                    arguments,
                    call_id: _,
                    id: _,
                } => {
                    if tool_calls.len() < MAX_TOOL_CALLS {
                        tool_calls.push(ToolCallSummary {
                            tool_name: name.clone(),
                            arguments: truncate_string(arguments, 200),
                            result: String::new(),
                            success: true, // Default, will be updated when we see output
                        });
                    }
                }
                ResponseItem::FunctionCallOutput { call_id: _, output } => {
                    // Update the last tool call with its result
                    if let Some(last_call) = tool_calls.last_mut() {
                        last_call.result = truncate_string(output, 500);
                        last_call.success = !output_indicates_error(output);
                    }
                }
                ResponseItem::Message { role, content, .. } => {
                    if role == "assistant" {
                        // Extract text from content - keep updating to get the last response
                        for c in content {
                            if let ContentItem::OutputText { text } = c {
                                final_response = Some(text.clone());
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        Self {
            initial_task,
            tool_calls,
            final_response,
            attempt,
            max_attempts,
        }
    }

    /// Build the judge prompt from this context.
    pub fn build_judge_prompt(&self) -> String {
        let tool_calls_str = if self.tool_calls.is_empty() {
            "No tool calls were made.".to_string()
        } else {
            self.tool_calls
                .iter()
                .enumerate()
                .map(|(i, tc)| {
                    format!(
                        "{}. {} ({})\n   Args: {}\n   Result: {}",
                        i + 1,
                        tc.tool_name,
                        if tc.success { "success" } else { "failed" },
                        tc.arguments,
                        if tc.result.is_empty() {
                            "(no output)"
                        } else {
                            &tc.result
                        }
                    )
                })
                .collect::<Vec<_>>()
                .join("\n\n")
        };

        let final_response_str = self
            .final_response
            .as_ref()
            .map(|r| truncate_string(r, 1000))
            .unwrap_or_else(|| "(No final response)".to_string());

        format!(
            r#"You are a strict task verification judge. Your job is to evaluate whether an AI coding assistant has FULLY completed a user's task.

## ORIGINAL TASK
{task}

## TOOL CALLS MADE (last {max_tools})
{tools}

## AGENT'S FINAL RESPONSE
{response}

## REFLECTION ATTEMPT
This is attempt {attempt} of {max_attempts}.

## YOUR TASK
Evaluate whether the task was completed correctly and fully. Be strict but fair.

Consider:
1. Did the agent address ALL parts of the user's request?
2. Were the tool calls appropriate and successful?
3. Is the final response accurate and complete?
4. Are there any obvious errors or missing steps?

Respond with ONLY a JSON object in this exact format:
{{
  "completed": true/false,
  "confidence": 0.0-1.0,
  "reasoning": "Brief explanation of your verdict",
  "feedback": "If not completed: specific instructions for what still needs to be done. If completed: null"
}}

Be concise. Do not include any text outside the JSON object."#,
            task = self.initial_task,
            max_tools = MAX_TOOL_CALLS,
            tools = tool_calls_str,
            response = final_response_str,
            attempt = self.attempt,
            max_attempts = self.max_attempts,
        )
    }
}

/// Parse the judge's response into a verdict.
pub fn parse_verdict(response: &str) -> Result<ReflectionVerdict> {
    // Try to extract JSON from the response
    let json_str = extract_json(response);

    match serde_json::from_str::<ReflectionVerdict>(&json_str) {
        Ok(verdict) => Ok(verdict),
        Err(e) => {
            warn!("Failed to parse judge verdict: {}", e);
            // Return a default "completed" verdict if parsing fails.
            // This prevents blocking on judge errors - we trust the agent
            // and only block when we have clear evidence of incompletion.
            Ok(ReflectionVerdict {
                completed: true,
                confidence: 0.5,
                reasoning: format!("Failed to parse judge response: {}", e),
                feedback: None,
            })
        }
    }
}

/// Extract JSON object from a string that might contain other text.
fn extract_json(s: &str) -> String {
    // Find the first { and last }
    if let (Some(start), Some(end)) = (s.find('{'), s.rfind('}')) {
        if end > start {
            return s[start..=end].to_string();
        }
    }
    s.to_string()
}

/// Truncate a string to a maximum length, adding ellipsis if needed.
fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}

/// Run reflection evaluation using a judge model.
///
/// This function creates a separate API call to evaluate the agent's work.
/// It uses the same model client infrastructure but with a judge-specific prompt.
///
/// # Arguments
/// * `client` - The model client to use for API calls
/// * `context` - The reflection context containing task and conversation info
/// * `judge_model` - Optional model to use for judging (overrides client's default)
pub async fn evaluate_reflection(
    client: &ModelClient,
    context: ReflectionContext,
    judge_model: Option<&str>,
) -> Result<ReflectionVerdict> {
    info!(
        "Running reflection evaluation (attempt {}), judge_model={:?}",
        context.attempt, judge_model
    );

    let judge_prompt = context.build_judge_prompt();
    debug!("Judge prompt length: {} chars", judge_prompt.len());

    // Create a client with the judge model override if specified
    let effective_client = match judge_model {
        Some(model) => client.with_model_override(model),
        None => client.clone(),
    };

    let prompt = Prompt {
        input: vec![ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText { text: judge_prompt }],
        }],
        tools: vec![], // Judge doesn't need tools
        parallel_tool_calls: false,
        base_instructions_override: Some(
            "You are a task verification judge. Respond only with JSON.".to_string(),
        ),
        output_schema: Some(verdict_json_schema()),
    };

    // Stream the response and collect it
    let mut stream = effective_client.stream(&prompt).await?;
    let mut response_text = String::new();

    while let Some(event) = stream.next().await {
        match event {
            Ok(ResponseEvent::OutputTextDelta(delta)) => {
                response_text.push_str(&delta);
            }
            Ok(ResponseEvent::Completed { .. }) => {
                break;
            }
            Err(e) => {
                warn!("Error during reflection stream: {}", e);
                break;
            }
            _ => {}
        }
    }

    debug!("Judge response: {}", response_text);
    parse_verdict(&response_text)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== Verdict Parsing Tests ====================

    #[test]
    fn test_parse_verdict_valid_pass() {
        let response = r#"{"completed": true, "confidence": 0.95, "reasoning": "Task was done", "feedback": null}"#;
        let verdict = parse_verdict(response).unwrap();
        assert!(verdict.completed);
        assert!(verdict.confidence > 0.9);
        assert!(verdict.feedback.is_none());
        assert_eq!(verdict.reasoning, "Task was done");
    }

    #[test]
    fn test_parse_verdict_valid_fail() {
        let response = r#"{"completed": false, "confidence": 0.7, "reasoning": "Task incomplete", "feedback": "Please add error handling"}"#;
        let verdict = parse_verdict(response).unwrap();
        assert!(!verdict.completed);
        assert!(verdict.confidence < 0.8);
        assert_eq!(
            verdict.feedback,
            Some("Please add error handling".to_string())
        );
    }

    #[test]
    fn test_parse_verdict_with_surrounding_text() {
        let response = r#"Here is my evaluation:
{"completed": false, "confidence": 0.8, "reasoning": "Missing tests", "feedback": "Add unit tests"}
That's my verdict."#;
        let verdict = parse_verdict(response).unwrap();
        assert!(!verdict.completed);
        assert_eq!(verdict.feedback, Some("Add unit tests".to_string()));
    }

    #[test]
    fn test_parse_verdict_malformed_json_returns_default() {
        let response = "This is not valid JSON at all!";
        let verdict = parse_verdict(response).unwrap();
        // Should return default "completed" verdict when parsing fails
        // This prevents blocking on judge parse errors
        assert!(verdict.completed);
        assert_eq!(verdict.confidence, 0.5);
        assert!(verdict.feedback.is_none());
    }

    #[test]
    fn test_parse_verdict_partial_json() {
        let response = r#"{"completed": true"#; // Incomplete JSON
        let verdict = parse_verdict(response).unwrap();
        // Should return default "completed" on parse failure
        assert!(verdict.completed);
    }

    // ==================== String Utilities Tests ====================

    #[test]
    fn test_truncate_string_no_truncation() {
        assert_eq!(truncate_string("hello", 10), "hello");
        assert_eq!(truncate_string("", 10), "");
    }

    #[test]
    fn test_truncate_string_with_truncation() {
        assert_eq!(truncate_string("hello world", 8), "hello...");
        assert_eq!(truncate_string("abcdefghij", 5), "ab...");
    }

    #[test]
    fn test_truncate_string_edge_cases() {
        assert_eq!(truncate_string("abc", 3), "abc"); // Exact length
        assert_eq!(truncate_string("abcd", 3), "..."); // Just under, adds ellipsis
    }

    // ==================== Error Detection Tests ====================

    #[test]
    fn test_output_indicates_error_real_errors() {
        // These should all be detected as errors
        assert!(output_indicates_error("Error: file not found"));
        assert!(output_indicates_error("Failed: connection refused"));
        assert!(output_indicates_error("permission denied"));
        assert!(output_indicates_error("Command 'foo' not found"));
        assert!(output_indicates_error("Build failed with 3 errors"));
        assert!(output_indicates_error("Test failed: expected 5, got 3"));
        assert!(output_indicates_error("Traceback (most recent call last):"));
        assert!(output_indicates_error("panic: runtime error"));
        assert!(output_indicates_error("Cannot open file"));
        assert!(output_indicates_error("exit code 1"));
    }

    #[test]
    fn test_output_indicates_error_false_positives() {
        // These should NOT be detected as errors (false positives in naive approach)
        assert!(!output_indicates_error("Implemented error handling for edge cases"));
        assert!(!output_indicates_error("Added error message display"));
        assert!(!output_indicates_error("No errors found in the codebase"));
        assert!(!output_indicates_error("The function handles errors gracefully"));
        assert!(!output_indicates_error("0 errors, 0 warnings"));
        assert!(!output_indicates_error("Build completed without error"));
    }

    #[test]
    fn test_output_indicates_error_success_messages() {
        // These are clearly successful outputs
        assert!(!output_indicates_error("File created successfully"));
        assert!(!output_indicates_error("All tests passed"));
        assert!(!output_indicates_error("Build completed in 2.3s"));
        assert!(!output_indicates_error("hello world"));
        assert!(!output_indicates_error(""));
    }

    #[test]
    fn test_output_indicates_error_mixed_with_strong_indicator() {
        // Even if false positive phrases exist, strong indicators should trigger
        assert!(output_indicates_error("Error: no error handler found"));
        assert!(output_indicates_error("0 errors in file, but Failed: compilation error"));
    }

    #[test]
    fn test_extract_json_simple() {
        let s = "Some text {\"key\": \"value\"} more text";
        assert_eq!(extract_json(s), r#"{"key": "value"}"#);
    }

    #[test]
    fn test_extract_json_nested() {
        let s = r#"Result: {"outer": {"inner": "value"}, "arr": [1,2,3]}"#;
        assert_eq!(
            extract_json(s),
            r#"{"outer": {"inner": "value"}, "arr": [1,2,3]}"#
        );
    }

    #[test]
    fn test_extract_json_no_json() {
        let s = "No JSON here";
        assert_eq!(extract_json(s), "No JSON here");
    }

    #[test]
    fn test_extract_json_only_json() {
        let s = r#"{"completed": true}"#;
        assert_eq!(extract_json(s), r#"{"completed": true}"#);
    }

    // ==================== ReflectionContext Tests ====================

    #[test]
    fn test_reflection_context_creation() {
        let context = ReflectionContext {
            initial_task: "Create a hello world program".to_string(),
            tool_calls: vec![],
            final_response: Some("Done!".to_string()),
            attempt: 1,
            max_attempts: 3,
        };

        assert_eq!(context.initial_task, "Create a hello world program");
        assert_eq!(context.attempt, 1);
        assert!(context.tool_calls.is_empty());
    }

    #[test]
    fn test_reflection_context_with_tool_calls() {
        let context = ReflectionContext {
            initial_task: "Run a shell command".to_string(),
            tool_calls: vec![ToolCallSummary {
                tool_name: "shell".to_string(),
                arguments: r#"{"command": "echo hello"}"#.to_string(),
                result: "hello\n".to_string(),
                success: true,
            }],
            final_response: Some("Executed successfully".to_string()),
            attempt: 1,
            max_attempts: 3,
        };

        assert_eq!(context.tool_calls.len(), 1);
        assert_eq!(context.tool_calls[0].tool_name, "shell");
        assert!(context.tool_calls[0].success);
    }

    #[test]
    fn test_build_judge_prompt_contains_task() {
        let context = ReflectionContext {
            initial_task: "Create a Python file".to_string(),
            tool_calls: vec![],
            final_response: Some("I created the file".to_string()),
            attempt: 1,
            max_attempts: 3,
        };

        let prompt = context.build_judge_prompt();
        assert!(prompt.contains("Create a Python file"));
        assert!(prompt.contains("I created the file"));
        assert!(prompt.contains("attempt 1"));
    }

    #[test]
    fn test_build_judge_prompt_with_tool_calls() {
        let context = ReflectionContext {
            initial_task: "Run tests".to_string(),
            tool_calls: vec![ToolCallSummary {
                tool_name: "shell".to_string(),
                arguments: "npm test".to_string(),
                result: "All tests passed".to_string(),
                success: true,
            }],
            final_response: None,
            attempt: 2,
            max_attempts: 3,
        };

        let prompt = context.build_judge_prompt();
        assert!(prompt.contains("shell"));
        assert!(prompt.contains("npm test"));
        assert!(prompt.contains("All tests passed"));
        assert!(prompt.contains("success"));
    }

    #[test]
    fn test_build_judge_prompt_no_tool_calls() {
        let context = ReflectionContext {
            initial_task: "Explain something".to_string(),
            tool_calls: vec![],
            final_response: Some("Here is the explanation".to_string()),
            attempt: 1,
            max_attempts: 3,
        };

        let prompt = context.build_judge_prompt();
        assert!(prompt.contains("No tool calls were made"));
    }

    // ==================== ToolCallSummary Tests ====================

    #[test]
    fn test_tool_call_summary_success() {
        let summary = ToolCallSummary {
            tool_name: "write_file".to_string(),
            arguments: r#"{"path": "/tmp/test.txt", "content": "hello"}"#.to_string(),
            result: "File written successfully".to_string(),
            success: true,
        };

        assert!(summary.success);
        assert_eq!(summary.tool_name, "write_file");
    }

    #[test]
    fn test_tool_call_summary_failure() {
        let summary = ToolCallSummary {
            tool_name: "shell".to_string(),
            arguments: "rm -rf /".to_string(),
            result: "Permission denied".to_string(),
            success: false,
        };

        assert!(!summary.success);
        assert!(summary.result.contains("Permission denied"));
    }

    // ==================== ReflectionVerdict Tests ====================

    #[test]
    fn test_reflection_verdict_serialize() {
        let verdict = ReflectionVerdict {
            completed: true,
            confidence: 0.95,
            feedback: None,
            reasoning: "All good".to_string(),
        };

        let json = serde_json::to_string(&verdict).unwrap();
        assert!(json.contains("\"completed\":true"));
        assert!(json.contains("\"confidence\":0.95"));
    }

    #[test]
    fn test_reflection_verdict_deserialize() {
        let json = r#"{"completed": false, "confidence": 0.6, "reasoning": "Needs work", "feedback": "Add tests"}"#;
        let verdict: ReflectionVerdict = serde_json::from_str(json).unwrap();

        assert!(!verdict.completed);
        assert_eq!(verdict.confidence, 0.6);
        assert_eq!(verdict.feedback, Some("Add tests".to_string()));
    }

    // ==================== Integration Tests ====================
    // These tests verify the reflection layer works with real ResponseItem data

    #[test]
    fn test_from_conversation_with_function_calls() {
        use codex_protocol::models::{ContentItem, FunctionCallOutputPayload, ResponseItem};

        // Simulate a conversation with tool calls
        let items = vec![
            // User message
            ResponseItem::Message {
                id: Some("msg1".to_string()),
                role: "user".to_string(),
                content: vec![ContentItem::InputText {
                    text: "Create a file called test.txt".to_string(),
                }],
            },
            // Assistant calls a tool
            ResponseItem::FunctionCall {
                id: Some("fc1".to_string()),
                name: "write_file".to_string(),
                arguments: r#"{"path": "test.txt", "content": "hello"}"#.to_string(),
                call_id: "call_123".to_string(),
            },
            // Tool output
            ResponseItem::FunctionCallOutput {
                call_id: "call_123".to_string(),
                output: FunctionCallOutputPayload {
                    content: "File created successfully".to_string(),
                    ..Default::default()
                },
            },
            // Assistant final response
            ResponseItem::Message {
                id: Some("msg2".to_string()),
                role: "assistant".to_string(),
                content: vec![ContentItem::OutputText {
                    text: "I've created the file test.txt with the content 'hello'.".to_string(),
                }],
            },
        ];

        let context = ReflectionContext::from_conversation(
            "Create a file called test.txt".to_string(),
            &items,
            1,
            3,
        );

        // Verify context was built correctly
        assert_eq!(context.initial_task, "Create a file called test.txt");
        assert_eq!(context.attempt, 1);
        assert!(context.final_response.is_some());
        assert!(
            context
                .final_response
                .as_ref()
                .unwrap()
                .contains("created the file")
        );

        // Verify tool calls were captured
        assert_eq!(context.tool_calls.len(), 1);
        assert_eq!(context.tool_calls[0].tool_name, "write_file");
        assert!(context.tool_calls[0].result.contains("successfully"));
        assert!(context.tool_calls[0].success); // No "error" in output
    }

    #[test]
    fn test_from_conversation_with_failed_tool_call() {
        use codex_protocol::models::{ContentItem, FunctionCallOutputPayload, ResponseItem};

        let items = vec![
            ResponseItem::FunctionCall {
                id: Some("fc1".to_string()),
                name: "shell".to_string(),
                arguments: r#"{"command": "rm important_file"}"#.to_string(),
                call_id: "call_456".to_string(),
            },
            ResponseItem::FunctionCallOutput {
                call_id: "call_456".to_string(),
                output: FunctionCallOutputPayload {
                    content: "Error: Permission denied".to_string(),
                    ..Default::default()
                },
            },
            ResponseItem::Message {
                id: Some("msg1".to_string()),
                role: "assistant".to_string(),
                content: vec![ContentItem::OutputText {
                    text: "The command failed with an error.".to_string(),
                }],
            },
        ];

        let context =
            ReflectionContext::from_conversation("Delete the file".to_string(), &items, 2, 3);

        // Tool call should be marked as failed due to "Error" in output
        assert_eq!(context.tool_calls.len(), 1);
        assert!(!context.tool_calls[0].success);
        assert!(context.tool_calls[0].result.contains("Permission denied"));
    }

    #[test]
    fn test_from_conversation_empty_items() {
        let items: Vec<codex_protocol::models::ResponseItem> = vec![];
        let context =
            ReflectionContext::from_conversation("Do something".to_string(), &items, 1, 3);

        assert_eq!(context.initial_task, "Do something");
        assert!(context.tool_calls.is_empty());
        assert!(context.final_response.is_none());
    }

    #[test]
    fn test_reflection_feedback_message_format() {
        // Test that the feedback message format is correct
        let verdict = ReflectionVerdict {
            completed: false,
            confidence: 0.7,
            reasoning: "The file was created but tests are missing".to_string(),
            feedback: Some("Please add unit tests for the new function".to_string()),
        };

        // Simulate the feedback message construction from codex.rs
        let feedback_msg = format!(
            "[Reflection Judge - Attempt {}/{}] Task verification failed.\n\nReasoning: {}\n\nFeedback: {}\n\nPlease address the above feedback and complete the task.",
            1,
            3,
            verdict.reasoning,
            verdict.feedback.as_ref().unwrap()
        );

        assert!(feedback_msg.contains("Attempt 1/3"));
        assert!(feedback_msg.contains("tests are missing"));
        assert!(feedback_msg.contains("add unit tests"));
    }

    #[test]
    fn test_judge_prompt_includes_all_context() {
        use codex_protocol::models::{ContentItem, FunctionCallOutputPayload, ResponseItem};

        // Build a realistic context
        let items = vec![
            ResponseItem::FunctionCall {
                id: None,
                name: "read_file".to_string(),
                arguments: r#"{"path": "src/main.rs"}"#.to_string(),
                call_id: "c1".to_string(),
            },
            ResponseItem::FunctionCallOutput {
                call_id: "c1".to_string(),
                output: FunctionCallOutputPayload {
                    content: "fn main() { println!(\"Hello\"); }".to_string(),
                    ..Default::default()
                },
            },
            ResponseItem::FunctionCall {
                id: None,
                name: "write_file".to_string(),
                arguments: r#"{"path": "src/main.rs", "content": "fn main() { println!(\"Hello, World!\"); }"}"#.to_string(),
                call_id: "c2".to_string(),
            },
            ResponseItem::FunctionCallOutput {
                call_id: "c2".to_string(),
                output: FunctionCallOutputPayload {
                    content: "File written".to_string(),
                    ..Default::default()
                },
            },
            ResponseItem::Message {
                id: None,
                role: "assistant".to_string(),
                content: vec![ContentItem::OutputText {
                    text: "I've updated the greeting message.".to_string(),
                }],
            },
        ];

        let context = ReflectionContext::from_conversation(
            "Update the greeting to say Hello, World!".to_string(),
            &items,
            1,
            3,
        );

        let prompt = context.build_judge_prompt();

        // Verify the judge prompt contains all necessary information
        assert!(prompt.contains("Update the greeting"));
        assert!(prompt.contains("read_file"));
        assert!(prompt.contains("write_file"));
        assert!(prompt.contains("updated the greeting"));
        assert!(prompt.contains("attempt 1"));
        assert!(prompt.contains(r#""completed": true/false"#)); // JSON schema hint
    }
}
