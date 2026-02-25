use async_trait::async_trait;
use codex_protocol::models::FunctionCallOutputBody;

use crate::features::Feature;
use crate::function_tool::FunctionCallError;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::parse_arguments;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use codex_protocol::config_types::ModeKind;
use codex_protocol::config_types::TUI_VISIBLE_COLLABORATION_MODES;
use codex_protocol::request_user_input::RequestUserInputArgs;

pub(crate) fn request_user_input_allowed_for_mode(
    mode: ModeKind,
    request_user_input_outside_plan_mode: bool,
) -> bool {
    mode.allows_request_user_input() || request_user_input_outside_plan_mode
}

fn format_allowed_modes(request_user_input_outside_plan_mode: bool) -> String {
    let mode_names: Vec<&str> = TUI_VISIBLE_COLLABORATION_MODES
        .into_iter()
        .filter(|mode| {
            request_user_input_allowed_for_mode(*mode, request_user_input_outside_plan_mode)
        })
        .map(ModeKind::display_name)
        .collect();

    match mode_names.as_slice() {
        [] => "no modes".to_string(),
        [mode] => format!("{mode} mode"),
        [first, second] => format!("{first} and {second} modes"),
        [..] => format!("modes: {}", mode_names.join(", ")),
    }
}

pub(crate) fn request_user_input_unavailable_message(
    mode: ModeKind,
    request_user_input_outside_plan_mode: bool,
) -> Option<String> {
    if request_user_input_allowed_for_mode(mode, request_user_input_outside_plan_mode) {
        None
    } else {
        let mode_name = mode.display_name();
        Some(format!(
            "request_user_input is unavailable in {mode_name} mode"
        ))
    }
}

pub(crate) fn request_user_input_tool_description(
    request_user_input_outside_plan_mode: bool,
) -> String {
    let allowed_modes = format_allowed_modes(request_user_input_outside_plan_mode);
    format!(
        "Request user input for one to three short questions and wait for the response. This tool is only available in {allowed_modes}."
    )
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

        let request_user_input_outside_plan_mode = session
            .features()
            .enabled(Feature::RequestUserInputOutsidePlanMode);
        let mode = session.collaboration_mode().await.mode;
        if let Some(message) =
            request_user_input_unavailable_message(mode, request_user_input_outside_plan_mode)
        {
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
        assert_eq!(
            request_user_input_unavailable_message(ModeKind::Plan, false),
            None
        );
        assert_eq!(
            request_user_input_unavailable_message(ModeKind::Default, false),
            Some("request_user_input is unavailable in Default mode".to_string())
        );
        assert_eq!(
            request_user_input_unavailable_message(ModeKind::Execute, false),
            Some("request_user_input is unavailable in Execute mode".to_string())
        );
        assert_eq!(
            request_user_input_unavailable_message(ModeKind::PairProgramming, false),
            Some("request_user_input is unavailable in Pair Programming mode".to_string())
        );
    }

    #[test]
    fn request_user_input_tool_description_mentions_plan_only() {
        assert_eq!(
            request_user_input_tool_description(false),
            "Request user input for one to three short questions and wait for the response. This tool is only available in Plan mode.".to_string()
        );
    }

    #[test]
    fn request_user_input_outside_plan_mode_allows_all_non_plan_modes() {
        assert!(request_user_input_allowed_for_mode(ModeKind::Plan, true));
        assert!(request_user_input_allowed_for_mode(ModeKind::Default, true));
        assert!(request_user_input_allowed_for_mode(ModeKind::Execute, true));
        assert!(request_user_input_allowed_for_mode(
            ModeKind::PairProgramming,
            true
        ));
        assert_eq!(
            request_user_input_unavailable_message(ModeKind::Default, true),
            None
        );
        assert_eq!(
            request_user_input_tool_description(true),
            "Request user input for one to three short questions and wait for the response. This tool is only available in Default and Plan modes.".to_string()
        );
    }
}
