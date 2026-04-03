use crate::function_tool::FunctionCallError;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::parse_arguments;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use codex_protocol::protocol::SessionSource;
use codex_protocol::request_user_input::RequestUserInputArgs;
use codex_tools::REQUEST_USER_INPUT_TOOL_NAME;
use codex_tools::normalize_request_user_input_args;
use codex_tools::request_user_input_unavailable_message;

pub struct RequestUserInputHandler {
    pub default_mode_request_user_input: bool,
}

impl ToolHandler for RequestUserInputHandler {
    type Output = FunctionToolOutput;

    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<Self::Output, FunctionCallError> {
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
                return Err(FunctionCallError::RespondToModel(format!(
                    "{REQUEST_USER_INPUT_TOOL_NAME} handler received unsupported payload"
                )));
            }
        };

        if matches!(turn.session_source, SessionSource::SubAgent(_)) {
            return Err(FunctionCallError::RespondToModel(
                "request_user_input can only be used by the root thread".to_string(),
            ));
        }

        let mode = session.collaboration_mode().await.mode;
        if let Some(message) =
            request_user_input_unavailable_message(mode, self.default_mode_request_user_input)
        {
            return Err(FunctionCallError::RespondToModel(message));
        }

        let args: RequestUserInputArgs = parse_arguments(&arguments)?;
        let args =
            normalize_request_user_input_args(args).map_err(FunctionCallError::RespondToModel)?;
        let response = session
            .request_user_input(turn.as_ref(), call_id, args)
            .await
            .ok_or_else(|| {
                FunctionCallError::RespondToModel(format!(
                    "{REQUEST_USER_INPUT_TOOL_NAME} was cancelled before receiving a response"
                ))
            })?;

        let content = serde_json::to_string(&response).map_err(|err| {
            FunctionCallError::Fatal(format!(
                "failed to serialize {REQUEST_USER_INPUT_TOOL_NAME} response: {err}"
            ))
        })?;

        Ok(FunctionToolOutput::from_text(content, Some(true)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codex::make_session_and_context;
    use crate::turn_diff_tracker::TurnDiffTracker;
    use codex_protocol::ThreadId;
    use codex_protocol::protocol::SubAgentSource;
    use pretty_assertions::assert_eq;
    use serde_json::json;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    #[tokio::test]
    async fn request_user_input_rejects_subagent_threads() {
        let (session, mut turn_context) = make_session_and_context().await;
        turn_context.session_source = SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
            parent_thread_id: ThreadId::new(),
            depth: 1,
            agent_path: None,
            agent_nickname: None,
            agent_role: None,
        });

        let err = match (RequestUserInputHandler {
            default_mode_request_user_input: true,
        })
        .handle(ToolInvocation {
            session: Arc::new(session),
            turn: Arc::new(turn_context),
            tracker: Arc::new(Mutex::new(TurnDiffTracker::default())),
            call_id: "call-1".to_string(),
            tool_name: REQUEST_USER_INPUT_TOOL_NAME.to_string(),
            tool_namespace: None,
            payload: ToolPayload::Function {
                arguments: json!({
                    "questions": [{
                        "header": "Hdr",
                        "question": "Pick one",
                        "id": "pick_one",
                        "options": [
                            {
                                "label": "A",
                                "description": "A"
                            },
                            {
                                "label": "B",
                                "description": "B"
                            }
                        ]
                    }]
                })
                .to_string(),
            },
        })
        .await
        {
            Ok(_) => panic!("subagents should not be allowed to request user input"),
            Err(err) => err,
        };

        assert_eq!(
            err,
            FunctionCallError::RespondToModel(
                "request_user_input can only be used by the root thread".to_string(),
            )
        );
    }
}
