use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use tokio::select;
use tokio::time::timeout;

/// Regression test for https://github.com/openai/codex/issues/8803.
#[tokio::test]
async fn malformed_rules_should_not_panic() -> anyhow::Result<()> {
    // run_codex_cli() does not work on Windows due to PTY limitations.
    if cfg!(windows) {
        return Ok(());
    }

    let tmp = tempfile::tempdir()?;
    let codex_home = tmp.path();
    std::fs::write(
        codex_home.join("rules"),
        "rules should be a directory not a file",
    )?;

    // TODO(mbolin): Figure out why using a temp dir as the cwd causes this test
    // to hang.
    let cwd = std::env::current_dir()?;
    let config_contents = format!(
        r#"
# Pick a local provider so the CLI doesn't prompt for OpenAI auth in this test.
model_provider = "ollama"

[projects]
"{cwd}" = {{ trust_level = "trusted" }}
"#,
        cwd = cwd.display()
    );
    std::fs::write(codex_home.join("config.toml"), config_contents)?;

    let CodexCliOutput { exit_code, output } = run_codex_cli(codex_home, cwd).await?;
    assert_ne!(0, exit_code, "Codex CLI should exit nonzero.");
    assert!(
        output.contains("ERROR: Failed to initialize codex:"),
        "expected startup error in output, got: {output}"
    );
    assert!(
        output.contains("failed to read rules files"),
        "expected rules read error in output, got: {output}"
    );
    Ok(())
}

struct CodexCliOutput {
    exit_code: i32,
    output: String,
}

async fn run_codex_cli(
    codex_home: impl AsRef<Path>,
    cwd: impl AsRef<Path>,
) -> anyhow::Result<CodexCliOutput> {
    let codex_cli = match codex_utils_cargo_bin::cargo_bin("codex") {
        Ok(path) => path,
        Err(err) => {
            if codex_utils_cargo_bin::runfiles_available() {
                return Err(err.into());
            }

            // `cargo test -p codex-tui` builds the `codex-tui` binary but not the `codex`
            // binary (owned by `codex-cli`), so build it on-demand for this test.
            build_codex_cli_bin()
                .await
                .map_err(|build_err| anyhow::anyhow!("{err}; {build_err}"))?
        }
    };
    let mut env = HashMap::new();
    env.insert(
        "CODEX_HOME".to_string(),
        codex_home.as_ref().display().to_string(),
    );

    let args = vec!["-c".to_string(), "analytics.enabled=false".to_string()];
    let spawned = codex_utils_pty::spawn_pty_process(
        codex_cli.to_string_lossy().as_ref(),
        &args,
        cwd.as_ref(),
        &env,
        &None,
    )
    .await?;
    let mut output = Vec::new();
    let mut output_rx = spawned.output_rx;
    let mut exit_rx = spawned.exit_rx;
    let writer_tx = spawned.session.writer_sender();
    let exit_code_result = timeout(Duration::from_secs(10), async {
        // Read PTY output until the process exits while replying to cursor
        // position queries so the TUI can initialize without a real terminal.
        loop {
            select! {
                result = output_rx.recv() => match result {
                    Ok(chunk) => {
                        // The TUI asks for the cursor position via ESC[6n.
                        // Respond with a valid position to unblock startup.
                        if chunk.windows(4).any(|window| window == b"\x1b[6n") {
                            let _ = writer_tx.send(b"\x1b[1;1R".to_vec()).await;
                        }
                        output.extend_from_slice(&chunk);
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break exit_rx.await,
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                },
                result = &mut exit_rx => break result,
            }
        }
    })
    .await;
    let exit_code = match exit_code_result {
        Ok(Ok(code)) => code,
        Ok(Err(err)) => return Err(err.into()),
        Err(_) => {
            spawned.session.terminate();
            anyhow::bail!("timed out waiting for codex CLI to exit");
        }
    };
    // Drain any output that raced with the exit notification.
    while let Ok(chunk) = output_rx.try_recv() {
        output.extend_from_slice(&chunk);
    }

    let output = String::from_utf8_lossy(&output);
    Ok(CodexCliOutput {
        exit_code,
        output: output.to_string(),
    })
}

async fn build_codex_cli_bin() -> anyhow::Result<PathBuf> {
    let tui_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let codex_rs_dir = tui_dir
        .parent()
        .ok_or_else(|| anyhow::anyhow!("CARGO_MANIFEST_DIR is missing its parent directory"))?;

    let mut cmd = tokio::process::Command::new("cargo");
    cmd.args(["build", "-p", "codex-cli", "--bin", "codex"]);
    if !cfg!(debug_assertions) {
        cmd.arg("--release");
    }
    cmd.current_dir(codex_rs_dir);
    let status = cmd.status().await?;
    if !status.success() {
        anyhow::bail!("cargo build exited with {status}");
    }

    let target_dir = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| codex_rs_dir.join("target"));
    let profile_dir = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };

    let exe = if cfg!(windows) { "codex.exe" } else { "codex" };
    let candidate = target_dir.join(profile_dir).join(exe);
    if !candidate.exists() {
        anyhow::bail!(
            "expected codex binary at {}, but it does not exist",
            candidate.display()
        );
    }

    Ok(candidate)
}
