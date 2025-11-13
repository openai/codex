//! Unified Exec: interactive PTY execution orchestrated with approvals + sandboxing.
//!
//! Responsibilities
//! - Manages interactive PTY sessions (create, reuse, buffer output with caps).
//! - Uses the shared ToolOrchestrator to handle approval, sandbox selection, and
//!   retry semantics in a single, descriptive flow.
//! - Spawns the PTY from a sandbox‑transformed `ExecEnv`; on sandbox denial,
//!   retries without sandbox when policy allows (no re‑prompt thanks to caching).
//! - Uses the shared `is_likely_sandbox_denied` heuristic to keep denial messages
//!   consistent with other exec paths.
//!
//! Flow at a glance (open session)
//! 1) Build a small request `{ command, cwd }`.
//! 2) Orchestrator: approval (bypass/cache/prompt) → select sandbox → run.
//! 3) Runtime: transform `CommandSpec` → `ExecEnv` → spawn PTY.
//! 4) If denial, orchestrator retries with `SandboxType::None`.
//! 5) Session is returned with streaming output + metadata.
//!
//! This keeps policy logic and user interaction centralized while the PTY/session
//! concerns remain isolated here. The implementation is split between:
//! - `session.rs`: PTY session lifecycle + output buffering.
//! - `session_manager.rs`: orchestration (approvals, sandboxing, reuse) and request handling.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicI32;
use std::time::Duration;

use rand::Rng;
use rand::rng;
use tokio::sync::Mutex;

use crate::codex::Session;
use crate::codex::TurnContext;

mod errors;
mod session;
mod session_manager;

pub(crate) use errors::UnifiedExecError;
pub(crate) use session::UnifiedExecSession;

pub(crate) const DEFAULT_YIELD_TIME_MS: u64 = 10_000;
pub(crate) const MIN_YIELD_TIME_MS: u64 = 250;
pub(crate) const MAX_YIELD_TIME_MS: u64 = 30_000;
pub(crate) const DEFAULT_MAX_OUTPUT_TOKENS: usize = 10_000;
pub(crate) const UNIFIED_EXEC_OUTPUT_MAX_BYTES: usize = 1024 * 1024; // 1 MiB

pub(crate) struct UnifiedExecContext {
    pub session: Arc<Session>,
    pub turn: Arc<TurnContext>,
    pub call_id: String,
}

impl UnifiedExecContext {
    pub fn new(session: Arc<Session>, turn: Arc<TurnContext>, call_id: String) -> Self {
        Self {
            session,
            turn,
            call_id,
        }
    }
}

#[derive(Debug)]
pub(crate) struct ExecCommandRequest<'a> {
    pub command: &'a str,
    pub shell: &'a str,
    pub login: bool,
    pub yield_time_ms: Option<u64>,
    pub max_output_tokens: Option<usize>,
    pub workdir: Option<PathBuf>,
    pub with_escalated_permissions: Option<bool>,
    pub justification: Option<String>,
}

#[derive(Debug)]
pub(crate) struct WriteStdinRequest<'a> {
    pub session_id: i32,
    pub input: &'a str,
    pub yield_time_ms: Option<u64>,
    pub max_output_tokens: Option<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct UnifiedExecResponse {
    pub event_call_id: String,
    pub chunk_id: String,
    pub wall_time: Duration,
    pub output: String,
    pub session_id: Option<i32>,
    pub exit_code: Option<i32>,
    pub original_token_count: Option<usize>,
}

#[derive(Default)]
pub(crate) struct UnifiedExecSessionManager {
    next_session_id: AtomicI32,
    sessions: Mutex<HashMap<i32, SessionEntry>>,
}

struct SessionEntry {
    session: session::UnifiedExecSession,
    session_ref: Arc<Session>,
    turn_ref: Arc<TurnContext>,
    call_id: String,
    command: String,
    cwd: PathBuf,
    started_at: tokio::time::Instant,
}

pub(crate) fn clamp_yield_time(yield_time_ms: Option<u64>) -> u64 {
    match yield_time_ms {
        Some(value) => value.clamp(MIN_YIELD_TIME_MS, MAX_YIELD_TIME_MS),
        None => DEFAULT_YIELD_TIME_MS,
    }
}

pub(crate) fn resolve_max_tokens(max_tokens: Option<usize>) -> usize {
    max_tokens.unwrap_or(DEFAULT_MAX_OUTPUT_TOKENS)
}

pub(crate) fn generate_chunk_id() -> String {
    let mut rng = rng();
    (0..6)
        .map(|_| format!("{:x}", rng.random_range(0..16)))
        .collect()
}

pub(crate) fn truncate_output_to_tokens(
    output: &str,
    max_tokens: usize,
) -> (String, Option<usize>) {
    if max_tokens == 0 {
        let total_tokens = output.chars().count();
        let message = format!("…{total_tokens} tokens truncated…");
        return (message, Some(total_tokens));
    }

    let tokens: Vec<char> = output.chars().collect();
    let total_tokens = tokens.len();
    if total_tokens <= max_tokens {
        return (output.to_string(), None);
    }

    let half = max_tokens / 2;
    if half == 0 {
        let truncated = total_tokens.saturating_sub(max_tokens);
        let message = format!("…{truncated} tokens truncated…");
        return (message, Some(total_tokens));
    }

    let truncated = total_tokens.saturating_sub(half * 2);
    let mut truncated_output = String::new();
    truncated_output.extend(&tokens[..half]);
    truncated_output.push_str(&format!("…{truncated} tokens truncated…"));
    truncated_output.extend(&tokens[total_tokens - half..]);
    (truncated_output, Some(total_tokens))
}

#[cfg(test)]
#[cfg(unix)]
mod tests {
    use super::*;
    use crate::codex::Session;
    use crate::codex::TurnContext;
    use crate::codex::make_session_and_context;
    use crate::protocol::AskForApproval;
    use crate::protocol::SandboxPolicy;
    use crate::unified_exec::ExecCommandRequest;
    use crate::unified_exec::WriteStdinRequest;
    use core_test_support::skip_if_sandbox;
    use std::sync::Arc;
    use tokio::time::Duration;

    use super::session::OutputBufferState;

    fn test_session_and_turn() -> (Arc<Session>, Arc<TurnContext>) {
        let (session, mut turn) = make_session_and_context();
        turn.approval_policy = AskForApproval::Never;
        turn.sandbox_policy = SandboxPolicy::DangerFullAccess;
        (Arc::new(session), Arc::new(turn))
    }

    async fn exec_command(
        session: &Arc<Session>,
        turn: &Arc<TurnContext>,
        cmd: &str,
        yield_time_ms: Option<u64>,
    ) -> Result<UnifiedExecResponse, UnifiedExecError> {
        let context =
            UnifiedExecContext::new(Arc::clone(session), Arc::clone(turn), "call".to_string());

        session
            .services
            .unified_exec_manager
            .exec_command(
                ExecCommandRequest {
                    command: cmd,
                    shell: "/bin/bash",
                    login: true,
                    yield_time_ms,
                    max_output_tokens: None,
                    workdir: None,
                    with_escalated_permissions: None,
                    justification: None,
                },
                &context,
            )
            .await
    }

    async fn write_stdin(
        session: &Arc<Session>,
        session_id: i32,
        input: &str,
        yield_time_ms: Option<u64>,
    ) -> Result<UnifiedExecResponse, UnifiedExecError> {
        session
            .services
            .unified_exec_manager
            .write_stdin(WriteStdinRequest {
                session_id,
                input,
                yield_time_ms,
                max_output_tokens: None,
            })
            .await
    }

    #[test]
    fn push_chunk_trims_only_excess_bytes() {
        let mut buffer = OutputBufferState::default();
        buffer.push_chunk(vec![b'a'; UNIFIED_EXEC_OUTPUT_MAX_BYTES]);
        buffer.push_chunk(vec![b'b']);
        buffer.push_chunk(vec![b'c']);

        assert_eq!(buffer.total_bytes, UNIFIED_EXEC_OUTPUT_MAX_BYTES);
        let snapshot = buffer.snapshot();
        assert_eq!(snapshot.len(), 3);
        assert_eq!(
            snapshot.first().unwrap().len(),
            UNIFIED_EXEC_OUTPUT_MAX_BYTES - 2
        );
        assert_eq!(snapshot.get(2).unwrap(), &vec![b'c']);
        assert_eq!(snapshot.get(1).unwrap(), &vec![b'b']);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn unified_exec_persists_across_requests() -> anyhow::Result<()> {
        skip_if_sandbox!(Ok(()));

        let (session, turn) = test_session_and_turn();

        let open_shell = exec_command(&session, &turn, "bash -i", Some(2_500)).await?;
        let session_id = open_shell.session_id.expect("expected session_id");

        write_stdin(
            &session,
            session_id,
            "export CODEX_INTERACTIVE_SHELL_VAR=codex\n",
            Some(2_500),
        )
        .await?;

        let out_2 = write_stdin(
            &session,
            session_id,
            "echo $CODEX_INTERACTIVE_SHELL_VAR\n",
            Some(2_500),
        )
        .await?;
        assert!(
            out_2.output.contains("codex"),
            "expected environment variable output"
        );

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn multi_unified_exec_sessions() -> anyhow::Result<()> {
        skip_if_sandbox!(Ok(()));

        let (session, turn) = test_session_and_turn();

        let shell_a = exec_command(&session, &turn, "bash -i", Some(2_500)).await?;
        let session_a = shell_a.session_id.expect("expected session id");

        write_stdin(
            &session,
            session_a,
            "export CODEX_INTERACTIVE_SHELL_VAR=codex\n",
            Some(2_500),
        )
        .await?;

        let out_2 = exec_command(
            &session,
            &turn,
            "echo $CODEX_INTERACTIVE_SHELL_VAR",
            Some(2_500),
        )
        .await?;
        assert!(
            out_2.session_id.is_none(),
            "short command should not retain a session"
        );
        assert!(
            !out_2.output.contains("codex"),
            "short command should run in a fresh shell"
        );

        let out_3 = write_stdin(
            &session,
            session_a,
            "echo $CODEX_INTERACTIVE_SHELL_VAR\n",
            Some(2_500),
        )
        .await?;
        assert!(
            out_3.output.contains("codex"),
            "session should preserve state"
        );

        Ok(())
    }

    #[tokio::test]
    async fn unified_exec_timeouts() -> anyhow::Result<()> {
        skip_if_sandbox!(Ok(()));

        let (session, turn) = test_session_and_turn();

        let open_shell = exec_command(&session, &turn, "bash -i", Some(2_500)).await?;
        let session_id = open_shell.session_id.expect("expected session id");

        write_stdin(
            &session,
            session_id,
            "export CODEX_INTERACTIVE_SHELL_VAR=codex\n",
            Some(2_500),
        )
        .await?;

        let out_2 = write_stdin(
            &session,
            session_id,
            "sleep 5 && echo $CODEX_INTERACTIVE_SHELL_VAR\n",
            Some(10),
        )
        .await?;
        assert!(
            !out_2.output.contains("codex"),
            "timeout too short should yield incomplete output"
        );

        tokio::time::sleep(Duration::from_secs(7)).await;

        let out_3 = write_stdin(&session, session_id, "", Some(100)).await?;

        assert!(
            out_3.output.contains("codex"),
            "subsequent poll should retrieve output"
        );

        Ok(())
    }

    #[tokio::test]
    #[ignore] // Ignored while we have a better way to test this.
    async fn requests_with_large_timeout_are_capped() -> anyhow::Result<()> {
        let (session, turn) = test_session_and_turn();

        let result = exec_command(&session, &turn, "echo codex", Some(120_000)).await?;

        assert!(result.session_id.is_none());
        assert!(result.output.contains("codex"));

        Ok(())
    }

    #[tokio::test]
    #[ignore] // Ignored while we have a better way to test this.
    async fn completed_commands_do_not_persist_sessions() -> anyhow::Result<()> {
        let (session, turn) = test_session_and_turn();
        let result = exec_command(&session, &turn, "echo codex", Some(2_500)).await?;

        assert!(
            result.session_id.is_none(),
            "completed command should not retain session"
        );
        assert!(result.output.contains("codex"));

        assert!(
            session
                .services
                .unified_exec_manager
                .sessions
                .lock()
                .await
                .is_empty()
        );

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn reusing_completed_session_returns_unknown_session() -> anyhow::Result<()> {
        skip_if_sandbox!(Ok(()));

        let (session, turn) = test_session_and_turn();

        let open_shell = exec_command(&session, &turn, "bash -i", Some(2_500)).await?;
        let session_id = open_shell.session_id.expect("expected session id");

        write_stdin(&session, session_id, "exit\n", Some(2_500)).await?;

        tokio::time::sleep(Duration::from_millis(200)).await;

        let err = write_stdin(&session, session_id, "", Some(100))
            .await
            .expect_err("expected unknown session error");

        match err {
            UnifiedExecError::UnknownSessionId { session_id: err_id } => {
                assert_eq!(err_id, session_id);
            }
            other => panic!("expected UnknownSessionId, got {other:?}"),
        }

        assert!(
            !session
                .services
                .unified_exec_manager
                .sessions
                .lock()
                .await
                .contains_key(&session_id)
        );

        Ok(())
    }
}
