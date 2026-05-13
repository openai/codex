use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseInputItem;
use codex_tool_api::ToolBundle as ExtensionToolBundle;
use codex_tool_api::ToolError as ExtensionToolError;
use codex_tools::ResponsesApiTool;
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
use crate::tools::registry::ToolHandler;

pub(crate) struct BundledToolOutput {
    value: Value,
}

impl ToolOutput for BundledToolOutput {
    fn log_preview(&self) -> String {
        self.value.to_string()
    }

    fn success_for_logging(&self) -> bool {
        true
    }

    fn to_response_item(&self, call_id: &str, _payload: &ToolPayload) -> ResponseInputItem {
        ResponseInputItem::FunctionCallOutput {
            call_id: call_id.to_string(),
            output: FunctionCallOutputPayload {
                body: FunctionCallOutputBody::Text(self.value.to_string()),
                success: Some(true),
            },
        }
    }

    fn post_tool_use_response(&self, _call_id: &str, _payload: &ToolPayload) -> Option<Value> {
        Some(self.value.clone())
    }

    fn code_mode_result(&self, _payload: &ToolPayload) -> Value {
        self.value.clone()
    }
}

pub(crate) struct BundledToolHandler {
    bundle: ExtensionToolBundle,
    spec: ToolSpec,
}

impl BundledToolHandler {
    pub(crate) fn new(bundle: ExtensionToolBundle, spec: ToolSpec) -> Self {
        Self { bundle, spec }
    }

    fn arguments_from_payload<'a>(&self, payload: &'a ToolPayload) -> Option<&'a str> {
        let ToolPayload::Function { arguments } = payload else {
            return None;
        };
        Some(arguments)
    }
}

impl ToolHandler for BundledToolHandler {
    type Output = BundledToolOutput;

    fn tool_name(&self) -> ToolName {
        ToolName::plain(self.bundle.tool_name())
    }

    fn spec(&self) -> Option<ToolSpec> {
        Some(self.spec.clone())
    }

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

    async fn handle(&self, invocation: ToolInvocation) -> Result<Self::Output, FunctionCallError> {
        let arguments = self
            .arguments_from_payload(&invocation.payload)
            .ok_or_else(|| {
                FunctionCallError::Fatal(format!(
                    "tool {} invoked with incompatible payload",
                    self.bundle.tool_name()
                ))
            })?
            .to_string();
        let value = self
            .bundle
            .executor()
            .execute(codex_tool_api::ToolCall {
                call_id: invocation.call_id,
                arguments,
            })
            .await
            .map_err(map_extension_tool_error)?;
        Ok(BundledToolOutput { value })
    }
}

pub(crate) fn extension_tool_spec(
    spec: &codex_tool_api::FunctionToolSpec,
) -> Result<ToolSpec, serde_json::Error> {
    Ok(ToolSpec::Function(ResponsesApiTool {
        name: spec.name.clone(),
        description: spec.description.clone(),
        strict: spec.strict,
        defer_loading: None,
        parameters: codex_tools::parse_tool_input_schema(&spec.parameters)?,
        output_schema: None,
    }))
}

fn map_extension_tool_error(error: ExtensionToolError) -> FunctionCallError {
    match error {
        ExtensionToolError::RespondToModel(message) => FunctionCallError::RespondToModel(message),
        ExtensionToolError::Fatal(message) => FunctionCallError::Fatal(message),
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

    use super::BundledToolHandler;
    use super::BundledToolOutput;
    use super::extension_tool_spec;
    use crate::tools::context::ToolCallSource;
    use crate::tools::context::ToolInvocation;
    use crate::tools::context::ToolPayload;
    use crate::tools::hook_names::HookToolName;
    use crate::tools::registry::PostToolUsePayload;
    use crate::tools::registry::PreToolUsePayload;
    use crate::tools::registry::ToolHandler;
    use crate::turn_diff_tracker::TurnDiffTracker;

    struct StubExtensionExecutor;

    impl codex_tool_api::ToolExecutor for StubExtensionExecutor {
        fn execute(&self, _call: codex_tool_api::ToolCall) -> codex_tool_api::ToolFuture<'_> {
            Box::pin(async { Ok(json!({ "ok": true })) })
        }
    }

    #[tokio::test]
    async fn exposes_generic_hook_payloads() {
        let bundle = codex_tool_api::ToolBundle::new(
            codex_tool_api::FunctionToolSpec {
                name: "extension_echo".to_string(),
                description: "Echoes arguments.".to_string(),
                strict: true,
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "message": { "type": "string" },
                    },
                    "required": ["message"],
                    "additionalProperties": false,
                }),
            },
            Arc::new(StubExtensionExecutor),
        );
        let spec = extension_tool_spec(bundle.spec()).expect("extension spec should convert");
        let handler = BundledToolHandler::new(bundle, spec);
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
        let output = BundledToolOutput {
            value: json!({ "ok": true }),
        };

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
