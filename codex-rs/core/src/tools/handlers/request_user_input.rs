use async_trait::async_trait;
use codex_protocol::approvals::ElicitationRequestEvent;
use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::request_user_input::RequestUserInputAnswer;
use codex_protocol::request_user_input::RequestUserInputQuestion;
use codex_protocol::request_user_input::RequestUserInputQuestionOption;
use codex_protocol::request_user_input::RequestUserInputResponse;
use codex_rmcp_client::ElicitationAction;
use codex_rmcp_client::ElicitationResponse;

use crate::function_tool::FunctionCallError;
use crate::mcp::elicitation_form::build_elicitation_content_from_response;
use crate::mcp::elicitation_form::build_elicitation_content_questions;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::parse_arguments;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use codex_protocol::config_types::ModeKind;
use codex_protocol::config_types::TUI_VISIBLE_COLLABORATION_MODES;
use codex_protocol::request_user_input::RequestUserInputArgs;

const MCP_ELICITATION_DECISION_QUESTION_ID: &str = "mcp_elicitation_decision";
const MCP_ELICITATION_ACCEPT: &str = "Accept";
const MCP_ELICITATION_DECLINE: &str = "Decline";
const MCP_ELICITATION_CANCEL: &str = "Cancel";
const REQUEST_USER_INPUT_NOTE_PREFIX: &str = "user_note: ";

fn format_allowed_modes() -> String {
    let mode_names: Vec<&str> = TUI_VISIBLE_COLLABORATION_MODES
        .into_iter()
        .filter(|mode| mode.allows_request_user_input())
        .map(ModeKind::display_name)
        .collect();

    match mode_names.as_slice() {
        [] => "no modes".to_string(),
        [mode] => format!("{mode} mode"),
        [first, second] => format!("{first} or {second} mode"),
        [..] => format!("modes: {}", mode_names.join(",")),
    }
}

pub(crate) fn request_user_input_unavailable_message(mode: ModeKind) -> Option<String> {
    if mode.allows_request_user_input() {
        None
    } else {
        let mode_name = mode.display_name();
        Some(format!(
            "request_user_input is unavailable in {mode_name} mode"
        ))
    }
}

pub(crate) fn request_user_input_tool_description() -> String {
    let allowed_modes = format_allowed_modes();
    format!(
        "Request user input for one to three short questions and wait for the response. This tool is only available in {allowed_modes}."
    )
}

pub(crate) fn build_mcp_elicitation_request_user_input_args(
    elicitation: &ElicitationRequestEvent,
) -> RequestUserInputArgs {
    let mut question = elicitation.message.clone();
    if let Some(url) = &elicitation.url {
        question = format!("{question}\nURL: {url}");
    }
    let mut questions = vec![RequestUserInputQuestion {
        id: MCP_ELICITATION_DECISION_QUESTION_ID.to_string(),
        header: "MCP elicitation".to_string(),
        question,
        is_other: true,
        is_secret: false,
        options: Some(vec![
            RequestUserInputQuestionOption {
                label: MCP_ELICITATION_ACCEPT.to_string(),
                description: "Accept this elicitation request.".to_string(),
            },
            RequestUserInputQuestionOption {
                label: MCP_ELICITATION_DECLINE.to_string(),
                description: "Decline this elicitation request.".to_string(),
            },
            RequestUserInputQuestionOption {
                label: MCP_ELICITATION_CANCEL.to_string(),
                description: "Cancel this elicitation request.".to_string(),
            },
        ]),
    }];

    questions.extend(build_elicitation_content_questions(
        elicitation.requested_schema.as_ref(),
    ));

    RequestUserInputArgs { questions }
}

pub(crate) fn build_mcp_elicitation_response_from_user_input(
    response: Option<RequestUserInputResponse>,
    elicitation: &ElicitationRequestEvent,
) -> ElicitationResponse {
    let Some(response) = response else {
        return ElicitationResponse {
            action: ElicitationAction::Cancel,
            content: None,
        };
    };

    let action = response
        .answers
        .get(MCP_ELICITATION_DECISION_QUESTION_ID)
        .and_then(request_user_input_answer_to_elicitation_action)
        .unwrap_or(ElicitationAction::Cancel);

    match action {
        ElicitationAction::Accept => {
            let content = if elicitation.requested_schema.is_some() {
                match build_elicitation_content_from_response(
                    &response,
                    elicitation.requested_schema.as_ref(),
                ) {
                    Ok(Some(value)) => Some(value),
                    Ok(None) => Some(serde_json::json!({})),
                    Err(()) => {
                        return ElicitationResponse {
                            action: ElicitationAction::Cancel,
                            content: None,
                        };
                    }
                }
            } else {
                Some(serde_json::json!({}))
            };
            ElicitationResponse { action, content }
        }
        ElicitationAction::Decline | ElicitationAction::Cancel => ElicitationResponse {
            action,
            content: None,
        },
    }
}

fn request_user_input_answer_to_elicitation_action(
    answer: &RequestUserInputAnswer,
) -> Option<ElicitationAction> {
    answer.answers.iter().find_map(|entry| {
        if entry.starts_with(REQUEST_USER_INPUT_NOTE_PREFIX) {
            return None;
        }
        match entry.as_str() {
            MCP_ELICITATION_ACCEPT => Some(ElicitationAction::Accept),
            MCP_ELICITATION_DECLINE => Some(ElicitationAction::Decline),
            MCP_ELICITATION_CANCEL => Some(ElicitationAction::Cancel),
            _ => None,
        }
    })
}

pub struct RequestUserInputHandler;

#[async_trait]
impl ToolHandler for RequestUserInputHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        let ToolInvocation {
            session,
            turn,
            call_id,
            payload,
            ..
        } = invocation;

        let arguments = match payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "request_user_input handler received unsupported payload".to_string(),
                ));
            }
        };

        let mode = session.collaboration_mode().await.mode;
        if let Some(message) = request_user_input_unavailable_message(mode) {
            return Err(FunctionCallError::RespondToModel(message));
        }

        let mut args: RequestUserInputArgs = parse_arguments(&arguments)?;
        let missing_options = args
            .questions
            .iter()
            .any(|question| question.options.as_ref().is_none_or(Vec::is_empty));
        if missing_options {
            return Err(FunctionCallError::RespondToModel(
                "request_user_input requires non-empty options for every question".to_string(),
            ));
        }
        for question in &mut args.questions {
            question.is_other = true;
        }
        let response = session
            .request_user_input(turn.as_ref(), call_id, args)
            .await
            .ok_or_else(|| {
                FunctionCallError::RespondToModel(
                    "request_user_input was cancelled before receiving a response".to_string(),
                )
            })?;

        let content = serde_json::to_string(&response).map_err(|err| {
            FunctionCallError::Fatal(format!(
                "failed to serialize request_user_input response: {err}"
            ))
        })?;

        Ok(ToolOutput::Function {
            body: FunctionCallOutputBody::Text(content),
            success: Some(true),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn request_user_input_mode_availability_is_plan_only() {
        assert!(ModeKind::Plan.allows_request_user_input());
        assert!(!ModeKind::Default.allows_request_user_input());
        assert!(!ModeKind::Execute.allows_request_user_input());
        assert!(!ModeKind::PairProgramming.allows_request_user_input());
    }

    #[test]
    fn request_user_input_unavailable_messages_use_default_name_for_default_modes() {
        assert_eq!(request_user_input_unavailable_message(ModeKind::Plan), None);
        assert_eq!(
            request_user_input_unavailable_message(ModeKind::Default),
            Some("request_user_input is unavailable in Default mode".to_string())
        );
        assert_eq!(
            request_user_input_unavailable_message(ModeKind::Execute),
            Some("request_user_input is unavailable in Execute mode".to_string())
        );
        assert_eq!(
            request_user_input_unavailable_message(ModeKind::PairProgramming),
            Some("request_user_input is unavailable in Pair Programming mode".to_string())
        );
    }

    #[test]
    fn request_user_input_tool_description_mentions_plan_only() {
        assert_eq!(
            request_user_input_tool_description(),
            "Request user input for one to three short questions and wait for the response. This tool is only available in Plan mode.".to_string()
        );
    }
}
