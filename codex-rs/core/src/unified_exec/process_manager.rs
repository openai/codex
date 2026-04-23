use rand::Rng;
use std::cmp::Reverse;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use tokio::sync::Notify;
use tokio::sync::watch;
use tokio::time::Duration;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;

use crate::exec_env::CODEX_THREAD_ID_ENV_VAR;
use crate::exec_env::create_env;
use crate::exec_policy::ExecApprovalRequest;
use crate::sandboxing::ExecRequest;
use crate::sandboxing::ExecServerEnvConfig;
use crate::tools::context::ExecCommandToolOutput;
use crate::tools::events::ToolEmitter;
use crate::tools::events::ToolEventCtx;
use crate::tools::events::ToolEventStage;
use crate::tools::network_approval::DeferredNetworkApproval;
use crate::tools::network_approval::finish_deferred_network_approval;
use crate::tools::orchestrator::ToolOrchestrator;
use crate::tools::runtimes::unified_exec::UnifiedExecRequest as UnifiedExecToolRequest;
use crate::tools::runtimes::unified_exec::UnifiedExecRuntime;
use crate::tools::sandboxing::ToolCtx;
use crate::tools::sandboxing::ToolError;
use crate::unified_exec::ExecCommandRequest;
use crate::unified_exec::MAX_UNIFIED_EXEC_PROCESSES;
use crate::unified_exec::MAX_YIELD_TIME_MS;
use crate::unified_exec::MIN_EMPTY_YIELD_TIME_MS;
use crate::unified_exec::MIN_YIELD_TIME_MS;
use crate::unified_exec::ProcessEntry;
use crate::unified_exec::ProcessStore;
use crate::unified_exec::UnifiedExecContext;
use crate::unified_exec::UnifiedExecError;
use crate::unified_exec::UnifiedExecProcessManager;
use crate::unified_exec::WARNING_UNIFIED_EXEC_PROCESSES;
use crate::unified_exec::WriteStdinRequest;
use crate::unified_exec::async_watcher::emit_exec_end_for_unified_exec;
use crate::unified_exec::async_watcher::emit_failed_exec_end_for_unified_exec;
use crate::unified_exec::async_watcher::spawn_exit_watcher;
use crate::unified_exec::async_watcher::start_streaming_output;
use crate::unified_exec::clamp_yield_time;
use crate::unified_exec::generate_chunk_id;
use crate::unified_exec::head_tail_buffer::HeadTailBuffer;
use crate::unified_exec::process::OutputBuffer;
use crate::unified_exec::process::OutputHandles;
use crate::unified_exec::process::SpawnLifecycleHandle;
use crate::unified_exec::process::UnifiedExecProcess;
use codex_config::types::ShellEnvironmentPolicy;
use codex_protocol::error::CodexErr;
use codex_protocol::error::SandboxErr;
use codex_protocol::protocol::ExecCommandSource;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_output_truncation::approx_token_count;

const UNIFIED_EXEC_ENV: [(&str, &str); 10] = [
    ("NO_COLOR", "1"),
    ("TERM", "dumb"),
    ("LANG", "C.UTF-8"),
    ("LC_CTYPE", "C.UTF-8"),
    ("LC_ALL", "C.UTF-8"),
    ("COLORTERM", ""),
    ("PAGER", "cat"),
    ("GIT_PAGER", "cat"),
    ("GH_PAGER", "cat"),
    ("CODEX_CI", "1"),
];

/// Test-only override for deterministic unified exec process IDs.
///
/// In production builds this value should remain at its default (`false`) and
/// must not be toggled.
static FORCE_DETERMINISTIC_PROCESS_IDS: AtomicBool = AtomicBool::new(false);

pub(super) fn set_deterministic_process_ids_for_tests(enabled: bool) {
    FORCE_DETERMINISTIC_PROCESS_IDS.store(enabled, Ordering::Relaxed);
}

fn deterministic_process_ids_forced_for_tests() -> bool {
    FORCE_DETERMINISTIC_PROCESS_IDS.load(Ordering::Relaxed)
}

fn should_use_deterministic_process_ids() -> bool {
    cfg!(test) || deterministic_process_ids_forced_for_tests()
}

fn apply_unified_exec_env(mut env: HashMap<String, String>) -> HashMap<String, String> {
    for (key, value) in UNIFIED_EXEC_ENV {
        env.insert(key.to_string(), value.to_string());
    }
    env
}

fn exec_env_policy_from_shell_policy(
    policy: &ShellEnvironmentPolicy,
) -> codex_exec_server::ExecEnvPolicy {
    codex_exec_server::ExecEnvPolicy {
        inherit: policy.inherit.clone(),
        ignore_default_excludes: policy.ignore_default_excludes,
        exclude: policy
            .exclude
            .iter()
            .map(std::string::ToString::to_string)
            .collect(),
        r#set: policy.r#set.clone(),
        include_only: policy
            .include_only
            .iter()
            .map(std::string::ToString::to_string)
            .collect(),
    }
}

fn env_overlay_for_exec_server(
    request_env: &HashMap<String, String>,
    local_policy_env: &HashMap<String, String>,
) -> HashMap<String, String> {
    request_env
        .iter()
        .filter(|(key, value)| local_policy_env.get(*key) != Some(*value))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect()
}

fn exec_server_env_for_request(
    request: &ExecRequest,
) -> (
    Option<codex_exec_server::ExecEnvPolicy>,
    HashMap<String, String>,
) {
    if let Some(exec_server_env_config) = &request.exec_server_env_config {
        (
            Some(exec_server_env_config.policy.clone()),
            env_overlay_for_exec_server(&request.env, &exec_server_env_config.local_policy_env),
        )
    } else {
        (None, request.env.clone())
    }
}

fn exec_server_params_for_request(
    process_id: i32,
    request: &ExecRequest,
    tty: bool,
) -> codex_exec_server::ExecParams {
    let (env_policy, env) = exec_server_env_for_request(request);
    codex_exec_server::ExecParams {
        process_id: exec_server_process_id(process_id).into(),
        argv: request.command.clone(),
        cwd: request.cwd.to_path_buf(),
        env_policy,
        env,
        tty,
        pipe_stdin: false,
        arg0: request.arg0.clone(),
    }
}

/// Borrowed process state prepared for a `write_stdin` or poll operation.
struct PreparedProcessHandles {
    process: Arc<UnifiedExecProcess>,
    output_buffer: OutputBuffer,
    output_notify: Arc<Notify>,
    output_closed: Arc<AtomicBool>,
    output_closed_notify: Arc<Notify>,
    cancellation_token: CancellationToken,
    pause_state: Option<watch::Receiver<bool>>,
    session: Option<Arc<crate::session::session::Session>>,
    network_approval: Option<DeferredNetworkApproval>,
    hook_command: String,
    process_id: i32,
    tty: bool,
}

fn exec_server_process_id(process_id: i32) -> String {
    process_id.to_string()
}

impl UnifiedExecProcessManager {
    pub(crate) async fn allocate_process_id(&self) -> i32 {
        loop {
            let mut store = self.process_store.lock().await;

            let process_id = if should_use_deterministic_process_ids() {
                // test or deterministic mode
                store
                    .reserved_process_ids
                    .iter()
                    .copied()
                    .max()
                    .map(|m| std::cmp::max(m, 999) + 1)
                    .unwrap_or(1000)
            } else {
                // production mode → random
                rand::rng().random_range(1_000..100_000)
            };

            if store.reserved_process_ids.contains(&process_id) {
                continue;
            }

            store.reserved_process_ids.insert(process_id);
            return process_id;
        }
    }

    pub(crate) async fn release_process_id(&self, process_id: i32) {
        let removed = {
            let mut store = self.process_store.lock().await;
            store.remove(process_id)
        };
        if let Some(entry) = removed {
            Self::unregister_network_approval_for_entry(&entry).await;
        }
    }

    async fn unregister_network_approval_for_entry(entry: &ProcessEntry) {
        if let Some(network_approval) = entry.network_approval.as_ref()
            && let Some(session) = entry.session.upgrade()
        {
            session
                .services
                .network_approval
                .unregister_call(network_approval.registration_id())
                .await;
        }
    }

    async fn finish_network_approval_for_entry(
        entry: &ProcessEntry,
    ) -> Result<(), UnifiedExecError> {
        let session = entry.session.upgrade();
        Self::finish_deferred_network_approval_for_session(
            session.as_ref(),
            entry.network_approval.clone(),
        )
        .await
    }

    async fn finish_deferred_network_approval_for_session(
        session: Option<&Arc<crate::session::session::Session>>,
        deferred: Option<DeferredNetworkApproval>,
    ) -> Result<(), UnifiedExecError> {
        let Some(session) = session else {
            return Ok(());
        };
        finish_deferred_network_approval(session.as_ref(), deferred)
            .await
            .map_err(Self::network_approval_error)
    }

    fn network_approval_error(err: ToolError) -> UnifiedExecError {
        match err {
            ToolError::Rejected(message) => UnifiedExecError::process_failed(message),
            ToolError::Codex(err) => UnifiedExecError::process_failed(err.to_string()),
        }
    }

    fn terminate_process_on_network_denial(
        process: Arc<UnifiedExecProcess>,
        deferred: &DeferredNetworkApproval,
    ) {
        let network_cancelled = deferred.cancellation_token();
        let process_exited = process.cancellation_token();
        tokio::spawn(async move {
            tokio::select! {
                _ = network_cancelled.cancelled() => process.terminate(),
                _ = process_exited.cancelled() => {}
            }
        });
    }

    pub(crate) async fn exec_command(
        &self,
        request: ExecCommandRequest,
        context: &UnifiedExecContext,
    ) -> Result<ExecCommandToolOutput, UnifiedExecError> {
        let cwd = request
            .workdir
            .clone()
            .unwrap_or_else(|| context.turn.cwd.clone());
        let process = self
            .open_session_with_sandbox(&request, cwd.clone(), context)
            .await;

        let (process, mut deferred_network_approval) = match process {
            Ok((process, deferred_network_approval)) => {
                (Arc::new(process), deferred_network_approval)
            }
            Err(err) => {
                self.release_process_id(request.process_id).await;
                return Err(err);
            }
        };
        if let Some(deferred) = deferred_network_approval.as_ref() {
            Self::terminate_process_on_network_denial(Arc::clone(&process), deferred);
        }

        let transcript = Arc::new(tokio::sync::Mutex::new(HeadTailBuffer::default()));
        let event_ctx = ToolEventCtx::new(
            context.session.as_ref(),
            context.turn.as_ref(),
            &context.call_id,
            /*turn_diff_tracker*/ None,
        );
        let emitter = ToolEmitter::unified_exec(
            &request.command,
            cwd.clone(),
            ExecCommandSource::UnifiedExecStartup,
            Some(request.process_id.to_string()),
        );
        emitter.emit(event_ctx, ToolEventStage::Begin).await;

        start_streaming_output(&process, context, Arc::clone(&transcript));
        let start = Instant::now();
        // Persist live sessions before the initial yield wait so interrupting the
        // turn cannot drop the last Arc and terminate the background process.
        let process_started_alive = !process.has_exited() && process.exit_code().is_none();
        if process_started_alive {
            self.store_process(
                Arc::clone(&process),
                context,
                &request.command,
                request.hook_command.clone(),
                cwd.clone(),
                start,
                request.process_id,
                request.tty,
                deferred_network_approval.clone(),
                Arc::clone(&transcript),
            )
            .await;
        }

        let yield_time_ms = clamp_yield_time(request.yield_time_ms);
        // For the initial exec_command call, we both stream output to events
        // (via start_streaming_output above) and collect a snapshot here for
        // the tool response body.
        let OutputHandles {
            output_buffer,
            output_notify,
            output_closed,
            output_closed_notify,
            cancellation_token,
        } = process.output_handles();
        let deadline = start + Duration::from_millis(yield_time_ms);
        let collected = Self::collect_output_until_deadline(
            &output_buffer,
            &output_notify,
            &output_closed,
            &output_closed_notify,
            &cancellation_token,
            Some(
                context
                    .session
                    .subscribe_out_of_band_elicitation_pause_state(),
            ),
            deadline,
        )
        .await;
        let wall_time = Instant::now().saturating_duration_since(start);

        let text = String::from_utf8_lossy(&collected).to_string();
        let chunk_id = generate_chunk_id();
        if deferred_network_approval
            .as_ref()
            .is_some_and(DeferredNetworkApproval::is_cancelled)
        {
            process.terminate();
            let finish_result = Self::finish_deferred_network_approval_for_session(
                Some(&context.session),
                deferred_network_approval.take(),
            )
            .await;
            self.release_process_id(request.process_id).await;
            finish_result?;
            return Err(UnifiedExecError::process_failed(
                "network access was denied".to_string(),
            ));
        }
        if let Some(message) = process.failure_message() {
            let finish_result = Self::finish_deferred_network_approval_for_session(
                Some(&context.session),
                deferred_network_approval.take(),
            )
            .await;
            if !process_started_alive {
                emit_failed_exec_end_for_unified_exec(
                    Arc::clone(&context.session),
                    Arc::clone(&context.turn),
                    context.call_id.clone(),
                    request.command.clone(),
                    cwd.clone(),
                    Some(request.process_id.to_string()),
                    Arc::clone(&transcript),
                    message.clone(),
                    wall_time,
                )
                .await;
            }
            self.release_process_id(request.process_id).await;
            finish_result?;
            return Err(UnifiedExecError::process_failed(message));
        }
        let process_id = request.process_id;
        let (response_process_id, exit_code) = if process_started_alive {
            match self.refresh_process_state(process_id).await {
                ProcessStatus::Alive {
                    exit_code,
                    process_id,
                    ..
                } => (Some(process_id), exit_code),
                ProcessStatus::Exited { exit_code, .. } => {
                    Self::finish_deferred_network_approval_for_session(
                        Some(&context.session),
                        deferred_network_approval.take(),
                    )
                    .await?;
                    process.check_for_sandbox_denial_with_text(&text).await?;
                    (None, exit_code)
                }
                ProcessStatus::Unknown => {
                    return Err(UnifiedExecError::UnknownProcessId { process_id });
                }
            }
        } else {
            // Short‑lived command: emit ExecCommandEnd immediately using the
            // same helper as the background watcher, so all end events share
            // one implementation.
            let finish_result = Self::finish_deferred_network_approval_for_session(
                Some(&context.session),
                deferred_network_approval.take(),
            )
            .await;
            if let Err(err) = finish_result {
                self.release_process_id(request.process_id).await;
                return Err(err);
            }
            let exit_code = process.exit_code();
            let exit = exit_code.unwrap_or(-1);
            emit_exec_end_for_unified_exec(
                Arc::clone(&context.session),
                Arc::clone(&context.turn),
                context.call_id.clone(),
                request.command.clone(),
                cwd.clone(),
                Some(process_id.to_string()),
                Arc::clone(&transcript),
                text.clone(),
                exit,
                wall_time,
            )
            .await;

            self.release_process_id(request.process_id).await;
            process.check_for_sandbox_denial_with_text(&text).await?;
            (None, exit_code)
        };

        let original_token_count = approx_token_count(&text);
        let response = ExecCommandToolOutput {
            event_call_id: context.call_id.clone(),
            chunk_id,
            wall_time,
            raw_output: collected,
            max_output_tokens: request.max_output_tokens,
            process_id: response_process_id,
            exit_code,
            original_token_count: Some(original_token_count),
            hook_command: Some(request.hook_command.clone()),
        };

        Ok(response)
    }

    pub(crate) async fn write_stdin(
        &self,
        request: WriteStdinRequest<'_>,
    ) -> Result<ExecCommandToolOutput, UnifiedExecError> {
        let process_id = request.process_id;

        let PreparedProcessHandles {
            process,
            output_buffer,
            output_notify,
            output_closed,
            output_closed_notify,
            cancellation_token,
            pause_state,
            session,
            network_approval,
            hook_command,
            process_id,
            tty,
            ..
        } = self.prepare_process_handles(process_id).await?;
        let mut status_after_write = None;

        if !request.input.is_empty() {
            if !tty {
                return Err(UnifiedExecError::StdinClosed);
            }
            match process.write(request.input.as_bytes()).await {
                Ok(()) => {
                    // Give the remote process a brief window to react so that we are
                    // more likely to capture its output in the poll below.
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
                Err(err) => {
                    let status = self.refresh_process_state(process_id).await;
                    if matches!(status, ProcessStatus::Exited { .. }) {
                        status_after_write = Some(status);
                    } else if matches!(err, UnifiedExecError::ProcessFailed { .. }) {
                        process.terminate();
                        self.release_process_id(process_id).await;
                        return Err(err);
                    } else {
                        return Err(err);
                    }
                }
            }
        }

        let yield_time_ms = {
            // Empty polls use configurable background timeout bounds. Non-empty
            // writes keep a fixed max cap so interactive stdin remains responsive.
            let time_ms = request.yield_time_ms.max(MIN_YIELD_TIME_MS);
            if request.input.is_empty() {
                time_ms.clamp(MIN_EMPTY_YIELD_TIME_MS, self.max_write_stdin_yield_time_ms)
            } else {
                time_ms.min(MAX_YIELD_TIME_MS)
            }
        };
        let start = Instant::now();
        let deadline = start + Duration::from_millis(yield_time_ms);
        let collected = Self::collect_output_until_deadline(
            &output_buffer,
            &output_notify,
            &output_closed,
            &output_closed_notify,
            &cancellation_token,
            pause_state,
            deadline,
        )
        .await;
        let wall_time = Instant::now().saturating_duration_since(start);

        let text = String::from_utf8_lossy(&collected).to_string();
        let original_token_count = approx_token_count(&text);
        let chunk_id = generate_chunk_id();
        if network_approval
            .as_ref()
            .is_some_and(DeferredNetworkApproval::is_cancelled)
        {
            process.terminate();
            let finish_result = Self::finish_deferred_network_approval_for_session(
                session.as_ref(),
                network_approval.clone(),
            )
            .await;
            self.release_process_id(process_id).await;
            finish_result?;
            return Err(UnifiedExecError::process_failed(
                "network access was denied".to_string(),
            ));
        }
        if let Some(message) = process.failure_message() {
            let finish_result = Self::finish_deferred_network_approval_for_session(
                session.as_ref(),
                network_approval.clone(),
            )
            .await;
            self.release_process_id(process_id).await;
            finish_result?;
            return Err(UnifiedExecError::process_failed(message));
        }

        // After polling, refresh_process_state tells us whether the PTY is
        // still alive or has exited and been removed from the store; we thread
        // that through so the handler can tag TerminalInteraction with an
        // appropriate process_id and exit_code.
        let status = if let Some(status) = status_after_write {
            status
        } else {
            self.refresh_process_state(process_id).await
        };
        let (process_id, exit_code, event_call_id) = match status {
            ProcessStatus::Alive {
                exit_code,
                call_id,
                process_id,
            } => (Some(process_id), exit_code, call_id),
            ProcessStatus::Exited { exit_code, entry } => {
                let call_id = entry.call_id.clone();
                Self::finish_network_approval_for_entry(&entry).await?;
                (None, exit_code, call_id)
            }
            ProcessStatus::Unknown => {
                return Err(UnifiedExecError::UnknownProcessId {
                    process_id: request.process_id,
                });
            }
        };

        let response = ExecCommandToolOutput {
            event_call_id,
            chunk_id,
            wall_time,
            raw_output: collected,
            max_output_tokens: request.max_output_tokens,
            process_id,
            exit_code,
            original_token_count: Some(original_token_count),
            hook_command: Some(hook_command),
        };

        Ok(response)
    }

    async fn refresh_process_state(&self, process_id: i32) -> ProcessStatus {
        {
            let mut store = self.process_store.lock().await;
            let Some(entry) = store.processes.get(&process_id) else {
                return ProcessStatus::Unknown;
            };

            let exit_code = entry.process.exit_code();
            let process_id = entry.process_id;

            if entry.process.has_exited() {
                let Some(entry) = store.remove(process_id) else {
                    return ProcessStatus::Unknown;
                };
                ProcessStatus::Exited {
                    exit_code,
                    entry: Box::new(entry),
                }
            } else {
                ProcessStatus::Alive {
                    exit_code,
                    call_id: entry.call_id.clone(),
                    process_id,
                }
            }
        }
    }

    async fn prepare_process_handles(
        &self,
        process_id: i32,
    ) -> Result<PreparedProcessHandles, UnifiedExecError> {
        let mut store = self.process_store.lock().await;
        let entry = store
            .processes
            .get_mut(&process_id)
            .ok_or(UnifiedExecError::UnknownProcessId { process_id })?;
        entry.last_used = Instant::now();
        let OutputHandles {
            output_buffer,
            output_notify,
            output_closed,
            output_closed_notify,
            cancellation_token,
        } = entry.process.output_handles();
        let pause_state = entry
            .session
            .upgrade()
            .map(|session| session.subscribe_out_of_band_elicitation_pause_state());
        let session = entry.session.upgrade();

        Ok(PreparedProcessHandles {
            process: Arc::clone(&entry.process),
            output_buffer,
            output_notify,
            output_closed,
            output_closed_notify,
            cancellation_token,
            pause_state,
            session,
            network_approval: entry.network_approval.clone(),
            hook_command: entry.hook_command.clone(),
            process_id: entry.process_id,
            tty: entry.tty,
        })
    }

    #[allow(clippy::too_many_arguments)]
    async fn store_process(
        &self,
        process: Arc<UnifiedExecProcess>,
        context: &UnifiedExecContext,
        command: &[String],
        hook_command: String,
        cwd: AbsolutePathBuf,
        started_at: Instant,
        process_id: i32,
        tty: bool,
        network_approval: Option<DeferredNetworkApproval>,
        transcript: Arc<tokio::sync::Mutex<HeadTailBuffer>>,
    ) {
        let entry = ProcessEntry {
            process: Arc::clone(&process),
            call_id: context.call_id.clone(),
            process_id,
            hook_command,
            tty,
            network_approval,
            session: Arc::downgrade(&context.session),
            last_used: started_at,
        };
        let (number_processes, pruned_entry) = {
            let mut store = self.process_store.lock().await;
            let pruned_entry = Self::prune_processes_if_needed(&mut store);
            store.processes.insert(process_id, entry);
            (store.processes.len(), pruned_entry)
        };
        // prune_processes_if_needed runs while holding process_store; do async
        // network-approval cleanup only after dropping that lock.
        if let Some(pruned_entry) = pruned_entry {
            Self::unregister_network_approval_for_entry(&pruned_entry).await;
            pruned_entry.process.terminate();
        }

        if number_processes >= WARNING_UNIFIED_EXEC_PROCESSES {
            context
                .session
                .record_model_warning(
                    format!("The maximum number of unified exec processes you can keep open is {WARNING_UNIFIED_EXEC_PROCESSES} and you currently have {number_processes} processes open. Reuse older processes or close them to prevent automatic pruning of old processes"),
                    &context.turn
                )
                .await;
        };

        spawn_exit_watcher(
            Arc::clone(&process),
            Arc::clone(&context.session),
            Arc::clone(&context.turn),
            context.call_id.clone(),
            command.to_vec(),
            cwd,
            process_id,
            transcript,
            started_at,
        );
    }

    pub(crate) async fn open_session_with_exec_env(
        &self,
        process_id: i32,
        request: &ExecRequest,
        tty: bool,
        mut spawn_lifecycle: SpawnLifecycleHandle,
        environment: &codex_exec_server::Environment,
    ) -> Result<UnifiedExecProcess, UnifiedExecError> {
        let inherited_fds = spawn_lifecycle.inherited_fds();

        #[cfg(target_os = "windows")]
        if request.sandbox == codex_sandboxing::SandboxType::WindowsRestrictedToken {
            let policy_json = serde_json::to_string(&request.sandbox_policy).map_err(|err| {
                UnifiedExecError::create_process(format!(
                    "failed to serialize Windows sandbox policy: {err}"
                ))
            })?;
            let codex_home = crate::config::find_codex_home().map_err(|err| {
                UnifiedExecError::create_process(format!(
                    "windows sandbox: failed to resolve codex_home: {err}"
                ))
            })?;
            let spawned = match request.windows_sandbox_level {
                codex_protocol::config_types::WindowsSandboxLevel::Elevated => {
                    codex_windows_sandbox::spawn_windows_sandbox_session_elevated(
                        policy_json.as_str(),
                        request.windows_sandbox_policy_cwd.as_path(),
                        codex_home.as_ref(),
                        request.command.clone(),
                        request.cwd.as_path(),
                        request.env.clone(),
                        None,
                        tty,
                        tty,
                        request.windows_sandbox_private_desktop,
                    )
                    .await
                }
                codex_protocol::config_types::WindowsSandboxLevel::RestrictedToken
                | codex_protocol::config_types::WindowsSandboxLevel::Disabled => {
                    codex_windows_sandbox::spawn_windows_sandbox_session_legacy(
                        policy_json.as_str(),
                        request.windows_sandbox_policy_cwd.as_path(),
                        codex_home.as_ref(),
                        request.command.clone(),
                        request.cwd.as_path(),
                        request.env.clone(),
                        None,
                        tty,
                        tty,
                        request.windows_sandbox_private_desktop,
                    )
                    .await
                }
            };
            spawn_lifecycle.after_spawn();
            return UnifiedExecProcess::from_spawned(
                spawned.map_err(|err| UnifiedExecError::create_process(err.to_string()))?,
                request.sandbox,
                spawn_lifecycle,
            )
            .await;
        }
        if environment.is_remote() {
            if !inherited_fds.is_empty() {
                return Err(UnifiedExecError::create_process(
                    "remote exec-server does not support inherited file descriptors".to_string(),
                ));
            }

            let started = environment
                .get_exec_backend()
                .start(exec_server_params_for_request(process_id, request, tty))
                .await
                .map_err(|err| UnifiedExecError::create_process(err.to_string()))?;
            spawn_lifecycle.after_spawn();
            return UnifiedExecProcess::from_exec_server_started(started, request.sandbox).await;
        }

        let (program, args) = request
            .command
            .split_first()
            .ok_or(UnifiedExecError::MissingCommandLine)?;
        let spawn_result = if tty {
            codex_utils_pty::pty::spawn_process_with_inherited_fds(
                program,
                args,
                request.cwd.as_path(),
                &request.env,
                &request.arg0,
                codex_utils_pty::TerminalSize::default(),
                &inherited_fds,
            )
            .await
        } else {
            codex_utils_pty::pipe::spawn_process_no_stdin_with_inherited_fds(
                program,
                args,
                request.cwd.as_path(),
                &request.env,
                &request.arg0,
                &inherited_fds,
            )
            .await
        };
        let spawned =
            spawn_result.map_err(|err| UnifiedExecError::create_process(err.to_string()))?;
        spawn_lifecycle.after_spawn();
        UnifiedExecProcess::from_spawned(spawned, request.sandbox, spawn_lifecycle).await
    }

    pub(super) async fn open_session_with_sandbox(
        &self,
        request: &ExecCommandRequest,
        cwd: AbsolutePathBuf,
        context: &UnifiedExecContext,
    ) -> Result<(UnifiedExecProcess, Option<DeferredNetworkApproval>), UnifiedExecError> {
        let local_policy_env = create_env(
            &context.turn.shell_environment_policy,
            /*thread_id*/ None,
        );
        let mut env = local_policy_env.clone();
        env.insert(
            CODEX_THREAD_ID_ENV_VAR.to_string(),
            context.session.conversation_id.to_string(),
        );
        let env = apply_unified_exec_env(env);
        let exec_server_env_config = ExecServerEnvConfig {
            policy: exec_env_policy_from_shell_policy(&context.turn.shell_environment_policy),
            local_policy_env,
        };
        let mut orchestrator = ToolOrchestrator::new();
        let mut runtime = UnifiedExecRuntime::new(
            self,
            context.turn.tools_config.unified_exec_shell_mode.clone(),
        );
        let exec_approval_requirement = context
            .session
            .services
            .exec_policy
            .create_exec_approval_requirement_for_command(ExecApprovalRequest {
                command: &request.command,
                approval_policy: context.turn.approval_policy.value(),
                sandbox_policy: context.turn.sandbox_policy.get(),
                file_system_sandbox_policy: &context.turn.file_system_sandbox_policy,
                sandbox_permissions: if request.additional_permissions_preapproved {
                    crate::sandboxing::SandboxPermissions::UseDefault
                } else {
                    request.sandbox_permissions
                },
                prefix_rule: request.prefix_rule.clone(),
            })
            .await;
        let req = UnifiedExecToolRequest {
            command: request.command.clone(),
            hook_command: request.hook_command.clone(),
            process_id: request.process_id,
            cwd,
            env,
            exec_server_env_config: Some(exec_server_env_config),
            explicit_env_overrides: context.turn.shell_environment_policy.r#set.clone(),
            network: request.network.clone(),
            tty: request.tty,
            sandbox_permissions: request.sandbox_permissions,
            additional_permissions: request.additional_permissions.clone(),
            #[cfg(unix)]
            additional_permissions_preapproved: request.additional_permissions_preapproved,
            justification: request.justification.clone(),
            exec_approval_requirement,
        };
        let tool_ctx = ToolCtx {
            session: context.session.clone(),
            turn: context.turn.clone(),
            call_id: context.call_id.clone(),
            tool_name: "exec_command".to_string(),
        };
        orchestrator
            .run(
                &mut runtime,
                &req,
                &tool_ctx,
                &context.turn,
                context.turn.approval_policy.value(),
            )
            .await
            .map(|result| (result.output, result.deferred_network_approval))
            .map_err(|err| match err {
                ToolError::Codex(CodexErr::Sandbox(SandboxErr::Denied { output, .. })) => {
                    let output = *output;
                    let message = if output.aggregated_output.text.is_empty() {
                        let exit_code = output.exit_code;
                        format!("Process exited with code {exit_code}")
                    } else {
                        output.aggregated_output.text.clone()
                    };
                    UnifiedExecError::sandbox_denied(message, output)
                }
                other => UnifiedExecError::create_process(format!("{other:?}")),
            })
    }

    pub(super) async fn collect_output_until_deadline(
        output_buffer: &OutputBuffer,
        output_notify: &Arc<Notify>,
        output_closed: &Arc<AtomicBool>,
        output_closed_notify: &Arc<Notify>,
        cancellation_token: &CancellationToken,
        mut pause_state: Option<watch::Receiver<bool>>,
        mut deadline: Instant,
    ) -> Vec<u8> {
        const POST_EXIT_CLOSE_WAIT_CAP: Duration = Duration::from_millis(50);

        let mut collected: Vec<u8> = Vec::with_capacity(4096);
        let mut exit_signal_received = cancellation_token.is_cancelled();
        let mut post_exit_deadline: Option<Instant> = None;
        loop {
            Self::extend_deadlines_while_paused(
                &mut pause_state,
                &mut deadline,
                &mut post_exit_deadline,
            )
            .await;
            let drained_chunks: Vec<Vec<u8>>;
            let mut wait_for_output = None;
            {
                let mut guard = output_buffer.lock().await;
                drained_chunks = guard.drain_chunks();
                if drained_chunks.is_empty() {
                    wait_for_output = Some(output_notify.notified());
                }
            }

            if drained_chunks.is_empty() {
                exit_signal_received |= cancellation_token.is_cancelled();
                if exit_signal_received && output_closed.load(std::sync::atomic::Ordering::Acquire)
                {
                    break;
                }
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining == Duration::ZERO {
                    break;
                }

                if exit_signal_received {
                    let now = Instant::now();
                    let close_wait_deadline = *post_exit_deadline
                        .get_or_insert_with(|| now + remaining.min(POST_EXIT_CLOSE_WAIT_CAP));
                    let close_wait_remaining = close_wait_deadline.saturating_duration_since(now);
                    if close_wait_remaining == Duration::ZERO {
                        break;
                    }
                    let notified = wait_for_output.unwrap_or_else(|| output_notify.notified());
                    let closed = output_closed_notify.notified();
                    tokio::pin!(notified);
                    tokio::pin!(closed);
                    tokio::select! {
                        _ = &mut notified => {}
                        _ = &mut closed => {}
                        _ = tokio::time::sleep(close_wait_remaining) => break,
                        _ = Self::wait_for_pause_change(pause_state.as_ref()) => {}
                    }
                    continue;
                }

                let notified = wait_for_output.unwrap_or_else(|| output_notify.notified());
                tokio::pin!(notified);
                let exit_notified = cancellation_token.cancelled();
                tokio::pin!(exit_notified);
                tokio::select! {
                    _ = &mut notified => {}
                    _ = &mut exit_notified => exit_signal_received = true,
                    _ = tokio::time::sleep(remaining) => break,
                    _ = Self::wait_for_pause_change(pause_state.as_ref()) => {}
                }
                continue;
            }

            for chunk in drained_chunks {
                collected.extend_from_slice(&chunk);
            }

            exit_signal_received |= cancellation_token.is_cancelled();
            if Instant::now() >= deadline {
                break;
            }
        }

        collected
    }

    async fn extend_deadlines_while_paused(
        pause_state: &mut Option<watch::Receiver<bool>>,
        deadline: &mut Instant,
        post_exit_deadline: &mut Option<Instant>,
    ) {
        let Some(receiver) = pause_state.as_mut() else {
            return;
        };
        if !*receiver.borrow() {
            return;
        }

        let paused_at = Instant::now();
        while *receiver.borrow() {
            if receiver.changed().await.is_err() {
                break;
            }
        }

        let paused_for = paused_at.elapsed();
        *deadline += paused_for;
        if let Some(post_exit_deadline) = post_exit_deadline.as_mut() {
            *post_exit_deadline += paused_for;
        }
    }

    async fn wait_for_pause_change(pause_state: Option<&watch::Receiver<bool>>) {
        match pause_state {
            Some(pause_state) => {
                let mut receiver = pause_state.clone();
                let _ = receiver.changed().await;
            }
            None => std::future::pending::<()>().await,
        }
    }

    fn prune_processes_if_needed(store: &mut ProcessStore) -> Option<ProcessEntry> {
        if store.processes.len() < MAX_UNIFIED_EXEC_PROCESSES {
            return None;
        }

        let meta: Vec<(i32, Instant, bool)> = store
            .processes
            .iter()
            .map(|(id, entry)| (*id, entry.last_used, entry.process.has_exited()))
            .collect();

        if let Some(process_id) = Self::process_id_to_prune_from_meta(&meta) {
            return store.remove(process_id);
        }

        None
    }

    // Centralized pruning policy so we can easily swap strategies later.
    fn process_id_to_prune_from_meta(meta: &[(i32, Instant, bool)]) -> Option<i32> {
        if meta.is_empty() {
            return None;
        }

        let mut by_recency = meta.to_vec();
        by_recency.sort_by_key(|(_, last_used, _)| Reverse(*last_used));
        let protected: HashSet<i32> = by_recency
            .iter()
            .take(8)
            .map(|(process_id, _, _)| *process_id)
            .collect();

        let mut lru = meta.to_vec();
        lru.sort_by_key(|(_, last_used, _)| *last_used);

        if let Some((process_id, _, _)) = lru
            .iter()
            .find(|(process_id, _, exited)| !protected.contains(process_id) && *exited)
        {
            return Some(*process_id);
        }

        lru.into_iter()
            .find(|(process_id, _, _)| !protected.contains(process_id))
            .map(|(process_id, _, _)| process_id)
    }

    pub(crate) async fn terminate_all_processes(&self) {
        let entries: Vec<ProcessEntry> = {
            let mut processes = self.process_store.lock().await;
            let entries: Vec<ProcessEntry> = processes
                .processes
                .drain()
                .map(|(_, entry)| entry)
                .collect();
            processes.reserved_process_ids.clear();
            entries
        };

        for entry in entries {
            Self::unregister_network_approval_for_entry(&entry).await;
            entry.process.terminate();
        }
    }
}

enum ProcessStatus {
    Alive {
        exit_code: Option<i32>,
        call_id: String,
        process_id: i32,
    },
    Exited {
        exit_code: Option<i32>,
        entry: Box<ProcessEntry>,
    },
    Unknown,
}

#[cfg(test)]
#[path = "process_manager_tests.rs"]
mod tests;
