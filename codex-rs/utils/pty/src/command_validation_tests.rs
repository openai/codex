//! Invalid command inputs must fail before a child can run on either backend.

use std::io;

use pretty_assertions::assert_eq;

use crate::Command;
use crate::ProcessMode;

enum Input {
    Program,
    Arg,
    Args,
    Cwd,
    Arg0,
    EnvKey,
    EnvValue,
}

#[tokio::test]
async fn embedded_nul_is_rejected_before_spawning() -> anyhow::Result<()> {
    for mode in [ProcessMode::Inherit, ProcessMode::NewSession] {
        for input in [
            Input::Program,
            Input::Arg,
            Input::Args,
            Input::Cwd,
            Input::Arg0,
            Input::EnvKey,
            Input::EnvValue,
        ] {
            let root = tempfile::tempdir()?;
            let marker = root.path().join("ran");
            let mut command = Command::new(match input {
                Input::Program => "/bin/sh\0",
                Input::Arg
                | Input::Args
                | Input::Cwd
                | Input::Arg0
                | Input::EnvKey
                | Input::EnvValue => "/bin/sh",
            });
            command
                .process_mode(mode)
                .args(["-c", "printf ran > \"$1\"", "sh"])
                .arg(&marker);
            match input {
                Input::Program => {}
                Input::Arg => {
                    command.arg("bad\0arg");
                }
                Input::Args => {
                    command.args(["bad\0arg"]);
                }
                Input::Cwd => {
                    command.current_dir("bad\0cwd").current_dir(root.path());
                }
                Input::Arg0 => {
                    command.arg0("bad\0arg0").arg0("sh");
                }
                Input::EnvKey => {
                    command.env("bad\0key", "value");
                }
                Input::EnvValue => {
                    command.env("key", "bad\0value");
                }
            }
            let error = match command.spawn() {
                Err(error) => error,
                Ok(child) => {
                    let output = child.wait_with_output().await?;
                    anyhow::bail!(
                        "invalid command ran: {output:?}, marker={}",
                        marker.exists()
                    );
                }
            };
            assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
            assert!(!marker.exists());
        }
    }
    Ok(())
}

#[tokio::test]
async fn literal_nul_placeholder_is_a_valid_argument() -> anyhow::Result<()> {
    let mut command = Command::new("/bin/echo");
    command
        .process_mode(ProcessMode::NewSession)
        .arg("<string-with-nul>");
    let output = command.spawn()?.wait_with_output().await?;
    assert!(output.status.success());
    assert_eq!(output.stdout, b"<string-with-nul>\n");
    Ok(())
}

#[tokio::test]
async fn pipe_rejects_nul_arguments_without_side_effects() -> anyhow::Result<()> {
    let root = tempfile::tempdir()?;
    let marker = root.path().join("ran");
    let result = crate::spawn_pipe_process(
        "/usr/bin/touch",
        &[
            marker.to_string_lossy().into_owned(),
            "bad\0arg".to_string(),
        ],
        root.path(),
        &std::collections::HashMap::new(),
        /*arg0*/ &None,
        &[],
    )
    .await;
    let error = match result {
        Err(error) => error,
        Ok(child) => {
            let _ = child.exit_rx.await;
            anyhow::bail!("invalid pipe command ran, marker={}", marker.exists());
        }
    };
    assert_eq!(
        error.downcast_ref::<io::Error>().map(io::Error::kind),
        Some(io::ErrorKind::InvalidInput)
    );
    assert!(!marker.exists());
    Ok(())
}
