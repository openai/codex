use std::collections::HashMap;
use std::sync::Arc;

use codex_core::CodexConversation;
use codex_core::protocol::AskUserQuestion;
use codex_core::protocol::AskUserQuestionResponse;
use codex_core::protocol::Op;
use mcp_types::ElicitRequest;
use mcp_types::ElicitRequestParamsRequestedSchema;
use mcp_types::ModelContextProtocolRequest;
use mcp_types::RequestId;
use serde::Deserialize;
use serde::Serialize;
use serde_json::json;
use tracing::error;

use crate::outgoing_message::OutgoingMessageSender;

#[derive(Debug, Serialize)]
pub struct AskUserQuestionElicitRequestParams {
    pub message: String,
    #[serde(rename = "requestedSchema")]
    pub requested_schema: ElicitRequestParamsRequestedSchema,
    pub codex_elicitation: String,
    pub codex_mcp_tool_call_id: String,
    pub codex_event_id: String,
    pub codex_call_id: String,
    pub codex_questions: Vec<AskUserQuestion>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AskUserQuestionElicitResponse {
    pub answers: HashMap<String, String>,
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn handle_ask_user_question_request(
    call_id: String,
    questions: Vec<AskUserQuestion>,
    outgoing: Arc<OutgoingMessageSender>,
    codex: Arc<CodexConversation>,
    _request_id: RequestId,
    tool_call_id: String,
    event_id: String,
) {
    let message = if questions.len() == 1 {
        questions[0].question.clone()
    } else {
        let question_lines = questions
            .iter()
            .map(|q| {
                format!(
                    "- {header}: {question}",
                    header = q.header,
                    question = q.question
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        format!("Codex needs your input:\n{question_lines}")
    };

    let params = AskUserQuestionElicitRequestParams {
        message,
        requested_schema: ElicitRequestParamsRequestedSchema {
            r#type: "object".to_string(),
            properties: json!({
                "answers": {
                    "type": "object",
                    "additionalProperties": { "type": "string" }
                }
            }),
            required: Some(vec!["answers".to_string()]),
        },
        codex_elicitation: "ask-user-question".to_string(),
        codex_mcp_tool_call_id: tool_call_id.clone(),
        codex_event_id: event_id.clone(),
        codex_call_id: call_id,
        codex_questions: questions,
    };

    let params_json = match serde_json::to_value(&params) {
        Ok(value) => value,
        Err(err) => {
            error!("Failed to serialize AskUserQuestionElicitRequestParams: {err}");
            let _ = codex
                .submit(Op::ResolveAskUserQuestion {
                    id: event_id,
                    response: AskUserQuestionResponse::Cancelled,
                })
                .await;
            return;
        }
    };

    let on_response = outgoing
        .send_request(ElicitRequest::METHOD, Some(params_json))
        .await;

    // Listen for the response on a separate task so we don't block the main agent loop.
    tokio::spawn(async move {
        on_ask_user_question_response(event_id, on_response, codex).await;
    });
}

async fn on_ask_user_question_response(
    event_id: String,
    receiver: tokio::sync::oneshot::Receiver<mcp_types::Result>,
    codex: Arc<CodexConversation>,
) {
    let value = match receiver.await {
        Ok(value) => value,
        Err(err) => {
            error!("ask_user_question request failed: {err:?}");
            let _ = codex
                .submit(Op::ResolveAskUserQuestion {
                    id: event_id,
                    response: AskUserQuestionResponse::Cancelled,
                })
                .await;
            return;
        }
    };

    let response = serde_json::from_value::<AskUserQuestionResponse>(value.clone())
        .or_else(|_| {
            serde_json::from_value::<AskUserQuestionElicitResponse>(value)
                .map(|r| AskUserQuestionResponse::Answered { answers: r.answers })
        })
        .unwrap_or_else(|err| {
            error!("failed to deserialize AskUserQuestion response: {err}");
            AskUserQuestionResponse::Cancelled
        });

    if let Err(err) = codex
        .submit(Op::ResolveAskUserQuestion {
            id: event_id,
            response,
        })
        .await
    {
        error!("failed to submit ResolveAskUserQuestion: {err}");
    }
}
