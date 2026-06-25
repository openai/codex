use super::collect_split_output;
use super::windows_job_test_support::TestDirectory;
use super::windows_job_test_support::wait_for_path;
use super::windows_job_test_support::write_descendant_scripts;
use crate::SpawnedProcess;
use crate::spawn_pipe_process_no_stdin;
use std::collections::HashMap;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn raw_pipe_root_exit_closes_job_and_inherited_output() -> anyhow::Result<()> {
    let directory = TestDirectory::new("pipe-root-exit")?;
    let (root, ready, escaped) = write_descendant_scripts(&directory, true)?;
    let env: HashMap<String, String> = std::env::vars().collect();
    let spawned = spawn_pipe_process_no_stdin(
        root.to_string_lossy().as_ref(),
        &[],
        &directory.path,
        &env,
        &None,
    )
    .await?;
    let SpawnedProcess {
        session: _session,
        stdout_rx,
        stderr_rx,
        exit_rx,
    } = spawned;
    let stdout_task = tokio::spawn(collect_split_output(stdout_rx));
    let stderr_task = tokio::spawn(collect_split_output(stderr_rx));
    let timeout = tokio::time::Duration::from_secs(10);
    let exit_code = tokio::time::timeout(timeout, exit_rx).await??;
    let stdout = tokio::time::timeout(timeout, stdout_task).await??;
    let _stderr = tokio::time::timeout(timeout, stderr_task).await??;

    assert_eq!(exit_code, 37);
    assert!(ready.exists());
    assert!(!escaped.exists());
    assert!(String::from_utf8_lossy(&stdout).contains("inherited-grandchild-ready"));
    assert!(!String::from_utf8_lossy(&stdout).contains("inherited-grandchild-escaped"));
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn raw_pipe_explicit_termination_kills_descendants() -> anyhow::Result<()> {
    let directory = TestDirectory::new("pipe-terminate")?;
    let (root, ready, escaped) = write_descendant_scripts(&directory, false)?;
    let env: HashMap<String, String> = std::env::vars().collect();
    let spawned = spawn_pipe_process_no_stdin(
        root.to_string_lossy().as_ref(),
        &[],
        &directory.path,
        &env,
        &None,
    )
    .await?;
    let SpawnedProcess {
        session,
        stdout_rx,
        stderr_rx,
        exit_rx,
    } = spawned;
    let stdout_task = tokio::spawn(collect_split_output(stdout_rx));
    let stderr_task = tokio::spawn(collect_split_output(stderr_rx));
    wait_for_path(&ready).await?;
    session.request_terminate();
    let timeout = tokio::time::Duration::from_secs(10);
    let _exit_code = tokio::time::timeout(timeout, exit_rx).await??;
    let stdout = tokio::time::timeout(timeout, stdout_task).await??;
    let _stderr = tokio::time::timeout(timeout, stderr_task).await??;

    assert!(!escaped.exists());
    assert!(String::from_utf8_lossy(&stdout).contains("inherited-grandchild-ready"));
    assert!(!String::from_utf8_lossy(&stdout).contains("inherited-grandchild-escaped"));
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dropping_raw_pipe_session_kills_descendants() -> anyhow::Result<()> {
    let directory = TestDirectory::new("pipe-drop")?;
    let (root, ready, escaped) = write_descendant_scripts(&directory, false)?;
    let env: HashMap<String, String> = std::env::vars().collect();
    let spawned = spawn_pipe_process_no_stdin(
        root.to_string_lossy().as_ref(),
        &[],
        &directory.path,
        &env,
        &None,
    )
    .await?;
    wait_for_path(&ready).await?;
    drop(spawned.session);
    drop(spawned.stdout_rx);
    drop(spawned.stderr_rx);
    drop(spawned.exit_rx);
    tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;

    assert!(!escaped.exists());
    Ok(())
}
