//! Iteration summarization and commit message generation.
//!
//! Reuses compact_v2 components for message filtering and validation.

use crate::Prompt;
use crate::client::ModelClient;
use crate::client_common::ResponseEvent;
use crate::compact_v2::TokenCounter;
use crate::compact_v2::cleanup_summary_tags;
use crate::compact_v2::filter_for_summarization;
use crate::compact_v2::is_valid_summary;
use crate::error::CodexErr;
use crate::models_manager::model_info::find_model_info_for_slug;
use codex_otel::OtelManager;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::SessionSource;
use futures::StreamExt;
use std::sync::Arc;

use super::driver::SummarizerContext;

/// Create a ModelClient for summarization purposes.
///
/// Creates a minimal ModelClient using the SummarizerContext's auth_manager and config.
/// This is used for lazy client creation in LoopDriver when LLM-based summarization is needed.
pub fn create_summarization_client(ctx: &SummarizerContext) -> ModelClient {
    let model_name = ctx.config.model.as_deref().unwrap_or("gpt-4");
    let model_info = find_model_info_for_slug(model_name);

    // Create minimal OtelManager for summarization
    let otel = OtelManager::new(
        ctx.conversation_id,
        model_name,
        model_info.slug.as_str(),
        None,  // account_id
        None,  // account_email
        None,  // auth_mode
        false, // log_user_prompts
        "spawn_agent_summarizer".to_string(),
        SessionSource::SpawnAgent,
    );

    ModelClient::new(
        ctx.config.clone(),
        Some(ctx.auth_manager.clone()),
        model_info,
        otel,
        ctx.config.model_provider.clone(),
        None, // reasoning_effort
        ReasoningSummary::default(),
        ctx.conversation_id,
        SessionSource::SpawnAgent,
    )
}

/// Iteration summary system prompt.
const ITERATION_SUMMARY_SYSTEM_PROMPT: &str = r#"You are a concise technical summarizer.
Your task is to summarize an AI agent's work in a single iteration.
Be factual and brief. Focus on what was actually done, not what was planned."#;

/// Iteration summary user prompt template.
const ITERATION_SUMMARY_USER_PROMPT: &str = r#"Summarize this agent iteration in 3-5 sentences:

1. What task was attempted
2. What was accomplished (files created/modified, features implemented)
3. Key decisions made or blockers encountered

This summary will be passed to the next iteration for context continuity.
Output ONLY the summary text, no explanations or formatting."#;

/// Commit message system prompt.
const COMMIT_MSG_SYSTEM_PROMPT: &str = r#"You are a git commit message generator.
Generate clear, conventional commit messages following this format:
- First line: [iter-N] Brief description (max 50 chars)
- Blank line
- Body: What was done (2-3 lines max)

Output ONLY the commit message, nothing else."#;

/// Commit message user prompt template.
const COMMIT_MSG_USER_PROMPT_TEMPLATE: &str = r#"Generate a git commit message for this iteration.

Iteration: {iteration}
Task (truncated): {task}
Changed files: {files}
Summary: {summary}

Output ONLY the commit message."#;

/// Generate iteration summary.
///
/// Uses compact_v2 filter_for_summarization and calls LLM for summary.
pub async fn summarize_iteration(
    messages: &[ResponseItem],
    client: Arc<ModelClient>,
) -> Result<String, CodexErr> {
    // 1. Filter messages (reuse compact_v2)
    let filtered = filter_for_summarization(messages);
    if filtered.is_empty() {
        return Ok("No significant actions in this iteration.".to_string());
    }

    // 2. Format messages as text
    let messages_text = format_messages_for_summary(&filtered);

    // 3. Build prompt
    let full_prompt = format!(
        "{}\n\nAgent conversation to summarize:\n{}",
        ITERATION_SUMMARY_USER_PROMPT, messages_text
    );

    // 4. Call LLM
    let response =
        call_llm_for_summary(&client, ITERATION_SUMMARY_SYSTEM_PROMPT, &full_prompt).await?;

    // 5. Validate (reuse compact_v2)
    if !is_valid_summary(&response) {
        return Ok("Summary generation produced invalid output.".to_string());
    }

    // 6. Clean up tags (reuse compact_v2)
    Ok(cleanup_summary_tags(&response))
}

/// Generate commit message using LLM.
pub async fn generate_commit_message(
    iteration: i32,
    task: &str,
    files: &[String],
    summary: &str,
    client: Arc<ModelClient>,
) -> Result<String, CodexErr> {
    // Truncate task
    let task_truncated = if task.len() > 200 {
        format!("{}...", &task[..200])
    } else {
        task.to_string()
    };

    // Format file list
    let files_str = if files.len() <= 10 {
        files.join(", ")
    } else {
        format!(
            "{}, ... ({} more)",
            files[..10].join(", "),
            files.len() - 10
        )
    };

    // Build prompt
    let user_prompt = COMMIT_MSG_USER_PROMPT_TEMPLATE
        .replace("{iteration}", &iteration.to_string())
        .replace("{task}", &task_truncated)
        .replace("{files}", &files_str)
        .replace("{summary}", summary);

    // Call LLM
    let response = call_llm_for_summary(&client, COMMIT_MSG_SYSTEM_PROMPT, &user_prompt).await?;

    let cleaned = response.trim();
    if cleaned.is_empty() {
        // Fallback commit message
        return Ok(format!(
            "[iter-{}] Iteration {} changes\n\n{}",
            iteration, iteration, summary
        ));
    }

    Ok(cleaned.to_string())
}

/// Call LLM for summary/commit message generation.
async fn call_llm_for_summary(
    client: &ModelClient,
    system_prompt: &str,
    user_prompt: &str,
) -> Result<String, CodexErr> {
    // Build input messages
    let input = vec![
        ResponseItem::Message {
            id: None,
            role: "system".to_string(),
            content: vec![ContentItem::InputText {
                text: system_prompt.to_string(),
            }],
            end_turn: None,
        },
        ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: user_prompt.to_string(),
            }],
            end_turn: None,
        },
    ];

    let prompt = Prompt {
        input,
        ..Default::default()
    };

    // Stream and collect response
    let mut stream = client.clone().new_session().stream(&prompt).await?;
    let mut response_text = String::new();

    loop {
        match stream.next().await {
            Some(Ok(ResponseEvent::OutputItemDone(item))) => {
                if let Some(text) = extract_text_from_item(&item) {
                    response_text.push_str(&text);
                }
            }
            Some(Ok(ResponseEvent::Completed { .. })) => break,
            Some(Err(e)) => return Err(e),
            None => break,
            _ => continue,
        }
    }

    Ok(response_text)
}

/// Extract text from ResponseItem.
fn extract_text_from_item(item: &ResponseItem) -> Option<String> {
    match item {
        ResponseItem::Message { content, .. } => {
            let texts: Vec<String> = content
                .iter()
                .filter_map(|c| match c {
                    ContentItem::OutputText { text } => Some(text.clone()),
                    _ => None,
                })
                .collect();
            if texts.is_empty() {
                None
            } else {
                Some(texts.join(""))
            }
        }
        _ => None,
    }
}

/// Format messages for summary input.
fn format_messages_for_summary(messages: &[ResponseItem]) -> String {
    let counter = TokenCounter::default();
    let mut result = Vec::new();
    let mut total_tokens: i64 = 0;
    const MAX_TOKENS: i64 = 4000;

    for item in messages {
        let text = format_single_message(item);
        if text.is_empty() {
            continue;
        }
        let tokens = counter.approximate(&text);

        if total_tokens + tokens > MAX_TOKENS {
            result.push("[... truncated for length ...]".to_string());
            break;
        }

        result.push(text);
        total_tokens += tokens;
    }

    result.join("\n\n")
}

/// Format single message.
fn format_single_message(item: &ResponseItem) -> String {
    match item {
        ResponseItem::Message { role, content, .. } => {
            let content_text: Vec<String> = content
                .iter()
                .filter_map(|c| match c {
                    ContentItem::InputText { text } => Some(text.clone()),
                    ContentItem::OutputText { text } => Some(text.clone()),
                    _ => None,
                })
                .collect();
            if content_text.is_empty() {
                String::new()
            } else {
                format!("[{}]: {}", role, content_text.join("\n"))
            }
        }
        ResponseItem::FunctionCall {
            name, arguments, ..
        } => {
            format!("[tool_use]: {} - {}", name, truncate_str(arguments, 200))
        }
        ResponseItem::FunctionCallOutput { output, .. } => {
            format!("[tool_result]: {}", truncate_str(output, 500))
        }
        _ => String::new(),
    }
}

/// Truncate string.
fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn make_user_msg(text: &str) -> ResponseItem {
        ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: text.to_string(),
            }],
            end_turn: None,
        }
    }

    fn make_assistant_msg(text: &str) -> ResponseItem {
        ResponseItem::Message {
            id: None,
            role: "assistant".to_string(),
            content: vec![ContentItem::OutputText {
                text: text.to_string(),
            }],
            end_turn: None,
        }
    }

    #[test]
    fn test_format_messages_basic() {
        let messages = vec![
            make_user_msg("Implement feature X"),
            make_assistant_msg("I'll help you implement feature X."),
        ];

        let result = format_messages_for_summary(&messages);
        assert!(result.contains("[user]: Implement feature X"));
        assert!(result.contains("[assistant]: I'll help you"));
    }

    #[test]
    fn test_truncate_str() {
        assert_eq!(truncate_str("hello", 10), "hello");
        assert_eq!(truncate_str("hello world", 5), "hello...");
    }

    #[test]
    fn test_format_single_message_empty() {
        let empty_msg = ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![],
            end_turn: None,
        };
        assert!(format_single_message(&empty_msg).is_empty());
    }

    #[test]
    fn test_format_function_call() {
        let call = ResponseItem::FunctionCall {
            id: Some("call-1".to_string()),
            call_id: "call-1".to_string(),
            name: "shell".to_string(),
            arguments: r#"{"command": "ls -la"}"#.to_string(),
        };
        let formatted = format_single_message(&call);
        assert!(formatted.contains("[tool_use]: shell"));
        assert!(formatted.contains("ls -la"));
    }
}
