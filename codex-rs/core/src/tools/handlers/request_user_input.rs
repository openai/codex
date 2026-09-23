use crate::function_tool::FunctionCallError;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::context::boxed_tool_output;
use crate::tools::handlers::parse_arguments;
use crate::tools::handlers::request_user_input_spec::REQUEST_USER_INPUT_TOOL_NAME;
use crate::tools::handlers::request_user_input_spec::RequestUserInputToolArgs;
use crate::tools::handlers::request_user_input_spec::create_request_user_input_tool;
use crate::tools::handlers::request_user_input_spec::normalize_request_user_input_tool_args;
use crate::tools::handlers::request_user_input_spec::request_user_input_tool_description;
use crate::tools::handlers::request_user_input_spec::request_user_input_unavailable_message;
use crate::tools::registry::CoreToolRuntime;
use crate::tools::registry::ToolExecutor;
use codex_features::Feature;
use codex_history::RetainedContextEvent;
use codex_history::VerifiedAnswer;
use codex_history::VerifiedQuestionAnswer;
use codex_protocol::config_types::ModeKind;
use codex_protocol::request_user_input::RequestUserInputArgs;
use codex_tools::ToolName;
use codex_tools::ToolSpec;

pub struct RequestUserInputHandler {
    pub available_modes: Vec<ModeKind>,
}

impl ToolExecutor<ToolInvocation> for RequestUserInputHandler {
    fn tool_name(&self) -> ToolName {
        ToolName::plain(REQUEST_USER_INPUT_TOOL_NAME)
    }

    fn spec(&self) -> ToolSpec {
        create_request_user_input_tool(request_user_input_tool_description(&self.available_modes))
    }

    fn handle<'a>(&'a self, invocation: ToolInvocation) -> codex_tools::ToolExecutorFuture<'a>
    where
        ToolInvocation: 'a,
    {
        Box::pin(self.handle_call(invocation))
    }
}

impl RequestUserInputHandler {
    async fn handle_call(
        &self,
        invocation: ToolInvocation,
    ) -> Result<Box<dyn crate::tools::context::ToolOutput>, FunctionCallError> {
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

        if turn.session_source.is_non_root_agent() {
            return Err(FunctionCallError::RespondToModel(
                "request_user_input can only be used by the root thread".to_string(),
            ));
        }

        let mode = turn.collaboration_mode().mode;
        if let Some(message) = request_user_input_unavailable_message(mode, &self.available_modes) {
            return Err(FunctionCallError::RespondToModel(message));
        }

        let args: RequestUserInputToolArgs = parse_arguments(&arguments)?;
        let args = normalize_request_user_input_tool_args(args)
            .map_err(FunctionCallError::RespondToModel)?;
        let args = RequestUserInputArgs {
            questions: args.questions,
            is_blocking: mode == ModeKind::Plan,
            auto_resolution_ms: None,
        };
        let questions = args.questions.clone();
        let accepted = session
            .request_user_input(turn.as_ref(), call_id.clone(), args)
            .await
            .ok_or_else(|| {
                FunctionCallError::RespondToModel(format!(
                    "{REQUEST_USER_INPUT_TOOL_NAME} was cancelled before receiving a response"
                ))
            })?;

        let response = accepted.response;

        let content = serde_json::to_string(&response).map_err(|err| {
            FunctionCallError::Fatal(format!(
                "failed to serialize {REQUEST_USER_INPUT_TOOL_NAME} response: {err}"
            ))
        })?;
        // Persist answers even while an older checkpoint needs compatibility review.
        if turn.config.features.enabled(Feature::GuardianApproval) {
            let user_input = questions
                .iter()
                .filter_map(|question| {
                    let response = response.answers.get(&question.id)?;
                    let answers = response
                        .answers
                        .iter()
                        .filter(|answer| !answer.trim().is_empty())
                        .cloned()
                        .collect::<Vec<_>>();
                    if answers.is_empty() {
                        return None;
                    }
                    let mut question_text = question.question.clone();
                    for option in question
                        .options
                        .iter()
                        .flatten()
                        .filter(|option| response.answers.contains(&option.label))
                    {
                        question_text
                            .push_str(&format!("\n{}: {}", option.label, option.description));
                    }
                    Some(VerifiedQuestionAnswer {
                        question: question_text,
                        answer: answers.join("\n"),
                    })
                })
                .collect::<Vec<_>>();
            if !user_input.is_empty() {
                session
                    .record_retained_context(RetainedContextEvent::VerifiedAnswer {
                        answer: VerifiedAnswer {
                            turn_id: turn.sub_id.clone(),
                            call_id,
                            questions: user_input,
                        },
                        acceptance_order: Some(accepted.acceptance_order),
                    })
                    .await;
            }
        }

        Ok(boxed_tool_output(FunctionToolOutput::from_text(
            content,
            /*success*/ Some(true),
        )))
    }
}

impl CoreToolRuntime for RequestUserInputHandler {
    fn is_builtin_control_tool(&self) -> bool {
        true
    }
}

#[cfg(test)]
#[path = "request_user_input_tests.rs"]
mod tests;
