use std::collections::BTreeMap;
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
use crate::memory_usage::shell_script_for_invocation;
use crate::session::session::Session;
use crate::session::turn_context::TurnContext;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::control_tool_analytics::ControlToolCallGuard;
use crate::tools::flat_tool_name;
use crate::tools::handlers::multi_agents_spec::MULTI_AGENT_V1_NAMESPACE;
use crate::tools::hook_names::HookToolName;
use crate::tools::lifecycle::notify_tool_finish;
use crate::tools::lifecycle::notify_tool_start;
use crate::tools::router::tool_log_payload;
use crate::tools::tool_dispatch_trace::ToolDispatchTrace;
use crate::util::error_or_panic;
use codex_analytics::ControlToolCallStatus;
use codex_extension_api::AllowedTools;
use codex_extension_api::ToolCallOutcome;
use codex_history::CodexHarnessMetadata;
use codex_history::ResponseItemEnvelope;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::parse_command::ParsedCommand;
use codex_protocol::protocol::EventMsg;
use codex_rollout::state_db;
use codex_shell_command::parse_command::parse_shell_script;
use codex_tools::ToolName;
use codex_tools::ToolSpec;
use futures::future::BoxFuture;
use indexmap::IndexMap;
use indexmap::map::Entry;
use serde_json::Value;

pub(crate) type ToolTelemetryTags = Vec<(&'static str, String)>;

pub use codex_tools::ToolExecutor;
pub use codex_tools::ToolExposure;

/// Typed runtime contract for locally executed tools.
///
/// Implementers provide the shared `ToolExecutor` behavior plus optional
/// core-owned metadata for hooks, telemetry, tool search, and argument diffs.
pub(crate) trait CoreToolRuntime: ToolExecutor<ToolInvocation> {
    /// Whether this built-in control tool needs a structured tool-call event.
    fn is_builtin_control_tool(&self) -> bool {
        false
    }

    /// Returns a shared spec when both the spec and search metadata are immutable.
    fn immutable_spec(&self) -> Option<&Arc<ToolSpec>> {
        None
    }

    /// Returns lazily cached Code Mode definitions owned by this runtime.
    fn cached_code_mode_definitions(&self) -> Option<&[codex_code_mode::ToolDefinition]> {
        None
    }

    /// Returns a readiness wait for this exact tool before taking the execution gate.
    fn wait_until_ready<'a>(&'a self, _session: &'a Arc<Session>) -> Option<BoxFuture<'a, ()>> {
        None
    }

    /// Returns the owning server only for MCP-backed tool runtimes.
    fn mcp_server_name(&self) -> Option<&str> {
        None
    }

    fn matches_kind(&self, payload: &ToolPayload) -> bool {
        matches!(
            payload,
            ToolPayload::Function { .. } | ToolPayload::ToolSearch { .. }
        )
    }

    fn telemetry_tags(&self, _invocation: &ToolInvocation) -> ToolTelemetryTags {
        Vec::new()
    }

    /// Observes a tool result only after all PostToolUse hooks accept it.
    fn on_tool_result_accepted(&self, _invocation: &ToolInvocation, _result: &dyn ToolOutput) {}

    fn post_tool_use_payload(
        &self,
        invocation: &ToolInvocation,
        result: &dyn ToolOutput,
    ) -> Option<PostToolUsePayload> {
        let ToolPayload::Function { arguments } = &invocation.payload else {
            return None;
        };

        Some(PostToolUsePayload {
            tool_name: function_hook_tool_name(invocation),
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
            tool_name: function_hook_tool_name(invocation),
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
    pub(crate) post_tool_use_payload: Option<PostToolUsePayload>,
}

impl AnyToolResult {
    pub(crate) fn into_response(self) -> ResponseItemEnvelope {
        let Self {
            call_id,
            payload,
            result,
            ..
        } = self;
        ResponseItemEnvelope {
            item: result.to_response_item(&call_id, &payload).into(),
            metadata: result
                .fallback_token_limit_override()
                .map(|limit| CodexHarnessMetadata {
                    history_truncation_token_limit: Some(limit),
                    ..Default::default()
                }),
        }
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
    fn log_output(&self) -> String {
        self.original.log_output()
    }

    fn success_for_logging(&self) -> bool {
        self.original.success_for_logging()
    }

    fn fallback_token_limit_override(&self) -> Option<usize> {
        self.original.fallback_token_limit_override()
    }

    fn to_response_item(&self, call_id: &str, payload: &ToolPayload) -> ResponseInputItem {
        self.model_visible.to_response_item(call_id, payload)
    }

    fn code_mode_result(&self, payload: &ToolPayload) -> Value {
        self.original.code_mode_result(payload)
    }

    fn tool_result_metadata(&self) -> Option<&Value> {
        self.original.tool_result_metadata()
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

/// A tool runtime together with its effective exposure for the current step.
pub(crate) struct RegisteredTool {
    pub(crate) runtime: Arc<dyn CoreToolRuntime>,
    pub(crate) exposure: ToolExposure,
}

#[derive(Default)]
pub struct ToolRegistry {
    tools: IndexMap<ToolName, RegisteredTool>,
    first_collision: Option<ToolName>,
    pub(crate) allowed_tools: Option<Arc<AllowedTools>>,
}

impl ToolRegistry {
    pub(crate) fn with_allowed_tools(allowed_tools: Option<Arc<AllowedTools>>) -> Self {
        Self {
            allowed_tools,
            ..Self::default()
        }
    }

    #[cfg(test)]
    pub(crate) fn from_tools(tools: impl IntoIterator<Item = Arc<dyn CoreToolRuntime>>) -> Self {
        let mut registry = Self::default();

        for runtime in tools {
            registry.register_trusted(runtime);
        }

        registry
    }

    pub(crate) fn add<T>(&mut self, handler: T)
    where
        T: CoreToolRuntime + 'static,
    {
        self.register_trusted(Arc::new(handler));
    }

    pub(crate) fn add_with_exposure<T>(&mut self, handler: T, exposure: ToolExposure)
    where
        T: CoreToolRuntime + 'static,
    {
        self.register_trusted_with_exposure(Arc::new(handler), exposure);
    }

    pub(crate) fn register_trusted(&mut self, runtime: Arc<dyn CoreToolRuntime>) {
        let exposure = runtime.exposure();
        self.register_trusted_with_exposure(runtime, exposure);
    }

    pub(crate) fn register_trusted_with_exposure(
        &mut self,
        runtime: Arc<dyn CoreToolRuntime>,
        exposure: ToolExposure,
    ) {
        let tool_name = runtime.tool_name().with_default_namespace();
        if self
            .allowed_tools
            .as_ref()
            .is_some_and(|allowed| !allowed.contains(&tool_name))
        {
            return;
        }
        match self.tools.entry(tool_name) {
            Entry::Vacant(entry) => {
                entry.insert(RegisteredTool { runtime, exposure });
            }
            Entry::Occupied(entry) => {
                let tool_name = entry.key();
                error_or_panic(format!("tool {tool_name} already registered"));
            }
        }
    }

    pub(crate) fn prepend_trusted(&mut self, runtime: Arc<dyn CoreToolRuntime>) {
        let tool_name = runtime.tool_name().with_default_namespace();
        if self
            .allowed_tools
            .as_ref()
            .is_some_and(|allowed| !allowed.contains(&tool_name))
        {
            return;
        }
        if self.tools.contains_key(&tool_name) {
            error_or_panic(format!("tool {tool_name} already registered"));
            return;
        }

        let exposure = runtime.exposure();
        self.tools
            .shift_insert(0, tool_name, RegisteredTool { runtime, exposure });
    }

    pub(crate) fn register_external(&mut self, runtime: Arc<dyn CoreToolRuntime>) -> bool {
        let exposure = runtime.exposure();
        self.register_external_with_exposure(runtime, exposure)
    }

    pub(crate) fn register_external_with_exposure(
        &mut self,
        runtime: Arc<dyn CoreToolRuntime>,
        exposure: ToolExposure,
    ) -> bool {
        let tool_name = runtime.tool_name().with_default_namespace();
        if self
            .allowed_tools
            .as_ref()
            .is_some_and(|allowed| !allowed.contains(&tool_name))
        {
            return false;
        }
        if tool_name.is_default_namespace()
            && matches!(tool_name.name.as_str(), "exec_command" | "shell_command")
        {
            tracing::warn!(tool_name = %tool_name, "skipping external tool with reserved name");
            if self.tools.contains_key(&tool_name) {
                self.record_collision(tool_name);
            }
            return false;
        }

        match self.tools.entry(tool_name) {
            Entry::Vacant(entry) => {
                entry.insert(RegisteredTool { runtime, exposure });
                true
            }
            Entry::Occupied(entry) => {
                tracing::warn!(
                    tool_name = %entry.key(),
                    "skipping duplicate external tool that is already registered"
                );
                self.first_collision
                    .get_or_insert_with(|| entry.key().clone());
                false
            }
        }
    }

    pub(crate) fn record_collision(&mut self, tool_name: ToolName) {
        self.first_collision.get_or_insert(tool_name);
    }

    pub(crate) fn first_collision(&self) -> Option<&ToolName> {
        self.first_collision.as_ref()
    }

    pub(crate) fn remove(&mut self, tool_name: &ToolName) -> Option<Arc<dyn CoreToolRuntime>> {
        self.tools
            .shift_remove(&tool_name.clone().with_default_namespace())
            .map(|tool| tool.runtime)
    }

    pub(crate) fn entries(&self) -> impl Iterator<Item = &RegisteredTool> {
        self.tools.values()
    }

    pub(crate) fn entries_mut(&mut self) -> impl Iterator<Item = &mut RegisteredTool> {
        self.tools.values_mut()
    }

    pub(crate) fn deferred_tool_namespaces(&self) -> BTreeMap<String, String> {
        let mut namespaces = BTreeMap::<String, String>::new();
        for (name, tool) in &self.tools {
            if !tool.exposure.is_deferred() || name.is_default_namespace() {
                continue;
            }
            let Some(namespace) = &name.namespace else {
                continue;
            };
            let existing_description = namespaces.entry(namespace.clone()).or_default();
            if !existing_description.trim().is_empty() {
                continue;
            }
            let owned_spec;
            let spec = if let Some(spec) = tool.runtime.immutable_spec() {
                spec.as_ref()
            } else {
                owned_spec = tool.runtime.spec();
                &owned_spec
            };
            let description = match spec {
                ToolSpec::Namespace(namespace) => namespace.description.as_str(),
                ToolSpec::Function(_)
                | ToolSpec::Freeform(_)
                | ToolSpec::ToolSearch { .. }
                | ToolSpec::WebSearch { .. } => "",
            };
            if !description.trim().is_empty() {
                *existing_description = description.to_string();
            }
        }
        namespaces
    }

    #[cfg(test)]
    pub(crate) fn empty_for_test() -> Self {
        Self::from_tools(std::iter::empty())
    }

    #[cfg(test)]
    pub(crate) fn with_handler_for_test<T>(handler: Arc<T>) -> Self
    where
        T: CoreToolRuntime + 'static,
    {
        Self::from_tools([handler as Arc<dyn CoreToolRuntime>])
    }

    pub(crate) fn tool(&self, name: &ToolName) -> Option<Arc<dyn CoreToolRuntime>> {
        self.tools
            .get(&name.clone().with_default_namespace())
            .map(|tool| Arc::clone(&tool.runtime))
    }

    #[cfg(test)]
    pub(crate) fn tool_names_for_test(&self) -> Vec<ToolName> {
        let mut names = self.tools.keys().cloned().collect::<Vec<_>>();
        names.sort();
        names
    }

    #[cfg(test)]
    pub(crate) fn tool_exposure(&self, name: &ToolName) -> Option<ToolExposure> {
        self.tools
            .get(&name.clone().with_default_namespace())
            .map(|tool| tool.exposure)
    }

    pub(crate) fn create_diff_consumer(
        &self,
        name: &ToolName,
    ) -> Option<Box<dyn ToolArgumentDiffConsumer>> {
        self.tool(name)?.create_diff_consumer()
    }

    pub(crate) fn supports_parallel_tool_calls(&self, name: &ToolName) -> Option<bool> {
        let tool = self.tools.get(&name.clone().with_default_namespace())?;
        Some(tool.exposure != ToolExposure::Hidden && tool.runtime.supports_parallel_tool_calls())
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
        let call_id_owned = invocation.call_id.clone();
        let otel = invocation.step_context.session_telemetry.clone();
        // TODO(anp): Reconcile these tags with TurnEnvironment::sandbox_context
        // instead of reporting the thread-wide backend for environment-scoped tools.
        let sandbox_tags = invocation.turn.turn_metadata_state.sandbox_tags;

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
                let log_payload = tool_log_payload(&invocation.payload, &invocation.source);
                let mut tool_result_tags = Vec::with_capacity(2);
                sandbox_tags.append_metric_tags(&mut tool_result_tags);
                otel.tool_result_with_tags(
                    &tool_name,
                    &call_id_owned,
                    log_payload.as_ref(),
                    Duration::ZERO,
                    /*success*/ false,
                    &message,
                    &tool_result_tags,
                    /*extra_trace_fields*/ &[],
                );
                let err = FunctionCallError::RespondToModel(message);
                dispatch_trace.record_failed(&err);
                return Err(err);
            }
        };
        let telemetry_tags = tool.telemetry_tags(&invocation);
        let mut tool_result_tags = Vec::with_capacity(2 + telemetry_tags.len() + 1);
        let mut extra_trace_fields = Vec::new();
        sandbox_tags.append_metric_tags(&mut tool_result_tags);
        for (key, value) in &telemetry_tags {
            if matches!(*key, "mcp_server" | "mcp_server_origin") {
                extra_trace_fields.push((*key, value.as_str()));
            } else {
                tool_result_tags.push((*key, value.as_str()));
            }
        }
        if !tool.matches_kind(&invocation.payload) {
            let message = format!("tool {tool_name} invoked with incompatible payload");
            let log_payload = tool_log_payload(&invocation.payload, &invocation.source);
            otel.tool_result_with_tags(
                &tool_name,
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

        if let Some(pre_tool_use_payload) = tool.pre_tool_use_payload(&invocation) {
            match run_pre_tool_use_hooks(
                &invocation.session,
                invocation.step_context.as_ref(),
                invocation.call_id.clone(),
                &pre_tool_use_payload.tool_name,
                &pre_tool_use_payload.tool_input,
            )
            .await
            {
                PreToolUseHookResult::Blocked(message) => {
                    if tool.is_builtin_control_tool() {
                        let mut analytics = ControlToolCallGuard::new(&invocation);
                        analytics.finish(ControlToolCallStatus::Rejected);
                    }
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
                        if tool.is_builtin_control_tool() {
                            let mut analytics = ControlToolCallGuard::new(&invocation);
                            analytics.finish(ControlToolCallStatus::Failed);
                        }
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

        if tool.mcp_server_name().is_none() {
            notify_tool_start(&invocation, /*mcp_tool*/ None).await;
        }
        let mut control_tool_analytics = tool
            .is_builtin_control_tool()
            .then(|| ControlToolCallGuard::new(&invocation));

        if let Some(command) = shell_script_for_invocation(&invocation) {
            let parsed = parse_shell_script(&command);
            let mut categories = parsed.iter().map(|command| match command {
                ParsedCommand::Read { .. } => "read",
                ParsedCommand::ListFiles { .. } => "list_files",
                ParsedCommand::Search { .. } => "search",
                ParsedCommand::Unknown { .. } => "unknown",
            });
            let category = match categories.next() {
                Some(first) if categories.all(|category| category == first) => first,
                Some(_) => "mixed",
                None => "unknown",
            };
            tool_result_tags.push(("command_category", category));
        }

        let log_payload = tool_log_payload(&invocation.payload, &invocation.source);

        let result = otel
            .log_tool_result_with_tags(
                &tool_name,
                &call_id_owned,
                log_payload.as_ref(),
                &tool_result_tags,
                &extra_trace_fields,
                || handle_any_tool(tool.as_ref(), invocation.clone()),
                |result| {
                    (
                        result.result.log_output(),
                        result.result.success_for_logging(),
                    )
                },
            )
            .await;
        let success = match &result {
            Ok(result) => result.result.success_for_logging(),
            Err(_) => false,
        };
        if let Some(analytics) = control_tool_analytics.as_mut() {
            analytics.finish(if success {
                ControlToolCallStatus::Completed
            } else {
                ControlToolCallStatus::Failed
            });
        }
        emit_metric_for_tool_read(&invocation, success);
        let post_tool_use_payload = if success {
            result
                .as_ref()
                .ok()
                .and_then(|result| result.post_tool_use_payload.clone())
        } else {
            None
        };
        let post_tool_use_outcome = if let Some(post_tool_use_payload) = post_tool_use_payload {
            Some(
                run_post_tool_use_hooks(
                    &invocation.session,
                    invocation.step_context.as_ref(),
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
        }

        // A PostToolUse block rejects the result, not the already-completed tool execution.
        let lifecycle_outcome = match &result {
            Ok(_) => ToolCallOutcome::Completed { success },
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
            Ok(mut result) => {
                if let Some(outcome) = post_tool_use_outcome {
                    if outcome.should_block {
                        let message = outcome.feedback_message.unwrap_or_else(|| {
                            "PostToolUse hook blocked the tool result".to_string()
                        });
                        let err = FunctionCallError::RespondToModel(message);
                        dispatch_trace.record_failed(&err);
                        return Err(err);
                    }
                    if let Some(feedback_message) = outcome.feedback_message {
                        result.result = Box::new(PostToolUseFeedbackOutput {
                            original: result.result,
                            model_visible: FunctionToolOutput::from_text(
                                feedback_message,
                                /*success*/ None,
                            ),
                        });
                    }
                }
                tool.on_tool_result_accepted(&invocation, result.result.as_ref());
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
    let post_tool_use_payload =
        CoreToolRuntime::post_tool_use_payload(tool, &invocation, output.as_ref());
    Ok(AnyToolResult {
        call_id,
        payload,
        result: output,
        post_tool_use_payload,
    })
}

fn function_hook_tool_name(invocation: &ToolInvocation) -> HookToolName {
    if invocation.tool_name.name == "spawn_agent"
        && (invocation.tool_name.is_default_namespace()
            || invocation.tool_name.namespace.as_deref() == Some(MULTI_AGENT_V1_NAMESPACE))
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
