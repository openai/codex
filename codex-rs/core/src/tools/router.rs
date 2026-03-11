use crate::client_common::tools::ToolSpec;
use crate::codex::Session;
use crate::codex::TurnContext;
use crate::function_tool::FunctionCallError;
use crate::mcp_connection_manager::ToolInfo;
use crate::sandboxing::SandboxPermissions;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::SharedTurnDiffTracker;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::context::ToolSearchOutput;
use crate::tools::registry::AnyToolResult;
use crate::tools::registry::ConfiguredToolSpec;
use crate::tools::registry::ToolRegistry;
use crate::tools::spec::ToolsConfig;
use crate::tools::spec::build_specs;
use codex_protocol::dynamic_tools::DynamicToolSpec;
use codex_protocol::models::LocalShellAction;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::models::SearchToolCallParams;
use codex_protocol::models::ShellToolCallParams;
use rmcp::model::Tool;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::instrument;

pub use crate::tools::context::ToolCallSource;

#[derive(Clone, Debug)]
pub struct ToolCall {
    pub tool_name: String,
    pub tool_namespace: Option<String>,
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
        app_tools: Option<HashMap<String, ToolInfo>>,
        dynamic_tools: &[DynamicToolSpec],
    ) -> Self {
        let builder = build_specs(config, mcp_tools, app_tools, dynamic_tools);
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
                namespace,
                arguments,
                call_id,
                ..
            } => {
                if let Some((server, tool)) = session.parse_mcp_tool_name(&name, &namespace).await {
                    Ok(Some(ToolCall {
                        tool_name: name,
                        tool_namespace: namespace,
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
                        tool_namespace: namespace,
                        call_id,
                        payload: ToolPayload::Function { arguments },
                    }))
                }
            }
            ResponseItem::ToolSearchCall {
                call_id: Some(call_id),
                execution,
                arguments,
                ..
            } if execution == "client" => {
                let arguments: SearchToolCallParams =
                    serde_json::from_value(arguments).map_err(|err| {
                        FunctionCallError::RespondToModel(format!(
                            "failed to parse tool_search arguments: {err}"
                        ))
                    })?;
                Ok(Some(ToolCall {
                    tool_name: "tool_search".to_string(),
                    tool_namespace: None,
                    call_id,
                    payload: ToolPayload::ToolSearch { arguments },
                }))
            }
            ResponseItem::ToolSearchCall { .. } => Ok(None),
            ResponseItem::CustomToolCall {
                name,
                input,
                call_id,
                ..
            } => Ok(Some(ToolCall {
                tool_name: name,
                tool_namespace: None,
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
                            additional_permissions: None,
                            prefix_rule: None,
                            justification: None,
                        };
                        Ok(Some(ToolCall {
                            tool_name: "local_shell".to_string(),
                            tool_namespace: None,
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
        source: ToolCallSource,
    ) -> Result<ResponseInputItem, FunctionCallError> {
        Ok(self
            .dispatch_tool_call_with_code_mode_result(session, turn, tracker, call, source)
            .await?
            .into_response())
    }

    #[instrument(level = "trace", skip_all, err)]
    pub async fn dispatch_tool_call_with_code_mode_result(
        &self,
        session: Arc<Session>,
        turn: Arc<TurnContext>,
        tracker: SharedTurnDiffTracker,
        call: ToolCall,
        source: ToolCallSource,
    ) -> Result<AnyToolResult, FunctionCallError> {
        let ToolCall {
            tool_name,
            tool_namespace,
            call_id,
            payload,
        } = call;
        let payload_outputs_custom = matches!(payload, ToolPayload::Custom { .. });
        let payload_outputs_tool_search = matches!(payload, ToolPayload::ToolSearch { .. });
        let failure_call_id = call_id.clone();

        if source == ToolCallSource::Direct
            && turn.tools_config.js_repl_tools_only
            && !matches!(tool_name.as_str(), "js_repl" | "js_repl_reset")
        {
            let err = FunctionCallError::RespondToModel(
                "direct tool calls are disabled; use js_repl and codex.tool(...) instead"
                    .to_string(),
            );
            return Ok(Self::failure_result(
                failure_call_id,
                payload_outputs_custom,
                payload_outputs_tool_search,
                err,
            ));
        }

        let invocation = ToolInvocation {
            session,
            turn,
            tracker,
            call_id,
            tool_name,
            tool_namespace,
            payload,
        };

        match self.registry.dispatch_any(invocation).await {
            Ok(response) => Ok(response),
            Err(FunctionCallError::Fatal(message)) => Err(FunctionCallError::Fatal(message)),
            Err(err) => Ok(Self::failure_result(
                failure_call_id,
                payload_outputs_custom,
                payload_outputs_tool_search,
                err,
            )),
        }
    }

    fn failure_result(
        call_id: String,
        payload_outputs_custom: bool,
        payload_outputs_tool_search: bool,
        err: FunctionCallError,
    ) -> AnyToolResult {
        let message = err.to_string();
        if payload_outputs_tool_search {
            AnyToolResult {
                call_id,
                payload: ToolPayload::ToolSearch {
                    arguments: SearchToolCallParams {
                        query: String::new(),
                        limit: None,
                    },
                },
                result: Box::new(ToolSearchOutput { tools: Vec::new() }),
            }
        } else if payload_outputs_custom {
            AnyToolResult {
                call_id,
                payload: ToolPayload::Custom {
                    input: String::new(),
                },
                result: Box::new(FunctionToolOutput::from_text(message, Some(false))),
            }
        } else {
            AnyToolResult {
                call_id,
                payload: ToolPayload::Function {
                    arguments: "{}".to_string(),
                },
                result: Box::new(FunctionToolOutput::from_text(message, Some(false))),
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::codex::make_session_and_context;
    use crate::tools::context::ToolPayload;
    use crate::turn_diff_tracker::TurnDiffTracker;
    use codex_protocol::models::ResponseInputItem;
    use codex_protocol::models::ResponseItem;

    use super::ToolCall;
    use super::ToolCallSource;
    use super::ToolRouter;

    #[tokio::test]
    async fn js_repl_tools_only_blocks_direct_tool_calls() -> anyhow::Result<()> {
        let (session, mut turn) = make_session_and_context().await;
        turn.tools_config.js_repl_tools_only = true;

        let session = Arc::new(session);
        let turn = Arc::new(turn);
        let mcp_tools = session
            .services
            .mcp_connection_manager
            .read()
            .await
            .list_all_tools()
            .await;
        let app_tools = Some(mcp_tools.clone());
        let router = ToolRouter::from_config(
            &turn.tools_config,
            Some(
                mcp_tools
                    .into_iter()
                    .map(|(name, tool)| (name, tool.tool))
                    .collect(),
            ),
            app_tools,
            turn.dynamic_tools.as_slice(),
        );

        let call = ToolCall {
            tool_name: "shell".to_string(),
            tool_namespace: None,
            call_id: "call-1".to_string(),
            payload: ToolPayload::Function {
                arguments: "{}".to_string(),
            },
        };
        let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::new()));
        let response = router
            .dispatch_tool_call(session, turn, tracker, call, ToolCallSource::Direct)
            .await?;

        match response {
            ResponseInputItem::FunctionCallOutput { output, .. } => {
                let content = output.text_content().unwrap_or_default();
                assert!(
                    content.contains("direct tool calls are disabled"),
                    "unexpected tool call message: {content}",
                );
            }
            other => panic!("expected function call output, got {other:?}"),
        }

        Ok(())
    }

    #[tokio::test]
    async fn js_repl_tools_only_allows_js_repl_source_calls() -> anyhow::Result<()> {
        let (session, mut turn) = make_session_and_context().await;
        turn.tools_config.js_repl_tools_only = true;

        let session = Arc::new(session);
        let turn = Arc::new(turn);
        let mcp_tools = session
            .services
            .mcp_connection_manager
            .read()
            .await
            .list_all_tools()
            .await;
        let app_tools = Some(mcp_tools.clone());
        let router = ToolRouter::from_config(
            &turn.tools_config,
            Some(
                mcp_tools
                    .into_iter()
                    .map(|(name, tool)| (name, tool.tool))
                    .collect(),
            ),
            app_tools,
            turn.dynamic_tools.as_slice(),
        );

        let call = ToolCall {
            tool_name: "shell".to_string(),
            tool_namespace: None,
            call_id: "call-2".to_string(),
            payload: ToolPayload::Function {
                arguments: "{}".to_string(),
            },
        };
        let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::new()));
        let response = router
            .dispatch_tool_call(session, turn, tracker, call, ToolCallSource::JsRepl)
            .await?;

        match response {
            ResponseInputItem::FunctionCallOutput { output, .. } => {
                let content = output.text_content().unwrap_or_default();
                assert!(
                    !content.contains("direct tool calls are disabled"),
                    "js_repl source should bypass direct-call policy gate"
                );
            }
            other => panic!("expected function call output, got {other:?}"),
        }

        Ok(())
    }

    #[tokio::test]
    async fn build_tool_call_uses_namespace_for_registry_name() -> anyhow::Result<()> {
        let (session, _) = make_session_and_context().await;
        let session = Arc::new(session);
        let tool_name = "create_event".to_string();

        let call = ToolRouter::build_tool_call(
            &session,
            ResponseItem::FunctionCall {
                id: None,
                name: tool_name.clone(),
                namespace: Some("mcp__codex_apps__calendar".to_string()),
                arguments: "{}".to_string(),
                call_id: "call-namespace".to_string(),
            },
        )
        .await?
        .expect("function_call should produce a tool call");

        assert_eq!(call.tool_name, tool_name);
        assert_eq!(
            call.tool_namespace,
            Some("mcp__codex_apps__calendar".to_string())
        );
        assert_eq!(call.call_id, "call-namespace");
        match call.payload {
            ToolPayload::Function { arguments } => {
                assert_eq!(arguments, "{}");
            }
            other => panic!("expected function payload, got {other:?}"),
        }

        Ok(())
    }
}
