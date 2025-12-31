//! AskUserQuestion Tool Handler
//!
//! Asks the user questions during execution for clarification.
//! Aligned with Claude Code's AskUserQuestion tool (chunks.153.mjs).
//!
//! ## Answer Injection Mechanism
//!
//! Unlike a polling-based approach, this handler uses a oneshot channel
//! to block until the user responds. This matches Claude Code's behavior
//! where the tool waits for user input via an `onAllow` callback.
//!
//! Flow:
//! 1. Handler creates a oneshot channel via `stores.create_answer_channel()`
//! 2. Handler sends `UserQuestionRequest` event to TUI
//! 3. Handler awaits the channel receiver (blocks until user responds)
//! 4. When user responds, `codex_ext.rs` calls `stores.send_user_answer()`
//! 5. Handler receives the answer and returns it as the tool result
//! 6. LLM receives the actual user answer (not "Waiting for response...")

use crate::function_tool::FunctionCallError;
use crate::subagent::get_or_create_stores;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use async_trait::async_trait;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol_ext::ExtEventMsg;
use codex_protocol::protocol_ext::QuestionOption;
use codex_protocol::protocol_ext::UserQuestion;
use codex_protocol::protocol_ext::UserQuestionRequestEvent;
use serde::Deserialize;

/// Input structure for AskUserQuestion tool.
#[derive(Debug, Clone, Deserialize)]
struct AskUserQuestionInput {
    questions: Vec<QuestionInput>,
}

/// Input structure for a single question.
#[derive(Debug, Clone, Deserialize)]
struct QuestionInput {
    question: String,
    header: String,
    options: Vec<OptionInput>,
    #[serde(rename = "multiSelect")]
    multi_select: bool,
}

/// Input structure for a question option.
#[derive(Debug, Clone, Deserialize)]
struct OptionInput {
    label: String,
    description: String,
}

/// AskUserQuestion Tool Handler
///
/// This tool:
/// 1. Parses and validates the questions input
/// 2. Creates a oneshot channel for receiving the answer
/// 3. Sends UserQuestionRequest event to TUI
/// 4. Awaits the channel (blocks until user responds)
/// 5. Returns the user's answer as the tool result
pub struct AskUserQuestionHandler;

#[async_trait]
impl ToolHandler for AskUserQuestionHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    fn matches_kind(&self, payload: &ToolPayload) -> bool {
        matches!(payload, ToolPayload::Function { .. })
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        // 1. Parse input
        let ToolPayload::Function { arguments } = &invocation.payload else {
            return Err(FunctionCallError::RespondToModel(
                "Invalid payload type for AskUserQuestion".to_string(),
            ));
        };

        let input: AskUserQuestionInput = serde_json::from_str(arguments).map_err(|e| {
            FunctionCallError::RespondToModel(format!("Failed to parse AskUserQuestion input: {e}"))
        })?;

        // 2. Validate questions
        if input.questions.is_empty() {
            return Err(FunctionCallError::RespondToModel(
                "At least one question is required".to_string(),
            ));
        }
        if input.questions.len() > 4 {
            return Err(FunctionCallError::RespondToModel(
                "Maximum 4 questions allowed".to_string(),
            ));
        }

        for (i, q) in input.questions.iter().enumerate() {
            if q.options.len() < 2 {
                return Err(FunctionCallError::RespondToModel(format!(
                    "Question {} must have at least 2 options",
                    i + 1
                )));
            }
            if q.options.len() > 4 {
                return Err(FunctionCallError::RespondToModel(format!(
                    "Question {} can have at most 4 options",
                    i + 1
                )));
            }
            if q.header.len() > 12 {
                return Err(FunctionCallError::RespondToModel(format!(
                    "Question {} header must be at most 12 characters",
                    i + 1
                )));
            }
        }

        // 3. Convert to protocol types
        let questions: Vec<UserQuestion> = input
            .questions
            .into_iter()
            .map(|q| UserQuestion {
                question: q.question,
                header: q.header,
                options: q
                    .options
                    .into_iter()
                    .map(|o| QuestionOption {
                        label: o.label,
                        description: o.description,
                    })
                    .collect(),
                multi_select: q.multi_select,
            })
            .collect();

        // 4. Create a oneshot channel for receiving the user's answer
        let stores = get_or_create_stores(invocation.session.conversation_id);
        let rx = stores.create_answer_channel(&invocation.call_id);

        // 5. Send event to TUI
        invocation
            .session
            .send_event(
                invocation.turn.as_ref(),
                EventMsg::Ext(ExtEventMsg::UserQuestionRequest(UserQuestionRequestEvent {
                    tool_call_id: invocation.call_id.clone(),
                    questions,
                })),
            )
            .await;

        // 6. Block until user responds (no timeout - matches Claude Code behavior)
        // When the user responds, codex_ext.rs calls stores.send_user_answer()
        // which sends the answer through the channel and unblocks this await.
        match rx.await {
            Ok(answer) => Ok(ToolOutput::Function {
                content: format!("User responded:\n{answer}"),
                content_items: None,
                success: Some(true),
            }),
            Err(_) => {
                // Channel was closed without receiving an answer
                // This can happen if the user cancels or the session ends
                Ok(ToolOutput::Function {
                    content: "User cancelled the question or session ended.".to_string(),
                    content_items: None,
                    success: Some(false),
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handler_kind() {
        let handler = AskUserQuestionHandler;
        assert_eq!(handler.kind(), ToolKind::Function);
    }

    #[test]
    fn test_matches_function_payload() {
        let handler = AskUserQuestionHandler;

        assert!(handler.matches_kind(&ToolPayload::Function {
            arguments: "{}".to_string(),
        }));
    }

    #[test]
    fn test_parse_valid_input() {
        let input_json = r#"{
            "questions": [{
                "question": "Which database should we use?",
                "header": "Database",
                "options": [
                    {"label": "PostgreSQL", "description": "Reliable relational database"},
                    {"label": "MongoDB", "description": "Flexible document store"}
                ],
                "multiSelect": false
            }]
        }"#;

        let input: AskUserQuestionInput = serde_json::from_str(input_json).unwrap();
        assert_eq!(input.questions.len(), 1);
        assert_eq!(input.questions[0].header, "Database");
        assert_eq!(input.questions[0].options.len(), 2);
    }
}
