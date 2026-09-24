//! Verify natural and terminated console clients close output without losing descendants.

use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

use pretty_assertions::assert_eq;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn output_closes_after_client_exit_while_session_is_retained() -> anyhow::Result<()> {
    if super::CONPTY_RELEASE.is_none() {
        eprintln!("automatic ConPTY closure requires Windows 11 24H2 or newer");
        return Ok(());
    }
    let env: HashMap<String, String> = std::env::vars().collect();
    let spawned = crate::spawn_pty_process(
        "cmd.exe",
        &[
            "/d".to_string(),
            "/c".to_string(),
            "echo complete".to_string(),
        ],
        Path::new("."),
        &env,
        /*arg0*/ &None,
        crate::TerminalSize::default(),
        crate::ChildFds::Inherited(&[]),
    )
    .await?;
    let mut output = spawned.stdout_rx;
    let bytes = tokio::time::timeout(Duration::from_secs(5), async {
        let mut bytes = Vec::new();
        while let Some(chunk) = output.recv().await {
            bytes.extend(chunk);
        }
        bytes
    })
    .await?;
    assert!(String::from_utf8_lossy(&bytes).contains("complete"));
    assert_eq!(spawned.exit_rx.await?, 0);
    assert!(spawned.session.has_exited());
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn output_remains_open_for_surviving_console_child() -> anyhow::Result<()> {
    if super::CONPTY_RELEASE.is_none() {
        eprintln!("automatic ConPTY closure requires Windows 11 24H2 or newer");
        return Ok(());
    }
    let Some(python) = crate::tests::find_python() else {
        eprintln!("python not found; skipping ConPTY surviving-child test");
        return Ok(());
    };
    let env: HashMap<String, String> = std::env::vars().collect();
    let spawned = crate::spawn_pty_process(
        &python,
        &[
            "-u".to_string(),
            "-c".to_string(),
            "import subprocess,sys; subprocess.Popen([sys.executable,'-u','-c',sys.argv[1]])".to_string(),
            "import sys; print('child-ready',flush=True); line=sys.stdin.readline().strip(); print('child-received:'+line,flush=True)".to_string(),
        ],
        Path::new("."),
        &env,
        /*arg0*/ &None,
        crate::TerminalSize::default(),
        crate::ChildFds::Inherited(&[]),
    )
    .await?;
    let mut output = spawned.stdout_rx;
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(5), spawned.exit_rx).await??,
        0
    );
    let mut bytes = Vec::new();
    tokio::time::timeout(Duration::from_secs(5), async {
        while !String::from_utf8_lossy(&bytes).contains("child-ready") {
            bytes.extend(
                output
                    .recv()
                    .await
                    .expect("child output closed before readiness"),
            );
        }
        spawned
            .session
            .writer_sender()
            .send(b"continued\r".to_vec())
            .await?;
        while let Some(chunk) = output.recv().await {
            bytes.extend(chunk);
        }
        Ok::<_, anyhow::Error>(())
    })
    .await??;
    assert!(String::from_utf8_lossy(&bytes).contains("child-received:continued"));
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn output_closes_after_termination_while_session_is_retained() -> anyhow::Result<()> {
    if super::CONPTY_RELEASE.is_none() {
        eprintln!("automatic ConPTY closure requires Windows 11 24H2 or newer");
        return Ok(());
    }
    let env: HashMap<String, String> = std::env::vars().collect();
    let spawned = crate::spawn_pty_process(
        "cmd.exe",
        &[
            "/d".to_string(),
            "/c".to_string(),
            "set /p line=child-ready".to_string(),
        ],
        Path::new("."),
        &env,
        /*arg0*/ &None,
        crate::TerminalSize::default(),
        crate::ChildFds::Inherited(&[]),
    )
    .await?;
    let mut output = spawned.stdout_rx;
    tokio::time::timeout(Duration::from_secs(5), async {
        let mut bytes = Vec::new();
        while !String::from_utf8_lossy(&bytes).contains("child-ready") {
            bytes.extend(
                output
                    .recv()
                    .await
                    .expect("child output closed before readiness"),
            );
        }
        spawned.session.request_terminate();
        while output.recv().await.is_some() {}
    })
    .await?;
    assert_ne!(spawned.exit_rx.await?, 0);
    assert!(spawned.session.has_exited());
    Ok(())
}
