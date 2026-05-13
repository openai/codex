use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use crate::function_tool::FunctionCallError;
use crate::goals::GoalRuntimeEvent;
use crate::hook_runtime::PreToolUseHookResult;
use crate::hook_runtime::record_additional_contexts;
use crate::hook_runtime::run_post_tool_use_hooks;
use crate::hook_runtime::run_pre_tool_use_hooks;
use crate::memory_usage::emit_metric_for_tool_read;
use crate::sandbox_tags::permission_profile_policy_tag;
use crate::sandbox_tags::permission_profile_sandbox_tag;
use crate::session::turn_context::TurnContext;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::flat_tool_name;
use crate::tools::handlers::extension_tools::ExtensionToolHandler;
use crate::tools::hook_names::HookToolName;
use crate::tools::tool_dispatch_trace::ToolDispatchTrace;
use crate::tools::tool_search_entry::ToolSearchInfo;
use crate::util::error_or_panic;
use codex_extension_api::ExtensionToolExecutor;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::protocol::EventMsg;
use codex_tools::ToolName;
use codex_tools::ToolSpec;
use futures::future::BoxFuture;
use serde_json::Value;
use tracing::warn;

pub(crate) type ToolTelemetryTags = Vec<(&'static str, String)>;

pub use codex_tools::ToolExecutor;
pub use codex_tools::ToolExposure;

pub trait ToolHandler: ToolExecutor<ToolInvocation> {
    fn search_info(&self) -> Option<ToolSearchInfo> {
        None
    }

    fn matches_kind(&self, payload: &ToolPayload) -> bool {
        matches!(
            payload,
            ToolPayload::Function { .. } | ToolPayload::ToolSearch { .. }
        )
    }

    fn telemetry_tags(
        &self,
        _invocation: &ToolInvocation,
    ) -> impl Future<Output = ToolTelemetryTags> + Send {
        async { Vec::new() }
    }

    fn post_tool_use_payload(
        &self,
        _invocation: &ToolInvocation,
        _result: &Self::Output,
    ) -> Option<PostToolUsePayload> {
        None
    }

    fn pre_tool_use_payload(&self, _invocation: &ToolInvocation) -> Option<PreToolUsePayload> {
        None
    }

    /// Rebuilds a tool invocation from hook-facing `tool_input`.
    ///
    /// Tools that opt into input-rewriting hooks should invert the same stable
    /// hook contract they expose from `pre_tool_use_payload`.
    fn with_updated_hook_input(
        &self,
        _invocation: ToolInvocation,
        _updated_input: Value,
    ) -> Result<ToolInvocation, FunctionCallError> {
        Err(FunctionCallError::RespondToModel(
            "tool does not support hook input rewriting".to_string(),
        ))
    }

    /// Creates an optional consumer for streamed tool argument diffs.
    fn create_diff_consumer(&self) -> Option<Box<dyn ToolArgumentDiffConsumer>> {
        None
    }
}

/// Consumes streamed argument diffs for a tool call and emits protocol events
/// derived from partial tool input.
pub(crate) trait ToolArgumentDiffConsumer: Send {
    /// Consume the next argument diff for a tool call.
    fn consume_diff(&mut self, turn: &TurnContext, call_id: String, diff: &str)
    -> Option<EventMsg>;

    /// Finish consuming argument diffs before the tool call completes.
    fn finish(&mut self) -> Result<Option<EventMsg>, FunctionCallError> {
        Ok(None)
    }
}

pub(crate) struct AnyToolResult {
    pub(crate) call_id: String,
    pub(crate) payload: ToolPayload,
    pub(crate) result: Box<dyn ToolOutput>,
    pub(crate) post_tool_use_payload: Option<PostToolUsePayload>,
}

impl AnyToolResult {
    pub(crate) fn into_response(self) -> ResponseInputItem {
        let Self {
            call_id,
            payload,
            result,
            ..
        } = self;
        result.to_response_item(&call_id, &payload)
    }

    pub(crate) fn code_mode_result(self) -> serde_json::Value {
        let Self {
            payload, result, ..
        } = self;
        result.code_mode_result(&payload)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PreToolUsePayload {
    /// Hook-facing tool name model.
    ///
    /// The canonical name is serialized to hook stdin, while aliases are used
    /// only for matcher compatibility.
    pub(crate) tool_name: HookToolName,
    /// Tool-specific input exposed at `tool_input`.
    ///
    /// Shell-like tools use `{ "command": ... }`; MCP tools use their resolved
    /// JSON arguments.
    pub(crate) tool_input: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PostToolUsePayload {
    /// Hook-facing tool name model.
    ///
    /// The canonical name is serialized to hook stdin, while aliases are used
    /// only for matcher compatibility.
    pub(crate) tool_name: HookToolName,
    /// The originating tool-use id exposed at `tool_use_id`.
    pub(crate) tool_use_id: String,
    /// Tool-specific input exposed at `tool_input`.
    pub(crate) tool_input: Value,
    /// Tool result exposed at `tool_response`.
    pub(crate) tool_response: Value,
}

/// Object-safe registry entry for heterogeneous tool handlers.
///
/// Concrete handlers keep their typed `ToolExecutor::Output`; the registry
/// boxes that output only after typed hooks have run.
pub(crate) trait RegisteredTool: Send + Sync {
    fn tool_name(&self) -> ToolName;

    fn spec(&self) -> Option<ToolSpec>;

    fn exposure(&self) -> ToolExposure;

    fn search_info(&self) -> Option<ToolSearchInfo>;

    fn supports_parallel_tool_calls(&self) -> bool;

    fn matches_kind(&self, payload: &ToolPayload) -> bool;

    fn pre_tool_use_payload(&self, invocation: &ToolInvocation) -> Option<PreToolUsePayload>;

    fn with_updated_hook_input(
        &self,
        invocation: ToolInvocation,
        updated_input: Value,
    ) -> Result<ToolInvocation, FunctionCallError>;

    fn telemetry_tags<'a>(
        &'a self,
        invocation: &'a ToolInvocation,
    ) -> BoxFuture<'a, ToolTelemetryTags>;

    fn create_diff_consumer(&self) -> Option<Box<dyn ToolArgumentDiffConsumer>>;
    fn handle_any<'a>(
        &'a self,
        invocation: ToolInvocation,
    ) -> BoxFuture<'a, Result<AnyToolResult, FunctionCallError>>;
}

impl<T> RegisteredTool for T
where
    T: ToolHandler,
{
    fn tool_name(&self) -> ToolName {
        ToolExecutor::tool_name(self)
    }

    fn spec(&self) -> Option<ToolSpec> {
        ToolExecutor::spec(self)
    }

    fn exposure(&self) -> ToolExposure {
        ToolExecutor::exposure(self)
    }

    fn search_info(&self) -> Option<ToolSearchInfo> {
        ToolHandler::search_info(self)
    }

    fn supports_parallel_tool_calls(&self) -> bool {
        ToolExecutor::supports_parallel_tool_calls(self)
    }

    fn matches_kind(&self, payload: &ToolPayload) -> bool {
        ToolHandler::matches_kind(self, payload)
    }

    fn pre_tool_use_payload(&self, invocation: &ToolInvocation) -> Option<PreToolUsePayload> {
        ToolHandler::pre_tool_use_payload(self, invocation)
    }

    fn with_updated_hook_input(
        &self,
        invocation: ToolInvocation,
        updated_input: Value,
    ) -> Result<ToolInvocation, FunctionCallError> {
        ToolHandler::with_updated_hook_input(self, invocation, updated_input)
    }

    fn telemetry_tags<'a>(
        &'a self,
        invocation: &'a ToolInvocation,
    ) -> BoxFuture<'a, ToolTelemetryTags> {
        Box::pin(ToolHandler::telemetry_tags(self, invocation))
    }

    fn create_diff_consumer(&self) -> Option<Box<dyn ToolArgumentDiffConsumer>> {
        ToolHandler::create_diff_consumer(self)
    }
    fn handle_any<'a>(
        &'a self,
        invocation: ToolInvocation,
    ) -> BoxFuture<'a, Result<AnyToolResult, FunctionCallError>> {
        Box::pin(async move {
            let call_id = invocation.call_id.clone();
            let payload = invocation.payload.clone();
            let output = ToolExecutor::handle(self, invocation.clone()).await?;
            let post_tool_use_payload =
                ToolHandler::post_tool_use_payload(self, &invocation, &output);
            Ok(AnyToolResult {
                call_id,
                payload,
                result: Box::new(output),
                post_tool_use_payload,
            })
        })
    }
}

pub(crate) fn override_tool_exposure(
    handler: Arc<dyn RegisteredTool>,
    exposure: ToolExposure,
) -> Arc<dyn RegisteredTool> {
    if handler.exposure() == exposure {
        return handler;
    }

    Arc::new(ExposureOverride { handler, exposure })
}

struct ExposureOverride {
    handler: Arc<dyn RegisteredTool>,
    exposure: ToolExposure,
}

impl RegisteredTool for ExposureOverride {
    fn tool_name(&self) -> ToolName {
        self.handler.tool_name()
    }

    fn spec(&self) -> Option<ToolSpec> {
        self.handler.spec()
    }

    fn exposure(&self) -> ToolExposure {
        self.exposure
    }

    fn search_info(&self) -> Option<ToolSearchInfo> {
        self.handler.search_info()
    }

    fn supports_parallel_tool_calls(&self) -> bool {
        self.handler.supports_parallel_tool_calls()
    }

    fn matches_kind(&self, payload: &ToolPayload) -> bool {
        self.handler.matches_kind(payload)
    }

    fn pre_tool_use_payload(&self, invocation: &ToolInvocation) -> Option<PreToolUsePayload> {
        self.handler.pre_tool_use_payload(invocation)
    }

    fn with_updated_hook_input(
        &self,
        invocation: ToolInvocation,
        updated_input: Value,
    ) -> Result<ToolInvocation, FunctionCallError> {
        self.handler
            .with_updated_hook_input(invocation, updated_input)
    }

    fn telemetry_tags<'a>(
        &'a self,
        invocation: &'a ToolInvocation,
    ) -> BoxFuture<'a, ToolTelemetryTags> {
        self.handler.telemetry_tags(invocation)
    }

    fn create_diff_consumer(&self) -> Option<Box<dyn ToolArgumentDiffConsumer>> {
        self.handler.create_diff_consumer()
    }

    fn handle_any<'a>(
        &'a self,
        invocation: ToolInvocation,
    ) -> BoxFuture<'a, Result<AnyToolResult, FunctionCallError>> {
        self.handler.handle_any(invocation)
    }
}

pub struct ToolRegistry {
    handlers: HashMap<ToolName, Arc<dyn RegisteredTool>>,
}

impl ToolRegistry {
    fn new(handlers: HashMap<ToolName, Arc<dyn RegisteredTool>>) -> Self {
        Self { handlers }
    }

    #[cfg(test)]
    pub(crate) fn empty_for_test() -> Self {
        Self::new(HashMap::new())
    }

    #[cfg(test)]
    pub(crate) fn with_handler_for_test<T>(handler: Arc<T>) -> Self
    where
        T: ToolHandler + 'static,
    {
        let name = handler.tool_name();
        Self::new(HashMap::from([(name, handler as Arc<dyn RegisteredTool>)]))
    }

    fn handler(&self, name: &ToolName) -> Option<Arc<dyn RegisteredTool>> {
        self.handlers.get(name).map(Arc::clone)
    }

    pub(crate) fn tool_exposure(&self, name: &ToolName) -> Option<ToolExposure> {
        self.handlers.get(name).map(|handler| handler.exposure())
    }

    #[cfg(test)]
    pub(crate) fn has_handler(&self, name: &ToolName) -> bool {
        self.handler(name).is_some()
    }

    pub(crate) fn create_diff_consumer(
        &self,
        name: &ToolName,
    ) -> Option<Box<dyn ToolArgumentDiffConsumer>> {
        self.handler(name)?.create_diff_consumer()
    }

    pub(crate) fn supports_parallel_tool_calls(&self, name: &ToolName) -> Option<bool> {
        let handler = self.handler(name)?;
        Some(handler.supports_parallel_tool_calls())
    }

    #[expect(
        clippy::await_holding_invalid_type,
        reason = "tool dispatch must keep active-turn accounting atomic"
    )]
    pub(crate) async fn dispatch_any(
        &self,
        mut invocation: ToolInvocation,
    ) -> Result<AnyToolResult, FunctionCallError> {
        let tool_name = invocation.tool_name.clone();
        let tool_name_flat = flat_tool_name(&tool_name);
        let call_id_owned = invocation.call_id.clone();
        let otel = invocation.turn.session_telemetry.clone();
        let base_tool_result_tags = [
            (
                "sandbox",
                permission_profile_sandbox_tag(
                    &invocation.turn.permission_profile,
                    invocation.turn.windows_sandbox_level,
                    invocation.turn.network.is_some(),
                ),
            ),
            (
                "sandbox_policy",
                permission_profile_policy_tag(
                    &invocation.turn.permission_profile,
                    #[allow(deprecated)]
                    invocation.turn.cwd.as_path(),
                ),
            ),
        ];

        {
            let mut active = invocation.session.active_turn.lock().await;
            if let Some(active_turn) = active.as_mut() {
                let mut turn_state = active_turn.turn_state.lock().await;
                turn_state.tool_calls = turn_state.tool_calls.saturating_add(1);
            }
        }

        let dispatch_trace = ToolDispatchTrace::start(&invocation);
        let handler = match self.handler(&tool_name) {
            Some(handler) => handler,
            None => {
                let message = unsupported_tool_call_message(&invocation.payload, &tool_name);
                let log_payload = invocation.payload.log_payload();
                otel.tool_result_with_tags(
                    tool_name_flat.as_ref(),
                    &call_id_owned,
                    log_payload.as_ref(),
                    Duration::ZERO,
                    /*success*/ false,
                    &message,
                    &base_tool_result_tags,
                    /*extra_trace_fields*/ &[],
                );
                let err = FunctionCallError::RespondToModel(message);
                dispatch_trace.record_failed(&err);
                return Err(err);
            }
        };

        let telemetry_tags = handler.telemetry_tags(&invocation).await;
        let mut tool_result_tags =
            Vec::with_capacity(base_tool_result_tags.len() + telemetry_tags.len());
        let mut extra_trace_fields = Vec::new();
        tool_result_tags.extend_from_slice(&base_tool_result_tags);
        for (key, value) in &telemetry_tags {
            if matches!(*key, "mcp_server" | "mcp_server_origin") {
                extra_trace_fields.push((*key, value.as_str()));
            } else {
                tool_result_tags.push((*key, value.as_str()));
            }
        }
        if !handler.matches_kind(&invocation.payload) {
            let message = format!("tool {tool_name} invoked with incompatible payload");
            let log_payload = invocation.payload.log_payload();
            otel.tool_result_with_tags(
                tool_name_flat.as_ref(),
                &call_id_owned,
                log_payload.as_ref(),
                Duration::ZERO,
                /*success*/ false,
                &message,
                &tool_result_tags,
                &extra_trace_fields,
            );
            let err = FunctionCallError::Fatal(message);
            dispatch_trace.record_failed(&err);
            return Err(err);
        }

        if let Some(pre_tool_use_payload) = handler.pre_tool_use_payload(&invocation) {
            match run_pre_tool_use_hooks(
                &invocation.session,
                &invocation.turn,
                invocation.call_id.clone(),
                &pre_tool_use_payload.tool_name,
                &pre_tool_use_payload.tool_input,
            )
            .await
            {
                PreToolUseHookResult::Blocked(message) => {
                    let err = FunctionCallError::RespondToModel(message);
                    dispatch_trace.record_failed(&err);
                    return Err(err);
                }
                PreToolUseHookResult::Continue {
                    updated_input: Some(updated_input),
                } => {
                    invocation = handler.with_updated_hook_input(invocation, updated_input)?;
                }
                PreToolUseHookResult::Continue {
                    updated_input: None,
                } => {}
            }
        }

        let response_cell = tokio::sync::Mutex::new(None);
        let invocation_for_tool = invocation.clone();
        let log_payload = invocation.payload.log_payload();

        let result = otel
            .log_tool_result_with_tags(
                tool_name_flat.as_ref(),
                &call_id_owned,
                log_payload.as_ref(),
                &tool_result_tags,
                &extra_trace_fields,
                || {
                    let handler = handler.clone();
                    let response_cell = &response_cell;
                    async move {
                        match handler.handle_any(invocation_for_tool).await {
                            Ok(result) => {
                                let preview = result.result.log_preview();
                                let success = result.result.success_for_logging();
                                let mut guard = response_cell.lock().await;
                                *guard = Some(result);
                                Ok((preview, success))
                            }
                            Err(err) => Err(err),
                        }
                    }
                },
            )
            .await;
        let success = match &result {
            Ok((_, success)) => *success,
            Err(_) => false,
        };
        emit_metric_for_tool_read(&invocation, success).await;
        let post_tool_use_payload = if success {
            let guard = response_cell.lock().await;
            guard
                .as_ref()
                .and_then(|result| result.post_tool_use_payload.clone())
        } else {
            None
        };
        let post_tool_use_outcome = if let Some(post_tool_use_payload) = post_tool_use_payload {
            Some(
                run_post_tool_use_hooks(
                    &invocation.session,
                    &invocation.turn,
                    post_tool_use_payload.tool_use_id,
                    post_tool_use_payload.tool_name.name().to_string(),
                    post_tool_use_payload.tool_name.matcher_aliases().to_vec(),
                    post_tool_use_payload.tool_input,
                    post_tool_use_payload.tool_response,
                )
                .await,
            )
        } else {
            None
        };

        if let Some(outcome) = &post_tool_use_outcome {
            record_additional_contexts(
                &invocation.session,
                &invocation.turn,
                outcome.additional_contexts.clone(),
            )
            .await;
            let replacement_text = if outcome.should_stop {
                Some(
                    outcome
                        .feedback_message
                        .clone()
                        .or_else(|| outcome.stop_reason.clone())
                        .unwrap_or_else(|| "PostToolUse hook stopped execution".to_string()),
                )
            } else {
                outcome.feedback_message.clone()
            };
            if let Some(replacement_text) = replacement_text {
                let mut guard = response_cell.lock().await;
                if let Some(result) = guard.as_mut() {
                    result.result = Box::new(FunctionToolOutput::from_text(
                        replacement_text,
                        /*success*/ None,
                    ));
                }
            }
        }

        if let Err(err) = invocation
            .session
            .goal_runtime_apply(GoalRuntimeEvent::ToolCompleted {
                turn_context: invocation.turn.as_ref(),
                tool_name: tool_name.name.as_str(),
            })
            .await
        {
            warn!("failed to account thread goal progress after tool call: {err}");
        }

        match result {
            Ok(_) => {
                let mut guard = response_cell.lock().await;
                let result = guard.take().ok_or_else(|| {
                    FunctionCallError::Fatal("tool produced no output".to_string())
                })?;
                dispatch_trace.record_completed(
                    &invocation,
                    &result.call_id,
                    &result.payload,
                    result.result.as_ref(),
                );
                Ok(result)
            }
            Err(err) => {
                dispatch_trace.record_failed(&err);
                Err(err)
            }
        }
    }
}

pub struct ToolRegistryBuilder {
    handlers: HashMap<ToolName, Arc<dyn RegisteredTool>>,
    specs: Vec<ToolSpec>,
}

impl ToolRegistryBuilder {
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
            specs: Vec::new(),
        }
    }

    pub(crate) fn push_spec(&mut self, spec: ToolSpec) {
        self.specs.push(spec);
    }

    pub fn register_handler<H>(&mut self, handler: Arc<H>)
    where
        H: ToolHandler + 'static,
    {
        self.register_tool(handler);
    }

    pub(crate) fn register_tool(&mut self, handler: Arc<dyn RegisteredTool>) {
        self.register_tool_internal(handler, /*include_spec*/ true);
    }

    pub(crate) fn register_tool_without_spec(&mut self, handler: Arc<dyn RegisteredTool>) {
        self.register_tool_internal(handler, /*include_spec*/ false);
    }

    fn register_tool_internal(&mut self, handler: Arc<dyn RegisteredTool>, include_spec: bool) {
        let name = handler.tool_name();
        if self.handlers.contains_key(&name) {
            error_or_panic(format!("handler for tool {name} already registered"));
            return;
        }

        if include_spec
            && handler.exposure().is_direct()
            && let Some(spec) = handler.spec()
        {
            self.push_spec(spec);
        }

        self.handlers.insert(name, handler);
    }

    pub fn register_extension_tool_executor(&mut self, executor: Arc<dyn ExtensionToolExecutor>) {
        let tool_name = executor.tool_name();
        if self.handlers.contains_key(&tool_name) {
            warn!("Skipping extension tool `{tool_name}`: handler already registered");
            return;
        }

        let handler: Arc<dyn RegisteredTool> = Arc::new(ExtensionToolHandler::new(executor));
        self.register_tool_internal(handler, /*include_spec*/ true);
    }

    pub fn build(self) -> (Vec<ToolSpec>, ToolRegistry) {
        let registry = ToolRegistry::new(self.handlers);
        (self.specs, registry)
    }
}

fn unsupported_tool_call_message(payload: &ToolPayload, tool_name: &ToolName) -> String {
    match payload {
        ToolPayload::Custom { .. } => format!("unsupported custom tool call: {tool_name}"),
        _ => format!("unsupported call: {tool_name}"),
    }
}
#[cfg(test)]
#[path = "registry_tests.rs"]
mod tests;
