use crate::client_common::tools::ToolSpec;
use crate::codex::Session;
use crate::codex::TurnContext;
use crate::function_tool::FunctionCallError;
use crate::hooks::HookEvent;
use crate::hooks::HookEventPostToolUse;
use crate::hooks::HookEventPreToolUse;
use crate::hooks::HookOutcome;
use crate::hooks::HookPayload;
use crate::sandboxing::SandboxPermissions;
use crate::tools::context::SharedTurnDiffTracker;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::registry::ConfiguredToolSpec;
use crate::tools::registry::ToolRegistry;
use crate::tools::spec::ToolsConfig;
use crate::tools::spec::build_specs;
use codex_protocol::dynamic_tools::DynamicToolSpec;
use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::models::LocalShellAction;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::models::ShellToolCallParams;
use rmcp::model::Tool;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::instrument;

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

        // Extract structured tool input for hooks (preserves shell arg
        // boundaries and workdir overrides, unlike log_payload()).
        let tool_input = payload.hook_input();

        // --- PreToolUse hook ---
        let pre_outcome = session
            .hooks()
            .dispatch(HookPayload {
                session_id: session.conversation_id,
                cwd: turn.cwd.clone(),
                triggered_at: chrono::Utc::now(),
                hook_event: HookEvent::PreToolUse {
                    event: HookEventPreToolUse {
                        tool_name: tool_name.clone(),
                        tool_input: tool_input.clone(),
                    },
                },
            })
            .await;

        match pre_outcome {
            HookOutcome::Proceed => {}
            HookOutcome::Block { message } => {
                let block_msg =
                    message.unwrap_or_else(|| "Blocked by pre_tool_use hook".to_string());
                return Ok(Self::failure_response(
                    failure_call_id,
                    payload_outputs_custom,
                    FunctionCallError::ToolCallBlocked(block_msg),
                ));
            }
            HookOutcome::Modify { content } => {
                // Apply the modified content to the tool arguments.
                match &mut payload {
                    ToolPayload::Function { arguments } => {
                        *arguments = content;
                    }
                    ToolPayload::Mcp { raw_arguments, .. } => {
                        *raw_arguments = content;
                    }
                    ToolPayload::Custom { input } => {
                        *input = content;
                    }
                    ToolPayload::LocalShell { .. } => {
                        // Modifying shell command structure from a hook is
                        // not safely supported.  Block the call so the
                        // hook's policy intent is not silently bypassed.
                        return Ok(Self::failure_response(
                            failure_call_id,
                            payload_outputs_custom,
                            FunctionCallError::ToolCallBlocked(
                                "pre_tool_use hook returned Modify for local_shell which is not supported; blocking execution".to_string(),
                            ),
                        ));
                    }
                }
            }
        }

        let invocation = ToolInvocation {
            session: Arc::clone(&session),
            turn: Arc::clone(&turn),
            tracker,
            call_id,
            tool_name: tool_name.clone(),
            payload,
        };

        let result = match self.registry.dispatch(invocation).await {
            Ok(response) => Ok(response),
            Err(FunctionCallError::Fatal(message)) => Err(FunctionCallError::Fatal(message)),
            Err(err) => Ok(Self::failure_response(
                failure_call_id,
                payload_outputs_custom,
                err,
            )),
        };

        // --- PostToolUse hook (fire-and-forget, does not alter the result) ---
        // Spawned as a background task so that slow/hung post-hooks do not
        // add latency to the tool response path.
        if let Ok(ref response) = result {
            let tool_output = Self::extract_output_text(response);
            let hooks = session.hooks().clone();
            let cwd = turn.cwd.clone();
            let conversation_id = session.conversation_id;
            tokio::spawn(async move {
                hooks
                    .dispatch(HookPayload {
                        session_id: conversation_id,
                        cwd,
                        triggered_at: chrono::Utc::now(),
                        hook_event: HookEvent::PostToolUse {
                            event: HookEventPostToolUse {
                                tool_name,
                                tool_output,
                            },
                        },
                    })
                    .await;
            });
        }

        result
    }

    /// Extract a textual preview from a `ResponseInputItem` for the PostToolUse hook.
    fn extract_output_text(item: &ResponseInputItem) -> String {
        match item {
            ResponseInputItem::FunctionCallOutput { output, .. } => {
                output.body.to_text().unwrap_or_default()
            }
            ResponseInputItem::McpToolCallOutput { result, .. } => match result {
                Ok(ctr) => {
                    let payload: codex_protocol::models::FunctionCallOutputPayload = ctr.into();
                    payload.body.to_text().unwrap_or_default()
                }
                Err(err) => err.clone(),
            },
            ResponseInputItem::CustomToolCallOutput { output, .. } => output.clone(),
            _ => String::new(),
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
                    body: FunctionCallOutputBody::Text(message),
                    success: Some(false),
                },
            }
        }
    }
}
