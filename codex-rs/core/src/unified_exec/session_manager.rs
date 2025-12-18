use rand::Rng;
use std::cmp::Reverse;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Notify;
use tokio::sync::mpsc;
use tokio::time::Duration;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;

use crate::bash::extract_bash_command;
use crate::codex::Session;
use crate::codex::TurnContext;
use crate::exec_env::create_env;
use crate::exec_policy::create_exec_approval_requirement_for_command;
use crate::protocol::BackgroundEventEvent;
use crate::protocol::EventMsg;
use crate::sandboxing::ExecEnv;
use crate::sandboxing::SandboxPermissions;
use crate::tools::orchestrator::ToolOrchestrator;
use crate::tools::runtimes::unified_exec::UnifiedExecRequest as UnifiedExecToolRequest;
use crate::tools::runtimes::unified_exec::UnifiedExecRuntime;
use crate::tools::sandboxing::ToolCtx;
use crate::truncate::TruncationPolicy;
use crate::truncate::approx_token_count;
use crate::truncate::formatted_truncate_text;

use super::CommandTranscript;
use super::ExecCommandRequest;
use super::MAX_UNIFIED_EXEC_SESSIONS;
use super::SessionEntry;
use super::SessionStore;
use super::UnifiedExecContext;
use super::UnifiedExecError;
use super::UnifiedExecResponse;
use super::UnifiedExecSessionManager;
use super::WARNING_UNIFIED_EXEC_SESSIONS;
use super::WriteStdinRequest;
use super::async_watcher::emit_exec_end_for_unified_exec;
use super::async_watcher::spawn_exit_watcher;
use super::async_watcher::start_streaming_output;
use super::clamp_yield_time;
use super::generate_chunk_id;
use super::resolve_max_tokens;
use super::session::OutputBuffer;
use super::session::OutputHandles;
use super::session::UnifiedExecSession;

const UNIFIED_EXEC_ENV: [(&str, &str); 8] = [
    ("NO_COLOR", "1"),
    ("TERM", "dumb"),
    ("LANG", "C.UTF-8"),
    ("LC_CTYPE", "C.UTF-8"),
    ("LC_ALL", "C.UTF-8"),
    ("COLORTERM", ""),
    ("PAGER", "cat"),
    ("GIT_PAGER", "cat"),
];

fn apply_unified_exec_env(mut env: HashMap<String, String>) -> HashMap<String, String> {
    for (key, value) in UNIFIED_EXEC_ENV {
        env.insert(key.to_string(), value.to_string());
    }
    env
}

struct PreparedSessionHandles {
    writer_tx: mpsc::Sender<Vec<u8>>,
    output_buffer: OutputBuffer,
    output_notify: Arc<Notify>,
    cancellation_token: CancellationToken,
    session_ref: Arc<Session>,
    turn_ref: Arc<TurnContext>,
    command: Vec<String>,
    process_id: String,
}

impl UnifiedExecSessionManager {
    pub(crate) async fn allocate_process_id(&self) -> String {
        loop {
            let mut store = self.session_store.lock().await;

            let process_id = if !cfg!(test) && !cfg!(feature = "deterministic_process_ids") {
                // production mode → random
                rand::rng().random_range(1_000..100_000).to_string()
            } else {
                // test or deterministic mode
                let next = store
                    .reserved_sessions_id
                    .iter()
                    .filter_map(|s| s.parse::<i32>().ok())
                    .max()
                    .map(|m| std::cmp::max(m, 999) + 1)
                    .unwrap_or(1000);

                next.to_string()
            };

            if store.reserved_sessions_id.contains(&process_id) {
                continue;
            }

            store.reserved_sessions_id.insert(process_id.clone());
            return process_id;
        }
    }

    pub(crate) async fn release_process_id(&self, process_id: &str) {
        let mut store = self.session_store.lock().await;
        store.remove(process_id);
    }

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
                request.sandbox_permissions,
                request.justification,
                context,
            )
            .await;

        let session = match session {
            Ok(session) => Arc::new(session),
            Err(err) => {
                self.release_process_id(&request.process_id).await;
                return Err(err);
            }
        };

        let transcript = Arc::new(tokio::sync::Mutex::new(CommandTranscript::default()));
        start_streaming_output(&session, context, Arc::clone(&transcript));

        let max_tokens = resolve_max_tokens(request.max_output_tokens);
        let yield_time_ms = clamp_yield_time(request.yield_time_ms);

        let start = Instant::now();
        // For the initial exec_command call, we both stream output to events
        // (via start_streaming_output above) and collect a snapshot here for
        // the tool response body.
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
        let exit_code = session.exit_code();
        let has_exited = session.has_exited() || exit_code.is_some();
        let chunk_id = generate_chunk_id();
        let process_id = request.process_id.clone();
        if has_exited {
            // Short‑lived command: emit ExecCommandEnd immediately using the
            // same helper as the background watcher, so all end events share
            // one implementation.
            let exit = exit_code.unwrap_or(-1);
            emit_exec_end_for_unified_exec(
                Arc::clone(&context.session),
                Arc::clone(&context.turn),
                context.call_id.clone(),
                request.command.clone(),
                cwd,
                Some(process_id),
                Arc::clone(&transcript),
                output.clone(),
                exit,
                wall_time,
            )
            .await;

            self.release_process_id(&request.process_id).await;
            session.check_for_sandbox_denial_with_text(&text).await?;
        } else {
            // Long‑lived command: persist the session so write_stdin can reuse
            // it, and register a background watcher that will emit
            // ExecCommandEnd when the PTY eventually exits (even if no further
            // tool calls are made).
            self.store_session(
                Arc::clone(&session),
                context,
                &request.command,
                cwd.clone(),
                start,
                process_id,
                Arc::clone(&transcript),
            )
            .await;

            Self::emit_waiting_status(&context.session, &context.turn, &request.command).await;
        };

        let original_token_count = approx_token_count(&text);
        let response = UnifiedExecResponse {
            event_call_id: context.call_id.clone(),
            chunk_id,
            wall_time,
            output,
            raw_output: collected,
            process_id: if has_exited {
                None
            } else {
                Some(request.process_id.clone())
            },
            exit_code,
            original_token_count: Some(original_token_count),
            session_command: Some(request.command.clone()),
        };

        Ok(response)
    }

    pub(crate) async fn write_stdin(
        &self,
        request: WriteStdinRequest<'_>,
    ) -> Result<UnifiedExecResponse, UnifiedExecError> {
        let process_id = request.process_id.to_string();

        let PreparedSessionHandles {
            writer_tx,
            output_buffer,
            output_notify,
            cancellation_token,
            session_ref,
            turn_ref,
            command: session_command,
            process_id,
            ..
        } = self.prepare_session_handles(process_id.as_str()).await?;

        if !request.input.is_empty() {
            Self::send_input(&writer_tx, request.input.as_bytes()).await?;
            // Give the remote process a brief window to react so that we are
            // more likely to capture its output in the poll below.
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

        // After polling, refresh_session_state tells us whether the PTY is
        // still alive or has exited and been removed from the store; we thread
        // that through so the handler can tag TerminalInteraction with an
        // appropriate process_id and exit_code.
        let status = self.refresh_session_state(process_id.as_str()).await;
        let (process_id, exit_code, event_call_id) = match status {
            SessionStatus::Alive {
                exit_code,
                call_id,
                process_id,
            } => (Some(process_id), exit_code, call_id),
            SessionStatus::Exited { exit_code, entry } => {
                let call_id = entry.call_id.clone();
                (None, exit_code, call_id)
            }
            SessionStatus::Unknown => {
                return Err(UnifiedExecError::UnknownSessionId {
                    process_id: request.process_id.to_string(),
                });
            }
        };

        let response = UnifiedExecResponse {
            event_call_id,
            chunk_id,
            wall_time,
            output,
            raw_output: collected,
            process_id,
            exit_code,
            original_token_count: Some(original_token_count),
            session_command: Some(session_command.clone()),
        };

        if response.process_id.is_some() {
            Self::emit_waiting_status(&session_ref, &turn_ref, &session_command).await;
        }

        Ok(response)
    }

    async fn refresh_session_state(&self, process_id: &str) -> SessionStatus {
        let mut store = self.session_store.lock().await;
        let Some(entry) = store.sessions.get(process_id) else {
            return SessionStatus::Unknown;
        };

        let exit_code = entry.session.exit_code();
        let process_id = entry.process_id.clone();

        if entry.session.has_exited() {
            let Some(entry) = store.remove(&process_id) else {
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
                process_id,
            }
        }
    }

    async fn prepare_session_handles(
        &self,
        process_id: &str,
    ) -> Result<PreparedSessionHandles, UnifiedExecError> {
        let mut store = self.session_store.lock().await;
        let entry =
            store
                .sessions
                .get_mut(process_id)
                .ok_or(UnifiedExecError::UnknownSessionId {
                    process_id: process_id.to_string(),
                })?;
        entry.last_used = Instant::now();
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
            process_id: entry.process_id.clone(),
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

    #[allow(clippy::too_many_arguments)]
    async fn store_session(
        &self,
        session: Arc<UnifiedExecSession>,
        context: &UnifiedExecContext,
        command: &[String],
        cwd: PathBuf,
        started_at: Instant,
        process_id: String,
        transcript: Arc<tokio::sync::Mutex<CommandTranscript>>,
    ) {
        let entry = SessionEntry {
            session: Arc::clone(&session),
            session_ref: Arc::clone(&context.session),
            turn_ref: Arc::clone(&context.turn),
            call_id: context.call_id.clone(),
            process_id: process_id.clone(),
            command: command.to_vec(),
            last_used: started_at,
        };
        let number_sessions = {
            let mut store = self.session_store.lock().await;
            Self::prune_sessions_if_needed(&mut store);
            store.sessions.insert(process_id.clone(), entry);
            store.sessions.len()
        };

        if number_sessions >= WARNING_UNIFIED_EXEC_SESSIONS {
            context
                .session
                .record_model_warning(
                    format!("The maximum number of unified exec sessions you can keep open is {WARNING_UNIFIED_EXEC_SESSIONS} and you currently have {number_sessions} sessions open. Reuse older sessions or close them to prevent automatic pruning of old session"),
                    &context.turn
                )
                .await;
        };

        spawn_exit_watcher(
            Arc::clone(&session),
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

    async fn emit_waiting_status(
        session: &Arc<Session>,
        turn: &Arc<TurnContext>,
        command: &[String],
    ) {
        let command_display = if let Some((_, script)) = extract_bash_command(command) {
            script.to_string()
        } else {
            command.join(" ")
        };
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
        sandbox_permissions: SandboxPermissions,
        justification: Option<String>,
        context: &UnifiedExecContext,
    ) -> Result<UnifiedExecSession, UnifiedExecError> {
        let env = apply_unified_exec_env(create_env(&context.turn.shell_environment_policy));
        let features = context.session.features();
        let mut orchestrator = ToolOrchestrator::new();
        let mut runtime = UnifiedExecRuntime::new(self);
        let exec_approval_requirement = create_exec_approval_requirement_for_command(
            &context.turn.exec_policy,
            &features,
            command,
            context.turn.approval_policy,
            &context.turn.sandbox_policy,
            sandbox_permissions,
        )
        .await;
        let req = UnifiedExecToolRequest::new(
            command.to_vec(),
            cwd,
            env,
            sandbox_permissions,
            justification,
            exec_approval_requirement,
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
        const POST_EXIT_OUTPUT_GRACE: Duration = Duration::from_millis(50);

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

    fn prune_sessions_if_needed(store: &mut SessionStore) -> bool {
        if store.sessions.len() < MAX_UNIFIED_EXEC_SESSIONS {
            return false;
        }

        let meta: Vec<(String, Instant, bool)> = store
            .sessions
            .iter()
            .map(|(id, entry)| (id.clone(), entry.last_used, entry.session.has_exited()))
            .collect();

        if let Some(session_id) = Self::session_id_to_prune_from_meta(&meta) {
            if let Some(entry) = store.remove(&session_id) {
                entry.session.terminate();
            }
            return true;
        }

        false
    }

    // Centralized pruning policy so we can easily swap strategies later.
    fn session_id_to_prune_from_meta(meta: &[(String, Instant, bool)]) -> Option<String> {
        if meta.is_empty() {
            return None;
        }

        let mut by_recency = meta.to_vec();
        by_recency.sort_by_key(|(_, last_used, _)| Reverse(*last_used));
        let protected: HashSet<String> = by_recency
            .iter()
            .take(8)
            .map(|(process_id, _, _)| process_id.clone())
            .collect();

        let mut lru = meta.to_vec();
        lru.sort_by_key(|(_, last_used, _)| *last_used);

        if let Some((process_id, _, _)) = lru
            .iter()
            .find(|(process_id, _, exited)| !protected.contains(process_id) && *exited)
        {
            return Some(process_id.clone());
        }

        lru.into_iter()
            .find(|(process_id, _, _)| !protected.contains(process_id))
            .map(|(process_id, _, _)| process_id)
    }

    pub(crate) async fn terminate_all_sessions(&self) {
        let entries: Vec<SessionEntry> = {
            let mut sessions = self.session_store.lock().await;
            let entries: Vec<SessionEntry> =
                sessions.sessions.drain().map(|(_, entry)| entry).collect();
            sessions.reserved_sessions_id.clear();
            entries
        };

        for entry in entries {
            entry.session.terminate();
        }
    }
}

enum SessionStatus {
    Alive {
        exit_code: Option<i32>,
        call_id: String,
        process_id: String,
    },
    Exited {
        exit_code: Option<i32>,
        entry: Box<SessionEntry>,
    },
    Unknown,
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use tokio::time::Duration;
    use tokio::time::Instant;

    #[test]
    fn unified_exec_env_injects_defaults() {
        let env = apply_unified_exec_env(HashMap::new());
        let expected = HashMap::from([
            ("NO_COLOR".to_string(), "1".to_string()),
            ("TERM".to_string(), "dumb".to_string()),
            ("LANG".to_string(), "C.UTF-8".to_string()),
            ("LC_CTYPE".to_string(), "C.UTF-8".to_string()),
            ("LC_ALL".to_string(), "C.UTF-8".to_string()),
            ("COLORTERM".to_string(), String::new()),
            ("PAGER".to_string(), "cat".to_string()),
            ("GIT_PAGER".to_string(), "cat".to_string()),
        ]);

        assert_eq!(env, expected);
    }

    #[test]
    fn unified_exec_env_overrides_existing_values() {
        let mut base = HashMap::new();
        base.insert("NO_COLOR".to_string(), "0".to_string());
        base.insert("PATH".to_string(), "/usr/bin".to_string());

        let env = apply_unified_exec_env(base);

        assert_eq!(env.get("NO_COLOR"), Some(&"1".to_string()));
        assert_eq!(env.get("PATH"), Some(&"/usr/bin".to_string()));
    }

    #[test]
    fn pruning_prefers_exited_sessions_outside_recently_used() {
        let now = Instant::now();
        let id = |n: i32| n.to_string();
        let meta = vec![
            (id(1), now - Duration::from_secs(40), false),
            (id(2), now - Duration::from_secs(30), true),
            (id(3), now - Duration::from_secs(20), false),
            (id(4), now - Duration::from_secs(19), false),
            (id(5), now - Duration::from_secs(18), false),
            (id(6), now - Duration::from_secs(17), false),
            (id(7), now - Duration::from_secs(16), false),
            (id(8), now - Duration::from_secs(15), false),
            (id(9), now - Duration::from_secs(14), false),
            (id(10), now - Duration::from_secs(13), false),
        ];

        let candidate = UnifiedExecSessionManager::session_id_to_prune_from_meta(&meta);

        assert_eq!(candidate, Some(id(2)));
    }

    #[test]
    fn pruning_falls_back_to_lru_when_no_exited() {
        let now = Instant::now();
        let id = |n: i32| n.to_string();
        let meta = vec![
            (id(1), now - Duration::from_secs(40), false),
            (id(2), now - Duration::from_secs(30), false),
            (id(3), now - Duration::from_secs(20), false),
            (id(4), now - Duration::from_secs(19), false),
            (id(5), now - Duration::from_secs(18), false),
            (id(6), now - Duration::from_secs(17), false),
            (id(7), now - Duration::from_secs(16), false),
            (id(8), now - Duration::from_secs(15), false),
            (id(9), now - Duration::from_secs(14), false),
            (id(10), now - Duration::from_secs(13), false),
        ];

        let candidate = UnifiedExecSessionManager::session_id_to_prune_from_meta(&meta);

        assert_eq!(candidate, Some(id(1)));
    }

    #[test]
    fn pruning_protects_recent_sessions_even_if_exited() {
        let now = Instant::now();
        let id = |n: i32| n.to_string();
        let meta = vec![
            (id(1), now - Duration::from_secs(40), false),
            (id(2), now - Duration::from_secs(30), false),
            (id(3), now - Duration::from_secs(20), true),
            (id(4), now - Duration::from_secs(19), false),
            (id(5), now - Duration::from_secs(18), false),
            (id(6), now - Duration::from_secs(17), false),
            (id(7), now - Duration::from_secs(16), false),
            (id(8), now - Duration::from_secs(15), false),
            (id(9), now - Duration::from_secs(14), false),
            (id(10), now - Duration::from_secs(13), true),
        ];

        let candidate = UnifiedExecSessionManager::session_id_to_prune_from_meta(&meta);

        // (10) is exited but among the last 8; we should drop the LRU outside that set.
        assert_eq!(candidate, Some(id(1)));
    }
}
