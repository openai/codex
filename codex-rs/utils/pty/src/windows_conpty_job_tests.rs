use super::collect_output_until_exit;
use super::combine_spawned_output;
use super::windows_job_test_support::TestDirectory;
use super::windows_job_test_support::wait_for_path;
use super::windows_job_test_support::write_descendant_scripts;
use crate::SpawnedProcess;
use crate::TerminalSize;
use crate::spawn_pty_process;
use std::collections::HashMap;
use std::path::Path;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn conpty_preserves_exit_code_that_matches_still_active() -> anyhow::Result<()> {
    let program = std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_string());
    let args = vec![
        "/D".to_string(),
        "/Q".to_string(),
        "/C".to_string(),
        "exit /b 259".to_string(),
    ];
    let env: HashMap<String, String> = std::env::vars().collect();
    let spawned = spawn_pty_process(
        &program,
        &args,
        Path::new("."),
        &env,
        &None,
        TerminalSize::default(),
    )
    .await?;
    let (_session, output_rx, exit_rx) = combine_spawned_output(spawned);
    let (_output, exit_code) =
        collect_output_until_exit(output_rx, exit_rx, /*timeout_ms*/ 10_000).await;

    assert_eq!(exit_code, 259);
    Ok(())
}

async fn spawn_conpty_script(
    script: &Path,
    cwd: &Path,
    env: &HashMap<String, String>,
) -> anyhow::Result<SpawnedProcess> {
    let command_interpreter = std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_string());
    let args = vec![
        "/D".to_string(),
        "/Q".to_string(),
        "/C".to_string(),
        format!("call \"{}\"", script.display()),
    ];
    spawn_pty_process(
        &command_interpreter,
        &args,
        cwd,
        env,
        &None,
        TerminalSize::default(),
    )
    .await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn conpty_explicit_termination_kills_descendants() -> anyhow::Result<()> {
    let directory = TestDirectory::new("conpty-terminate")?;
    let (root, ready, escaped) = write_descendant_scripts(&directory, false)?;
    let env: HashMap<String, String> = std::env::vars().collect();
    let spawned = spawn_conpty_script(&root, &directory.path, &env).await?;
    let (session, output_rx, exit_rx) = combine_spawned_output(spawned);
    wait_for_path(&ready).await?;
    session.request_terminate();
    let (output, _exit_code) =
        collect_output_until_exit(output_rx, exit_rx, /*timeout_ms*/ 10_000).await;

    assert!(!escaped.exists());
    assert!(String::from_utf8_lossy(&output).contains("inherited-grandchild-ready"));
    assert!(!String::from_utf8_lossy(&output).contains("inherited-grandchild-escaped"));
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn conpty_natural_root_exit_closes_job() -> anyhow::Result<()> {
    let directory = TestDirectory::new("conpty-root-exit")?;
    let (root, ready, escaped) = write_descendant_scripts(&directory, true)?;
    let env: HashMap<String, String> = std::env::vars().collect();
    let spawned = spawn_conpty_script(&root, &directory.path, &env).await?;
    let (_session, output_rx, exit_rx) = combine_spawned_output(spawned);
    let (output, exit_code) =
        collect_output_until_exit(output_rx, exit_rx, /*timeout_ms*/ 10_000).await;

    assert_eq!(exit_code, 37);
    assert!(ready.exists());
    assert!(!escaped.exists());
    assert!(String::from_utf8_lossy(&output).contains("inherited-grandchild-ready"));
    assert!(!String::from_utf8_lossy(&output).contains("inherited-grandchild-escaped"));
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dropping_conpty_session_kills_descendants() -> anyhow::Result<()> {
    let directory = TestDirectory::new("conpty-drop")?;
    let (root, ready, escaped) = write_descendant_scripts(&directory, false)?;
    let env: HashMap<String, String> = std::env::vars().collect();
    let spawned = spawn_conpty_script(&root, &directory.path, &env).await?;
    wait_for_path(&ready).await?;
    drop(spawned.session);
    drop(spawned.stdout_rx);
    drop(spawned.stderr_rx);
    drop(spawned.exit_rx);
    tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;

    assert!(!escaped.exists());
    Ok(())
}
