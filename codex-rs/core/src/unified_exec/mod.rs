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
use std::sync::atomic::AtomicI32;

use tokio::sync::Mutex;

use crate::codex::Session;
use crate::codex::TurnContext;

mod errors;
mod session;
mod session_manager;

pub(crate) use errors::UnifiedExecError;
pub(crate) use session::UnifiedExecSession;

const DEFAULT_TIMEOUT_MS: u64 = 1_000;
const MAX_TIMEOUT_MS: u64 = 60_000;
const UNIFIED_EXEC_OUTPUT_MAX_BYTES: usize = 128 * 1024; // 128 KiB

pub(crate) struct UnifiedExecContext<'a> {
    pub session: &'a Session,
    pub turn: &'a TurnContext,
    pub sub_id: &'a str,
    pub call_id: &'a str,
    pub session_id: Option<i32>,
}

#[derive(Debug)]
pub(crate) struct UnifiedExecRequest<'a> {
    pub input_chunks: &'a [String],
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct UnifiedExecResult {
    pub session_id: Option<i32>,
    pub output: String,
}

#[derive(Debug, Default)]
pub(crate) struct UnifiedExecSessionManager {
    next_session_id: AtomicI32,
    sessions: Mutex<HashMap<i32, session::UnifiedExecSession>>,
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

    async fn run_unified_exec_request(
        session: &Arc<Session>,
        turn: &Arc<TurnContext>,
        session_id: Option<i32>,
        input: Vec<String>,
        timeout_ms: Option<u64>,
    ) -> Result<UnifiedExecResult, UnifiedExecError> {
        let request_input = input;
        let request = UnifiedExecRequest {
            input_chunks: &request_input,
            timeout_ms,
        };

        session
            .services
            .unified_exec_manager
            .handle_request(
                request,
                UnifiedExecContext {
                    session,
                    turn: turn.as_ref(),
                    sub_id: "sub",
                    call_id: "call",
                    session_id,
                },
            )
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

        let open_shell = run_unified_exec_request(
            &session,
            &turn,
            None,
            vec!["bash".to_string(), "-i".to_string()],
            Some(2_500),
        )
        .await?;
        let session_id = open_shell.session_id.expect("expected session_id");

        run_unified_exec_request(
            &session,
            &turn,
            Some(session_id),
            vec![
                "export".to_string(),
                "CODEX_INTERACTIVE_SHELL_VAR=codex\n".to_string(),
            ],
            Some(2_500),
        )
        .await?;

        let out_2 = run_unified_exec_request(
            &session,
            &turn,
            Some(session_id),
            vec!["echo $CODEX_INTERACTIVE_SHELL_VAR\n".to_string()],
            Some(2_500),
        )
        .await?;
        assert!(out_2.output.contains("codex"));

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn multi_unified_exec_sessions() -> anyhow::Result<()> {
        skip_if_sandbox!(Ok(()));

        let (session, turn) = test_session_and_turn();

        let shell_a = run_unified_exec_request(
            &session,
            &turn,
            None,
            vec!["/bin/bash".to_string(), "-i".to_string()],
            Some(2_500),
        )
        .await?;
        let session_a = shell_a.session_id.expect("expected session id");

        run_unified_exec_request(
            &session,
            &turn,
            Some(session_a),
            vec!["export CODEX_INTERACTIVE_SHELL_VAR=codex\n".to_string()],
            Some(2_500),
        )
        .await?;

        let out_2 = run_unified_exec_request(
            &session,
            &turn,
            None,
            vec![
                "echo".to_string(),
                "$CODEX_INTERACTIVE_SHELL_VAR\n".to_string(),
            ],
            Some(2_500),
        )
        .await?;
        assert!(!out_2.output.contains("codex"));

        let out_3 = run_unified_exec_request(
            &session,
            &turn,
            Some(session_a),
            vec!["echo $CODEX_INTERACTIVE_SHELL_VAR\n".to_string()],
            Some(2_500),
        )
        .await?;
        assert!(out_3.output.contains("codex"));

        Ok(())
    }

    #[tokio::test]
    async fn unified_exec_timeouts() -> anyhow::Result<()> {
        skip_if_sandbox!(Ok(()));

        let (session, turn) = test_session_and_turn();

        let open_shell = run_unified_exec_request(
            &session,
            &turn,
            None,
            vec!["bash".to_string(), "-i".to_string()],
            Some(2_500),
        )
        .await?;
        let session_id = open_shell.session_id.expect("expected session id");

        run_unified_exec_request(
            &session,
            &turn,
            Some(session_id),
            vec![
                "export".to_string(),
                "CODEX_INTERACTIVE_SHELL_VAR=codex\n".to_string(),
            ],
            Some(2_500),
        )
        .await?;

        let out_2 = run_unified_exec_request(
            &session,
            &turn,
            Some(session_id),
            vec!["sleep 5 && echo $CODEX_INTERACTIVE_SHELL_VAR\n".to_string()],
            Some(10),
        )
        .await?;
        assert!(!out_2.output.contains("codex"));

        tokio::time::sleep(Duration::from_secs(7)).await;

        let out_3 =
            run_unified_exec_request(&session, &turn, Some(session_id), Vec::new(), Some(100))
                .await?;

        assert!(out_3.output.contains("codex"));

        Ok(())
    }

    #[tokio::test]
    #[ignore] // Ignored while we have a better way to test this.
    async fn requests_with_large_timeout_are_capped() -> anyhow::Result<()> {
        let (session, turn) = test_session_and_turn();

        let result = run_unified_exec_request(
            &session,
            &turn,
            None,
            vec!["echo".to_string(), "codex".to_string()],
            Some(120_000),
        )
        .await?;

        assert!(result.output.starts_with(
            "Warning: requested timeout 120000ms exceeds maximum of 60000ms; clamping to 60000ms.\n"
        ));
        assert!(result.output.contains("codex"));

        Ok(())
    }

    #[tokio::test]
    #[ignore] // Ignored while we have a better way to test this.
    async fn completed_commands_do_not_persist_sessions() -> anyhow::Result<()> {
        let (session, turn) = test_session_and_turn();
        let result = run_unified_exec_request(
            &session,
            &turn,
            None,
            vec!["/bin/echo".to_string(), "codex".to_string()],
            Some(2_500),
        )
        .await?;

        assert!(result.session_id.is_none());
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

        let open_shell = run_unified_exec_request(
            &session,
            &turn,
            None,
            vec!["/bin/bash".to_string(), "-i".to_string()],
            Some(2_500),
        )
        .await?;
        let session_id = open_shell.session_id.expect("expected session id");

        run_unified_exec_request(
            &session,
            &turn,
            Some(session_id),
            vec!["exit\n".to_string()],
            Some(2_500),
        )
        .await?;

        tokio::time::sleep(Duration::from_millis(200)).await;

        let err =
            run_unified_exec_request(&session, &turn, Some(session_id), Vec::new(), Some(100))
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
