use crate::client_common::tools::ToolSpec;
use crate::codex::Session;
use crate::codex::TurnContext;
use crate::function_tool::FunctionCallError;
use crate::sandboxing::SandboxPermissions;
use crate::tools::context::SharedTurnDiffTracker;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::registry::ConfiguredToolSpec;
use crate::tools::registry::ToolRegistry;
use crate::tools::spec::ToolsConfig;
use crate::tools::spec::build_specs;
use codex_protocol::dynamic_tools::DynamicToolSpec;
use codex_protocol::models::LocalShellAction;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::models::ShellToolCallParams;
use codex_protocol::protocol::ToolCallPreExecuteDecision;
use codex_protocol::protocol::ToolCallType;
use rmcp::model::Tool;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::instrument;
use tracing::warn;

#[derive(Clone, Debug)]
pub struct ToolCall {
    pub tool_name: String,
    pub call_id: String,
    pub payload: ToolPayload,
}

pub struct ToolRouter {
    registry: ToolRegistry,
    specs: Vec<ConfiguredToolSpec>,
}

impl ToolRouter {
    pub fn from_config(
        config: &ToolsConfig,
        mcp_tools: Option<HashMap<String, Tool>>,
        dynamic_tools: &[DynamicToolSpec],
    ) -> Self {
        let builder = build_specs(config, mcp_tools, dynamic_tools);
        let (specs, registry) = builder.build();

        Self { registry, specs }
    }

    pub fn specs(&self) -> Vec<ToolSpec> {
        self.specs
            .iter()
            .map(|config| config.spec.clone())
            .collect()
    }

    pub fn tool_supports_parallel(&self, tool_name: &str) -> bool {
        self.specs
            .iter()
            .filter(|config| config.supports_parallel_tool_calls)
            .any(|config| config.spec.name() == tool_name)
    }

    #[instrument(level = "trace", skip_all, err)]
    pub async fn build_tool_call(
        session: &Session,
        item: ResponseItem,
    ) -> Result<Option<ToolCall>, FunctionCallError> {
        match item {
            ResponseItem::FunctionCall {
                name,
                arguments,
                call_id,
                ..
            } => {
                if let Some((server, tool)) = session.parse_mcp_tool_name(&name).await {
                    Ok(Some(ToolCall {
                        tool_name: name,
                        call_id,
                        payload: ToolPayload::Mcp {
                            server,
                            tool,
                            raw_arguments: arguments,
                        },
                    }))
                } else {
                    Ok(Some(ToolCall {
                        tool_name: name,
                        call_id,
                        payload: ToolPayload::Function { arguments },
                    }))
                }
            }
            ResponseItem::CustomToolCall {
                name,
                input,
                call_id,
                ..
            } => Ok(Some(ToolCall {
                tool_name: name,
                call_id,
                payload: ToolPayload::Custom { input },
            })),
            ResponseItem::LocalShellCall {
                id,
                call_id,
                action,
                ..
            } => {
                let call_id = call_id
                    .or(id)
                    .ok_or(FunctionCallError::MissingLocalShellCallId)?;

                match action {
                    LocalShellAction::Exec(exec) => {
                        let params = ShellToolCallParams {
                            command: exec.command,
                            workdir: exec.working_directory,
                            timeout_ms: exec.timeout_ms,
                            sandbox_permissions: Some(SandboxPermissions::UseDefault),
                            prefix_rule: None,
                            justification: None,
                        };
                        Ok(Some(ToolCall {
                            tool_name: "local_shell".to_string(),
                            call_id,
                            payload: ToolPayload::LocalShell { params },
                        }))
                    }
                }
            }
            _ => Ok(None),
        }
    }

    #[instrument(level = "trace", skip_all, err)]
    pub async fn dispatch_tool_call(
        &self,
        session: Arc<Session>,
        turn: Arc<TurnContext>,
        tracker: SharedTurnDiffTracker,
        call: ToolCall,
    ) -> Result<ResponseInputItem, FunctionCallError> {
        let ToolCall {
            tool_name,
            call_id,
            mut payload,
        } = call;
        let payload_outputs_custom = matches!(payload, ToolPayload::Custom { .. });
        let failure_call_id = call_id.clone();

        // CRAFT AGENTS: PreToolUse hook - intercept before execution
        let (tool_type, input_json, mcp_server, mcp_tool) = match &payload {
            ToolPayload::Function { arguments } => {
                (ToolCallType::Function, arguments.clone(), None, None)
            }
            ToolPayload::Custom { input } => {
                (ToolCallType::Custom, input.clone(), None, None)
            }
            ToolPayload::LocalShell { params } => {
                let input = serde_json::to_string(params).unwrap_or_default();
                (ToolCallType::LocalShell, input, None, None)
            }
            ToolPayload::Mcp { server, tool, raw_arguments } => {
                (ToolCallType::Mcp, raw_arguments.clone(), Some(server.clone()), Some(tool.clone()))
            }
        };

        let preexecute_response = session.request_tool_preexecute(
            &turn,
            call_id.clone(),
            tool_type,
            tool_name.clone(),
            input_json,
            mcp_server,
            mcp_tool,
        ).await;

        match preexecute_response.decision {
            ToolCallPreExecuteDecision::Block => {
                let reason = preexecute_response.reason.unwrap_or_else(|| "Blocked by permission system".to_string());
                return Ok(Self::failure_response(
                    failure_call_id,
                    payload_outputs_custom,
                    FunctionCallError::Blocked(reason),
                ));
            }
            ToolCallPreExecuteDecision::Modify => {
                // Update the payload with modified input
                if let Some(modified_input) = preexecute_response.modified_input {
                    payload = match payload {
                        ToolPayload::Function { .. } => {
                            ToolPayload::Function { arguments: modified_input }
                        }
                        ToolPayload::Custom { .. } => {
                            ToolPayload::Custom { input: modified_input }
                        }
                        ToolPayload::LocalShell { .. } => {
                            match serde_json::from_str(&modified_input) {
                                Ok(params) => ToolPayload::LocalShell { params },
                                Err(e) => {
                                    warn!("CRAFT AGENTS: Failed to parse modified LocalShell params: {e}");
                                    payload // Keep original on parse error
                                }
                            }
                        }
                        ToolPayload::Mcp { server, tool, .. } => {
                            ToolPayload::Mcp { server, tool, raw_arguments: modified_input }
                        }
                    };
                }
            }
            ToolCallPreExecuteDecision::Allow => {
                // Continue with original payload
            }
            ToolCallPreExecuteDecision::AskUser => {
                // AskUser should have been handled by the client and converted to Allow/Block.
                // If we get here, something went wrong - treat as Block for safety.
                warn!("CRAFT AGENTS: Received AskUser decision in router - blocking");
                return Ok(Self::failure_response(
                    failure_call_id,
                    payload_outputs_custom,
                    FunctionCallError::Blocked("Permission pending - AskUser response expected".to_string()),
                ));
            }
        }

        let invocation = ToolInvocation {
            session,
            turn,
            tracker,
            call_id,
            tool_name,
            payload,
        };

        match self.registry.dispatch(invocation).await {
            Ok(response) => Ok(response),
            Err(FunctionCallError::Fatal(message)) => Err(FunctionCallError::Fatal(message)),
            Err(err) => Ok(Self::failure_response(
                failure_call_id,
                payload_outputs_custom,
                err,
            )),
        }
    }

    fn failure_response(
        call_id: String,
        payload_outputs_custom: bool,
        err: FunctionCallError,
    ) -> ResponseInputItem {
        let message = err.to_string();
        if payload_outputs_custom {
            ResponseInputItem::CustomToolCallOutput {
                call_id,
                output: message,
            }
        } else {
            ResponseInputItem::FunctionCallOutput {
                call_id,
                output: codex_protocol::models::FunctionCallOutputPayload {
                    content: message,
                    success: Some(false),
                    ..Default::default()
                },
            }
        }
    }
}
