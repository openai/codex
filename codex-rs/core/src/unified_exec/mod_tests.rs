use super::head_tail_buffer::HeadTailBuffer;
use super::*;
use crate::codex::Session;
use crate::codex::TurnContext;
use crate::codex::make_session_and_context;
use crate::codex::make_session_and_context_with_rx;
use crate::config::ToolExecutionMode;
use crate::protocol::AskForApproval;
use crate::protocol::SandboxPolicy;
use crate::state::ActiveTurn;
use crate::tools::context::ExecCommandToolOutput;
use crate::unified_exec::ExecCommandRequest;
use crate::unified_exec::WriteStdinRequest;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ReviewDecision;
use core_test_support::skip_if_sandbox;
use std::sync::Arc;
use tokio::time::Duration;

async fn test_session_and_turn() -> (Arc<Session>, Arc<TurnContext>) {
    let (session, mut turn) = make_session_and_context().await;
    turn.approval_policy
        .set(AskForApproval::Never)
        .expect("test setup should allow updating approval policy");
    turn.sandbox_policy
        .set(SandboxPolicy::DangerFullAccess)
        .expect("test setup should allow updating sandbox policy");
    turn.file_system_sandbox_policy =
        codex_protocol::permissions::FileSystemSandboxPolicy::from(turn.sandbox_policy.get());
    turn.network_sandbox_policy =
        codex_protocol::permissions::NetworkSandboxPolicy::from(turn.sandbox_policy.get());
    (Arc::new(session), Arc::new(turn))
}

async fn exec_command(
    session: &Arc<Session>,
    turn: &Arc<TurnContext>,
    cmd: &str,
    yield_time_ms: u64,
) -> Result<ExecCommandToolOutput, UnifiedExecError> {
    let context =
        UnifiedExecContext::new(Arc::clone(session), Arc::clone(turn), "call".to_string());
    let process_id = session
        .services
        .unified_exec_manager
        .allocate_process_id()
        .await;

    session
        .services
        .unified_exec_manager
        .exec_command(
            ExecCommandRequest {
                command: vec!["bash".to_string(), "-lc".to_string(), cmd.to_string()],
                process_id,
                yield_time_ms,
                max_output_tokens: None,
                workdir: None,
                network: None,
                tty: true,
                sandbox_permissions: SandboxPermissions::UseDefault,
                additional_permissions: None,
                additional_permissions_preapproved: false,
                justification: None,
                prefix_rule: None,
            },
            &context,
        )
        .await
}

async fn write_stdin(
    session: &Arc<Session>,
    process_id: i32,
    input: &str,
    yield_time_ms: u64,
) -> Result<ExecCommandToolOutput, UnifiedExecError> {
    session
        .services
        .unified_exec_manager
        .write_stdin(WriteStdinRequest {
            process_id,
            input,
            yield_time_ms,
            max_output_tokens: None,
            context: None,
        })
        .await
}

#[test]
fn push_chunk_preserves_prefix_and_suffix() {
    let mut buffer = HeadTailBuffer::default();
    buffer.push_chunk(vec![b'a'; UNIFIED_EXEC_OUTPUT_MAX_BYTES]);
    buffer.push_chunk(vec![b'b']);
    buffer.push_chunk(vec![b'c']);

    assert_eq!(buffer.retained_bytes(), UNIFIED_EXEC_OUTPUT_MAX_BYTES);
    let snapshot = buffer.snapshot_chunks();

    let first = snapshot.first().expect("expected at least one chunk");
    assert_eq!(first.first(), Some(&b'a'));
    assert!(snapshot.iter().any(|chunk| chunk.as_slice() == b"b"));
    assert_eq!(
        snapshot
            .last()
            .expect("expected at least one chunk")
            .as_slice(),
        b"c"
    );
}

#[test]
fn head_tail_buffer_default_preserves_prefix_and_suffix() {
    let mut buffer = HeadTailBuffer::default();
    buffer.push_chunk(vec![b'a'; UNIFIED_EXEC_OUTPUT_MAX_BYTES]);
    buffer.push_chunk(b"bc".to_vec());

    let rendered = buffer.to_bytes();
    assert_eq!(rendered.first(), Some(&b'a'));
    assert!(rendered.ends_with(b"bc"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unified_exec_persists_across_requests() -> anyhow::Result<()> {
    skip_if_sandbox!(Ok(()));

    let (session, turn) = test_session_and_turn().await;

    let open_shell = exec_command(&session, &turn, "bash -i", 2_500).await?;
    let process_id = open_shell.process_id.expect("expected process_id");

    write_stdin(
        &session,
        process_id,
        "export CODEX_INTERACTIVE_SHELL_VAR=codex\n",
        2_500,
    )
    .await?;

    let out_2 = write_stdin(
        &session,
        process_id,
        "echo $CODEX_INTERACTIVE_SHELL_VAR\n",
        2_500,
    )
    .await?;
    assert!(
        out_2.truncated_output().contains("codex"),
        "expected environment variable output"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unified_exec_write_stdin_requires_manual_approval() -> anyhow::Result<()> {
    skip_if_sandbox!(Ok(()));

    let (session, turn, rx) = make_session_and_context_with_rx().await;
    *session.active_turn.lock().await = Some(ActiveTurn::default());

    let mut turn = Arc::try_unwrap(turn).expect("single turn context ref");
    turn.sandbox_policy
        .set(SandboxPolicy::DangerFullAccess)
        .expect("test setup should allow updating sandbox policy");
    turn.file_system_sandbox_policy =
        codex_protocol::permissions::FileSystemSandboxPolicy::from(turn.sandbox_policy.get());
    turn.network_sandbox_policy =
        codex_protocol::permissions::NetworkSandboxPolicy::from(turn.sandbox_policy.get());
    turn.tools_config.execution_mode = ToolExecutionMode::Manual;
    let turn = Arc::new(turn);

    let approval_task = tokio::spawn({
        let session = Arc::clone(&session);
        async move {
            let mut approvals = Vec::new();
            while approvals.len() < 2 {
                let event = rx.recv().await.expect("approval event");
                if let EventMsg::ExecApprovalRequest(request) = event.msg {
                    approvals.push(request.clone());
                    let approval_id = request
                        .approval_id
                        .as_deref()
                        .unwrap_or(request.call_id.as_str());
                    session
                        .notify_approval(approval_id, ReviewDecision::Approved)
                        .await;
                }
            }
            approvals
        }
    });

    let open_shell = exec_command(&session, &turn, "bash -i", 2_500).await?;
    let process_id = open_shell.process_id.expect("expected process_id");
    let context = UnifiedExecContext::new(Arc::clone(&session), Arc::clone(&turn), "call".into());

    let output = session
        .services
        .unified_exec_manager
        .write_stdin(WriteStdinRequest {
            process_id,
            input: "printf codex-manual-approval\n",
            yield_time_ms: 2_500,
            max_output_tokens: None,
            context: Some(&context),
        })
        .await?;

    assert!(
        output.truncated_output().contains("codex-manual-approval"),
        "interactive input should run after approval"
    );

    let approvals = approval_task.await?;
    assert_eq!(approvals.len(), 2);
    assert_eq!(
        approvals[1].reason.as_deref(),
        Some("interactive terminal input: printf codex-manual-approval\\n")
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn multi_unified_exec_sessions() -> anyhow::Result<()> {
    skip_if_sandbox!(Ok(()));

    let (session, turn) = test_session_and_turn().await;

    let shell_a = exec_command(&session, &turn, "bash -i", 2_500).await?;
    let session_a = shell_a.process_id.expect("expected process id");

    write_stdin(
        &session,
        session_a,
        "export CODEX_INTERACTIVE_SHELL_VAR=codex\n",
        2_500,
    )
    .await?;

    let out_2 = exec_command(&session, &turn, "echo $CODEX_INTERACTIVE_SHELL_VAR", 2_500).await?;
    tokio::time::sleep(Duration::from_secs(2)).await;
    assert!(
        out_2.process_id.is_none(),
        "short command should not report a process id if it exits quickly"
    );
    assert!(
        !out_2.truncated_output().contains("codex"),
        "short command should run in a fresh shell"
    );

    let out_3 = write_stdin(
        &session,
        shell_a.process_id.expect("expected process id"),
        "echo $CODEX_INTERACTIVE_SHELL_VAR\n",
        2_500,
    )
    .await?;
    assert!(
        out_3.truncated_output().contains("codex"),
        "session should preserve state"
    );

    Ok(())
}

#[tokio::test]
async fn unified_exec_timeouts() -> anyhow::Result<()> {
    skip_if_sandbox!(Ok(()));

    const TEST_VAR_VALUE: &str = "unified_exec_var_123";

    let (session, turn) = test_session_and_turn().await;

    let open_shell = exec_command(&session, &turn, "bash -i", 2_500).await?;
    let process_id = open_shell.process_id.expect("expected process id");

    write_stdin(
        &session,
        process_id,
        format!("export CODEX_INTERACTIVE_SHELL_VAR={TEST_VAR_VALUE}\n").as_str(),
        2_500,
    )
    .await?;

    let out_2 = write_stdin(
        &session,
        process_id,
        "sleep 5 && echo $CODEX_INTERACTIVE_SHELL_VAR\n",
        10,
    )
    .await?;
    assert!(
        !out_2.truncated_output().contains(TEST_VAR_VALUE),
        "timeout too short should yield incomplete output"
    );

    tokio::time::sleep(Duration::from_secs(7)).await;

    let out_3 = write_stdin(&session, process_id, "", 100).await?;

    assert!(
        out_3.truncated_output().contains(TEST_VAR_VALUE),
        "subsequent poll should retrieve output"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unified_exec_pause_blocks_yield_timeout() -> anyhow::Result<()> {
    skip_if_sandbox!(Ok(()));

    let (session, turn) = test_session_and_turn().await;
    session.set_out_of_band_elicitation_pause_state(true);

    let paused_session = Arc::clone(&session);
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(2)).await;
        paused_session.set_out_of_band_elicitation_pause_state(false);
    });

    let started = tokio::time::Instant::now();
    let response = exec_command(&session, &turn, "sleep 1 && echo unified-exec-done", 250).await?;

    assert!(
        started.elapsed() >= Duration::from_secs(2),
        "pause should block the unified exec yield timeout"
    );
    assert!(
        response.truncated_output().contains("unified-exec-done"),
        "exec_command should wait for output after the pause lifts"
    );
    assert!(
        response.process_id.is_none(),
        "completed command should not leave a background process"
    );

    Ok(())
}

#[tokio::test]
#[ignore] // Ignored while we have a better way to test this.
async fn requests_with_large_timeout_are_capped() -> anyhow::Result<()> {
    let (session, turn) = test_session_and_turn().await;

    let result = exec_command(&session, &turn, "echo codex", 120_000).await?;

    assert!(result.process_id.is_some());
    assert!(result.truncated_output().contains("codex"));

    Ok(())
}

#[tokio::test]
#[ignore] // Ignored while we have a better way to test this.
async fn completed_commands_do_not_persist_sessions() -> anyhow::Result<()> {
    let (session, turn) = test_session_and_turn().await;
    let result = exec_command(&session, &turn, "echo codex", 2_500).await?;

    assert!(
        result.process_id.is_some(),
        "completed command should report a process id"
    );
    assert!(result.truncated_output().contains("codex"));

    assert!(
        session
            .services
            .unified_exec_manager
            .process_store
            .lock()
            .await
            .processes
            .is_empty()
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reusing_completed_process_returns_unknown_process() -> anyhow::Result<()> {
    skip_if_sandbox!(Ok(()));

    let (session, turn) = test_session_and_turn().await;

    let open_shell = exec_command(&session, &turn, "bash -i", 2_500).await?;
    let process_id = open_shell.process_id.expect("expected process id");

    write_stdin(&session, process_id, "exit\n", 2_500).await?;

    tokio::time::sleep(Duration::from_millis(200)).await;

    let err = write_stdin(&session, process_id, "", 100)
        .await
        .expect_err("expected unknown process error");

    match err {
        UnifiedExecError::UnknownProcessId { process_id: err_id } => {
            assert_eq!(err_id, process_id, "process id should match request");
        }
        other => panic!("expected UnknownProcessId, got {other:?}"),
    }

    assert!(
        session
            .services
            .unified_exec_manager
            .process_store
            .lock()
            .await
            .processes
            .is_empty()
    );

    Ok(())
}
