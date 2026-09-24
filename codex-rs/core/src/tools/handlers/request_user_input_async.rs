use crate::function_tool::FunctionCallError;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::context::boxed_tool_output;
use crate::tools::handlers::parse_arguments;
use crate::tools::registry::CoreToolRuntime;
use crate::tools::registry::ToolExecutor;
use codex_protocol::items::AgentMessageContent;
use codex_protocol::items::AgentMessageDelivery;
use codex_protocol::items::AgentMessageItem;
use codex_protocol::items::AsyncUserInputQuestion;
use codex_protocol::items::TurnItem;
use codex_protocol::models::MessagePhase;
use codex_tools::JsonSchema;
use codex_tools::ResponsesApiTool;
use codex_tools::ToolName;
use codex_tools::ToolSpec;
use serde::Deserialize;
use std::collections::BTreeMap;

const TOOL_NAME: &str = "request_user_input_async";

pub struct RequestUserInputAsyncHandler {
    pub description: String,
    pub parameters: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RequestUserInputAsyncArgs {
    questions: Vec<AsyncUserInputQuestion>,
}

impl ToolExecutor<ToolInvocation> for RequestUserInputAsyncHandler {
    fn tool_name(&self) -> ToolName {
        ToolName::plain(TOOL_NAME)
    }

    fn spec(&self) -> ToolSpec {
        let mut options = JsonSchema::array(
            JsonSchema::string(/*description*/ None),
            Some("Suggested answers, in display order. Put the recommended answer first; the first option is preselected by default. The user can select one option or enter a free-text answer. Do not include an Other option or a free-text placeholder; the UI provides free-text input automatically. Omit options for a free-text-only question.".to_string()),
        );
        options.min_items = Some(1);
        let question = JsonSchema::object(
            BTreeMap::from([
                ("title".to_string(), JsonSchema::string(Some("The complete question shown to the user, including any context needed to answer it.".to_string()))),
                ("options".to_string(), options),
            ]),
            Some(vec!["title".to_string()]),
            /*additional_properties*/ Some(false.into()),
        );
        let mut questions = JsonSchema::array(
            question,
            Some(
                "One or more self-contained questions to present together, in display order."
                    .to_string(),
            ),
        );
        questions.min_items = Some(1);
        let properties = BTreeMap::from([("questions".to_string(), questions)]);
        let mut parameters = JsonSchema::object(
            properties,
            Some(vec!["questions".to_string()]),
            /*additional_properties*/ Some(false.into()),
        );
        if let Some(parameters_override) = &self.parameters {
            match crate::tools::catalog_parameters::parse(parameters_override) {
                Ok(schema) => parameters = schema,
                Err(reason) => tracing::warn!(
                    tool = TOOL_NAME,
                    reason,
                    "Invalid catalog tool parameters; using bundled parameters"
                ),
            }
        }

        ToolSpec::Function(ResponsesApiTool {
            name: TOOL_NAME.to_string(),
            description: self.description.clone(),
            strict: false,
            defer_loading: None,
            parameters,
            output_schema: None,
        })
    }

    fn handle<'a>(&'a self, invocation: ToolInvocation) -> codex_tools::ToolExecutorFuture<'a>
    where
        ToolInvocation: 'a,
    {
        Box::pin(async move {
            let ToolInvocation {
                session,
                turn,
                call_id,
                payload,
                ..
            } = invocation;
            let ToolPayload::Function { arguments } = payload else {
                return Err(FunctionCallError::RespondToModel(format!(
                    "{TOOL_NAME} handler received unsupported payload"
                )));
            };
            let args: RequestUserInputAsyncArgs = parse_arguments(&arguments)?;
            if args.questions.is_empty() {
                return Err(FunctionCallError::RespondToModel(
                    "questions must not be empty".to_string(),
                ));
            }
            let mut messages = Vec::with_capacity(args.questions.len());
            for question in &args.questions {
                if question.title.trim().is_empty() {
                    return Err(FunctionCallError::RespondToModel(
                        "question titles must not be empty".to_string(),
                    ));
                }
                let mut lines = vec![question.title.clone()];
                if let Some(options) = &question.options {
                    if options.is_empty() || options.iter().any(|option| option.trim().is_empty()) {
                        return Err(FunctionCallError::RespondToModel(
                            "options must contain at least one non-empty answer".to_string(),
                        ));
                    }
                    lines.extend(options.iter().map(|option| format!("- {option}")));
                }
                messages.push(lines.join("\n"));
            }

            let item = TurnItem::AgentMessage(AgentMessageItem {
                id: call_id,
                content: vec![AgentMessageContent::Text {
                    text: messages.join("\n\n"),
                }],
                phase: Some(MessagePhase::FinalAnswer),
                memory_citation: None,
                delivery: Some(AgentMessageDelivery::Async),
                questions: Some(args.questions),
            });
            session.emit_turn_item_started(turn.as_ref(), &item).await;
            session.emit_turn_item_completed(turn.as_ref(), item).await;

            Ok(boxed_tool_output(FunctionToolOutput::from_text(
                r#"{"accepted":true}"#.to_string(),
                /*success*/ Some(true),
            )))
        })
    }
}

impl CoreToolRuntime for RequestUserInputAsyncHandler {
    fn is_builtin_control_tool(&self) -> bool {
        true
    }
}
