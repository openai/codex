use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use crate::client_common::tools::ToolSpec;
use crate::features::Feature;
use crate::function_tool::FunctionCallError;
use crate::memories::usage::emit_metric_for_tool_read;
use crate::protocol::SandboxPolicy;
use crate::sandbox_tags::sandbox_tag;
use crate::tools::code_mode::PUBLIC_TOOL_NAME;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::SEARCH_TOOL_BM25_TOOL_NAME;
use async_trait::async_trait;
use codex_hooks::HookEvent;
use codex_hooks::HookEventAfterToolUse;
use codex_hooks::HookPayload;
use codex_hooks::HookResult;
use codex_hooks::HookToolInput;
use codex_hooks::HookToolInputLocalShell;
use codex_hooks::HookToolKind;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::models::VIEW_IMAGE_TOOL_NAME;
use codex_utils_readiness::Readiness;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;
use tracing::warn;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, EnumIter)]
pub(crate) enum BuiltinToolKey {
    CodeMode,
    ExecCommand,
    WriteStdin,
    ListMcpResources,
    ListMcpResourceTemplates,
    ReadMcpResource,
    UpdatePlan,
    JsRepl,
    JsReplReset,
    RequestUserInput,
    RequestPermissions,
    SearchToolBm25,
    ApplyPatch,
    GrepFiles,
    ReadFile,
    ListDir,
    TestSyncTool,
    WebSearch,
    ImageGeneration,
    ViewImage,
    Artifacts,
    SpawnAgent,
    SendInput,
    ResumeAgent,
    Wait,
    CloseAgent,
    SpawnAgentsOnCsv,
    ReportAgentJobResult,
}

impl BuiltinToolKey {
    pub(crate) const fn invocation_names(self) -> &'static [&'static str] {
        match self {
            Self::CodeMode => &[PUBLIC_TOOL_NAME],
            Self::ExecCommand => &[
                "exec_command",
                "shell",
                "container.exec",
                "local_shell",
                "shell_command",
            ],
            Self::WriteStdin => &["write_stdin"],
            Self::ListMcpResources => &["list_mcp_resources"],
            Self::ListMcpResourceTemplates => &["list_mcp_resource_templates"],
            Self::ReadMcpResource => &["read_mcp_resource"],
            Self::UpdatePlan => &["update_plan"],
            Self::JsRepl => &["js_repl"],
            Self::JsReplReset => &["js_repl_reset"],
            Self::RequestUserInput => &["request_user_input"],
            Self::RequestPermissions => &["request_permissions"],
            Self::SearchToolBm25 => &[SEARCH_TOOL_BM25_TOOL_NAME],
            Self::ApplyPatch => &["apply_patch"],
            Self::GrepFiles => &["grep_files"],
            Self::ReadFile => &["read_file"],
            Self::ListDir => &["list_dir"],
            Self::TestSyncTool => &["test_sync_tool"],
            Self::WebSearch => &["web_search"],
            Self::ImageGeneration => &["image_generation"],
            Self::ViewImage => &[VIEW_IMAGE_TOOL_NAME],
            Self::Artifacts => &["artifacts"],
            Self::SpawnAgent => &["spawn_agent"],
            Self::SendInput => &["send_input"],
            Self::ResumeAgent => &["resume_agent"],
            Self::Wait => &["wait"],
            Self::CloseAgent => &["close_agent"],
            Self::SpawnAgentsOnCsv => &["spawn_agents_on_csv"],
            Self::ReportAgentJobResult => &["report_agent_job_result"],
        }
    }
}

pub(crate) fn builtin_tool_key(name: &str) -> Option<BuiltinToolKey> {
    BuiltinToolKey::iter().find(|key| key.invocation_names().contains(&name))
}

pub(crate) fn builtin_tool_invocation_names() -> impl Iterator<Item = &'static str> {
    BuiltinToolKey::iter().flat_map(|key| key.invocation_names().iter().copied())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ToolKind {
    Function,
    Mcp,
}

#[async_trait]
pub trait ToolHandler: Send + Sync {
    type Output: ToolOutput + 'static;

    fn kind(&self) -> ToolKind;

    fn matches_kind(&self, payload: &ToolPayload) -> bool {
        matches!(
            (self.kind(), payload),
            (ToolKind::Function, ToolPayload::Function { .. })
                | (ToolKind::Mcp, ToolPayload::Mcp { .. })
        )
    }

    /// Returns `true` if the [ToolInvocation] *might* mutate the environment of the
    /// user (through file system, OS operations, ...).
    /// This function must remains defensive and return `true` if a doubt exist on the
    /// exact effect of a ToolInvocation.
    async fn is_mutating(&self, _invocation: &ToolInvocation) -> bool {
        false
    }

    /// Perform the actual [ToolInvocation] and returns a [ToolOutput] containing
    /// the final output to return to the model.
    async fn handle(&self, invocation: ToolInvocation) -> Result<Self::Output, FunctionCallError>;
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

#[async_trait]
trait AnyToolHandler: Send + Sync {
    fn matches_kind(&self, payload: &ToolPayload) -> bool;

    async fn is_mutating(&self, invocation: &ToolInvocation) -> bool;

    async fn handle_any(
        &self,
        invocation: ToolInvocation,
    ) -> Result<AnyToolResult, FunctionCallError>;
}

#[async_trait]
impl<T> AnyToolHandler for T
where
    T: ToolHandler,
{
    fn matches_kind(&self, payload: &ToolPayload) -> bool {
        ToolHandler::matches_kind(self, payload)
    }

    async fn is_mutating(&self, invocation: &ToolInvocation) -> bool {
        ToolHandler::is_mutating(self, invocation).await
    }

    async fn handle_any(
        &self,
        invocation: ToolInvocation,
    ) -> Result<AnyToolResult, FunctionCallError> {
        let call_id = invocation.call_id.clone();
        let payload = invocation.payload.clone();
        let output = self.handle(invocation).await?;
        Ok(AnyToolResult {
            call_id,
            payload,
            result: Box::new(output),
        })
    }
}

pub struct ToolRegistry {
    handlers: HashMap<String, Arc<dyn AnyToolHandler>>,
    builtin_handler_names: std::collections::HashSet<String>,
}

impl ToolRegistry {
    fn new(
        handlers: HashMap<String, Arc<dyn AnyToolHandler>>,
        builtin_handler_names: std::collections::HashSet<String>,
    ) -> Self {
        Self {
            handlers,
            builtin_handler_names,
        }
    }

    fn handler(&self, name: &str) -> Option<Arc<dyn AnyToolHandler>> {
        self.handlers.get(name).map(Arc::clone)
    }

    // TODO(jif) for dynamic tools.
    // pub fn register(&mut self, name: impl Into<String>, handler: Arc<dyn ToolHandler>) {
    //     let name = name.into();
    //     if self.handlers.insert(name.clone(), handler).is_some() {
    //         warn!("overwriting handler for tool {name}");
    //     }
    // }

    pub(crate) async fn dispatch_any(
        &self,
        invocation: ToolInvocation,
    ) -> Result<AnyToolResult, FunctionCallError> {
        let tool_name = invocation.tool_name.clone();
        if self.builtin_handler_names.contains(tool_name.as_str())
            && !invocation
                .turn
                .tools_config
                .builtin_tool_enabled(tool_name.as_str())
        {
            return Err(FunctionCallError::RespondToModel(format!(
                "unsupported call: {tool_name}"
            )));
        }
        let call_id_owned = invocation.call_id.clone();
        let otel = invocation.turn.session_telemetry.clone();
        let payload_for_response = invocation.payload.clone();
        let log_payload = payload_for_response.log_payload();
        let metric_tags = [
            (
                "sandbox",
                sandbox_tag(
                    &invocation.turn.sandbox_policy,
                    invocation.turn.windows_sandbox_level,
                    invocation
                        .turn
                        .features
                        .enabled(Feature::UseLinuxSandboxBwrap),
                ),
            ),
            (
                "sandbox_policy",
                sandbox_policy_tag(&invocation.turn.sandbox_policy),
            ),
        ];
        let (mcp_server, mcp_server_origin) = match &invocation.payload {
            ToolPayload::Mcp { server, .. } => {
                let manager = invocation
                    .session
                    .services
                    .mcp_connection_manager
                    .read()
                    .await;
                let origin = manager.server_origin(server).map(str::to_owned);
                (Some(server.clone()), origin)
            }
            _ => (None, None),
        };
        let mcp_server_ref = mcp_server.as_deref();
        let mcp_server_origin_ref = mcp_server_origin.as_deref();

        {
            let mut active = invocation.session.active_turn.lock().await;
            if let Some(active_turn) = active.as_mut() {
                let mut turn_state = active_turn.turn_state.lock().await;
                turn_state.tool_calls = turn_state.tool_calls.saturating_add(1);
            }
        }

        let handler = match self.handler(tool_name.as_ref()) {
            Some(handler) => handler,
            None => {
                let message =
                    unsupported_tool_call_message(&invocation.payload, tool_name.as_ref());
                otel.tool_result_with_tags(
                    tool_name.as_ref(),
                    &call_id_owned,
                    log_payload.as_ref(),
                    Duration::ZERO,
                    false,
                    &message,
                    &metric_tags,
                    mcp_server_ref,
                    mcp_server_origin_ref,
                );
                return Err(FunctionCallError::RespondToModel(message));
            }
        };

        if !handler.matches_kind(&invocation.payload) {
            let message = format!("tool {tool_name} invoked with incompatible payload");
            otel.tool_result_with_tags(
                tool_name.as_ref(),
                &call_id_owned,
                log_payload.as_ref(),
                Duration::ZERO,
                false,
                &message,
                &metric_tags,
                mcp_server_ref,
                mcp_server_origin_ref,
            );
            return Err(FunctionCallError::Fatal(message));
        }

        let is_mutating = handler.is_mutating(&invocation).await;
        let response_cell = tokio::sync::Mutex::new(None);
        let invocation_for_tool = invocation.clone();

        let started = Instant::now();
        let result = otel
            .log_tool_result_with_tags(
                tool_name.as_ref(),
                &call_id_owned,
                log_payload.as_ref(),
                &metric_tags,
                mcp_server_ref,
                mcp_server_origin_ref,
                || {
                    let handler = handler.clone();
                    let response_cell = &response_cell;
                    async move {
                        if is_mutating {
                            tracing::trace!("waiting for tool gate");
                            invocation_for_tool.turn.tool_call_gate.wait_ready().await;
                            tracing::trace!("tool gate released");
                        }
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
        let duration = started.elapsed();
        let (output_preview, success) = match &result {
            Ok((preview, success)) => (preview.clone(), *success),
            Err(err) => (err.to_string(), false),
        };
        emit_metric_for_tool_read(&invocation, success).await;
        let hook_abort_error = dispatch_after_tool_use_hook(AfterToolUseHookDispatch {
            invocation: &invocation,
            output_preview,
            success,
            executed: true,
            duration,
            mutating: is_mutating,
        })
        .await;

        if let Some(err) = hook_abort_error {
            return Err(err);
        }

        match result {
            Ok(_) => {
                let mut guard = response_cell.lock().await;
                let result = guard.take().ok_or_else(|| {
                    FunctionCallError::Fatal("tool produced no output".to_string())
                })?;
                Ok(result)
            }
            Err(err) => Err(err),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConfiguredToolSpec {
    pub spec: ToolSpec,
    pub supports_parallel_tool_calls: bool,
    pub builtin: bool,
}

impl ConfiguredToolSpec {
    pub fn new(spec: ToolSpec, supports_parallel_tool_calls: bool, builtin: bool) -> Self {
        Self {
            spec,
            supports_parallel_tool_calls,
            builtin,
        }
    }
}

pub struct ToolRegistryBuilder {
    handlers: HashMap<String, Arc<dyn AnyToolHandler>>,
    builtin_handler_names: std::collections::HashSet<String>,
    specs: Vec<ConfiguredToolSpec>,
}

impl ToolRegistryBuilder {
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
            builtin_handler_names: std::collections::HashSet::new(),
            specs: Vec::new(),
        }
    }

    pub fn push_spec(&mut self, key: BuiltinToolKey, spec: ToolSpec) {
        self.push_spec_with_parallel_support(key, spec, false);
    }

    pub fn push_spec_with_parallel_support(
        &mut self,
        key: BuiltinToolKey,
        spec: ToolSpec,
        supports_parallel_tool_calls: bool,
    ) {
        debug_assert_eq!(builtin_tool_key(spec.name()), Some(key));
        self.specs.push(ConfiguredToolSpec::new(
            spec,
            supports_parallel_tool_calls,
            true,
        ));
    }

    pub fn push_external_spec(&mut self, spec: ToolSpec) {
        self.push_external_spec_with_parallel_support(spec, false);
    }

    pub fn push_external_spec_with_parallel_support(
        &mut self,
        spec: ToolSpec,
        supports_parallel_tool_calls: bool,
    ) {
        self.specs.push(ConfiguredToolSpec::new(
            spec,
            supports_parallel_tool_calls,
            false,
        ));
    }

    pub fn register_builtin_handler<H>(&mut self, key: BuiltinToolKey, handler: Arc<H>)
    where
        H: ToolHandler + 'static,
    {
        self.register_builtin_handler_for_names(key, key.invocation_names(), handler);
    }

    pub fn register_builtin_handler_for_names<H>(
        &mut self,
        key: BuiltinToolKey,
        names: &[&str],
        handler: Arc<H>,
    ) where
        H: ToolHandler + 'static,
    {
        for name in names {
            debug_assert_eq!(builtin_tool_key(name), Some(key));
            self.builtin_handler_names.insert((*name).to_string());
            self.register_external_handler(*name, handler.clone());
        }
    }

    pub fn register_external_handler<H>(&mut self, name: impl Into<String>, handler: Arc<H>)
    where
        H: ToolHandler + 'static,
    {
        let name = name.into();
        let handler: Arc<dyn AnyToolHandler> = handler;
        if self
            .handlers
            .insert(name.clone(), handler.clone())
            .is_some()
        {
            warn!("overwriting handler for tool {name}");
        }
    }

    // TODO(jif) for dynamic tools.
    // pub fn register_many<I>(&mut self, names: I, handler: Arc<dyn ToolHandler>)
    // where
    //     I: IntoIterator,
    //     I::Item: Into<String>,
    // {
    //     for name in names {
    //         let name = name.into();
    //         if self
    //             .handlers
    //             .insert(name.clone(), handler.clone())
    //             .is_some()
    //         {
    //             warn!("overwriting handler for tool {name}");
    //         }
    //     }
    // }

    pub fn build(self) -> (Vec<ConfiguredToolSpec>, ToolRegistry) {
        let registry = ToolRegistry::new(self.handlers, self.builtin_handler_names);
        (self.specs, registry)
    }
}

fn unsupported_tool_call_message(payload: &ToolPayload, tool_name: &str) -> String {
    match payload {
        ToolPayload::Custom { .. } => format!("unsupported custom tool call: {tool_name}"),
        _ => format!("unsupported call: {tool_name}"),
    }
}

fn sandbox_policy_tag(policy: &SandboxPolicy) -> &'static str {
    match policy {
        SandboxPolicy::ReadOnly { .. } => "read-only",
        SandboxPolicy::WorkspaceWrite { .. } => "workspace-write",
        SandboxPolicy::DangerFullAccess => "danger-full-access",
        SandboxPolicy::ExternalSandbox { .. } => "external-sandbox",
    }
}

// Hooks use a separate wire-facing input type so hook payload JSON stays stable
// and decoupled from core's internal tool runtime representation.
impl From<&ToolPayload> for HookToolInput {
    fn from(payload: &ToolPayload) -> Self {
        match payload {
            ToolPayload::Function { arguments } => HookToolInput::Function {
                arguments: arguments.clone(),
            },
            ToolPayload::Custom { input } => HookToolInput::Custom {
                input: input.clone(),
            },
            ToolPayload::LocalShell { params } => HookToolInput::LocalShell {
                params: HookToolInputLocalShell {
                    command: params.command.clone(),
                    workdir: params.workdir.clone(),
                    timeout_ms: params.timeout_ms,
                    sandbox_permissions: params.sandbox_permissions,
                    prefix_rule: params.prefix_rule.clone(),
                    justification: params.justification.clone(),
                },
            },
            ToolPayload::Mcp {
                server,
                tool,
                raw_arguments,
            } => HookToolInput::Mcp {
                server: server.clone(),
                tool: tool.clone(),
                arguments: raw_arguments.clone(),
            },
        }
    }
}

fn hook_tool_kind(tool_input: &HookToolInput) -> HookToolKind {
    match tool_input {
        HookToolInput::Function { .. } => HookToolKind::Function,
        HookToolInput::Custom { .. } => HookToolKind::Custom,
        HookToolInput::LocalShell { .. } => HookToolKind::LocalShell,
        HookToolInput::Mcp { .. } => HookToolKind::Mcp,
    }
}

struct AfterToolUseHookDispatch<'a> {
    invocation: &'a ToolInvocation,
    output_preview: String,
    success: bool,
    executed: bool,
    duration: Duration,
    mutating: bool,
}

async fn dispatch_after_tool_use_hook(
    dispatch: AfterToolUseHookDispatch<'_>,
) -> Option<FunctionCallError> {
    let AfterToolUseHookDispatch { invocation, .. } = dispatch;
    let session = invocation.session.as_ref();
    let turn = invocation.turn.as_ref();
    let tool_input = HookToolInput::from(&invocation.payload);
    let hook_outcomes = session
        .hooks()
        .dispatch(HookPayload {
            session_id: session.conversation_id,
            cwd: turn.cwd.clone(),
            client: turn.app_server_client_name.clone(),
            triggered_at: chrono::Utc::now(),
            hook_event: HookEvent::AfterToolUse {
                event: HookEventAfterToolUse {
                    turn_id: turn.sub_id.clone(),
                    call_id: invocation.call_id.clone(),
                    tool_name: invocation.tool_name.clone(),
                    tool_kind: hook_tool_kind(&tool_input),
                    tool_input,
                    executed: dispatch.executed,
                    success: dispatch.success,
                    duration_ms: u64::try_from(dispatch.duration.as_millis()).unwrap_or(u64::MAX),
                    mutating: dispatch.mutating,
                    sandbox: sandbox_tag(
                        &turn.sandbox_policy,
                        turn.windows_sandbox_level,
                        turn.features.enabled(Feature::UseLinuxSandboxBwrap),
                    )
                    .to_string(),
                    sandbox_policy: sandbox_policy_tag(&turn.sandbox_policy).to_string(),
                    output_preview: dispatch.output_preview.clone(),
                },
            },
        })
        .await;

    for hook_outcome in hook_outcomes {
        let hook_name = hook_outcome.hook_name;
        match hook_outcome.result {
            HookResult::Success => {}
            HookResult::FailedContinue(error) => {
                warn!(
                    call_id = %invocation.call_id,
                    tool_name = %invocation.tool_name,
                    hook_name = %hook_name,
                    error = %error,
                    "after_tool_use hook failed; continuing"
                );
            }
            HookResult::FailedAbort(error) => {
                warn!(
                    call_id = %invocation.call_id,
                    tool_name = %invocation.tool_name,
                    hook_name = %hook_name,
                    error = %error,
                    "after_tool_use hook failed; aborting operation"
                );
                return Some(FunctionCallError::Fatal(format!(
                    "after_tool_use hook '{hook_name}' failed and aborted operation: {error}"
                )));
            }
        }
    }

    None
}
