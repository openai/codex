use std::sync::Arc;

use tokio::sync::Notify;
use tokio::sync::mpsc;
use tokio::time::Duration;
use tokio::time::Instant;

use crate::exec_env::create_env;
use crate::sandboxing::ExecEnv;
use crate::tools::orchestrator::ToolOrchestrator;
use crate::tools::runtimes::unified_exec::UnifiedExecRequest as UnifiedExecToolRequest;
use crate::tools::runtimes::unified_exec::UnifiedExecRuntime;
use crate::tools::sandboxing::ToolCtx;
use crate::truncate::truncate_middle;

use super::DEFAULT_TIMEOUT_MS;
use super::MAX_TIMEOUT_MS;
use super::UNIFIED_EXEC_OUTPUT_MAX_BYTES;
use super::UnifiedExecContext;
use super::UnifiedExecError;
use super::UnifiedExecRequest;
use super::UnifiedExecResult;
use super::UnifiedExecSessionManager;
use super::session::OutputBuffer;
use super::session::UnifiedExecSession;

pub(super) struct SessionAcquisition {
    pub(super) session_id: i32,
    pub(super) writer_tx: mpsc::Sender<Vec<u8>>,
    pub(super) output_buffer: OutputBuffer,
    pub(super) output_notify: Arc<Notify>,
    pub(super) new_session: Option<UnifiedExecSession>,
    pub(super) reuse_requested: bool,
}

impl UnifiedExecSessionManager {
    pub(super) async fn acquire_session(
        &self,
        request: &UnifiedExecRequest<'_>,
        context: &UnifiedExecContext<'_>,
    ) -> Result<SessionAcquisition, UnifiedExecError> {
        if let Some(existing_id) = context.session_id {
            let mut sessions = self.sessions.lock().await;
            match sessions.get(&existing_id) {
                Some(session) => {
                    if session.has_exited() {
                        sessions.remove(&existing_id);
                        return Err(UnifiedExecError::UnknownSessionId {
                            session_id: existing_id,
                        });
                    }
                    let (buffer, notify) = session.output_handles();
                    let writer_tx = session.writer_sender();
                    Ok(SessionAcquisition {
                        session_id: existing_id,
                        writer_tx,
                        output_buffer: buffer,
                        output_notify: notify,
                        new_session: None,
                        reuse_requested: true,
                    })
                }
                None => Err(UnifiedExecError::UnknownSessionId {
                    session_id: existing_id,
                }),
            }
        } else {
            let new_id = self
                .next_session_id
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let managed_session = self
                .open_session_with_sandbox(request.input_chunks.to_vec(), context)
                .await?;
            let (buffer, notify) = managed_session.output_handles();
            let writer_tx = managed_session.writer_sender();
            Ok(SessionAcquisition {
                session_id: new_id,
                writer_tx,
                output_buffer: buffer,
                output_notify: notify,
                new_session: Some(managed_session),
                reuse_requested: false,
            })
        }
    }

    pub(crate) async fn open_session_with_exec_env(
        &self,
        env: &ExecEnv,
    ) -> Result<UnifiedExecSession, UnifiedExecError> {
        let (program, args) = env
            .command
            .split_first()
            .ok_or(UnifiedExecError::MissingCommandLine)?;
        let spawned =
            codex_utils_pty::spawn_pty_process(program, args, env.cwd.as_path(), &env.env)
                .await
                .map_err(|err| UnifiedExecError::create_session(err.to_string()))?;
        UnifiedExecSession::from_spawned(spawned, env.sandbox).await
    }

    pub(super) async fn open_session_with_sandbox(
        &self,
        command: Vec<String>,
        context: &UnifiedExecContext<'_>,
    ) -> Result<UnifiedExecSession, UnifiedExecError> {
        let mut orchestrator = ToolOrchestrator::new();
        let mut runtime = UnifiedExecRuntime::new(self);
        let req = UnifiedExecToolRequest::new(
            command,
            context.turn.cwd.clone(),
            create_env(&context.turn.shell_environment_policy),
        );
        let tool_ctx = ToolCtx {
            session: context.session,
            sub_id: context.sub_id.to_string(),
            call_id: context.call_id.to_string(),
            tool_name: "unified_exec".to_string(),
        };
        orchestrator
            .run(
                &mut runtime,
                &req,
                &tool_ctx,
                context.turn,
                context.turn.approval_policy,
            )
            .await
            .map_err(|e| UnifiedExecError::create_session(format!("{e:?}")))
    }

    pub(super) async fn collect_output_until_deadline(
        output_buffer: &OutputBuffer,
        output_notify: &Arc<Notify>,
        deadline: Instant,
    ) -> Vec<u8> {
        let mut collected: Vec<u8> = Vec::with_capacity(4096);
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
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining == Duration::ZERO {
                    break;
                }

                let notified = wait_for_output.unwrap_or_else(|| output_notify.notified());
                tokio::pin!(notified);
                tokio::select! {
                    _ = &mut notified => {}
                    _ = tokio::time::sleep(remaining) => break,
                }
                continue;
            }

            for chunk in drained_chunks {
                collected.extend_from_slice(&chunk);
            }

            if Instant::now() >= deadline {
                break;
            }
        }

        collected
    }

    pub(super) async fn should_store_session(&self, acquisition: &SessionAcquisition) -> bool {
        if let Some(session) = acquisition.new_session.as_ref() {
            !session.has_exited()
        } else if acquisition.reuse_requested {
            let mut sessions = self.sessions.lock().await;
            if let Some(existing) = sessions.get(&acquisition.session_id) {
                if existing.has_exited() {
                    sessions.remove(&acquisition.session_id);
                    false
                } else {
                    true
                }
            } else {
                false
            }
        } else {
            true
        }
    }

    pub(super) async fn send_input_chunks(
        writer_tx: &mpsc::Sender<Vec<u8>>,
        chunks: &[String],
    ) -> Result<(), UnifiedExecError> {
        let mut trailing_whitespace = true;
        for chunk in chunks {
            if chunk.is_empty() {
                continue;
            }

            let leading_whitespace = chunk
                .chars()
                .next()
                .map(char::is_whitespace)
                .unwrap_or(true);

            if !trailing_whitespace
                && !leading_whitespace
                && writer_tx.send(vec![b' ']).await.is_err()
            {
                return Err(UnifiedExecError::WriteToStdin);
            }

            if writer_tx.send(chunk.as_bytes().to_vec()).await.is_err() {
                return Err(UnifiedExecError::WriteToStdin);
            }

            trailing_whitespace = chunk
                .chars()
                .next_back()
                .map(char::is_whitespace)
                .unwrap_or(trailing_whitespace);
        }

        Ok(())
    }

    pub async fn handle_request(
        &self,
        request: UnifiedExecRequest<'_>,
        context: UnifiedExecContext<'_>,
    ) -> Result<UnifiedExecResult, UnifiedExecError> {
        let (timeout_ms, timeout_warning) = match request.timeout_ms {
            Some(requested) if requested > MAX_TIMEOUT_MS => (
                MAX_TIMEOUT_MS,
                Some(format!(
                    "Warning: requested timeout {requested}ms exceeds maximum of {MAX_TIMEOUT_MS}ms; clamping to {MAX_TIMEOUT_MS}ms.\n"
                )),
            ),
            Some(requested) => (requested, None),
            None => (DEFAULT_TIMEOUT_MS, None),
        };

        let mut acquisition = self.acquire_session(&request, &context).await?;

        if acquisition.reuse_requested {
            Self::send_input_chunks(&acquisition.writer_tx, request.input_chunks).await?;
        }

        let deadline = Instant::now() + Duration::from_millis(timeout_ms);
        let collected = Self::collect_output_until_deadline(
            &acquisition.output_buffer,
            &acquisition.output_notify,
            deadline,
        )
        .await;

        let (output, _maybe_tokens) = truncate_middle(
            &String::from_utf8_lossy(&collected),
            UNIFIED_EXEC_OUTPUT_MAX_BYTES,
        );
        let output = if let Some(warning) = timeout_warning {
            format!("{warning}{output}")
        } else {
            output
        };

        let should_store_session = self.should_store_session(&acquisition).await;
        let session_id = if should_store_session {
            if let Some(session) = acquisition.new_session.take() {
                self.sessions
                    .lock()
                    .await
                    .insert(acquisition.session_id, session);
            }
            Some(acquisition.session_id)
        } else {
            None
        };

        Ok(UnifiedExecResult { session_id, output })
    }
}
