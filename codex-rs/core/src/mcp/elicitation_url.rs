use codex_protocol::approvals::ElicitationRequestEvent;
use codex_protocol::request_user_input::RequestUserInputAnswer;
use codex_protocol::request_user_input::RequestUserInputArgs;
use codex_protocol::request_user_input::RequestUserInputQuestion;
use codex_protocol::request_user_input::RequestUserInputQuestionOption;
use codex_protocol::request_user_input::RequestUserInputResponse;
use codex_rmcp_client::ElicitationAction;
use codex_rmcp_client::ElicitationResponse;

const REQUEST_USER_INPUT_NOTE_PREFIX: &str = "user_note: ";
const MCP_URL_ELICITATION_DECISION_QUESTION_ID: &str = "mcp_url_elicitation_decision";
const MCP_URL_ELICITATION_COMPLETED: &str = "Completed";
const MCP_URL_ELICITATION_DECLINE: &str = "Decline";
const MCP_URL_ELICITATION_CANCEL: &str = "Cancel";

pub(crate) fn is_url_elicitation_request(elicitation: &ElicitationRequestEvent) -> bool {
    elicitation.url.is_some()
}

pub(crate) struct UrlElicitationPromptOutcome {
    pub(crate) response: ElicitationResponse,
    pub(crate) completion_elicitation_id: Option<String>,
}

pub(crate) fn build_url_elicitation_request_user_input_args(
    elicitation: &ElicitationRequestEvent,
) -> RequestUserInputArgs {
    let mut question_lines = vec![elicitation.message.clone()];
    if let Some(url) = &elicitation.url {
        question_lines.push(String::new());
        question_lines.push("Open this URL in your browser to continue:".to_string());
        question_lines.push(url.clone());
        question_lines.push(String::new());
        question_lines.push(
            "After completing the flow in your browser, return here and choose Completed."
                .to_string(),
        );
    }
    if elicitation.elicitation_id.is_none() {
        question_lines.push(String::new());
        question_lines.push(
            "This request is missing required URL elicitation metadata, so it cannot be completed safely."
                .to_string(),
        );
    }

    RequestUserInputArgs {
        questions: vec![RequestUserInputQuestion {
            id: MCP_URL_ELICITATION_DECISION_QUESTION_ID.to_string(),
            header: "MCP URL elicitation".to_string(),
            question: question_lines.join("\n"),
            is_other: false,
            is_secret: false,
            options: Some(vec![
                RequestUserInputQuestionOption {
                    label: MCP_URL_ELICITATION_COMPLETED.to_string(),
                    description: "You completed the browser flow and want to continue.".to_string(),
                },
                RequestUserInputQuestionOption {
                    label: MCP_URL_ELICITATION_DECLINE.to_string(),
                    description: "Decline this URL elicitation request.".to_string(),
                },
                RequestUserInputQuestionOption {
                    label: MCP_URL_ELICITATION_CANCEL.to_string(),
                    description: "Cancel this URL elicitation request.".to_string(),
                },
            ]),
        }],
    }
}

pub(crate) fn build_url_elicitation_outcome_from_user_input(
    response: Option<RequestUserInputResponse>,
    elicitation: &ElicitationRequestEvent,
) -> UrlElicitationPromptOutcome {
    let Some(response) = response else {
        return UrlElicitationPromptOutcome {
            response: ElicitationResponse {
                action: ElicitationAction::Cancel,
                content: None,
            },
            completion_elicitation_id: None,
        };
    };

    let action = response
        .answers
        .get(MCP_URL_ELICITATION_DECISION_QUESTION_ID)
        .and_then(request_user_input_answer_to_url_elicitation_action)
        .unwrap_or(ElicitationAction::Cancel);

    match action {
        ElicitationAction::Accept => {
            let Some(elicitation_id) = elicitation.elicitation_id.clone() else {
                return UrlElicitationPromptOutcome {
                    response: ElicitationResponse {
                        action: ElicitationAction::Cancel,
                        content: None,
                    },
                    completion_elicitation_id: None,
                };
            };
            UrlElicitationPromptOutcome {
                response: ElicitationResponse {
                    action: ElicitationAction::Accept,
                    content: None,
                },
                completion_elicitation_id: Some(elicitation_id),
            }
        }
        ElicitationAction::Decline | ElicitationAction::Cancel => UrlElicitationPromptOutcome {
            response: ElicitationResponse {
                action,
                content: None,
            },
            completion_elicitation_id: None,
        },
    }
}

fn request_user_input_answer_to_url_elicitation_action(
    answer: &RequestUserInputAnswer,
) -> Option<ElicitationAction> {
    answer.answers.iter().find_map(|entry| {
        if entry.starts_with(REQUEST_USER_INPUT_NOTE_PREFIX) {
            return None;
        }
        match entry.as_str() {
            MCP_URL_ELICITATION_COMPLETED => Some(ElicitationAction::Accept),
            MCP_URL_ELICITATION_DECLINE => Some(ElicitationAction::Decline),
            MCP_URL_ELICITATION_CANCEL => Some(ElicitationAction::Cancel),
            _ => None,
        }
    })
}
