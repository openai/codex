use super::head_tail_buffer::HeadTailBuffer;
use super::*;
use crate::codex::Session;
use crate::codex::TurnContext;
use crate::codex::make_session_and_context;
use crate::exec::ExecCapturePolicy;
use crate::exec::ExecExpiration;
use crate::sandboxing::ExecRequest;
use crate::tools::context::ExecCommandToolOutput;
use crate::unified_exec::WriteStdinRequest;
use crate::unified_exec::process::OutputHandles;
use codex_sandboxing::SandboxType;
use codex_utils_output_truncation::approx_token_count;
use core_test_support::PathBufExt;
use core_test_support::get_remote_test_env;
use core_test_support::skip_if_sandbox;
use core_test_support::test_codex::test_env as remote_test_env;
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::time::Duration;
use tokio::time::Instant;

async fn test_session_and_turn() -> (Arc<Session>, Arc<TurnContext>) {
    let (session, turn) = make_session_and_context().await;
    (Arc::new(session), Arc::new(turn))
}

async fn exec_command(
    session: &Arc<Session>,
    turn: &Arc<TurnContext>,
    cmd: &str,
    yield_time_ms: u64,
    workdir: Option<AbsolutePathBuf>,
) -> Result<ExecCommandToolOutput, UnifiedExecError> {
    exec_command_with_tty(
        session,
        turn,
        cmd,
        yield_time_ms,
        workdir,
        /*tty*/ true,
    )
    .await
}

fn shell_env() -> HashMap<String, String> {
    std::env::vars().collect()
}

fn test_exec_request(
    turn: &TurnContext,
    command: Vec<String>,
    cwd: AbsolutePathBuf,
    env: HashMap<String, String>,
) -> ExecRequest {
    let windows_sandbox_private_desktop = false;
    let sandbox_policy = turn.sandbox_policy.get().clone();
    let file_system_sandbox_policy = turn.file_system_sandbox_policy.clone();
    let network_sandbox_policy = turn.network_sandbox_policy;
    let network = None;
    let arg0 = None;
    ExecRequest::new(
        command,
        cwd,
        env,
        network,
        ExecExpiration::DefaultTimeout,
        ExecCapturePolicy::ShellTool,
        SandboxType::None,
        turn.windows_sandbox_level,
        windows_sandbox_private_desktop,
        sandbox_policy,
        file_system_sandbox_policy,
        network_sandbox_policy,
        arg0,
    )
}

async fn exec_command_with_tty(
    session: &Arc<Session>,
    turn: &Arc<TurnContext>,
    cmd: &str,
    yield_time_ms: u64,
    workdir: Option<AbsolutePathBuf>,
    tty: bool,
) -> Result<ExecCommandToolOutput, UnifiedExecError> {
    let manager = &session.services.unified_exec_manager;
    let process_id = manager.allocate_process_id().await;
    let cwd = workdir.unwrap_or_else(|| turn.cwd.clone());
    let command = vec!["bash".to_string(), "-lc".to_string(), cmd.to_string()];
    let request = test_exec_request(turn, command.clone(), cwd.clone(), shell_env());

    let process = Arc::new(
        manager
            .open_session_with_exec_env(
                process_id,
                &request,
                tty,
                Box::new(NoopSpawnLifecycle),
                turn.environment.as_ref().expect("turn environment"),
            )
            .await?,
    );
    let context =
        UnifiedExecContext::new(Arc::clone(session), Arc::clone(turn), "call".to_string());
    let started_at = Instant::now();
    let process_started_alive = !process.has_exited() && process.exit_code().is_none();
    if process_started_alive {
        let entry = ProcessEntry {
            process: Arc::clone(&process),
            call_id: context.call_id.clone(),
            process_id,
            command: command.clone(),
            tty,
            network_approval_id: None,
            session: Arc::downgrade(session),
            last_used: started_at,
        };
        manager
            .process_store
            .lock()
            .await
            .processes
            .insert(process_id, entry);
    }

    let OutputHandles {
        output_buffer,
        output_notify,
        output_closed,
        output_closed_notify,
        cancellation_token,
    } = process.output_handles();
    let deadline = started_at + Duration::from_millis(yield_time_ms);
    let collected = UnifiedExecProcessManager::collect_output_until_deadline(
        &output_buffer,
        &output_notify,
        &output_closed,
        &output_closed_notify,
        &cancellation_token,
        Some(session.subscribe_out_of_band_elicitation_pause_state()),
        deadline,
    )
    .await;
    let wall_time = Instant::now().saturating_duration_since(started_at);
    let text = String::from_utf8_lossy(&collected).to_string();
    let has_exited = process.has_exited();
    let exit_code = process.exit_code();
    let response_process_id = if process_started_alive && !has_exited {
        Some(process_id)
    } else {
        manager.release_process_id(process_id).await;
        None
    };

    Ok(ExecCommandToolOutput {
        event_call_id: context.call_id,
        chunk_id: generate_chunk_id(),
        wall_time,
        raw_output: collected,
        max_output_tokens: None,
        process_id: response_process_id,
        exit_code,
        original_token_count: Some(approx_token_count(&text)),
        session_command: Some(command),
    })
}

#[derive(Debug)]
struct TestSpawnLifecycle {
    inherited_fds: Vec<i32>,
}

impl SpawnLifecycle for TestSpawnLifecycle {
    fn inherited_fds(&self) -> Vec<i32> {
        self.inherited_fds.clone()
    }
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

    let open_shell = exec_command(
        &session, &turn, "bash -i", /*yield_time_ms*/ 2_500, /*workdir*/ None,
    )
    .await?;
    let process_id = open_shell.process_id.expect("expected process_id");

    write_stdin(
        &session,
        process_id,
        "export CODEX_INTERACTIVE_SHELL_VAR=codex\n",
        /*yield_time_ms*/ 2_500,
    )
    .await?;

    let out_2 = write_stdin(
        &session,
        process_id,
        "echo $CODEX_INTERACTIVE_SHELL_VAR\n",
        /*yield_time_ms*/ 2_500,
    )
    .await?;
    assert!(
        out_2.truncated_output().contains("codex"),
        "expected environment variable output"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn multi_unified_exec_sessions() -> anyhow::Result<()> {
    skip_if_sandbox!(Ok(()));

    let (session, turn) = test_session_and_turn().await;

    let shell_a = exec_command(
        &session, &turn, "bash -i", /*yield_time_ms*/ 2_500, /*workdir*/ None,
    )
    .await?;
    let session_a = shell_a.process_id.expect("expected process id");

    write_stdin(
        &session,
        session_a,
        "export CODEX_INTERACTIVE_SHELL_VAR=codex\n",
        /*yield_time_ms*/ 2_500,
    )
    .await?;

    let out_2 = exec_command(
        &session,
        &turn,
        "echo $CODEX_INTERACTIVE_SHELL_VAR",
        /*yield_time_ms*/ 2_500,
        /*workdir*/ None,
    )
    .await?;
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
        /*yield_time_ms*/ 2_500,
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

    let open_shell = exec_command(
        &session, &turn, "bash -i", /*yield_time_ms*/ 2_500, /*workdir*/ None,
    )
    .await?;
    let process_id = open_shell.process_id.expect("expected process id");

    write_stdin(
        &session,
        process_id,
        format!("export CODEX_INTERACTIVE_SHELL_VAR={TEST_VAR_VALUE}\n").as_str(),
        /*yield_time_ms*/ 2_500,
    )
    .await?;

    let out_2 = write_stdin(
        &session,
        process_id,
        "sleep 5 && echo $CODEX_INTERACTIVE_SHELL_VAR\n",
        /*yield_time_ms*/ 10,
    )
    .await?;
    assert!(
        !out_2.truncated_output().contains(TEST_VAR_VALUE),
        "timeout too short should yield incomplete output"
    );

    tokio::time::sleep(Duration::from_secs(7)).await;

    let out_3 = write_stdin(&session, process_id, "", /*yield_time_ms*/ 100).await?;

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
    session.set_out_of_band_elicitation_pause_state(/*paused*/ true);

    let paused_session = Arc::clone(&session);
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(2)).await;
        paused_session.set_out_of_band_elicitation_pause_state(/*paused*/ false);
    });

    let started = tokio::time::Instant::now();
    let response = exec_command(
        &session,
        &turn,
        "sleep 1 && echo unified-exec-done",
        /*yield_time_ms*/ 250,
        /*workdir*/ None,
    )
    .await?;

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

    let result = exec_command(
        &session,
        &turn,
        "echo codex",
        /*yield_time_ms*/ 120_000,
        /*workdir*/ None,
    )
    .await?;

    assert!(result.process_id.is_some());
    assert!(result.truncated_output().contains("codex"));

    Ok(())
}

#[tokio::test]
#[ignore] // Ignored while we have a better way to test this.
async fn completed_commands_do_not_persist_sessions() -> anyhow::Result<()> {
    let (session, turn) = test_session_and_turn().await;
    let result = exec_command(
        &session,
        &turn,
        "echo codex",
        /*yield_time_ms*/ 2_500,
        /*workdir*/ None,
    )
    .await?;

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

    let open_shell = exec_command(
        &session, &turn, "bash -i", /*yield_time_ms*/ 2_500, /*workdir*/ None,
    )
    .await?;
    let process_id = open_shell.process_id.expect("expected process id");

    write_stdin(&session, process_id, "exit\n", /*yield_time_ms*/ 2_500).await?;

    tokio::time::sleep(Duration::from_millis(200)).await;

    let err = write_stdin(&session, process_id, "", /*yield_time_ms*/ 100)
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn completed_pipe_commands_preserve_exit_code() -> anyhow::Result<()> {
    let (_, turn) = make_session_and_context().await;
    let request = test_exec_request(
        &turn,
        vec!["bash".to_string(), "-lc".to_string(), "exit 17".to_string()],
        PathBuf::from("/tmp").abs(),
        shell_env(),
    );

    let environment = codex_exec_server::Environment::default();
    let process = UnifiedExecProcessManager::default()
        .open_session_with_exec_env(
            /*process_id*/ 1234,
            &request,
            /*tty*/ false,
            Box::new(NoopSpawnLifecycle),
            &environment,
        )
        .await?;

    if !process.has_exited() {
        let exit_signal = process.cancellation_token();
        assert!(
            tokio::time::timeout(Duration::from_secs(2), exit_signal.cancelled())
                .await
                .is_ok(),
            "process did not report exit within timeout"
        );
    }

    assert!(process.has_exited());
    assert_eq!(process.exit_code(), Some(17));
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unified_exec_uses_remote_exec_server_when_configured() -> anyhow::Result<()> {
    skip_if_sandbox!(Ok(()));
    let Some(_remote_env) = get_remote_test_env() else {
        return Ok(());
    };

    let remote_test_env = remote_test_env().await?;
    let (_, turn) = make_session_and_context().await;
    let request = test_exec_request(
        &turn,
        vec!["bash".to_string(), "-i".to_string()],
        PathBuf::from("/tmp").abs(),
        shell_env(),
    );

    let manager = UnifiedExecProcessManager::default();
    let process = manager
        .open_session_with_exec_env(
            /*process_id*/ 1234,
            &request,
            /*tty*/ true,
            Box::new(NoopSpawnLifecycle),
            remote_test_env.environment(),
        )
        .await?;

    process.write(b"printf 'remote-unified-exec\\n'\n").await?;
    tokio::time::sleep(Duration::from_millis(100)).await;

    let crate::unified_exec::process::OutputHandles {
        output_buffer,
        output_notify,
        output_closed,
        output_closed_notify,
        cancellation_token,
    } = process.output_handles();
    let collected = UnifiedExecProcessManager::collect_output_until_deadline(
        &output_buffer,
        &output_notify,
        &output_closed,
        &output_closed_notify,
        &cancellation_token,
        /*pause_state*/ None,
        Instant::now() + Duration::from_millis(2_500),
    )
    .await;

    assert!(String::from_utf8_lossy(&collected).contains("remote-unified-exec"));
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_exec_server_rejects_inherited_fd_launches() -> anyhow::Result<()> {
    skip_if_sandbox!(Ok(()));
    let Some(_remote_env) = get_remote_test_env() else {
        return Ok(());
    };

    let remote_test_env = remote_test_env().await?;
    let (_, mut turn) = make_session_and_context().await;
    turn.environment = Some(Arc::new(remote_test_env.environment().clone()));

    let request = test_exec_request(
        &turn,
        vec!["bash".to_string(), "-lc".to_string(), "echo ok".to_string()],
        PathBuf::from("/tmp").abs(),
        shell_env(),
    );

    let manager = UnifiedExecProcessManager::default();
    let err = manager
        .open_session_with_exec_env(
            /*process_id*/ 1234,
            &request,
            /*tty*/ true,
            Box::new(TestSpawnLifecycle {
                inherited_fds: vec![42],
            }),
            turn.environment.as_ref().expect("turn environment"),
        )
        .await
        .expect_err("expected inherited fd rejection");

    assert_eq!(
        err.to_string(),
        "Failed to create unified exec process: remote exec-server does not support inherited file descriptors"
    );
    Ok(())
}
