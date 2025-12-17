use super::conpty_supported;
use super::fallback;
use super::spawn_pty_process;
use super::ExecCommandSession;
use super::SpawnedPty;
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::time::Instant;

const OUTPUT_TIMEOUT: Duration = Duration::from_secs(5);
const TERMINATE_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, Clone)]
struct CommandSpec {
    program: String,
    args: Vec<String>,
}

#[derive(Debug, PartialEq, Eq)]
enum ExitOutcome {
    Exited(i32),
    Dropped,
}

#[derive(Debug)]
struct RunResult {
    output: Vec<u8>,
    exit: ExitOutcome,
}

#[derive(Debug)]
struct InteractiveSession {
    session: ExecCommandSession,
    writer: mpsc::Sender<Vec<u8>>,
    output_rx: broadcast::Receiver<Vec<u8>>,
    exit_rx: oneshot::Receiver<i32>,
    buffer: Vec<u8>,
}

fn shell_command(script: &str) -> CommandSpec {
    #[cfg(unix)]
    {
        CommandSpec {
            program: "/bin/sh".to_string(),
            args: vec!["-c".to_string(), script.to_string()],
        }
    }
    #[cfg(windows)]
    {
        CommandSpec {
            program: windows_cmd_path(),
            args: vec!["/C".to_string(), script.to_string()],
        }
    }
}

fn shell_repl_command() -> CommandSpec {
    #[cfg(unix)]
    {
        CommandSpec {
            program: "/bin/sh".to_string(),
            args: Vec::new(),
        }
    }
    #[cfg(windows)]
    {
        CommandSpec {
            program: windows_cmd_path(),
            args: vec!["/Q".to_string(), "/D".to_string()],
        }
    }
}

#[cfg(windows)]
fn windows_cmd_path() -> String {
    if let Ok(comspec) = std::env::var("ComSpec") {
        return comspec;
    }
    if let Ok(system_root) = std::env::var("SystemRoot") {
        return format!(r"{system_root}\System32\cmd.exe");
    }
    r"C:\Windows\System32\cmd.exe".to_string()
}

fn base_env() -> HashMap<String, String> {
    let mut env = HashMap::new();
    env.insert("FOO".to_string(), "bar".to_string());
    #[cfg(windows)]
    {
        if let Ok(system_root) = std::env::var("SystemRoot") {
            env.insert("SystemRoot".to_string(), system_root.clone());
            env.insert("PATH".to_string(), format!(r"{system_root}\System32"));
            env.insert("PROMPT".to_string(), "".to_string());
        }
    }
    env
}

fn temp_dir() -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("codex-pty-{nanos}"));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

async fn run_pty(
    command: &CommandSpec,
    cwd: &Path,
    env: &HashMap<String, String>,
    input: Option<Vec<u8>>,
) -> anyhow::Result<RunResult> {
    let spawned = spawn_pty_process(&command.program, &command.args, cwd, env, &None).await?;
    run_spawned(spawned, input, OUTPUT_TIMEOUT).await
}

async fn run_piped(
    command: &CommandSpec,
    cwd: &Path,
    env: &HashMap<String, String>,
    input: Option<Vec<u8>>,
) -> anyhow::Result<RunResult> {
    let spawned =
        fallback::spawn_piped_process(&command.program, &command.args, cwd, env, &None).await?;
    run_spawned(spawned, input, OUTPUT_TIMEOUT).await
}

async fn run_spawned(
    spawned: SpawnedPty,
    input: Option<Vec<u8>>,
    timeout: Duration,
) -> anyhow::Result<RunResult> {
    let writer = spawned.session.writer_sender();
    if let Some(bytes) = input {
        writer.send(bytes).await?;
    }
    drop(writer);

    tokio::time::timeout(timeout, collect_output(spawned.output_rx, spawned.exit_rx)).await?
}

async fn spawn_interactive_pty(
    command: &CommandSpec,
    cwd: &Path,
    env: &HashMap<String, String>,
) -> anyhow::Result<InteractiveSession> {
    let spawned = spawn_pty_process(&command.program, &command.args, cwd, env, &None).await?;
    Ok(InteractiveSession {
        writer: spawned.session.writer_sender(),
        session: spawned.session,
        output_rx: spawned.output_rx,
        exit_rx: spawned.exit_rx,
        buffer: Vec::new(),
    })
}

async fn spawn_interactive_piped(
    command: &CommandSpec,
    cwd: &Path,
    env: &HashMap<String, String>,
) -> anyhow::Result<InteractiveSession> {
    let spawned =
        fallback::spawn_piped_process(&command.program, &command.args, cwd, env, &None).await?;
    Ok(InteractiveSession {
        writer: spawned.session.writer_sender(),
        session: spawned.session,
        output_rx: spawned.output_rx,
        exit_rx: spawned.exit_rx,
        buffer: Vec::new(),
    })
}

async fn collect_output(
    mut output_rx: broadcast::Receiver<Vec<u8>>,
    mut exit_rx: oneshot::Receiver<i32>,
) -> anyhow::Result<RunResult> {
    let mut output = Vec::new();
    let mut lagged = None;
    let exit = loop {
        tokio::select! {
            received = output_rx.recv() => {
                match received {
                    Ok(bytes) => output.extend_from_slice(&bytes),
                    Err(broadcast::error::RecvError::Closed) => {}
                    Err(broadcast::error::RecvError::Lagged(skipped)) => {
                        lagged = Some(skipped);
                    }
                }
            }
            res = &mut exit_rx => {
                break match res {
                    Ok(code) => ExitOutcome::Exited(code),
                    Err(_) => ExitOutcome::Dropped,
                };
            }
        }
    };
    if let Some(skipped) = lagged {
        anyhow::bail!("output lagged by {skipped} messages");
    }

    let drain_deadline = Instant::now() + Duration::from_millis(50);
    while Instant::now() < drain_deadline {
        match output_rx.try_recv() {
            Ok(bytes) => output.extend_from_slice(&bytes),
            Err(broadcast::error::TryRecvError::Empty) => break,
            Err(broadcast::error::TryRecvError::Closed) => break,
            Err(broadcast::error::TryRecvError::Lagged(skipped)) => {
                anyhow::bail!("output lagged by {skipped} messages");
            }
        }
    }

    Ok(RunResult { output, exit })
}

async fn wait_for_output_contains(
    output_rx: &mut broadcast::Receiver<Vec<u8>>,
    buffer: &mut Vec<u8>,
    marker: &str,
    timeout: Duration,
) -> anyhow::Result<()> {
    tokio::time::timeout(timeout, async {
        loop {
            if normalize_output(buffer).contains(marker) {
                return Ok(());
            }
            match output_rx.recv().await {
                Ok(bytes) => buffer.extend_from_slice(&bytes),
                Err(broadcast::error::RecvError::Closed) => {
                    anyhow::bail!("output channel closed before receiving {marker}");
                }
                Err(broadcast::error::RecvError::Lagged(skipped)) => {
                    anyhow::bail!("output lagged by {skipped} messages");
                }
            }
        }
    })
    .await?
}

async fn finish_interactive(session: InteractiveSession) -> anyhow::Result<RunResult> {
    let mut result = collect_output(session.output_rx, session.exit_rx).await?;
    let mut output = session.buffer;
    output.extend_from_slice(&result.output);
    result.output = output;
    Ok(result)
}

fn normalize_output(output: &[u8]) -> String {
    String::from_utf8_lossy(output)
        .replace("\r\n", "\n")
        .replace('\r', "\n")
}

fn normalize_lines(output: &[u8]) -> Vec<String> {
    normalize_output(output)
        .lines()
        .map(str::to_string)
        .collect()
}

fn normalize_cwd_lines(output: &[u8]) -> Vec<String> {
    normalize_lines(output)
        .into_iter()
        .map(|line| {
            let Some(stripped) = line.strip_prefix("CWD=/private/") else {
                return line;
            };
            format!("CWD=/{stripped}")
        })
        .collect()
}

fn strip_echoed_input(lines: Vec<String>, input: &str) -> Vec<String> {
    lines.into_iter().filter(|line| line != input).collect()
}

fn assert_lines_match(actual: &[u8], expected: &[&str]) {
    let lines = normalize_lines(actual);
    let expected = expected
        .iter()
        .copied()
        .map(str::to_string)
        .collect::<Vec<_>>();
    assert_eq!(lines, expected);
}

fn assert_line_set_match(actual: &[u8], expected: &[&str]) {
    let mut lines = normalize_lines(actual);
    lines.retain(|line| !line.is_empty());
    lines.sort();
    let mut expected = expected
        .iter()
        .copied()
        .map(str::to_string)
        .collect::<Vec<_>>();
    expected.sort();
    assert_eq!(lines, expected);
}

fn assert_parity(pty: &RunResult, piped: &RunResult) {
    assert_eq!(pty.exit, piped.exit);
    assert_eq!(
        normalize_output(&pty.output),
        normalize_output(&piped.output)
    );
}

fn line_ending() -> &'static str {
    #[cfg(windows)]
    {
        "\r\n"
    }
    #[cfg(unix)]
    {
        "\n"
    }
}

async fn send_line(
    writer: &mut mpsc::Sender<Vec<u8>>,
    line: &str,
) -> Result<(), mpsc::error::SendError<Vec<u8>>> {
    writer
        .send(format!("{line}{line_ending}", line_ending = line_ending()).into_bytes())
        .await
}

fn extract_marker_lines(output: &[u8]) -> Vec<String> {
    normalize_output(output)
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim_start();
            trimmed
                .starts_with("CMD_MARKER_")
                .then(|| trimmed.to_string())
        })
        .collect()
}

fn output_has_marker_line(output: &[u8], marker: &str) -> bool {
    normalize_output(output).lines().any(|line| {
        let trimmed = line.trim_start();
        trimmed.starts_with(marker)
    })
}

#[tokio::test]
// Verifies basic stdout and exit code parity for a simple command.
async fn standard_output_parity() -> anyhow::Result<()> {
    if cfg!(windows) && !conpty_supported() {
        return Ok(());
    }
    let command = shell_command(
        #[cfg(unix)]
        r#"printf "hello\nworld\n"; exit 7"#,
        #[cfg(windows)]
        r#"echo hello & echo world & exit /b 7"#,
    );
    let env = base_env();
    let cwd = temp_dir();

    let pty = run_pty(&command, &cwd, &env, None).await?;
    let piped = run_piped(&command, &cwd, &env, None).await?;

    assert_eq!(pty.exit, ExitOutcome::Exited(7));
    assert_eq!(piped.exit, ExitOutcome::Exited(7));
    assert_lines_match(&pty.output, &["hello", "world"]);
    assert_parity(&pty, &piped);
    Ok(())
}

#[tokio::test]
// Verifies commands with no output stay empty and match parity.
async fn no_output_parity() -> anyhow::Result<()> {
    if cfg!(windows) && !conpty_supported() {
        return Ok(());
    }
    let command = shell_command(
        #[cfg(unix)]
        "true",
        #[cfg(windows)]
        "exit /b 0",
    );
    let env = base_env();
    let cwd = temp_dir();

    let pty = run_pty(&command, &cwd, &env, None).await?;
    let piped = run_piped(&command, &cwd, &env, None).await?;

    assert_eq!(normalize_output(&pty.output), "");
    assert_eq!(normalize_output(&piped.output), "");
    assert_parity(&pty, &piped);
    Ok(())
}

#[tokio::test]
// Verifies exit state and stored exit code match PTY and are consistent with exit_rx.
async fn exit_state_parity() -> anyhow::Result<()> {
    if cfg!(windows) && !conpty_supported() {
        return Ok(());
    }
    let command = shell_command(
        #[cfg(unix)]
        r#"printf "done\n"; exit 3"#,
        #[cfg(windows)]
        r#"echo done & exit /b 3"#,
    );
    let env = base_env();
    let cwd = temp_dir();

    let pty_spawned = spawn_pty_process(&command.program, &command.args, &cwd, &env, &None).await?;
    let pty_result = tokio::time::timeout(
        OUTPUT_TIMEOUT,
        collect_output(pty_spawned.output_rx, pty_spawned.exit_rx),
    )
    .await??;
    let pty_code = match pty_result.exit {
        ExitOutcome::Exited(code) => code,
        ExitOutcome::Dropped => anyhow::bail!("pty exit dropped"),
    };
    assert!(pty_spawned.session.has_exited());
    assert_eq!(pty_spawned.session.exit_code(), Some(pty_code));

    let piped_spawned =
        fallback::spawn_piped_process(&command.program, &command.args, &cwd, &env, &None).await?;
    let piped_result = tokio::time::timeout(
        OUTPUT_TIMEOUT,
        collect_output(piped_spawned.output_rx, piped_spawned.exit_rx),
    )
    .await??;
    let piped_code = match piped_result.exit {
        ExitOutcome::Exited(code) => code,
        ExitOutcome::Dropped => anyhow::bail!("piped exit dropped"),
    };
    assert!(piped_spawned.session.has_exited());
    assert_eq!(piped_spawned.session.exit_code(), Some(piped_code));

    assert_eq!(pty_code, piped_code);
    Ok(())
}

#[tokio::test]
// Verifies env propagation and working directory parity.
async fn env_and_cwd_parity() -> anyhow::Result<()> {
    if cfg!(windows) && !conpty_supported() {
        return Ok(());
    }
    let command = shell_command(
        #[cfg(unix)]
        r#"printf "FOO=%s\n" "$FOO"; printf "CWD=%s\n" "$(pwd)""#,
        #[cfg(windows)]
        r#"echo FOO=%FOO% & echo CWD=%CD%"#,
    );
    let env = base_env();
    let cwd = temp_dir();

    let pty = run_pty(&command, &cwd, &env, None).await?;
    let piped = run_piped(&command, &cwd, &env, None).await?;

    let cwd_line = format!("CWD={}", cwd.display());
    let expected = vec!["FOO=bar".to_string(), cwd_line];
    assert_eq!(normalize_cwd_lines(&pty.output), expected);
    assert_eq!(normalize_cwd_lines(&piped.output), expected);
    Ok(())
}

#[tokio::test]
// Verifies large output throughput parity.
async fn large_output_parity() -> anyhow::Result<()> {
    if cfg!(windows) && !conpty_supported() {
        return Ok(());
    }
    let command = shell_command(
        #[cfg(unix)]
        r#"i=0; while [ $i -lt 200 ]; do printf "line-%s\n" "$i"; i=$((i+1)); done"#,
        #[cfg(windows)]
        r#"for /L %i in (0,1,199) do @echo line-%i"#,
    );
    let env = base_env();
    let cwd = temp_dir();

    let pty = run_pty(&command, &cwd, &env, None).await?;
    let piped = run_piped(&command, &cwd, &env, None).await?;

    let mut expected = Vec::new();
    for i in 0..200 {
        expected.push(format!("line-{i}"));
    }
    assert_eq!(normalize_lines(&pty.output), expected);
    assert_parity(&pty, &piped);
    Ok(())
}

#[tokio::test]
// Verifies merged stdout/stderr output parity, ignoring ordering.
async fn stderr_merge_parity() -> anyhow::Result<()> {
    if cfg!(windows) && !conpty_supported() {
        return Ok(());
    }
    let command = shell_command(
        #[cfg(unix)]
        r#"printf "stdout\n"; printf "stderr\n" 1>&2"#,
        #[cfg(windows)]
        r#"echo stdout & echo stderr 1>&2"#,
    );
    let env = base_env();
    let cwd = temp_dir();

    let pty = run_pty(&command, &cwd, &env, None).await?;
    let piped = run_piped(&command, &cwd, &env, None).await?;

    assert_line_set_match(&pty.output, &["stdout", "stderr"]);
    assert_line_set_match(&piped.output, &["stdout", "stderr"]);
    assert_eq!(pty.exit, piped.exit);
    Ok(())
}

#[tokio::test]
// Verifies terminate behavior parity for long-running children.
async fn terminate_parity() -> anyhow::Result<()> {
    if cfg!(windows) && !conpty_supported() {
        return Ok(());
    }
    let command = shell_command(
        #[cfg(unix)]
        "sleep 60",
        #[cfg(windows)]
        r#"ping -n 60 127.0.0.1 >nul"#,
    );
    let env = base_env();
    let cwd = temp_dir();

    let pty = spawn_pty_process(&command.program, &command.args, &cwd, &env, &None).await?;
    let piped =
        fallback::spawn_piped_process(&command.program, &command.args, &cwd, &env, &None).await?;

    tokio::time::sleep(Duration::from_millis(50)).await;
    pty.session.terminate();
    piped.session.terminate();

    let pty_exit = tokio::time::timeout(TERMINATE_TIMEOUT, pty.exit_rx).await;
    let piped_exit = tokio::time::timeout(TERMINATE_TIMEOUT, piped.exit_rx).await;

    assert!(pty_exit.is_ok());
    assert!(piped_exit.is_ok());

    let pty_exit = pty_exit.unwrap();
    let piped_exit = piped_exit.unwrap();
    match pty_exit {
        Ok(code) => {
            let piped_code = piped_exit?;
            assert_eq!(code, piped_code);
        }
        Err(_) => {
            assert!(piped_exit.is_err());
        }
    }
    Ok(())
}

#[tokio::test]
// Verifies terminate can be called twice and piped matches PTY.
async fn terminate_idempotency_parity() -> anyhow::Result<()> {
    if cfg!(windows) && !conpty_supported() {
        return Ok(());
    }
    let command = shell_command(
        #[cfg(unix)]
        "sleep 60",
        #[cfg(windows)]
        r#"ping -n 60 127.0.0.1 >nul"#,
    );
    let env = base_env();
    let cwd = temp_dir();

    let pty = spawn_pty_process(&command.program, &command.args, &cwd, &env, &None).await?;
    let piped =
        fallback::spawn_piped_process(&command.program, &command.args, &cwd, &env, &None).await?;

    tokio::time::sleep(Duration::from_millis(50)).await;
    pty.session.terminate();
    pty.session.terminate();
    piped.session.terminate();
    piped.session.terminate();

    let pty_exit = tokio::time::timeout(TERMINATE_TIMEOUT, pty.exit_rx).await?;
    let piped_exit = tokio::time::timeout(TERMINATE_TIMEOUT, piped.exit_rx).await?;
    match pty_exit {
        Ok(code) => {
            let piped_code = piped_exit?;
            assert_eq!(code, piped_code);
        }
        Err(pty_err) => {
            let piped_err = piped_exit.unwrap_err();
            assert_eq!(pty_err.to_string(), piped_err.to_string());
        }
    }
    Ok(())
}

#[tokio::test]
// Verifies empty program errors are consistent across implementations.
async fn empty_program_errors() {
    if cfg!(windows) && !conpty_supported() {
        return;
    }
    let env = base_env();
    let cwd = temp_dir();
    let args = Vec::new();

    let pty_err = spawn_pty_process("", &args, &cwd, &env, &None)
        .await
        .unwrap_err();
    let piped_err = fallback::spawn_piped_process("", &args, &cwd, &env, &None)
        .await
        .unwrap_err();

    let pty_msg = pty_err.to_string();
    let piped_msg = piped_err.to_string();
    assert!(pty_msg.contains("missing program"));
    assert!(piped_msg.contains("missing program"));
}

#[tokio::test]
// Verifies arg0 overrides the program path for both implementations.
async fn arg0_overrides_program() -> anyhow::Result<()> {
    if cfg!(windows) && !conpty_supported() {
        return Ok(());
    }
    let command = shell_command(
        #[cfg(unix)]
        r#"printf "ok\n""#,
        #[cfg(windows)]
        r#"echo ok"#,
    );
    let env = base_env();
    let cwd = temp_dir();
    let bogus = "this-does-not-exist".to_string();
    let arg0 = Some(command.program.clone());

    let pty = spawn_pty_process(&bogus, &command.args, &cwd, &env, &arg0).await?;
    let piped = fallback::spawn_piped_process(&bogus, &command.args, &cwd, &env, &arg0).await?;

    let pty = run_spawned(pty, None, OUTPUT_TIMEOUT).await?;
    let piped = run_spawned(piped, None, OUTPUT_TIMEOUT).await?;

    assert_lines_match(&pty.output, &["ok"]);
    assert_parity(&pty, &piped);
    Ok(())
}

#[tokio::test]
// Verifies multi-command interactive sessions return outputs in order.
async fn multi_command_parity() -> anyhow::Result<()> {
    if cfg!(windows) && !conpty_supported() {
        return Ok(());
    }
    let command = shell_repl_command();
    let env = base_env();
    let cwd = temp_dir();

    let mut pty = spawn_interactive_pty(&command, &cwd, &env).await?;
    let mut piped = spawn_interactive_piped(&command, &cwd, &env).await?;

    let marker_one = "CMD_MARKER_ONE";
    send_line(&mut pty.writer, &format!("echo {marker_one}")).await?;
    send_line(&mut piped.writer, &format!("echo {marker_one}")).await?;
    wait_for_output_contains(
        &mut pty.output_rx,
        &mut pty.buffer,
        marker_one,
        OUTPUT_TIMEOUT,
    )
    .await?;
    wait_for_output_contains(
        &mut piped.output_rx,
        &mut piped.buffer,
        marker_one,
        OUTPUT_TIMEOUT,
    )
    .await?;

    let marker_two = "CMD_MARKER_TWO";
    send_line(&mut pty.writer, &format!("echo {marker_two}")).await?;
    send_line(&mut piped.writer, &format!("echo {marker_two}")).await?;
    wait_for_output_contains(
        &mut pty.output_rx,
        &mut pty.buffer,
        marker_two,
        OUTPUT_TIMEOUT,
    )
    .await?;
    wait_for_output_contains(
        &mut piped.output_rx,
        &mut piped.buffer,
        marker_two,
        OUTPUT_TIMEOUT,
    )
    .await?;

    send_line(&mut pty.writer, "exit").await?;
    send_line(&mut piped.writer, "exit").await?;

    let pty = finish_interactive(pty).await?;
    let piped = finish_interactive(piped).await?;

    assert_eq!(pty.exit, ExitOutcome::Exited(0));
    assert_eq!(piped.exit, ExitOutcome::Exited(0));
    let pty_markers = extract_marker_lines(&pty.output);
    let piped_markers = extract_marker_lines(&piped.output);
    assert_eq!(pty_markers, piped_markers);
    assert_eq!(
        pty_markers,
        vec![marker_one.to_string(), marker_two.to_string()]
    );
    Ok(())
}

#[tokio::test]
// Verifies late output subscribers receive subsequent output.
async fn output_subscriber_parity() -> anyhow::Result<()> {
    if cfg!(windows) && !conpty_supported() {
        return Ok(());
    }
    let command = shell_repl_command();
    let env = base_env();
    let cwd = temp_dir();

    let mut pty = spawn_interactive_pty(&command, &cwd, &env).await?;
    let mut piped = spawn_interactive_piped(&command, &cwd, &env).await?;

    let mut pty_late_rx = pty.session.output_receiver();
    let mut piped_late_rx = piped.session.output_receiver();
    let mut pty_late_buffer = Vec::new();
    let mut piped_late_buffer = Vec::new();

    let marker = "CMD_MARKER_SUB";
    send_line(&mut pty.writer, &format!("echo {marker}")).await?;
    send_line(&mut piped.writer, &format!("echo {marker}")).await?;
    tokio::time::timeout(OUTPUT_TIMEOUT, async {
        loop {
            if output_has_marker_line(&pty_late_buffer, marker) {
                break;
            }
            match pty_late_rx.recv().await {
                Ok(bytes) => pty_late_buffer.extend_from_slice(&bytes),
                Err(broadcast::error::RecvError::Closed) => {
                    anyhow::bail!("output channel closed before receiving {marker}");
                }
                Err(broadcast::error::RecvError::Lagged(skipped)) => {
                    anyhow::bail!("output lagged by {skipped} messages");
                }
            }
        }
        Ok::<_, anyhow::Error>(())
    })
    .await??;
    tokio::time::timeout(OUTPUT_TIMEOUT, async {
        loop {
            if output_has_marker_line(&piped_late_buffer, marker) {
                break;
            }
            match piped_late_rx.recv().await {
                Ok(bytes) => piped_late_buffer.extend_from_slice(&bytes),
                Err(broadcast::error::RecvError::Closed) => {
                    anyhow::bail!("output channel closed before receiving {marker}");
                }
                Err(broadcast::error::RecvError::Lagged(skipped)) => {
                    anyhow::bail!("output lagged by {skipped} messages");
                }
            }
        }
        Ok::<_, anyhow::Error>(())
    })
    .await??;

    send_line(&mut pty.writer, "exit").await?;
    send_line(&mut piped.writer, "exit").await?;

    let pty = finish_interactive(pty).await?;
    let piped = finish_interactive(piped).await?;
    assert_eq!(pty.exit, ExitOutcome::Exited(0));
    assert_eq!(piped.exit, ExitOutcome::Exited(0));
    assert_eq!(
        extract_marker_lines(&pty_late_buffer),
        vec![marker.to_string()]
    );
    assert_eq!(
        extract_marker_lines(&piped_late_buffer),
        vec![marker.to_string()]
    );
    Ok(())
}

#[tokio::test]
// Verifies post-kill write failures and exit errors match.
async fn multi_command_after_kill_parity() -> anyhow::Result<()> {
    if cfg!(windows) && !conpty_supported() {
        return Ok(());
    }
    let command = shell_repl_command();
    let env = base_env();
    let cwd = temp_dir();

    let mut pty = spawn_interactive_pty(&command, &cwd, &env).await?;
    let mut piped = spawn_interactive_piped(&command, &cwd, &env).await?;

    let marker = "CMD_MARKER_KILL";
    send_line(&mut pty.writer, &format!("echo {marker}")).await?;
    send_line(&mut piped.writer, &format!("echo {marker}")).await?;
    wait_for_output_contains(&mut pty.output_rx, &mut pty.buffer, marker, OUTPUT_TIMEOUT).await?;
    wait_for_output_contains(
        &mut piped.output_rx,
        &mut piped.buffer,
        marker,
        OUTPUT_TIMEOUT,
    )
    .await?;

    pty.session.terminate();
    piped.session.terminate();

    let pty_send = send_line(&mut pty.writer, "echo CMD_MARKER_AFTER_KILL")
        .await
        .map_err(|err| err.to_string());
    let piped_send = send_line(&mut piped.writer, "echo CMD_MARKER_AFTER_KILL")
        .await
        .map_err(|err| err.to_string());
    match (pty_send, piped_send) {
        (Ok(()), Ok(())) => {}
        (Err(pty_err), Err(piped_err)) => {
            assert_eq!(pty_err, piped_err);
        }
        (Ok(()), Err(piped_err)) => {
            panic!("piped write failed while PTY succeeded: {piped_err}");
        }
        (Err(pty_err), Ok(())) => {
            panic!("piped write succeeded while PTY failed: {pty_err}");
        }
    }

    let pty_exit = tokio::time::timeout(TERMINATE_TIMEOUT, pty.exit_rx).await?;
    let piped_exit = tokio::time::timeout(TERMINATE_TIMEOUT, piped.exit_rx).await?;
    match pty_exit {
        Ok(code) => {
            let piped_code = piped_exit?;
            assert_eq!(code, piped_code);
        }
        Err(pty_err) => {
            let piped_err = piped_exit.unwrap_err();
            assert_eq!(pty_err.to_string(), piped_err.to_string());
        }
    }
    Ok(())
}

#[cfg(unix)]
#[tokio::test]
// Unix-only: relies on /bin/sh + stty to disable echo; cmd/ConPTY behavior differs.
// Verifies reading from stdin works and matches output parity.
async fn stdin_read_parity() -> anyhow::Result<()> {
    let Some(stty) = stty_path() else {
        return Ok(());
    };
    let command = shell_command(&format!(
        r#"if [ -t 0 ]; then "{stty}" -echo; fi; IFS= read -r line; printf "got:%s\n" "$line""#,
    ));
    let env = base_env();
    let cwd = temp_dir();
    let input = Some(b"hello\n".to_vec());

    let pty = run_pty(&command, &cwd, &env, input.clone()).await?;
    let piped = run_piped(&command, &cwd, &env, input).await?;

    let expected = vec!["got:hello".to_string()];
    let pty_lines = strip_echoed_input(normalize_lines(&pty.output), "hello");
    let piped_lines = strip_echoed_input(normalize_lines(&piped.output), "hello");
    assert_eq!(pty_lines, expected);
    assert_eq!(piped_lines, expected);
    Ok(())
}

#[cfg(unix)]
fn stty_path() -> Option<String> {
    let candidates = ["/bin/stty", "/usr/bin/stty"];
    candidates
        .into_iter()
        .find(|path| Path::new(*path).exists())
        .map(str::to_string)
}
