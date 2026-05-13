use std::sync::Arc;

use codex_extension_api::ExtensionToolExecutor;
use codex_extension_api::ExtensionToolOutput;
use codex_tools::ToolCall as ExtensionToolCall;
use codex_tools::ToolName;
use codex_tools::ToolSpec;
use serde_json::Value;

use crate::function_tool::FunctionCallError;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::flat_tool_name;
use crate::tools::hook_names::HookToolName;
use crate::tools::registry::PostToolUsePayload;
use crate::tools::registry::PreToolUsePayload;
use crate::tools::registry::ToolExecutor;
use crate::tools::registry::ToolHandler;

pub(crate) struct ExtensionToolHandler {
    executor: Arc<dyn ExtensionToolExecutor>,
}

impl ExtensionToolHandler {
    pub(crate) fn new(executor: Arc<dyn ExtensionToolExecutor>) -> Self {
        Self { executor }
    }

    fn arguments_from_payload<'a>(&self, payload: &'a ToolPayload) -> Option<&'a str> {
        let ToolPayload::Function { arguments } = payload else {
            return None;
        };
        Some(arguments)
    }
}

impl ToolExecutor<ToolInvocation> for ExtensionToolHandler {
    type Output = ExtensionToolOutput;

    fn tool_name(&self) -> ToolName {
        self.executor.tool_name()
    }

    fn spec(&self) -> Option<ToolSpec> {
        self.executor.spec()
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<Self::Output, FunctionCallError> {
        self.executor.handle(to_extension_call(&invocation)).await
    }
}

impl ToolHandler for ExtensionToolHandler {
    fn matches_kind(&self, payload: &ToolPayload) -> bool {
        self.arguments_from_payload(payload).is_some()
    }

    fn pre_tool_use_payload(&self, invocation: &ToolInvocation) -> Option<PreToolUsePayload> {
        let arguments = self.arguments_from_payload(&invocation.payload)?;
        Some(PreToolUsePayload {
            tool_name: HookToolName::new(flat_tool_name(&self.tool_name()).into_owned()),
            tool_input: extension_tool_hook_input(arguments),
        })
    }

    fn post_tool_use_payload(
        &self,
        invocation: &ToolInvocation,
        result: &Self::Output,
    ) -> Option<PostToolUsePayload> {
        let arguments = self.arguments_from_payload(&invocation.payload)?;
        Some(PostToolUsePayload {
            tool_name: HookToolName::new(flat_tool_name(&self.tool_name()).into_owned()),
            tool_use_id: invocation.call_id.clone(),
            tool_input: extension_tool_hook_input(arguments),
            tool_response: result
                .post_tool_use_response(&invocation.call_id, &invocation.payload)?,
        })
    }
}

fn to_extension_call(invocation: &ToolInvocation) -> ExtensionToolCall {
    ExtensionToolCall {
        call_id: invocation.call_id.clone(),
        tool_name: invocation.tool_name.clone(),
        payload: invocation.payload.clone(),
    }
}

fn extension_tool_hook_input(arguments: &str) -> Value {
    if arguments.trim().is_empty() {
        return Value::Object(serde_json::Map::new());
    }

    serde_json::from_str(arguments).unwrap_or_else(|_| Value::String(arguments.to_string()))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use pretty_assertions::assert_eq;
    use serde_json::json;

    use super::ExtensionToolHandler;
    use crate::tools::context::ToolCallSource;
    use crate::tools::context::ToolInvocation;
    use crate::tools::context::ToolPayload;
    use crate::tools::hook_names::HookToolName;
    use crate::tools::registry::PostToolUsePayload;
    use crate::tools::registry::PreToolUsePayload;
    use crate::tools::registry::ToolHandler;
    use crate::turn_diff_tracker::TurnDiffTracker;

    struct StubExtensionExecutor;

    impl codex_extension_api::ExtensionToolExecutor for StubExtensionExecutor {
        fn tool_name(&self) -> codex_tools::ToolName {
            codex_tools::ToolName::plain("extension_echo")
        }

        fn spec(&self) -> Option<codex_tools::ToolSpec> {
            Some(codex_tools::ToolSpec::Function(
                codex_tools::ResponsesApiTool {
                    name: "extension_echo".to_string(),
                    description: "Echoes arguments.".to_string(),
                    strict: true,
                    parameters: codex_tools::parse_tool_input_schema(&json!({
                        "type": "object",
                        "properties": {
                            "message": { "type": "string" },
                        },
                        "required": ["message"],
                        "additionalProperties": false,
                    }))
                    .expect("extension schema should parse"),
                    output_schema: None,
                    defer_loading: None,
                },
            ))
        }

        fn handle(
            &self,
            _call: codex_tools::ToolCall,
        ) -> codex_extension_api::ExtensionToolFuture<'_> {
            Box::pin(async { Ok(codex_tools::JsonToolOutput::new(json!({ "ok": true }))) })
        }
    }

    #[tokio::test]
    async fn exposes_generic_hook_payloads() {
        let handler = ExtensionToolHandler::new(Arc::new(StubExtensionExecutor));
        let (session, turn) = crate::session::tests::make_session_and_context().await;
        let invocation = ToolInvocation {
            session: session.into(),
            turn: turn.into(),
            cancellation_token: tokio_util::sync::CancellationToken::new(),
            tracker: Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::new())),
            call_id: "call-extension".to_string(),
            tool_name: codex_tools::ToolName::plain("extension_echo"),
            source: ToolCallSource::Direct,
            payload: ToolPayload::Function {
                arguments: json!({ "message": "hello" }).to_string(),
            },
        };
        let output = codex_tools::JsonToolOutput::new(json!({ "ok": true }));

        assert_eq!(
            ToolHandler::pre_tool_use_payload(&handler, &invocation),
            Some(PreToolUsePayload {
                tool_name: HookToolName::new("extension_echo"),
                tool_input: json!({ "message": "hello" }),
            })
        );
        assert_eq!(
            ToolHandler::post_tool_use_payload(&handler, &invocation, &output),
            Some(PostToolUsePayload {
                tool_name: HookToolName::new("extension_echo"),
                tool_use_id: "call-extension".to_string(),
                tool_input: json!({ "message": "hello" }),
                tool_response: json!({ "ok": true }),
            })
        );
    }
}
