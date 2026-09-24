//! Exercises the public macOS sandbox runner with a disposable controlling terminal.

#![cfg(target_os = "macos")]

use std::collections::HashMap;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use anyhow::Context as _;
use codex_utils_pty::SpawnedProcess;
use codex_utils_pty::TerminalSize;
use codex_utils_pty::spawn_pty_process;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

const PROBE: &str = r#"
import errno, fcntl, os, select, signal, struct, sys, termios, tty

signal.alarm(15)
assert os.isatty(0)
assert os.tcgetpgrp(0) == os.getpgrp()
assert struct.unpack("HHHH", fcntl.ioctl(0, termios.TIOCGWINSZ, b"\0" * 8))[:2] == (24, 80)
original = termios.tcgetattr(0)
try:
    tty.setraw(0)
    print("__ready__", flush=True)
    assert os.read(0, 1) == b"k"
    try:
        fcntl.ioctl(0, termios.TIOCSTI, b"x")
    except OSError as error:
        assert sys.argv[1] == "deny", error
        assert error.errno == errno.EPERM, error
        assert not select.select([0], [], [], 0)[0]
    else:
        assert os.read(0, 1) == b"x"
        assert sys.argv[1] == "allow", "TIOCSTI unexpectedly succeeded"
finally:
    termios.tcsetattr(0, termios.TCSANOW, original)
print("__passed__", flush=True)
"#;

#[tokio::test]
async fn sandbox_blocks_terminal_input_injection() -> anyhow::Result<()> {
    let availability = Command::new("/usr/bin/sandbox-exec")
        .args(["-p", "(version 1)(allow default)", "/usr/bin/true"])
        .output()?;
    if !availability.status.success()
        && String::from_utf8_lossy(&availability.stderr)
            .contains("sandbox-exec: sandbox_apply: Operation not permitted")
    {
        eprintln!("skipping terminal injection test: nested Seatbelt is unavailable");
        return Ok(());
    }
    assert!(availability.status.success(), "{availability:?}");

    let codex_home = TempDir::new()?;
    let mut env: HashMap<String, String> = std::env::vars().collect();
    env.insert(
        "CODEX_HOME".to_string(),
        codex_home.path().to_string_lossy().into_owned(),
    );

    // The control proves this is a readable, foreground controlling terminal on
    // which TIOCSTI would otherwise work. All injected input stays in this PTY.
    run_probe(
        "/usr/bin/python3",
        vec!["-c".to_string(), PROBE.to_string(), "allow".to_string()],
        codex_home.path(),
        &env,
    )
    .await?;

    let codex = codex_utils_cargo_bin::cargo_bin("codex")?;
    run_probe(
        &codex.to_string_lossy(),
        vec![
            "sandbox".to_string(),
            "-P".to_string(),
            ":read-only".to_string(),
            "--".to_string(),
            "/usr/bin/python3".to_string(),
            "-c".to_string(),
            PROBE.to_string(),
            "deny".to_string(),
        ],
        codex_home.path(),
        &env,
    )
    .await
}

async fn run_probe(
    program: &str,
    args: Vec<String>,
    cwd: &Path,
    env: &HashMap<String, String>,
) -> anyhow::Result<()> {
    let SpawnedProcess {
        session,
        mut stdout_rx,
        exit_rx,
        ..
    } = spawn_pty_process(
        program,
        &args,
        cwd,
        env,
        /*arg0*/ &None,
        TerminalSize::default(),
        codex_utils_pty::ChildFds::Inherited(&[]),
    )
    .await?;
    let mut output = Vec::new();
    let mut sent_input = false;
    let code = tokio::time::timeout(Duration::from_secs(30), async {
        while let Some(bytes) = stdout_rx.recv().await {
            output.extend_from_slice(&bytes);
            if !sent_input && String::from_utf8_lossy(&output).contains("__ready__") {
                session.writer_sender().send(b"k".to_vec()).await?;
                sent_input = true;
            }
        }
        Ok::<_, anyhow::Error>(exit_rx.await?)
    })
    .await
    .context("terminal probe timed out")??;
    let output = String::from_utf8_lossy(&output);
    assert_eq!(code, 0, "terminal probe failed: {output}");
    assert!(output.contains("__passed__"), "{output}");
    Ok(())
}
