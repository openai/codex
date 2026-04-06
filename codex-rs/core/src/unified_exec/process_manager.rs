use rand::Rng;
use std::cmp::Reverse;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use tokio::sync::Notify;
use tokio::sync::watch;
use tokio::time::Duration;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;

use crate::command_canonicalization::canonicalize_command_for_approval;
use crate::exec_env::create_env;
use crate::exec_policy::ExecApprovalRequest;
use crate::sandboxing::ExecRequest;
use crate::tools::context::ExecCommandToolOutput;
use crate::tools::events::ToolEmitter;
use crate::tools::events::ToolEventCtx;
use crate::tools::events::ToolEventStage;
use crate::tools::network_approval::DeferredNetworkApproval;
use crate::tools::network_approval::finish_deferred_network_approval;
use crate::tools::orchestrator::ToolOrchestrator;
use crate::tools::runtimes::unified_exec::UnifiedExecApprovalKey;
use crate::tools::runtimes::unified_exec::UnifiedExecRequest as UnifiedExecToolRequest;
use crate::tools::runtimes::unified_exec::UnifiedExecRuntime;
use crate::tools::sandboxing::ToolCtx;
use crate::tools::sandboxing::with_cached_approval;
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
use codex_app_server_protocol::CommandExecutionApprovalDecision;
use codex_protocol::protocol::ExecCommandSource;
use codex_protocol::protocol::ReviewDecision;
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

/// Borrowed process state prepared for a `write_stdin` or poll operation.
struct PreparedProcessHandles {
    process: Arc<UnifiedExecProcess>,
    output_buffer: OutputBuffer,
    output_notify: Arc<Notify>,
    output_closed: Arc<AtomicBool>,
    output_closed_notify: Arc<Notify>,
    cancellation_token: CancellationToken,
    pause_state: Option<watch::Receiver<bool>>,
    command: Vec<String>,
    process_id: i32,
    tty: bool,
}

fn exec_server_process_id(process_id: i32) -> String {
    process_id.to_string()
}

fn startup_exec_approval_request(
    context: &UnifiedExecContext,
    request: &ExecCommandRequest,
    cwd: &Path,
    exec_approval_requirement: &crate::tools::sandboxing::ExecApprovalRequirement,
) -> Option<codex_exec_server::ExecApprovalRequest> {
    match exec_approval_requirement {
        crate::tools::sandboxing::ExecApprovalRequirement::NeedsApproval {
            reason,
            proposed_execpolicy_amendment,
        } => Some(codex_exec_server::ExecApprovalRequest {
            call_id: context.call_id.clone(),
            approval_id: None,
            turn_id: context.turn.sub_id.clone(),
            command: request.command.clone(),
            cwd: cwd.to_path_buf(),
            reason: reason.clone().or_else(|| request.justification.clone()),
            additional_permissions: request.additional_permissions.clone().map(Into::into),
            proposed_execpolicy_amendment: proposed_execpolicy_amendment
                .clone()
                .map(codex_app_server_protocol::ExecPolicyAmendment::from),
            available_decisions: None,
        }),
        crate::tools::sandboxing::ExecApprovalRequirement::Skip { .. }
        | crate::tools::sandboxing::ExecApprovalRequirement::Forbidden { .. } => None,
    }
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
        if let Some(network_approval_id) = entry.network_approval_id.as_deref()
            && let Some(session) = entry.session.upgrade()
        {
            session
                .services
                .network_approval
                .unregister_call(network_approval_id)
                .await;
        }
    }

    pub(crate) async fn exec_command(
        &self,
        request: ExecCommandRequest,
        context: &UnifiedExecContext,
    ) -> Result<ExecCommandToolOutput, UnifiedExecError> {
        let cwd = request
            .workdir
            .clone()
            .unwrap_or_else(|| context.turn.cwd.to_path_buf());
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
            let network_approval_id = deferred_network_approval
                .as_ref()
                .map(|deferred| deferred.registration_id().to_string());
            self.store_process(
                Arc::clone(&process),
                context,
                &request.command,
                cwd.clone(),
                start,
                request.process_id,
                request.tty,
                network_approval_id,
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
        let mut collected = Self::collect_output_until_deadline(
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

        if let Some(exec_approval) = process.take_pending_exec_approval().await {
            let call_id = exec_approval.call_id.clone();
            let approval_id = exec_approval.approval_id.clone();
            let effective_approval_id = approval_id.clone().unwrap_or_else(|| call_id.clone());
            let approval_keys = vec![UnifiedExecApprovalKey {
                command: canonicalize_command_for_approval(&request.command),
                cwd: cwd.clone(),
                tty: request.tty,
                sandbox_permissions: request.sandbox_permissions,
                additional_permissions: request.additional_permissions.clone(),
            }];
            let decision = with_cached_approval(
                &context.session.services,
                "unified_exec",
                approval_keys,
                || async move {
                    context
                        .session
                        .request_command_approval(
                            &context.turn,
                            call_id,
                            approval_id,
                            exec_approval.command,
                            exec_approval.cwd,
                            exec_approval.reason,
                            /*network_approval_context*/ None,
                            exec_approval
                                .proposed_execpolicy_amendment
                                .map(codex_app_server_protocol::ExecPolicyAmendment::into_core),
                            exec_approval.additional_permissions.map(Into::into),
                            exec_approval.available_decisions.map(|decisions| {
                                decisions
                                    .into_iter()
                                    .map(command_execution_approval_decision_to_core)
                                    .collect()
                            }),
                        )
                        .await
                },
            )
            .await;
            process
                .resolve_exec_approval(
                    effective_approval_id,
                    command_execution_approval_decision_from_core(decision),
                )
                .await?;
            let remaining_deadline = deadline.max(Instant::now() + Duration::from_millis(1));
            let additional_output = Self::collect_output_until_deadline(
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
                remaining_deadline,
            )
            .await;
            collected.extend(additional_output);
        }
        let wall_time = Instant::now().saturating_duration_since(start);

        let text = String::from_utf8_lossy(&collected).to_string();
        let chunk_id = generate_chunk_id();
        if let Some(message) = process.failure_message() {
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
            finish_deferred_network_approval(
                context.session.as_ref(),
                deferred_network_approval.take(),
            )
            .await;
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
            finish_deferred_network_approval(
                context.session.as_ref(),
                deferred_network_approval.take(),
            )
            .await;
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
            session_command: Some(request.command.clone()),
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
            command: session_command,
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
        if let Some(message) = process.failure_message() {
            self.release_process_id(process_id).await;
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
            session_command: Some(session_command.clone()),
        };

        Ok(response)
    }

    async fn refresh_process_state(&self, process_id: i32) -> ProcessStatus {
        let status = {
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
        };
        if let ProcessStatus::Exited { entry, .. } = &status {
            Self::unregister_network_approval_for_entry(entry).await;
        }
        status
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

        Ok(PreparedProcessHandles {
            process: Arc::clone(&entry.process),
            output_buffer,
            output_notify,
            output_closed,
            output_closed_notify,
            cancellation_token,
            pause_state,
            command: entry.command.clone(),
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
        cwd: PathBuf,
        started_at: Instant,
        process_id: i32,
        tty: bool,
        network_approval_id: Option<String>,
        transcript: Arc<tokio::sync::Mutex<HeadTailBuffer>>,
    ) {
        let entry = ProcessEntry {
            process: Arc::clone(&process),
            call_id: context.call_id.clone(),
            process_id,
            command: command.to_vec(),
            tty,
            network_approval_id,
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
        env: &ExecRequest,
        tty: bool,
        startup_exec_approval: Option<codex_exec_server::ExecApprovalRequest>,
        mut spawn_lifecycle: SpawnLifecycleHandle,
        environment: &codex_exec_server::Environment,
    ) -> Result<UnifiedExecProcess, UnifiedExecError> {
        let (program, args) = env
            .command
            .split_first()
            .ok_or(UnifiedExecError::MissingCommandLine)?;
        let inherited_fds = spawn_lifecycle.inherited_fds();

        if environment.exec_server_url().is_some() {
            if !inherited_fds.is_empty() {
                return Err(UnifiedExecError::create_process(
                    "remote exec-server does not support inherited file descriptors".to_string(),
                ));
            }

            let started = environment
                .get_exec_backend()
                .start(codex_exec_server::ExecParams {
                    process_id: exec_server_process_id(process_id).into(),
                    argv: env.command.clone(),
                    cwd: env.cwd.clone(),
                    env: env.env.clone(),
                    tty,
                    arg0: env.arg0.clone(),
                    startup_exec_approval,
                })
                .await
                .map_err(|err| UnifiedExecError::create_process(err.to_string()))?;
            return UnifiedExecProcess::from_remote_started(started, env.sandbox).await;
        }

        let spawn_result = if tty {
            codex_utils_pty::pty::spawn_process_with_inherited_fds(
                program,
                args,
                env.cwd.as_path(),
                &env.env,
                &env.arg0,
                codex_utils_pty::TerminalSize::default(),
                &inherited_fds,
            )
            .await
        } else {
            codex_utils_pty::pipe::spawn_process_no_stdin_with_inherited_fds(
                program,
                args,
                env.cwd.as_path(),
                &env.env,
                &env.arg0,
                &inherited_fds,
            )
            .await
        };
        let spawned =
            spawn_result.map_err(|err| UnifiedExecError::create_process(err.to_string()))?;
        spawn_lifecycle.after_spawn();
        UnifiedExecProcess::from_spawned(spawned, env.sandbox, spawn_lifecycle).await
    }

    pub(super) async fn open_session_with_sandbox(
        &self,
        request: &ExecCommandRequest,
        cwd: PathBuf,
        context: &UnifiedExecContext,
    ) -> Result<(UnifiedExecProcess, Option<DeferredNetworkApproval>), UnifiedExecError> {
        let env = apply_unified_exec_env(create_env(
            &context.turn.shell_environment_policy,
            Some(context.session.conversation_id),
        ));
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
        let remote_startup_exec_approval = if context.turn.environment.exec_server_url().is_some() {
            startup_exec_approval_request(context, request, &cwd, &exec_approval_requirement)
        } else {
            None
        };
        let req = UnifiedExecToolRequest {
            command: request.command.clone(),
            process_id: request.process_id,
            cwd,
            env,
            explicit_env_overrides: context.turn.shell_environment_policy.r#set.clone(),
            network: request.network.clone(),
            tty: request.tty,
            sandbox_permissions: request.sandbox_permissions,
            additional_permissions: request.additional_permissions.clone(),
            #[cfg(unix)]
            additional_permissions_preapproved: request.additional_permissions_preapproved,
            justification: request.justification.clone(),
            exec_approval_requirement,
            remote_startup_exec_approval,
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
            .map_err(|e| UnifiedExecError::create_process(format!("{e:?}")))
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

fn command_execution_approval_decision_to_core(
    decision: CommandExecutionApprovalDecision,
) -> ReviewDecision {
    match decision {
        CommandExecutionApprovalDecision::Accept => ReviewDecision::Approved,
        CommandExecutionApprovalDecision::AcceptForSession => ReviewDecision::ApprovedForSession,
        CommandExecutionApprovalDecision::AcceptWithExecpolicyAmendment {
            execpolicy_amendment,
        } => ReviewDecision::ApprovedExecpolicyAmendment {
            proposed_execpolicy_amendment: execpolicy_amendment.into_core(),
        },
        CommandExecutionApprovalDecision::ApplyNetworkPolicyAmendment {
            network_policy_amendment,
        } => ReviewDecision::NetworkPolicyAmendment {
            network_policy_amendment: network_policy_amendment.into_core(),
        },
        CommandExecutionApprovalDecision::Decline => ReviewDecision::Denied,
        CommandExecutionApprovalDecision::Cancel => ReviewDecision::Abort,
    }
}

fn command_execution_approval_decision_from_core(
    decision: ReviewDecision,
) -> CommandExecutionApprovalDecision {
    match decision {
        ReviewDecision::Approved => CommandExecutionApprovalDecision::Accept,
        ReviewDecision::ApprovedExecpolicyAmendment {
            proposed_execpolicy_amendment,
        } => CommandExecutionApprovalDecision::AcceptWithExecpolicyAmendment {
            execpolicy_amendment: proposed_execpolicy_amendment.into(),
        },
        ReviewDecision::ApprovedForSession => CommandExecutionApprovalDecision::AcceptForSession,
        ReviewDecision::NetworkPolicyAmendment {
            network_policy_amendment,
        } => CommandExecutionApprovalDecision::ApplyNetworkPolicyAmendment {
            network_policy_amendment: network_policy_amendment.into(),
        },
        ReviewDecision::Denied => CommandExecutionApprovalDecision::Decline,
        ReviewDecision::Abort => CommandExecutionApprovalDecision::Cancel,
    }
}

#[cfg(test)]
#[path = "process_manager_tests.rs"]
mod tests;
