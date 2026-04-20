use crate::codex::Session;
use crate::codex::TurnContext;
use crate::function_tool::FunctionCallError;
use crate::sandboxing::SandboxPermissions;
use crate::tools::context::SharedTurnDiffTracker;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::registry::AnyToolResult;
use crate::tools::registry::ToolRegistry;
use crate::tools::spec::build_specs_with_discoverable_tools;
use codex_mcp::ToolInfo;
use codex_protocol::dynamic_tools::DynamicToolSpec;
use codex_protocol::models::LocalShellAction;
use codex_protocol::models::ResponseItem;
use codex_protocol::models::SearchToolCallParams;
use codex_protocol::models::ShellToolCallParams;
use codex_tools::ConfiguredToolSpec;
use codex_tools::DiscoverableTool;
use codex_tools::ResponsesApiNamespaceTool;
use codex_tools::ToolName;
use codex_tools::ToolSpec;
use codex_tools::ToolsConfig;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;
use tracing::instrument;

pub use crate::tools::context::ToolCallSource;

#[derive(Clone, Debug)]
pub struct ToolCall {
    pub tool_name: ToolName,
    pub call_id: String,
    pub payload: ToolPayload,
}

pub struct ToolRouter {
    registry: ToolRegistry,
    specs: Vec<ConfiguredToolSpec>,
    model_visible_specs: Vec<ToolSpec>,
    parallel_mcp_server_names: HashSet<String>,
}

pub(crate) struct ToolRouterParams<'a> {
    pub(crate) mcp_tools: Option<HashMap<String, ToolInfo>>,
    pub(crate) deferred_mcp_tools: Option<HashMap<String, ToolInfo>>,
    pub(crate) unavailable_called_tools: Vec<ToolName>,
    pub(crate) parallel_mcp_server_names: HashSet<String>,
    pub(crate) discoverable_tools: Option<Vec<DiscoverableTool>>,
    pub(crate) dynamic_tools: &'a [DynamicToolSpec],
}

impl ToolRouter {
    pub fn from_config(config: &ToolsConfig, params: ToolRouterParams<'_>) -> Self {
        let ToolRouterParams {
            mcp_tools,
            deferred_mcp_tools,
            unavailable_called_tools,
            parallel_mcp_server_names,
            discoverable_tools,
            dynamic_tools,
        } = params;
        let builder = build_specs_with_discoverable_tools(
            config,
            mcp_tools,
            deferred_mcp_tools,
            unavailable_called_tools,
            discoverable_tools,
            dynamic_tools,
        );
        let (specs, registry) = builder.build();
        let model_visible_specs = if config.code_mode_only_enabled {
            specs
                .iter()
                .filter_map(|configured_tool| {
                    if !codex_code_mode::is_code_mode_nested_tool(configured_tool.name()) {
                        Some(configured_tool.spec.clone())
                    } else {
                        None
                    }
                })
                .collect()
        } else {
            specs
                .iter()
                .map(|configured_tool| configured_tool.spec.clone())
                .collect()
        };

        Self {
            registry,
            specs,
            model_visible_specs,
            parallel_mcp_server_names,
        }
    }

    pub fn specs(&self) -> Vec<ToolSpec> {
        self.specs
            .iter()
            .map(|config| config.spec.clone())
            .collect()
    }

    pub fn model_visible_specs(&self) -> Vec<ToolSpec> {
        self.model_visible_specs.clone()
    }

    pub fn find_spec(&self, tool_name: &ToolName) -> Option<ToolSpec> {
        self.specs.iter().find_map(|config| match &config.spec {
            ToolSpec::Function(tool)
                if tool_name.namespace.is_none() && tool.name == tool_name.name =>
            {
                Some(config.spec.clone())
            }
            ToolSpec::Freeform(tool)
                if tool_name.namespace.is_none() && tool.name == tool_name.name =>
            {
                Some(config.spec.clone())
            }
            ToolSpec::Namespace(namespace) => namespace.tools.iter().find_map(|tool| match tool {
                ResponsesApiNamespaceTool::Function(tool)
                    if tool_name.namespace.as_deref() == Some(namespace.name.as_str())
                        && tool.name == tool_name.name =>
                {
                    Some(ToolSpec::Function(tool.clone()))
                }
                _ => None,
            }),
            _ => None,
        })
    }

    fn configured_tool_supports_parallel(&self, tool_name: &ToolName) -> bool {
        if tool_name.namespace.is_some() {
            return false;
        }

        self.specs
            .iter()
            .filter(|config| config.supports_parallel_tool_calls)
            .any(|config| match &config.spec {
                ToolSpec::Function(tool) => tool.name == tool_name.name.as_str(),
                ToolSpec::Freeform(tool) => tool.name == tool_name.name.as_str(),
                ToolSpec::Namespace(_)
                | ToolSpec::ToolSearch { .. }
                | ToolSpec::LocalShell {}
                | ToolSpec::ImageGeneration { .. }
                | ToolSpec::WebSearch { .. } => false,
            })
    }

    pub fn tool_supports_parallel(&self, call: &ToolCall) -> bool {
        match &call.payload {
            // MCP parallel support is configured per server, including for deferred
            // tools that may not have a matching spec entry. Use the parsed payload
            // server so similarly named servers/tools cannot collide.
            ToolPayload::Mcp { server, .. } => self.parallel_mcp_server_names.contains(server),
            _ => self.configured_tool_supports_parallel(&call.tool_name),
        }
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
                let tool_name = ToolName::new(namespace, name);
                if let Some(tool_info) = session.resolve_mcp_tool_info(&tool_name).await {
                    Ok(Some(ToolCall {
                        tool_name: tool_info.canonical_tool_name(),
                        call_id,
                        payload: ToolPayload::Mcp {
                            server: tool_info.server_name,
                            tool: tool_info.tool.name.to_string(),
                            raw_arguments: arguments,
                        },
                    }))
                } else {
                    Ok(Some(ToolCall {
                        tool_name,
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
                    tool_name: ToolName::plain("tool_search"),
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
                tool_name: ToolName::plain(name),
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
                            tool_name: ToolName::plain("local_shell"),
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
            call_id,
            payload,
        } = call;

        let direct_js_repl_call = tool_name.namespace.is_none()
            && matches!(tool_name.name.as_str(), "js_repl" | "js_repl_reset");
        if source == ToolCallSource::Direct
            && turn.tools_config.js_repl_tools_only
            && !direct_js_repl_call
        {
            return Err(FunctionCallError::RespondToModel(
                "direct tool calls are disabled; use js_repl and codex.tool(...) instead"
                    .to_string(),
            ));
        }

        let invocation = ToolInvocation {
            session,
            turn,
            tracker,
            source,
            call_id,
            tool_name,
            payload,
        };

        self.registry.dispatch_any(invocation).await
    }
}
#[cfg(test)]
#[path = "router_tests.rs"]
mod tests;
