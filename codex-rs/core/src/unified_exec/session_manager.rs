use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::Notify;
use tokio::sync::mpsc;
use tokio::time::Duration;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;

use crate::codex::Session;
use crate::codex::TurnContext;
use crate::exec::ExecToolCallOutput;
use crate::exec::StreamOutput;
use crate::exec_env::create_env;
use crate::exec_policy::create_approval_requirement_for_command;
use crate::protocol::BackgroundEventEvent;
use crate::protocol::EventMsg;
use crate::protocol::ExecCommandSource;
use crate::sandboxing::ExecEnv;
use crate::sandboxing::SandboxPermissions;
use crate::tools::events::ToolEmitter;
use crate::tools::events::ToolEventCtx;
use crate::tools::events::ToolEventFailure;
use crate::tools::events::ToolEventStage;
use crate::tools::orchestrator::ToolOrchestrator;
use crate::tools::runtimes::unified_exec::UnifiedExecRequest as UnifiedExecToolRequest;
use crate::tools::runtimes::unified_exec::UnifiedExecRuntime;
use crate::tools::sandboxing::ToolCtx;
use crate::truncate::TruncationPolicy;
use crate::truncate::approx_token_count;
use crate::truncate::formatted_truncate_text;

use super::ExecCommandRequest;
use super::SessionEntry;
use super::UnifiedExecContext;
use super::UnifiedExecError;
use super::UnifiedExecResponse;
use super::UnifiedExecSessionManager;
use super::WriteStdinRequest;
use super::clamp_yield_time;
use super::generate_chunk_id;
use super::resolve_max_tokens;
use super::session::OutputBuffer;
use super::session::OutputHandles;
use super::session::UnifiedExecSession;

struct PreparedSessionHandles {
    writer_tx: mpsc::Sender<Vec<u8>>,
    output_buffer: OutputBuffer,
    output_notify: Arc<Notify>,
    cancellation_token: CancellationToken,
    session_ref: Arc<Session>,
    turn_ref: Arc<TurnContext>,
    command: Vec<String>,
    cwd: PathBuf,
}

impl UnifiedExecSessionManager {
    pub(crate) async fn exec_command(
        &self,
        request: ExecCommandRequest,
        context: &UnifiedExecContext,
    ) -> Result<UnifiedExecResponse, UnifiedExecError> {
        let cwd = request
            .workdir
            .clone()
            .unwrap_or_else(|| context.turn.cwd.clone());

        let session = self
            .open_session_with_sandbox(
                &request.command,
                cwd.clone(),
                request.with_escalated_permissions,
                request.justification,
                context,
            )
            .await?;

        let max_tokens = resolve_max_tokens(request.max_output_tokens);
        let yield_time_ms = clamp_yield_time(request.yield_time_ms);

        let start = Instant::now();
        let OutputHandles {
            output_buffer,
            output_notify,
            cancellation_token,
        } = session.output_handles();
        let deadline = start + Duration::from_millis(yield_time_ms);
        let collected = Self::collect_output_until_deadline(
            &output_buffer,
            &output_notify,
            &cancellation_token,
            deadline,
        )
        .await;
        let wall_time = Instant::now().saturating_duration_since(start);

        let text = String::from_utf8_lossy(&collected).to_string();
        let output = formatted_truncate_text(&text, TruncationPolicy::Tokens(max_tokens));
        let chunk_id = generate_chunk_id();
        let has_exited = session.has_exited();
        let stored_id = self
            .store_session(session, context, &request.command, cwd.clone(), start)
            .await;
        let exit_code = self
            .sessions
            .lock()
            .await
            .get(&stored_id)
            .map(|entry| entry.session.exit_code());
        // Only include a session_id in the response if the process is still alive.
        let session_id = if has_exited { None } else { Some(stored_id) };

        let original_token_count = approx_token_count(&text);

        let response = UnifiedExecResponse {
            event_call_id: context.call_id.clone(),
            chunk_id,
            wall_time,
            output,
            session_id,
            exit_code: exit_code.flatten(),
            original_token_count: Some(original_token_count),
            session_command: Some(request.command.clone()),
        };

        if response.session_id.is_some() {
            Self::emit_waiting_status(&context.session, &context.turn, &request.command).await;
        }

        // If the command completed during this call, emit an ExecCommandEnd via the emitter.
        if response.session_id.is_none() {
            let exit = response.exit_code.unwrap_or(-1);
            Self::emit_exec_end_from_context(
                context,
                &request.command,
                cwd,
                response.output.clone(),
                exit,
                response.wall_time,
            )
            .await;
        }

        Ok(response)
    }

    pub(crate) async fn write_stdin(
        &self,
        request: WriteStdinRequest<'_>,
    ) -> Result<UnifiedExecResponse, UnifiedExecError> {
        let session_id = request.session_id;

        let PreparedSessionHandles {
            writer_tx,
            output_buffer,
            output_notify,
            cancellation_token,
            session_ref,
            turn_ref,
            command: session_command,
            cwd: session_cwd,
        } = self.prepare_session_handles(session_id).await?;

        let interaction_emitter = ToolEmitter::unified_exec(
            &session_command,
            session_cwd.clone(),
            ExecCommandSource::UnifiedExecInteraction,
            (!request.input.is_empty()).then(|| request.input.to_string()),
        );
        let make_event_ctx = || {
            ToolEventCtx::new(
                session_ref.as_ref(),
                turn_ref.as_ref(),
                request.call_id,
                None,
            )
        };
        interaction_emitter
            .emit(make_event_ctx(), ToolEventStage::Begin)
            .await;

        if !request.input.is_empty() {
            if let Err(err) = Self::send_input(&writer_tx, request.input.as_bytes()).await {
                interaction_emitter
                    .emit(
                        make_event_ctx(),
                        ToolEventStage::Failure(ToolEventFailure::Message(format!(
                            "write_stdin failed: {err:?}"
                        ))),
                    )
                    .await;
                return Err(err);
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        let max_tokens = resolve_max_tokens(request.max_output_tokens);
        let yield_time_ms = clamp_yield_time(request.yield_time_ms);
        let start = Instant::now();
        let deadline = start + Duration::from_millis(yield_time_ms);
        let collected = Self::collect_output_until_deadline(
            &output_buffer,
            &output_notify,
            &cancellation_token,
            deadline,
        )
        .await;
        let wall_time = Instant::now().saturating_duration_since(start);

        let text = String::from_utf8_lossy(&collected).to_string();
        let output = formatted_truncate_text(&text, TruncationPolicy::Tokens(max_tokens));
        let original_token_count = approx_token_count(&text);
        let chunk_id = generate_chunk_id();

        let status = self.refresh_session_state(session_id).await;
        let (session_id, exit_code, completion_entry, event_call_id) = match status {
            SessionStatus::Alive { exit_code, call_id } => {
                (Some(session_id), exit_code, None, call_id)
            }
            SessionStatus::Exited { exit_code, entry } => {
                let call_id = entry.call_id.clone();
                (None, exit_code, Some(*entry), call_id)
            }
            SessionStatus::Unknown => {
                return Err(UnifiedExecError::UnknownSessionId { session_id });
            }
        };

        let response = UnifiedExecResponse {
            event_call_id,
            chunk_id,
            wall_time,
            output,
            session_id,
            exit_code,
            original_token_count: Some(original_token_count),
            session_command: Some(session_command.clone()),
        };

        let interaction_output = ExecToolCallOutput {
            exit_code: response.exit_code.unwrap_or(0),
            stdout: StreamOutput::new(response.output.clone()),
            stderr: StreamOutput::new(String::new()),
            aggregated_output: StreamOutput::new(response.output.clone()),
            duration: response.wall_time,
            timed_out: false,
        };
        interaction_emitter
            .emit(
                make_event_ctx(),
                ToolEventStage::Success(interaction_output),
            )
            .await;

        if response.session_id.is_some() {
            Self::emit_waiting_status(&session_ref, &turn_ref, &session_command).await;
        }

        if let (Some(exit), Some(entry)) = (response.exit_code, completion_entry) {
            let total_duration = Instant::now().saturating_duration_since(entry.started_at);
            Self::emit_exec_end_from_entry(entry, response.output.clone(), exit, total_duration)
                .await;
        }

        Ok(response)
    }

    async fn refresh_session_state(&self, session_id: i32) -> SessionStatus {
        let mut sessions = self.sessions.lock().await;
        let Some(entry) = sessions.get(&session_id) else {
            return SessionStatus::Unknown;
        };

        let exit_code = entry.session.exit_code();

        if entry.session.has_exited() {
            let Some(entry) = sessions.remove(&session_id) else {
                return SessionStatus::Unknown;
            };
            SessionStatus::Exited {
                exit_code,
                entry: Box::new(entry),
            }
        } else {
            SessionStatus::Alive {
                exit_code,
                call_id: entry.call_id.clone(),
            }
        }
    }

    async fn prepare_session_handles(
        &self,
        session_id: i32,
    ) -> Result<PreparedSessionHandles, UnifiedExecError> {
        let sessions = self.sessions.lock().await;
        let entry = sessions
            .get(&session_id)
            .ok_or(UnifiedExecError::UnknownSessionId { session_id })?;
        let OutputHandles {
            output_buffer,
            output_notify,
            cancellation_token,
        } = entry.session.output_handles();

        Ok(PreparedSessionHandles {
            writer_tx: entry.session.writer_sender(),
            output_buffer,
            output_notify,
            cancellation_token,
            session_ref: Arc::clone(&entry.session_ref),
            turn_ref: Arc::clone(&entry.turn_ref),
            command: entry.command.clone(),
            cwd: entry.cwd.clone(),
        })
    }

    async fn send_input(
        writer_tx: &mpsc::Sender<Vec<u8>>,
        data: &[u8],
    ) -> Result<(), UnifiedExecError> {
        writer_tx
            .send(data.to_vec())
            .await
            .map_err(|_| UnifiedExecError::WriteToStdin)
    }

    async fn store_session(
        &self,
        session: UnifiedExecSession,
        context: &UnifiedExecContext,
        command: &[String],
        cwd: PathBuf,
        started_at: Instant,
    ) -> i32 {
        let session_id = self
            .next_session_id
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let entry = SessionEntry {
            session,
            session_ref: Arc::clone(&context.session),
            turn_ref: Arc::clone(&context.turn),
            call_id: context.call_id.clone(),
            command: command.to_vec(),
            cwd,
            started_at,
        };
        self.sessions.lock().await.insert(session_id, entry);
        session_id
    }

    async fn emit_exec_end_from_entry(
        entry: SessionEntry,
        aggregated_output: String,
        exit_code: i32,
        duration: Duration,
    ) {
        let output = ExecToolCallOutput {
            exit_code,
            stdout: StreamOutput::new(aggregated_output.clone()),
            stderr: StreamOutput::new(String::new()),
            aggregated_output: StreamOutput::new(aggregated_output),
            duration,
            timed_out: false,
        };
        let event_ctx = ToolEventCtx::new(
            entry.session_ref.as_ref(),
            entry.turn_ref.as_ref(),
            &entry.call_id,
            None,
        );
        let emitter = ToolEmitter::unified_exec(
            &entry.command,
            entry.cwd,
            ExecCommandSource::UnifiedExecStartup,
            None,
        );
        emitter
            .emit(event_ctx, ToolEventStage::Success(output))
            .await;
    }

    async fn emit_exec_end_from_context(
        context: &UnifiedExecContext,
        command: &[String],
        cwd: PathBuf,
        aggregated_output: String,
        exit_code: i32,
        duration: Duration,
    ) {
        let output = ExecToolCallOutput {
            exit_code,
            stdout: StreamOutput::new(aggregated_output.clone()),
            stderr: StreamOutput::new(String::new()),
            aggregated_output: StreamOutput::new(aggregated_output),
            duration,
            timed_out: false,
        };
        let event_ctx = ToolEventCtx::new(
            context.session.as_ref(),
            context.turn.as_ref(),
            &context.call_id,
            None,
        );
        let emitter =
            ToolEmitter::unified_exec(command, cwd, ExecCommandSource::UnifiedExecStartup, None);
        emitter
            .emit(event_ctx, ToolEventStage::Success(output))
            .await;
    }

    async fn emit_waiting_status(
        session: &Arc<Session>,
        turn: &Arc<TurnContext>,
        command: &[String],
    ) {
        let command_display = command.join(" ");
        let message = format!("Waiting for `{command_display}`");
        session
            .send_event(
                turn.as_ref(),
                EventMsg::BackgroundEvent(BackgroundEventEvent { message }),
            )
            .await;
    }

    pub(crate) async fn open_session_with_exec_env(
        &self,
        env: &ExecEnv,
    ) -> Result<UnifiedExecSession, UnifiedExecError> {
        let (program, args) = env
            .command
            .split_first()
            .ok_or(UnifiedExecError::MissingCommandLine)?;

        let spawned = codex_utils_pty::spawn_pty_process(
            program,
            args,
            env.cwd.as_path(),
            &env.env,
            &env.arg0,
        )
        .await
        .map_err(|err| UnifiedExecError::create_session(err.to_string()))?;
        UnifiedExecSession::from_spawned(spawned, env.sandbox).await
    }

    pub(super) async fn open_session_with_sandbox(
        &self,
        command: &[String],
        cwd: PathBuf,
        with_escalated_permissions: Option<bool>,
        justification: Option<String>,
        context: &UnifiedExecContext,
    ) -> Result<UnifiedExecSession, UnifiedExecError> {
        let mut orchestrator = ToolOrchestrator::new();
        let mut runtime = UnifiedExecRuntime::new(self);
        let req = UnifiedExecToolRequest::new(
            command.to_vec(),
            cwd,
            create_env(&context.turn.shell_environment_policy),
            with_escalated_permissions,
            justification,
            create_approval_requirement_for_command(
                &context.turn.exec_policy,
                command,
                context.turn.approval_policy,
                &context.turn.sandbox_policy,
                SandboxPermissions::from(with_escalated_permissions.unwrap_or(false)),
            ),
        );
        let tool_ctx = ToolCtx {
            session: context.session.as_ref(),
            turn: context.turn.as_ref(),
            call_id: context.call_id.clone(),
            tool_name: "exec_command".to_string(),
        };
        orchestrator
            .run(
                &mut runtime,
                &req,
                &tool_ctx,
                context.turn.as_ref(),
                context.turn.approval_policy,
            )
            .await
            .map_err(|e| UnifiedExecError::create_session(format!("{e:?}")))
    }

    pub(super) async fn collect_output_until_deadline(
        output_buffer: &OutputBuffer,
        output_notify: &Arc<Notify>,
        cancellation_token: &CancellationToken,
        deadline: Instant,
    ) -> Vec<u8> {
        const POST_EXIT_OUTPUT_GRACE: Duration = Duration::from_millis(25);

        let mut collected: Vec<u8> = Vec::with_capacity(4096);
        let mut exit_signal_received = cancellation_token.is_cancelled();
        loop {
            let drained_chunks;
            let mut wait_for_output = None;
            {
                let mut guard = output_buffer.lock().await;
                drained_chunks = guard.drain();
                if drained_chunks.is_empty() {
                    wait_for_output = Some(output_notify.notified());
                }
            }

            if drained_chunks.is_empty() {
                exit_signal_received |= cancellation_token.is_cancelled();
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining == Duration::ZERO {
                    break;
                }

                let notified = wait_for_output.unwrap_or_else(|| output_notify.notified());
                if exit_signal_received {
                    let grace = remaining.min(POST_EXIT_OUTPUT_GRACE);
                    if tokio::time::timeout(grace, notified).await.is_err() {
                        break;
                    }
                    continue;
                }

                tokio::pin!(notified);
                let exit_notified = cancellation_token.cancelled();
                tokio::pin!(exit_notified);
                tokio::select! {
                    _ = &mut notified => {}
                    _ = &mut exit_notified => exit_signal_received = true,
                    _ = tokio::time::sleep(remaining) => break,
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
}

enum SessionStatus {
    Alive {
        exit_code: Option<i32>,
        call_id: String,
    },
    Exited {
        exit_code: Option<i32>,
        entry: Box<SessionEntry>,
    },
    Unknown,
}
