use crate::codex_thread::GuardianRootMessage;
use crate::context::GuardianReviewEvidence;
use crate::function_tool::FunctionCallError;
use crate::guardian::guardian_truncate_text;
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
use codex_protocol::config_types::ModeKind;
use codex_protocol::request_user_input::RequestUserInputArgs;
use codex_tools::ToolName;
use codex_tools::ToolSpec;

pub struct RequestUserInputHandler {
    pub available_modes: Vec<ModeKind>,
}

const MAX_GUARDIAN_USER_INPUT_ANSWERS: usize = 8;
const MAX_GUARDIAN_USER_INPUT_TOKENS: usize = 900;

impl ToolExecutor<ToolInvocation> for RequestUserInputHandler {
    fn tool_name(&self) -> ToolName {
        ToolName::plain(REQUEST_USER_INPUT_TOOL_NAME)
    }

    fn spec(&self) -> ToolSpec {
        create_request_user_input_tool(request_user_input_tool_description(&self.available_modes))
    }

    fn handle(&self, invocation: ToolInvocation) -> codex_tools::ToolExecutorFuture<'_> {
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
        let response = session
            .request_user_input(turn.as_ref(), call_id.clone(), args)
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
        let user_input = questions
            .iter()
            .filter_map(|question| {
                let response = response.answers.get(&question.id)?;
                let answers = response
                    .answers
                    .iter()
                    .filter(|answer| !answer.trim().is_empty())
                    .take(MAX_GUARDIAN_USER_INPUT_ANSWERS)
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
                    .take(MAX_GUARDIAN_USER_INPUT_ANSWERS)
                {
                    question_text.push_str(&format!("\n{}: {}", option.label, option.description));
                }
                Some(format!(
                    "{}{}",
                    GuardianRootMessage::Assistant(question_text).render(),
                    GuardianRootMessage::User(answers.join("\n")).render()
                ))
            })
            .take(MAX_GUARDIAN_USER_INPUT_ANSWERS)
            .collect::<String>();
        if !user_input.is_empty() && turn.config.features.enabled(Feature::GuardianApproval) {
            let fragment = guardian_truncate_text(&user_input, MAX_GUARDIAN_USER_INPUT_TOKENS).0;
            session
                .services
                .thread_extension_data
                .get_or_init(GuardianReviewEvidence::default)
                .record_user_input(&call_id, fragment);
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
