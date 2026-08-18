use crate::engine::ClaudeHooksEngine;
use crate::engine::CommandShell;
use crate::engine::HookListEntry;
use crate::engine::command_runner::CommandHookRuntime;
use crate::events::compact::PostCompactRequest;
use crate::events::compact::PreCompactOutcome;
use crate::events::compact::PreCompactRequest;
use crate::events::compact::StatelessHookOutcome;
use crate::events::permission_request::PermissionRequestOutcome;
use crate::events::permission_request::PermissionRequestRequest;
use crate::events::post_tool_use::PostToolUseOutcome;
use crate::events::post_tool_use::PostToolUseRequest;
use crate::events::pre_tool_use::PreToolUseOutcome;
use crate::events::pre_tool_use::PreToolUseRequest;
use crate::events::session_end::SessionEndOutcome;
use crate::events::session_end::SessionEndRequest;
use crate::events::session_start::SessionStartOutcome;
use crate::events::session_start::SessionStartRequest;
use crate::events::stop::StopOutcome;
use crate::events::stop::StopRequest;
use crate::events::user_prompt_submit::UserPromptSubmitOutcome;
use crate::events::user_prompt_submit::UserPromptSubmitRequest;
use crate::mcp::HookMcpExecutor;
use crate::types::Hook;
use crate::types::HookEvent;
use crate::types::HookPayload;
use crate::types::HookResponse;
use async_channel::Receiver;
use codex_config::ConfigLayerStack;
use codex_plugin::PluginHookSource;
use codex_protocol::ThreadId;
use codex_protocol::shell_environment::scrub_non_inheritable_env_vars;
use std::ffi::OsString;
use std::sync::Arc;
use std::time::Duration;
use tokio::process::Command;

#[derive(Default, Clone)]
pub struct HooksConfig {
    pub legacy_notify_argv: Option<Vec<String>>,
    pub feature_enabled: bool,
    pub bypass_hook_trust: bool,
    pub config_layer_stack: Option<ConfigLayerStack>,
    pub plugin_hook_sources: Vec<PluginHookSource>,
    pub plugin_hook_load_warnings: Vec<String>,
    pub shell_program: Option<String>,
    pub shell_args: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HookListOutcome {
    pub hooks: Vec<HookListEntry>,
    pub warnings: Vec<String>,
}

#[derive(Clone)]
pub struct Hooks {
    // TODO: Once legacy `notify` is removed, capture this snapshot in `CommandHookRuntime::new`
    // and remove the environment plumbing from `Hooks` and `from_config`.
    environment: Arc<Vec<(OsString, OsString)>>,
    after_agent: Vec<Hook>,
    engine: ClaudeHooksEngine,
}

impl Hooks {
    /// Bind this session's hook runtime and output files to its thread, rejecting unloadable
    /// required managed hooks.
    pub fn new(
        config: HooksConfig,
        thread_id: ThreadId,
        mcp_executor: Arc<dyn HookMcpExecutor>,
    ) -> anyhow::Result<(Self, Receiver<codex_protocol::protocol::HookCompletedEvent>)> {
        let (result_sender, result_receiver) = async_channel::unbounded();
        let environment = Arc::new(std::env::vars_os().collect());
        let hooks = Self::from_config(config, mcp_executor, Arc::clone(&environment), |shell| {
            CommandHookRuntime::new(shell, environment, thread_id, result_sender)
        });
        let required_load_errors = hooks.engine.required_load_errors();
        if !required_load_errors.is_empty() {
            anyhow::bail!(
                "failed to load required managed hooks: {}",
                required_load_errors.join("; ")
            );
        }
        Ok((hooks, result_receiver))
    }

    /// Preserve in-flight background hooks while applying a refreshed configuration.
    pub fn reconfigured(&self, config: HooksConfig) -> Self {
        Self::from_config(
            config,
            Arc::clone(&self.engine.mcp_executor),
            Arc::clone(&self.environment),
            |shell| self.engine.command_runtime.reconfigured(shell),
        )
    }

    fn from_config(
        config: HooksConfig,
        mcp_executor: Arc<dyn HookMcpExecutor>,
        environment: Arc<Vec<(OsString, OsString)>>,
        build_runtime: impl FnOnce(CommandShell) -> CommandHookRuntime,
    ) -> Self {
        let after_agent = config
            .legacy_notify_argv
            .filter(|argv| !argv.is_empty() && !argv[0].is_empty())
            .map(|argv| crate::legacy_notify::notify_hook(argv, Arc::clone(&environment)))
            .into_iter()
            .collect();
        let command_runtime = build_runtime(CommandShell {
            program: config.shell_program.unwrap_or_default(),
            args: config.shell_args,
        });
        let engine = ClaudeHooksEngine::new(
            config.feature_enabled,
            config.bypass_hook_trust,
            config.config_layer_stack.as_ref(),
            config.plugin_hook_sources,
            config.plugin_hook_load_warnings,
            command_runtime,
            mcp_executor,
        );
        Self {
            environment,
            after_agent,
            engine,
        }
    }

    /// Abort and join outstanding async hooks during session shutdown.
    pub async fn shutdown(&self) {
        self.engine.command_runtime.shutdown().await;
    }

    pub fn startup_warnings(&self) -> &[String] {
        self.engine.warnings()
    }

    fn hooks_for_event(&self, hook_event: &HookEvent) -> &[Hook] {
        match hook_event {
            HookEvent::AfterAgent { .. } => &self.after_agent,
        }
    }

    pub async fn dispatch(&self, hook_payload: HookPayload) -> Vec<HookResponse> {
        let hooks = self.hooks_for_event(&hook_payload.hook_event);
        let mut outcomes = Vec::with_capacity(hooks.len());
        for hook in hooks {
            let outcome = hook.execute(&hook_payload).await;
            let should_abort_operation = outcome.result.should_abort_operation();
            outcomes.push(outcome);
            if should_abort_operation {
                break;
            }
        }

        outcomes
    }

    pub fn preview_session_start(
        &self,
        request: &SessionStartRequest,
    ) -> Vec<codex_protocol::protocol::HookRunSummary> {
        self.engine.preview_session_start(request)
    }

    pub fn preview_pre_tool_use(
        &self,
        request: &PreToolUseRequest,
    ) -> Vec<codex_protocol::protocol::HookRunSummary> {
        self.engine.preview_pre_tool_use(request)
    }

    pub fn preview_permission_request(
        &self,
        request: &PermissionRequestRequest,
    ) -> Vec<codex_protocol::protocol::HookRunSummary> {
        self.engine.preview_permission_request(request)
    }

    /// Maximum configured timeout among PermissionRequest hooks.
    ///
    /// Matching handlers run concurrently, so their aggregate timeout is bounded by this maximum.
    pub fn max_permission_request_timeout(&self) -> Duration {
        self.engine.max_permission_request_timeout()
    }

    pub fn preview_post_tool_use(
        &self,
        request: &PostToolUseRequest,
    ) -> Vec<codex_protocol::protocol::HookRunSummary> {
        self.engine.preview_post_tool_use(request)
    }

    pub async fn run_session_start(
        &self,
        request: SessionStartRequest,
        turn_id: Option<String>,
    ) -> SessionStartOutcome {
        self.engine.run_session_start(request, turn_id).await
    }

    pub async fn run_pre_tool_use(&self, request: PreToolUseRequest) -> PreToolUseOutcome {
        self.engine.run_pre_tool_use(request).await
    }

    pub async fn run_permission_request(
        &self,
        request: PermissionRequestRequest,
    ) -> PermissionRequestOutcome {
        self.engine.run_permission_request(request).await
    }

    pub async fn run_post_tool_use(&self, request: PostToolUseRequest) -> PostToolUseOutcome {
        self.engine.run_post_tool_use(request).await
    }

    pub fn preview_pre_compact(
        &self,
        request: &PreCompactRequest,
    ) -> Vec<codex_protocol::protocol::HookRunSummary> {
        self.engine.preview_pre_compact(request)
    }

    pub async fn run_pre_compact(&self, request: PreCompactRequest) -> PreCompactOutcome {
        self.engine.run_pre_compact(request).await
    }

    pub fn preview_post_compact(
        &self,
        request: &PostCompactRequest,
    ) -> Vec<codex_protocol::protocol::HookRunSummary> {
        self.engine.preview_post_compact(request)
    }

    pub async fn run_post_compact(&self, request: PostCompactRequest) -> StatelessHookOutcome {
        self.engine.run_post_compact(request).await
    }

    pub fn preview_user_prompt_submit(
        &self,
        request: &UserPromptSubmitRequest,
    ) -> Vec<codex_protocol::protocol::HookRunSummary> {
        self.engine.preview_user_prompt_submit(request)
    }

    pub async fn run_user_prompt_submit(
        &self,
        request: UserPromptSubmitRequest,
    ) -> UserPromptSubmitOutcome {
        self.engine.run_user_prompt_submit(request).await
    }

    pub fn preview_stop(
        &self,
        request: &StopRequest,
    ) -> Vec<codex_protocol::protocol::HookRunSummary> {
        self.engine.preview_stop(request)
    }

    pub async fn run_stop(&self, request: StopRequest) -> StopOutcome {
        self.engine.run_stop(request).await
    }

    pub fn preview_session_end(&self) -> Vec<codex_protocol::protocol::HookRunSummary> {
        self.engine.preview_session_end()
    }

    pub async fn run_session_end(&self, request: SessionEndRequest) -> SessionEndOutcome {
        self.engine.run_session_end(request).await
    }
}

pub fn list_hooks(config: HooksConfig) -> HookListOutcome {
    if !config.feature_enabled {
        return HookListOutcome::default();
    }

    let discovered = crate::engine::discovery::discover_handlers(
        config.config_layer_stack.as_ref(),
        config.plugin_hook_sources,
        config.plugin_hook_load_warnings,
        config.bypass_hook_trust,
    );
    HookListOutcome {
        hooks: discovered.hook_entries,
        warnings: discovered.warnings,
    }
}

// TODO: Remove this legacy-notify-only command builder when `notify` support is removed.
pub(crate) fn command_from_argv(
    argv: &[String],
    environment: impl IntoIterator<Item = (OsString, OsString)>,
) -> Option<Command> {
    let (program, args) = argv.split_first()?;
    if program.is_empty() {
        return None;
    }
    let mut command = Command::new(program);
    command.args(args);
    command.env_clear();
    command.envs(environment);
    scrub_non_inheritable_env_vars(command.as_std_mut());
    Some(command)
}
