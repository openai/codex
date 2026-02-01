//! Output formatting for CLI responses.
//!
//! These functions provide formatted output for streaming responses,
//! tool calls, errors, and session information.

use std::io::Write;
use std::io::{self};

/// Print a streaming text response.
#[allow(dead_code)]
pub fn print_streaming_text(text: &str) {
    print!("{text}");
    io::stdout().flush().ok();
}

/// Print a tool call notification.
#[allow(dead_code)]
pub fn print_tool_call(name: &str, _input: &serde_json::Value) {
    println!("\n[Tool: {name}]");
}

/// Print a tool result.
#[allow(dead_code)]
pub fn print_tool_result(name: &str, output: &str, is_error: bool) {
    let prefix = if is_error { "Error" } else { "Result" };
    println!("[{name} {prefix}]: {output}");
}

/// Print an error message.
pub fn print_error(error: &str) {
    eprintln!("Error: {error}");
}

/// Print a separator line.
pub fn print_separator() {
    println!("─────────────────────────────────────────");
}

/// Print session start information.
pub fn print_session_start(session_id: &str, model: &str, provider: &str) {
    println!("Session: {session_id}");
    println!("Model:   {provider}/{model}");
    print_separator();
    println!("Type your message. Press Ctrl+D to exit.");
    println!();
}

/// Print turn completion summary.
pub fn print_turn_summary(input_tokens: i64, output_tokens: i64) {
    println!();
    println!("[Tokens: {input_tokens} in / {output_tokens} out]");
}

/// Print thinking/reasoning text.
#[allow(dead_code)]
pub fn print_thinking(text: &str) {
    println!("<thinking>{text}</thinking>");
}

/// Print a newline and flush.
#[allow(dead_code)]
pub fn newline() {
    println!();
    io::stdout().flush().ok();
}
