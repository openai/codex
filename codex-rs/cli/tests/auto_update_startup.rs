use std::collections::HashMap;
use std::time::Duration;

use pretty_assertions::assert_eq;
use tokio::select;
use tokio::time::timeout;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

#[tokio::test]
#[cfg_attr(
    debug_assertions,
    ignore = "update checks are disabled in debug builds (cfg(not(debug_assertions)))"
)]
/// Runs the real CLI startup path with a seeded cached update result and a
/// mocked `npm` binary so the auto-update branch is exercised end to end.
async fn startup_auto_update_runs_detected_npm_command() -> anyhow::Result<()> {
    if cfg!(windows) {
        // This test installs a POSIX shell script as the mocked updater.
        return Ok(());
    }

    let tmp = tempfile::tempdir()?;
    let codex_home = tmp.path().join("codex-home");
    std::fs::create_dir_all(&codex_home)?;

    let cwd = std::env::current_dir()?;
    let config_contents = format!(
        r#"
model_provider = "ollama"

[projects]
"{cwd}" = {{ trust_level = "trusted" }}

[features]
startup_auto_update = true
"#,
        cwd = cwd.display()
    );
    std::fs::write(codex_home.join("config.toml"), config_contents)?;

    // Seed a cached "newer version" result so startup takes the normal update
    // flow without depending on a live version check.
    let version_json = serde_json::json!({
        "latest_version": "999.0.0",
        "last_checked_at": "9999-01-01T00:00:00Z",
        "dismissed_version": serde_json::Value::Null,
    });
    std::fs::write(
        codex_home.join("version.json"),
        format!("{}\n", serde_json::to_string(&version_json)?),
    )?;

    // Put a fake `npm` first on PATH and record the exact working directory and
    // arguments the CLI uses.
    let bin_dir = tmp.path().join("bin");
    std::fs::create_dir_all(&bin_dir)?;
    let npm_log_path = tmp.path().join("mock-npm.log");
    let npm_script_path = bin_dir.join("npm");
    std::fs::write(
        &npm_script_path,
        format!(
            r#"#!/bin/sh
set -eu
{{
  printf 'cwd=%s\n' "$(pwd)"
  for arg in "$@"; do
    printf 'arg=%s\n' "$arg"
  done
}} > "{}"
"#,
            npm_log_path.display()
        ),
    )?;
    #[cfg(unix)]
    {
        let mut perms = std::fs::metadata(&npm_script_path)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&npm_script_path, perms)?;
    }

    let codex_cli = codex_utils_cargo_bin::cargo_bin("codex")?;
    let mut env = HashMap::new();
    env.insert("CODEX_HOME".to_string(), codex_home.display().to_string());
    env.insert("CODEX_MANAGED_BY_NPM".to_string(), "1".to_string());
    env.insert(
        "PATH".to_string(),
        match std::env::var("PATH") {
            Ok(path) => format!("{}:{path}", bin_dir.display()),
            Err(_) => bin_dir.display().to_string(),
        },
    );

    let args = vec!["-c".to_string(), "analytics.enabled=false".to_string()];
    let spawned = codex_utils_pty::spawn_pty_process(
        codex_cli.to_string_lossy().as_ref(),
        &args,
        &cwd,
        &env,
        &None,
    )
    .await?;
    let mut output = Vec::new();
    let mut output_rx = spawned.output_rx;
    let mut exit_rx = spawned.exit_rx;
    let writer_tx = spawned.session.writer_sender();

    let exit_code_result = timeout(Duration::from_secs(20), async {
        loop {
            select! {
                result = output_rx.recv() => match result {
                    Ok(chunk) => {
                        if chunk.windows(4).any(|window| window == b"\x1b[6n") {
                            // The TUI asks the terminal for cursor position
                            // during startup; reply so the PTY session can
                            // finish initializing.
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

    while let Ok(chunk) = output_rx.try_recv() {
        output.extend_from_slice(&chunk);
    }

    let output = String::from_utf8_lossy(&output).to_string();
    assert_eq!(
        exit_code, 0,
        "Codex should exit successfully. Output:\n{output}"
    );
    assert!(
        output.contains("Updating Codex via `npm install -g @openai/codex`..."),
        "expected auto-update execution message, got: {output}"
    );
    assert!(
        output.contains("Update ran successfully! Please restart Codex."),
        "expected successful update message, got: {output}"
    );

    let npm_log = std::fs::read_to_string(&npm_log_path)?;
    let cwd_line = format!("cwd={}", cwd.display());
    assert!(
        npm_log.lines().any(|line| line == cwd_line),
        "expected npm to run in cwd {}, got log:\n{npm_log}",
        cwd.display(),
    );

    let args_seen: Vec<String> = npm_log
        .lines()
        .filter_map(|line| line.strip_prefix("arg=").map(ToString::to_string))
        .collect();
    assert_eq!(
        args_seen,
        vec![
            "install".to_string(),
            "-g".to_string(),
            "@openai/codex".to_string(),
        ]
    );

    Ok(())
}
