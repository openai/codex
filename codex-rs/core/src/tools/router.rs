use crate::function_tool::FunctionCallError;
use crate::sandboxing::SandboxPermissions;
use crate::session::session::Session;
use crate::session::turn_context::TurnContext;
use crate::tools::context::SharedTurnDiffTracker;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::registry::AnyToolResult;
use crate::tools::registry::ToolArgumentDiffConsumer;
use crate::tools::registry::ToolRegistry;
use crate::tools::spec::build_specs_with_discoverable_tools;
use codex_mcp::ToolInfo;
use codex_protocol::dynamic_tools::DynamicToolSpec;
use codex_protocol::models::LocalShellAction;
use codex_protocol::models::ResponseItem;
use codex_protocol::models::SearchToolCallParams;
use codex_protocol::models::ShellToolCallParams;
use codex_tool_api::ToolBundle as ExtensionToolBundle;
use codex_tools::DiscoverableTool;
use codex_tools::ResponsesApiNamespaceTool;
use codex_tools::ToolName;
use codex_tools::ToolSpec;
use codex_tools::ToolsConfig;
use std::collections::HashSet;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
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
    specs: Vec<ToolSpec>,
    model_visible_specs: Vec<ToolSpec>,
}

pub(crate) struct ToolRouterParams<'a> {
    pub(crate) mcp_tools: Option<Vec<ToolInfo>>,
    pub(crate) deferred_mcp_tools: Option<Vec<ToolInfo>>,
    pub(crate) unavailable_called_tools: Vec<ToolName>,
    pub(crate) discoverable_tools: Option<Vec<DiscoverableTool>>,
    pub(crate) extension_tool_bundles: Vec<ExtensionToolBundle>,
    pub(crate) dynamic_tools: &'a [DynamicToolSpec],
}

impl ToolRouter {
    pub fn from_config(config: &ToolsConfig, params: ToolRouterParams<'_>) -> Self {
        let ToolRouterParams {
            mcp_tools,
            deferred_mcp_tools,
            unavailable_called_tools,
            discoverable_tools,
            extension_tool_bundles,
            dynamic_tools,
        } = params;
        let builder = build_specs_with_discoverable_tools(
            config,
            mcp_tools,
            deferred_mcp_tools,
            unavailable_called_tools,
            discoverable_tools,
            &extension_tool_bundles,
            dynamic_tools,
        );
        let (specs, registry) = builder.build();
        let deferred_dynamic_tools = dynamic_tools
            .iter()
            .filter(|tool| tool.defer_loading)
            .map(|tool| ToolName::new(tool.namespace.clone(), tool.name.clone()))
            .collect::<HashSet<_>>();
        let model_visible_specs = specs
            .iter()
            .filter_map(|spec| {
                if config.code_mode_only_enabled
                    && codex_code_mode::is_code_mode_nested_tool(spec.name())
                {
                    return None;
                }

                filter_deferred_dynamic_tool_spec(spec.clone(), &deferred_dynamic_tools)
            })
            .collect();

        Self {
            registry,
            specs,
            model_visible_specs,
        }
    }

    pub fn specs(&self) -> Vec<ToolSpec> {
        self.specs.clone()
    }

    pub fn model_visible_specs(&self) -> Vec<ToolSpec> {
        self.model_visible_specs.clone()
    }

    pub(crate) fn create_diff_consumer(
        &self,
        tool_name: &ToolName,
    ) -> Option<Box<dyn ToolArgumentDiffConsumer>> {
        self.registry.create_diff_consumer(tool_name)
    }

    pub fn tool_supports_parallel(&self, call: &ToolCall) -> bool {
        self.registry
            .supports_parallel_tool_calls(&call.tool_name)
            .unwrap_or(false)
    }

    #[instrument(level = "trace", skip_all, err)]
    pub fn build_tool_call(item: ResponseItem) -> Result<Option<ToolCall>, FunctionCallError> {
        match item {
            ResponseItem::FunctionCall {
                name,
                namespace,
                arguments,
                call_id,
                ..
            } => {
                let tool_name = ToolName::new(namespace, name);
                Ok(Some(ToolCall {
                    tool_name,
                    call_id,
                    payload: ToolPayload::Function { arguments },
                }))
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
        cancellation_token: CancellationToken,
        tracker: SharedTurnDiffTracker,
        call: ToolCall,
        source: ToolCallSource,
    ) -> Result<AnyToolResult, FunctionCallError> {
        let ToolCall {
            tool_name,
            call_id,
            payload,
        } = call;

        let invocation = ToolInvocation {
            session,
            turn,
            cancellation_token,
            tracker,
            call_id,
            tool_name,
            source,
            payload,
        };

        self.registry.dispatch_any(invocation).await
    }
}

pub(crate) fn extension_tool_bundles(session: &Session) -> Vec<ExtensionToolBundle> {
    session
        .services
        .extensions
        .tool_contributors()
        .iter()
        .flat_map(|contributor| {
            contributor.tools(
                &session.services.session_extension_data,
                &session.services.thread_extension_data,
            )
        })
        .collect()
}

fn filter_deferred_dynamic_tool_spec(
    spec: ToolSpec,
    deferred_dynamic_tools: &HashSet<ToolName>,
) -> Option<ToolSpec> {
    if deferred_dynamic_tools.is_empty() {
        return Some(spec);
    }

    match spec {
        ToolSpec::Function(tool) => {
            if deferred_dynamic_tools.contains(&ToolName::plain(tool.name.as_str())) {
                None
            } else {
                Some(ToolSpec::Function(tool))
            }
        }
        ToolSpec::Namespace(mut namespace) => {
            let namespace_name = namespace.name.clone();
            namespace.tools.retain(|tool| match tool {
                ResponsesApiNamespaceTool::Function(tool) => !deferred_dynamic_tools.contains(
                    &ToolName::namespaced(namespace_name.as_str(), tool.name.as_str()),
                ),
            });
            if namespace.tools.is_empty() {
                None
            } else {
                Some(ToolSpec::Namespace(namespace))
            }
        }
        spec => Some(spec),
    }
}
#[cfg(test)]
#[path = "router_tests.rs"]
mod tests;
