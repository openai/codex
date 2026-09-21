use crate::function_tool::FunctionCallError;
use crate::responses_metadata::TurnToolNamespacesInfo;
use crate::session::session::Session;
use crate::session::step_context::StepContext;
#[cfg(test)]
use crate::session::turn_context::TurnContext;
use crate::tools::context::SharedTurnDiffTracker;
use crate::tools::context::ToolCallState;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
#[cfg(test)]
use crate::tools::handlers::ToolSearchHandlerCache;
use crate::tools::registry::AnyToolResult;
use crate::tools::registry::CoreToolRuntime;
use crate::tools::registry::ToolArgumentDiffConsumer;
use crate::tools::registry::ToolRegistry;
#[cfg(test)]
use crate::tools::spec_plan::finalize_tool_router;
use codex_protocol::models::ResponseItem;
use codex_protocol::models::SearchToolCallParams;
#[cfg(test)]
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ToolMode;
use codex_tools::DiscoverableTool;
use codex_tools::ResponsesApiNamespaceTool;
use codex_tools::ToolName;
use codex_tools::ToolSpec;
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use tracing::instrument;

pub use crate::tools::context::ToolCallSource;

#[derive(Clone, Debug, PartialEq)]
pub struct ToolCall {
    pub tool_name: ToolName,
    pub call_id: String,
    pub payload: ToolPayload,
    pub encrypted_function_args: Option<Vec<String>>,
}

impl ToolCall {
    pub(crate) fn direct_source(&self) -> ToolCallSource {
        if self.tool_name.namespace.as_deref() == Some("collaboration")
            && matches!(
                self.tool_name.name.as_str(),
                "spawn_agent" | "send_message" | "followup_task"
            )
            && self
                .encrypted_function_args
                .as_ref()
                .is_some_and(Vec::is_empty)
        {
            ToolCallSource::DirectPlaintextMessage
        } else {
            ToolCallSource::Direct
        }
    }
}

pub(crate) fn tool_log_payload<'a>(
    payload: &'a ToolPayload,
    source: &ToolCallSource,
) -> Cow<'a, str> {
    if matches!(source, ToolCallSource::DirectPlaintextMessage) {
        return Cow::Borrowed("[plaintext arguments]");
    }
    payload.log_payload()
}

/// One finalized tool plan: its advertised surfaces and matching executable runtimes.
pub struct ToolRouter {
    registry: ToolRegistry,
    model_visible_specs: Arc<[ToolSpec]>,
    tool_mode: ToolMode,
    code_mode_tool_names: BTreeMap<String, ToolName>,
    tool_namespaces_info: Option<TurnToolNamespacesInfo>,
    can_manage_children: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ToolSuggestPresentation {
    ListTool,
    RecommendationContext,
}

#[derive(Clone, Debug)]
pub(crate) struct ToolSuggestCandidates {
    pub(crate) tools: Vec<DiscoverableTool>,
    pub(crate) presentation: ToolSuggestPresentation,
}

impl ToolRouter {
    #[cfg(test)]
    pub(crate) fn from_registry(
        turn_context: &TurnContext,
        model_info: &ModelInfo,
        registry: ToolRegistry,
        hosted_specs: Vec<ToolSpec>,
        tool_search_handler_cache: &ToolSearchHandlerCache,
    ) -> Self {
        finalize_tool_router(
            turn_context,
            model_info,
            registry,
            hosted_specs,
            tool_search_handler_cache,
        )
        .expect("test tool registry should not contain duplicate tools")
    }

    pub(crate) fn from_parts(
        registry: ToolRegistry,
        model_visible_specs: Vec<ToolSpec>,
        tool_mode: ToolMode,
        code_mode_tool_names: BTreeMap<String, ToolName>,
        tool_namespaces_info: Option<TurnToolNamespacesInfo>,
        child_management_tools: &[ToolName],
    ) -> Self {
        let mut router = Self {
            registry,
            model_visible_specs: model_visible_specs.into(),
            tool_mode,
            code_mode_tool_names,
            tool_namespaces_info,
            can_manage_children: false,
        };
        router.can_manage_children = !child_management_tools.is_empty()
            && child_management_tools
                .iter()
                .all(|name| router.exposes_tool(name));
        router
    }

    pub(crate) fn model_visible_specs(&self) -> Arc<[ToolSpec]> {
        Arc::clone(&self.model_visible_specs)
    }

    pub(crate) fn tool_mode(&self) -> ToolMode {
        self.tool_mode
    }

    /// Code Mode still needs its dispatcher when the nested tool set is empty.
    pub(crate) fn requires_code_mode_worker(&self) -> bool {
        matches!(self.tool_mode, ToolMode::CodeMode | ToolMode::CodeModeOnly)
    }

    /// The normalized nested identities chosen after exclusions and collisions.
    // Consumed by the follow-up cell-origin migration.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn code_mode_tool_names(&self) -> &BTreeMap<String, ToolName> {
        &self.code_mode_tool_names
    }

    /// Optional request inventory for this exact plan, without publishing it to turn state.
    pub(crate) fn tool_namespaces_info(&self) -> Option<&TurnToolNamespacesInfo> {
        self.tool_namespaces_info.as_ref()
    }

    /// Whether the model can both start and interact with a terminal process.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn has_terminal_controls(&self) -> bool {
        self.exposes_tool(&ToolName::plain("exec_command"))
            && self.exposes_tool(&ToolName::plain("write_stdin"))
    }

    /// Whether the configured collaboration backend's child-management tools remain exposed.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn can_manage_children(&self) -> bool {
        self.can_manage_children
    }

    // Answers if the tool plan lets the model invoke the tool directly, through code mode, or deferred tool search.
    fn exposes_tool(&self, name: &ToolName) -> bool {
        let name = name.clone().with_default_namespace();
        if self
            .code_mode_tool_names
            .values()
            .any(|nested| nested.clone().with_default_namespace() == name)
            || self.model_visible_specs.iter().any(|spec| match spec {
                ToolSpec::Function(_) | ToolSpec::Freeform(_) => {
                    name.is_default_namespace() && spec.name() == name.name
                }
                ToolSpec::Namespace(namespace) => {
                    name.namespace.as_deref() == Some(namespace.name.as_str())
                        && namespace.tools.iter().any(|tool| match tool {
                            ResponsesApiNamespaceTool::Function(tool) => tool.name == name.name,
                            ResponsesApiNamespaceTool::Custom(tool) => tool.name == name.name,
                        })
                }
                ToolSpec::ToolSearch { .. } | ToolSpec::WebSearch { .. } => false,
            })
        {
            return true;
        }
        self.model_visible_specs
            .iter()
            .any(|spec| matches!(spec, ToolSpec::ToolSearch { .. }))
            && self.registry.entries().any(|tool| {
                tool.exposure.is_deferred()
                    && tool.runtime.tool_name().with_default_namespace() == name
                    && (tool.runtime.immutable_spec().is_some()
                        || tool.runtime.search_info().is_some())
            })
    }

    pub(crate) fn deferred_tool_namespaces(&self) -> BTreeMap<String, String> {
        self.registry.deferred_tool_namespaces()
    }

    #[cfg(test)]
    pub(crate) fn registered_tool_names_for_test(&self) -> Vec<ToolName> {
        self.registry.tool_names_for_test()
    }

    #[cfg(test)]
    pub(crate) fn tool_exposure_for_test(
        &self,
        name: &ToolName,
    ) -> Option<crate::tools::registry::ToolExposure> {
        self.registry.tool_exposure(name)
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

    pub(crate) fn tool_runtime(&self, tool_name: &ToolName) -> Option<Arc<dyn CoreToolRuntime>> {
        self.registry.tool(tool_name)
    }

    #[instrument(level = "trace", skip_all, err)]
    pub fn build_tool_call(item: ResponseItem) -> Result<Option<ToolCall>, FunctionCallError> {
        match item {
            ResponseItem::FunctionCall {
                name,
                namespace,
                arguments,
                encrypted_function_args,
                call_id,
                ..
            } => {
                let tool_name = ToolName::new(namespace, name).with_default_namespace();
                Ok(Some(ToolCall {
                    tool_name,
                    call_id,
                    payload: ToolPayload::Function { arguments },
                    encrypted_function_args,
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
                    encrypted_function_args: None,
                }))
            }
            ResponseItem::ToolSearchCall { .. } => Ok(None),
            ResponseItem::CustomToolCall {
                name,
                namespace,
                input,
                call_id,
                ..
            } => Ok(Some(ToolCall {
                tool_name: ToolName::new(namespace, name).with_default_namespace(),
                call_id,
                payload: ToolPayload::Custom { input },
                encrypted_function_args: None,
            })),
            _ => Ok(None),
        }
    }

    #[allow(dead_code)]
    #[instrument(level = "trace", skip_all, err)]
    pub async fn dispatch_tool_call_with_code_mode_result(
        &self,
        session: Arc<Session>,
        step_context: Arc<StepContext>,
        cancellation_token: CancellationToken,
        tracker: SharedTurnDiffTracker,
        call: ToolCall,
        source: ToolCallSource,
    ) -> Result<AnyToolResult, FunctionCallError> {
        self.dispatch_tool_call_with_code_mode_result_inner(
            session,
            step_context,
            cancellation_token,
            tracker,
            call,
            source,
            /*call_state*/ None,
        )
        .await
    }

    #[instrument(level = "trace", skip_all, err)]
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn dispatch_tool_call_with_state(
        &self,
        session: Arc<Session>,
        step_context: Arc<StepContext>,
        cancellation_token: CancellationToken,
        tracker: SharedTurnDiffTracker,
        call: ToolCall,
        source: ToolCallSource,
        call_state: Arc<ToolCallState>,
    ) -> Result<AnyToolResult, FunctionCallError> {
        self.dispatch_tool_call_with_code_mode_result_inner(
            session,
            step_context,
            cancellation_token,
            tracker,
            call,
            source,
            Some(call_state),
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn dispatch_tool_call_with_code_mode_result_inner(
        &self,
        session: Arc<Session>,
        step_context: Arc<StepContext>,
        cancellation_token: CancellationToken,
        tracker: SharedTurnDiffTracker,
        call: ToolCall,
        source: ToolCallSource,
        call_state: Option<Arc<ToolCallState>>,
    ) -> Result<AnyToolResult, FunctionCallError> {
        let ToolCall {
            tool_name,
            call_id,
            payload,
            ..
        } = call;

        // Keep the legacy ToolInvocation.turn field tied to the same request state until handlers migrate.
        let turn = Arc::clone(&step_context.turn);
        let invocation = ToolInvocation {
            session,
            turn,
            step_context,
            cancellation_token,
            tracker,
            call_id,
            tool_name,
            source,
            payload,
        };

        self.registry
            .dispatch_any_with_state(invocation, call_state)
            .await
    }
}

#[cfg(test)]
#[path = "router_tests.rs"]
mod tests;
