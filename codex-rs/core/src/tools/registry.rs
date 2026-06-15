use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;

use crate::function_tool::FunctionCallError;
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
use crate::tools::handlers::multi_agents_spec::MULTI_AGENT_V1_NAMESPACE;
use crate::tools::hook_names::HookToolName;
use crate::tools::lifecycle::notify_tool_finish;
use crate::tools::lifecycle::notify_tool_start;
use crate::tools::tool_dispatch_trace::ToolDispatchTrace;
use crate::util::error_or_panic;
use codex_extension_api::ToolCallOutcome;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::protocol::EventMsg;
use codex_rollout::state_db;
use codex_tools::ToolName;
use codex_tools::ToolSearchInfo;
use codex_tools::ToolSpec;
use futures::future::BoxFuture;
use serde_json::Value;

pub(crate) type ToolTelemetryTags = Vec<(&'static str, String)>;

pub use codex_tools::ToolExecutor;
pub use codex_tools::ToolExposure;

/// Typed runtime contract for locally executed tools.
///
/// Implementers provide the shared `ToolExecutor` behavior plus optional
/// core-owned metadata for hooks, telemetry, tool search, and argument diffs.
pub(crate) trait CoreToolRuntime: ToolExecutor<ToolInvocation> {
    fn matches_kind(&self, payload: &ToolPayload) -> bool {
        matches!(
            payload,
            ToolPayload::Function { .. } | ToolPayload::ToolSearch { .. }
        )
    }

    /// Whether cancellation should let the handler finish teardown before the
    /// host returns an aborted tool response.
    fn waits_for_runtime_cancellation(&self) -> bool {
        false
    }

    fn telemetry_tags<'a>(
        &'a self,
        _invocation: &'a ToolInvocation,
    ) -> BoxFuture<'a, ToolTelemetryTags> {
        Box::pin(async { Vec::new() })
    }

    /// Returns the stable hook identity without constructing the hook payload.
    fn hook_tool_name(&self, invocation: &ToolInvocation) -> Option<HookToolName> {
        matches!(&invocation.payload, ToolPayload::Function { .. })
            .then(|| function_hook_tool_name(invocation))
    }

    fn post_tool_use_payload(
        &self,
        invocation: &ToolInvocation,
        result: &dyn ToolOutput,
    ) -> Option<PostToolUsePayload> {
        let ToolPayload::Function { arguments } = &invocation.payload else {
            return None;
        };

        Some(PostToolUsePayload {
            tool_name: self.hook_tool_name(invocation)?,
            tool_use_id: result.post_tool_use_id(&invocation.call_id),
            tool_input: result
                .post_tool_use_input(&invocation.payload)
                .unwrap_or_else(|| function_hook_tool_input(arguments)),
            tool_response: result
                .post_tool_use_response(&invocation.call_id, &invocation.payload)
                .or_else(|| {
                    // Most function tools can expose their model-facing output
                    // as the hook response. Outputs with a more stable hook
                    // contract should override post_tool_use_response above.
                    let ResponseInputItem::FunctionCallOutput {
                        output: FunctionCallOutputPayload { body, .. },
                        ..
                    } = result.to_response_item(&invocation.call_id, &invocation.payload)
                    else {
                        return None;
                    };

                    serde_json::to_value(body).ok()
                })?,
        })
    }

    fn pre_tool_use_payload(&self, invocation: &ToolInvocation) -> Option<PreToolUsePayload> {
        let ToolPayload::Function { arguments } = &invocation.payload else {
            return None;
        };

        Some(PreToolUsePayload {
            tool_name: self.hook_tool_name(invocation)?,
            tool_input: function_hook_tool_input(arguments),
        })
    }

    /// Rebuilds a tool invocation from hook-facing `tool_input`.
    ///
    /// Tools that opt into input-rewriting hooks should invert the same stable
    /// hook contract they expose from `pre_tool_use_payload`.
    fn with_updated_hook_input(
        &self,
        invocation: ToolInvocation,
        updated_input: Value,
    ) -> Result<ToolInvocation, FunctionCallError> {
        let ToolPayload::Function { .. } = &invocation.payload else {
            return Err(FunctionCallError::RespondToModel(
                "hook input rewrite received unsupported function tool payload".to_string(),
            ));
        };

        let arguments = serde_json::to_string(&updated_input).map_err(|err| {
            FunctionCallError::RespondToModel(format!(
                "failed to serialize rewritten {} arguments: {err}",
                flat_tool_name(&invocation.tool_name)
            ))
        })?;
        Ok(ToolInvocation {
            payload: ToolPayload::Function { arguments },
            ..invocation
        })
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

struct PostToolUseFeedbackOutput {
    original: Box<dyn ToolOutput>,
    model_visible: FunctionToolOutput,
}

impl ToolOutput for PostToolUseFeedbackOutput {
    fn log_preview(&self) -> String {
        self.original.log_preview()
    }

    fn success_for_logging(&self) -> bool {
        self.original.success_for_logging()
    }

    fn to_response_item(&self, call_id: &str, payload: &ToolPayload) -> ResponseInputItem {
        self.model_visible.to_response_item(call_id, payload)
    }

    fn code_mode_result(&self, payload: &ToolPayload) -> Value {
        self.original.code_mode_result(payload)
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

pub(crate) fn override_tool_exposure(
    handler: Arc<dyn CoreToolRuntime>,
    exposure: ToolExposure,
) -> Arc<dyn CoreToolRuntime> {
    if handler.exposure() == exposure {
        return handler;
    }

    Arc::new(ExposureOverride { handler, exposure })
}

struct ExposureOverride {
    handler: Arc<dyn CoreToolRuntime>,
    exposure: ToolExposure,
}

impl ToolExecutor<ToolInvocation> for ExposureOverride {
    fn tool_name(&self) -> ToolName {
        self.handler.tool_name()
    }

    fn spec(&self) -> ToolSpec {
        self.handler.spec()
    }

    fn exposure(&self) -> ToolExposure {
        self.exposure
    }

    fn supports_parallel_tool_calls(&self) -> bool {
        self.exposure != ToolExposure::Hidden && self.handler.supports_parallel_tool_calls()
    }

    fn search_info(&self) -> Option<ToolSearchInfo> {
        self.handler.search_info()
    }

    fn handle(&self, invocation: ToolInvocation) -> codex_tools::ToolExecutorFuture<'_> {
        self.handler.handle(invocation)
    }
}

impl CoreToolRuntime for ExposureOverride {
    fn matches_kind(&self, payload: &ToolPayload) -> bool {
        self.handler.matches_kind(payload)
    }

    fn waits_for_runtime_cancellation(&self) -> bool {
        self.handler.waits_for_runtime_cancellation()
    }

    fn hook_tool_name(&self, invocation: &ToolInvocation) -> Option<HookToolName> {
        self.handler.hook_tool_name(invocation)
    }

    fn pre_tool_use_payload(&self, invocation: &ToolInvocation) -> Option<PreToolUsePayload> {
        self.handler.pre_tool_use_payload(invocation)
    }

    fn post_tool_use_payload(
        &self,
        invocation: &ToolInvocation,
        result: &dyn ToolOutput,
    ) -> Option<PostToolUsePayload> {
        self.handler.post_tool_use_payload(invocation, result)
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
}

pub struct ToolRegistry {
    tools: HashMap<ToolName, Arc<dyn CoreToolRuntime>>,
}

impl ToolRegistry {
    fn new(tools: HashMap<ToolName, Arc<dyn CoreToolRuntime>>) -> Self {
        Self { tools }
    }

    pub(crate) fn from_tools(tools: impl IntoIterator<Item = Arc<dyn CoreToolRuntime>>) -> Self {
        let mut tools_by_name = HashMap::new();
        for tool in tools {
            let name = tool.tool_name();
            if tools_by_name.contains_key(&name) {
                error_or_panic(format!("tool {name} already registered"));
                continue;
            }
            tools_by_name.insert(name, tool);
        }
        Self::new(tools_by_name)
    }

    #[cfg(test)]
    pub(crate) fn empty_for_test() -> Self {
        Self::new(HashMap::new())
    }

    #[cfg(test)]
    pub(crate) fn with_handler_for_test<T>(handler: Arc<T>) -> Self
    where
        T: CoreToolRuntime + 'static,
    {
        let name = handler.tool_name();
        Self::new(HashMap::from([(name, handler as Arc<dyn CoreToolRuntime>)]))
    }

    fn tool(&self, name: &ToolName) -> Option<Arc<dyn CoreToolRuntime>> {
        self.tools.get(name).map(Arc::clone)
    }

    #[cfg(test)]
    pub(crate) fn tool_names_for_test(&self) -> Vec<ToolName> {
        let mut names = self.tools.keys().cloned().collect::<Vec<_>>();
        names.sort();
        names
    }

    #[cfg(test)]
    pub(crate) fn tool_exposure(&self, name: &ToolName) -> Option<ToolExposure> {
        self.tools.get(name).map(|tool| tool.exposure())
    }

    pub(crate) fn create_diff_consumer(
        &self,
        name: &ToolName,
    ) -> Option<Box<dyn ToolArgumentDiffConsumer>> {
        self.tool(name)?.create_diff_consumer()
    }

    pub(crate) fn supports_parallel_tool_calls(&self, name: &ToolName) -> Option<bool> {
        let tool = self.tool(name)?;
        Some(tool.supports_parallel_tool_calls())
    }

    pub(crate) fn waits_for_runtime_cancellation(&self, name: &ToolName) -> Option<bool> {
        let tool = self.tool(name)?;
        Some(tool.waits_for_runtime_cancellation())
    }

    #[allow(dead_code)]
    pub(crate) async fn dispatch_any(
        &self,
        invocation: ToolInvocation,
    ) -> Result<AnyToolResult, FunctionCallError> {
        self.dispatch_any_with_terminal_outcome(invocation, /*terminal_outcome_reached*/ None)
            .await
    }

    #[expect(
        clippy::await_holding_invalid_type,
        reason = "tool dispatch must keep active-turn accounting atomic"
    )]
    pub(crate) async fn dispatch_any_with_terminal_outcome(
        &self,
        mut invocation: ToolInvocation,
        terminal_outcome_reached: Option<Arc<AtomicBool>>,
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
        let tool = match self.tool(&tool_name) {
            Some(tool) => tool,
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

        let telemetry_tags = tool.telemetry_tags(&invocation).await;
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
        if !tool.matches_kind(&invocation.payload) {
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

        notify_tool_start(&invocation).await;

        let pre_tool_use_hooks = invocation.session.hooks();
        let has_matching_pre_tool_use = tool.hook_tool_name(&invocation).is_some_and(|tool_name| {
            pre_tool_use_hooks
                .has_matching_pre_tool_use(tool_name.name(), tool_name.matcher_aliases())
        });
        if has_matching_pre_tool_use
            && let Some(pre_tool_use_payload) = tool.pre_tool_use_payload(&invocation)
        {
            match run_pre_tool_use_hooks(
                &invocation.session,
                &invocation.turn,
                pre_tool_use_hooks.as_ref(),
                invocation.call_id.clone(),
                &pre_tool_use_payload.tool_name,
                &pre_tool_use_payload.tool_input,
            )
            .await
            {
                PreToolUseHookResult::Blocked(message) => {
                    let err = FunctionCallError::RespondToModel(message);
                    dispatch_trace.record_failed(&err);
                    notify_tool_finish_if_unclaimed(
                        &invocation,
                        terminal_outcome_reached.as_deref(),
                        ToolCallOutcome::Blocked,
                    )
                    .await;
                    return Err(err);
                }
                PreToolUseHookResult::Continue {
                    updated_input: Some(updated_input),
                } => match tool.with_updated_hook_input(invocation.clone(), updated_input) {
                    Ok(updated_invocation) => {
                        invocation = updated_invocation;
                    }
                    Err(err) => {
                        dispatch_trace.record_failed(&err);
                        notify_tool_finish_if_unclaimed(
                            &invocation,
                            terminal_outcome_reached.as_deref(),
                            ToolCallOutcome::Failed {
                                handler_executed: false,
                            },
                        )
                        .await;
                        return Err(err);
                    }
                },
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
                    let tool = tool.clone();
                    let response_cell = &response_cell;
                    async move {
                        match handle_any_tool(tool.as_ref(), invocation_for_tool).await {
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
        let post_tool_use_hooks = invocation.session.hooks();
        let has_matching_post_tool_use =
            tool.hook_tool_name(&invocation).is_some_and(|tool_name| {
                post_tool_use_hooks
                    .has_matching_post_tool_use(tool_name.name(), tool_name.matcher_aliases())
            });
        let post_tool_use_payload = if success && has_matching_post_tool_use {
            let guard = response_cell.lock().await;
            guard
                .as_ref()
                .and_then(|result| tool.post_tool_use_payload(&invocation, result.result.as_ref()))
        } else {
            None
        };
        let post_tool_use_outcome = if let Some(post_tool_use_payload) = post_tool_use_payload {
            Some(
                run_post_tool_use_hooks(
                    &invocation.session,
                    &invocation.turn,
                    post_tool_use_hooks.as_ref(),
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
                if let Some(mut result) = guard.take() {
                    result.result = Box::new(PostToolUseFeedbackOutput {
                        original: result.result,
                        model_visible: FunctionToolOutput::from_text(
                            replacement_text,
                            /*success*/ None,
                        ),
                    });
                    *guard = Some(result);
                }
            }
        }

        let lifecycle_outcome = match &result {
            Ok(_) => {
                let guard = response_cell.lock().await;
                match guard.as_ref() {
                    Some(result) => ToolCallOutcome::Completed {
                        success: result.result.success_for_logging(),
                    },
                    None => ToolCallOutcome::Failed {
                        handler_executed: true,
                    },
                }
            }
            Err(_) => ToolCallOutcome::Failed {
                handler_executed: true,
            },
        };
        notify_tool_finish_if_unclaimed(
            &invocation,
            terminal_outcome_reached.as_deref(),
            lifecycle_outcome,
        )
        .await;

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

async fn notify_tool_finish_if_unclaimed(
    invocation: &ToolInvocation,
    terminal_outcome_reached: Option<&AtomicBool>,
    outcome: ToolCallOutcome,
) -> bool {
    if terminal_outcome_reached.is_some_and(|reached| reached.swap(true, Ordering::AcqRel)) {
        return false;
    }

    notify_tool_finish(invocation, outcome).await;
    true
}

async fn handle_any_tool(
    tool: &dyn CoreToolRuntime,
    invocation: ToolInvocation,
) -> Result<AnyToolResult, FunctionCallError> {
    let call_id = invocation.call_id.clone();
    let payload = invocation.payload.clone();
    let output = tool.handle(invocation.clone()).await?;
    if output.contains_external_context()
        && invocation.turn.config.memories.disable_on_external_context
    {
        state_db::mark_thread_memory_mode_polluted(
            invocation.session.services.state_db.as_deref(),
            invocation.session.thread_id,
            "tool_output",
        )
        .await;
    }
    Ok(AnyToolResult {
        call_id,
        payload,
        result: output,
    })
}

fn function_hook_tool_name(invocation: &ToolInvocation) -> HookToolName {
    if invocation.tool_name.name == "spawn_agent"
        && matches!(
            invocation.tool_name.namespace.as_deref(),
            None | Some(MULTI_AGENT_V1_NAMESPACE)
        )
    {
        return HookToolName::spawn_agent();
    }

    HookToolName::new(flat_tool_name(&invocation.tool_name).into_owned())
}

fn function_hook_tool_input(arguments: &str) -> Value {
    if arguments.trim().is_empty() {
        return Value::Object(serde_json::Map::new());
    }

    serde_json::from_str(arguments).unwrap_or_else(|_| Value::String(arguments.to_string()))
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
