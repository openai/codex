//! Black-box coverage for diagnostics under a repository-controlled PATH.

use std::ffi::OsString;
use std::path::PathBuf;
use std::time::Duration;

#[cfg(unix)]
use anyhow::Context;
use anyhow::Result;
use app_test_support::TestAppServer;
use codex_app_server_protocol::RequestId;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use tempfile::TempDir;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tokio::net::TcpListener;
use tokio::time::timeout;

struct Fixture {
    root: TempDir,
    program: PathBuf,
    workspace: PathBuf,
    home: PathBuf,
    marker: PathBuf,
    path: OsString,
}

impl Fixture {
    fn new() -> Result<Self> {
        let root = TempDir::new()?;
        // Cargo-built paths deliberately ignore npm provenance. Launch outside
        // target/ so this fixture also exercises the packaged-install checks.
        let program = root
            .path()
            .join(format!("codex{}", std::env::consts::EXE_SUFFIX));
        let source = codex_utils_cargo_bin::cargo_bin("codex")?;
        if std::fs::hard_link(&source, &program).is_err() {
            std::fs::copy(&source, &program)?;
        }
        let workspace = root.path().join("workspace");
        let bin = workspace.join("node_modules/.bin");
        let home = root.path().join("home");
        let marker = root.path().join("helper-ran");
        std::fs::create_dir_all(&bin)?;
        std::fs::create_dir_all(workspace.join(".git"))?;
        std::fs::create_dir(&home)?;
        std::fs::write(
            home.join("config.toml"),
            r#"
cli_auth_credentials_store = "file"
check_for_update_on_startup = false
model_provider = "local"
[analytics]
enabled = false
[model_providers.local]
name = "local test"
base_url = "http://127.0.0.1:9/v1"
wire_api = "responses"
"#,
        )?;
        for name in [
            "zellij", "tmux", "which", "where", "npm", "rg", "git", "curl", "cmd.exe",
        ] {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let executable = bin.join(name);
                std::fs::write(
                    &executable,
                    "#!/bin/sh\nprintf 'helper ran\\n' >> \"$CODEX_TEST_HELPER_MARKER\"\nexit 0\n",
                )?;
                std::fs::set_permissions(
                    executable,
                    std::fs::Permissions::from_mode(/*mode*/ 0o755),
                )?;
            }
            #[cfg(windows)]
            std::fs::write(
                bin.join(format!("{name}.cmd")),
                "@echo helper ran>>\"%CODEX_TEST_HELPER_MARKER%\"\r\n@exit /b 0\r\n",
            )?;
        }
        // Windows selects rg.exe explicitly, so rg.cmd cannot satisfy discovery.
        // An invalid image still counts as found: doctor must not execute it.
        #[cfg(windows)]
        std::fs::write(bin.join("rg.exe"), "not an executable image")?;
        let path = std::env::join_paths(std::iter::once(bin).chain(std::env::split_paths(
            &std::env::var_os("PATH").unwrap_or_default(),
        )))?;
        Ok(Self {
            root,
            program,
            workspace,
            home,
            marker,
            path,
        })
    }

    fn command(&self) -> Result<assert_cmd::Command> {
        let mut command = assert_cmd::Command::new(&self.program);
        command
            .current_dir(&self.workspace)
            .env("CODEX_HOME", &self.home)
            .env("HOME", self.root.path())
            .env("PATH", &self.path)
            .env("CODEX_TEST_HELPER_MARKER", &self.marker)
            .env("CODEX_MANAGED_BY_NPM", "1")
            .env(
                "CODEX_MANAGED_PACKAGE_ROOT",
                self.workspace.join("node_modules/@openai/codex"),
            )
            .env(
                "CODEX_APP_SERVER_MANAGED_CONFIG_PATH",
                self.home.join("managed_config.toml"),
            )
            .env("HTTPS_PROXY", "http://127.0.0.1:9")
            .env("HTTP_PROXY", "http://127.0.0.1:9")
            .env("ALL_PROXY", "http://127.0.0.1:9")
            .env("https_proxy", "http://127.0.0.1:9")
            .env("http_proxy", "http://127.0.0.1:9")
            .env("all_proxy", "http://127.0.0.1:9")
            .env("NO_PROXY", "127.0.0.1,localhost")
            .env("no_proxy", "127.0.0.1,localhost")
            .env_remove("TMUX")
            .env_remove("TMUX_PANE")
            .env_remove("ZELLIJ_VERSION")
            .env_remove("ZELLIJ_SESSION_NAME")
            .env_remove("TERM_PROGRAM")
            .env("TERM", "dumb")
            .env("ZELLIJ", "0")
            .timeout(Duration::from_secs(/*secs*/ 45));
        Ok(command)
    }
}

#[test]
fn startup_and_doctor_do_not_execute_path_helpers() -> Result<()> {
    let fixture = Fixture::new()?;
    // Non-TTY dumb-terminal startup exits immediately after terminal detection.
    fixture.command()?.assert().failure();
    assert!(!fixture.marker.exists(), "startup executed a PATH helper");

    for args in [
        vec!["doctor", "--json"],
        vec!["doctor", "--json", "--feedback"],
    ] {
        let mut command = fixture.command()?;
        command
            .args(args)
            .env("TMUX", "test-tmux")
            .env("TERM_PROGRAM", "tmux");
        let output = command.output()?;
        let report: Value = serde_json::from_slice(&output.stdout)?;
        assert_eq!(
            report["checks"]["installation"]["details"]["managed by npm"],
            "true"
        );
        assert_eq!(report["checks"]["runtime.search"]["status"], "ok");
        assert_eq!(
            report["checks"]["runtime.search"]["details"]["search command path"],
            fixture
                .workspace
                .join("node_modules/.bin")
                .join(format!("rg{}", std::env::consts::EXE_SUFFIX))
                .display()
                .to_string()
        );
        assert_eq!(
            report["checks"]["git.environment"]["summary"],
            "git executable found; execution not verified"
        );
        assert!(!fixture.marker.exists(), "doctor executed a PATH helper");
    }
    Ok(())
}

#[test]
fn non_interactive_dumb_terminal_preserves_other_doctor_failures() -> Result<()> {
    let fixture = Fixture::new()?;
    let output = fixture
        .command()?
        .args(["doctor", "--json"])
        // Explicit identity takes precedence over inherited terminal-specific variables.
        .env("TERM_PROGRAM", "dumb")
        .env_remove("TERMINFO")
        .env_remove("TERMINFO_DIRS")
        .output()?;
    // The fixture's provider is unreachable, regardless of terminal capabilities.
    assert_eq!(output.status.code(), Some(1));
    let report: Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(report["checks"]["terminal.env"]["status"], "warning");
    assert_eq!(report["overallStatus"], "fail");
    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn interactive_tmux_startup_does_not_execute_workspace_helpers() -> Result<()> {
    let fixture = Fixture::new()?;
    let command = fixture.command()?;
    let mut env: std::collections::HashMap<String, String> = std::env::vars().collect();
    for (key, value) in command.get_envs() {
        let key = key.to_string_lossy().into_owned();
        if let Some(value) = value {
            env.insert(key, value.to_string_lossy().into_owned());
        } else {
            env.remove(&key);
        }
    }
    env.insert("TERM".to_string(), "xterm-256color".to_string());
    env.insert("TERM_PROGRAM".to_string(), "tmux".to_string());
    env.insert("TMUX".to_string(), "test-tmux".to_string());
    env.insert(
        "CODEX_TUI_DISABLE_KEYBOARD_ENHANCEMENT".to_string(),
        "0".to_string(),
    );
    env.insert(
        "GHOSTTY_RESOURCES_DIR".to_string(),
        "/test/ghostty".to_string(),
    );
    let spawned = codex_utils_pty::spawn_pty_process(
        fixture.program.to_str().unwrap(),
        &[],
        &fixture.workspace,
        &env,
        /*arg0*/ &None,
        codex_utils_pty::TerminalSize {
            rows: 40,
            cols: 120,
        },
        &[],
    )
    .await?;
    let session = spawned.session;
    let mut output_rx = spawned.stdout_rx;
    let writer = session.writer_sender();
    let mut output = String::new();
    let ansi = regex_lite::Regex::new(r"\x1b\[[0-?]*[ -/]*[@-~]")?;
    let ready = timeout(Duration::from_secs(/*secs*/ 45), async {
        while let Some(bytes) = output_rx.recv().await {
            let chunk = String::from_utf8_lossy(&bytes);
            output.push_str(&chunk);
            if chunk.contains("\x1b[6n") {
                writer.send(b"\x1b[1;1R".to_vec()).await?;
            }
            if chunk.contains("\x1b[c") {
                writer.send(b"\x1b[?1;2c".to_vec()).await?;
            }
            // Ratatui can position over spaces instead of writing them.
            let text: String = ansi
                .replace_all(&output, "")
                .chars()
                .filter(|character| !character.is_whitespace())
                .collect();
            if text.contains("Doyoutrustthecontentsofthisdirectory?") {
                return Ok::<_, anyhow::Error>(());
            }
        }
        anyhow::bail!("TUI exited before trust prompt")
    })
    .await;
    ready.map_err(|_| {
        anyhow::anyhow!(
            "trust prompt timed out: {}",
            output.chars().take(/*n*/ 4000).collect::<String>()
        )
    })??;
    // The trust screen discards pending input after its first draw. Retry the quit key
    // until that drain has finished, and keep consuming output during normal shutdown.
    let mut quit_retry = tokio::time::interval(Duration::from_millis(/*millis*/ 250));
    let mut exit_rx = spawned.exit_rx;
    timeout(Duration::from_secs(/*secs*/ 10), async {
        loop {
            tokio::select! {
                result = &mut exit_rx => return result,
                _ = quit_retry.tick() => {
                    let _ = writer.send(b"2".to_vec()).await;
                }
                Some(_) = output_rx.recv() => {}
            }
        }
    })
    .await
    .with_context(|| {
        format!(
            "TUI did not exit after declining directory trust: {}",
            output.chars().take(/*n*/ 4000).collect::<String>()
        )
    })??;
    assert!(
        output.contains("\x1b[>5u"),
        "expected safe Ghostty/tmux keyboard flags: {output}"
    );
    assert!(
        !fixture.marker.exists(),
        "interactive startup executed a workspace helper"
    );
    Ok(())
}

#[tokio::test]
async fn feedback_with_logs_does_not_execute_path_helpers() -> Result<()> {
    let fixture = Fixture::new()?;
    let proxy = TcpListener::bind("127.0.0.1:0").await?;
    let proxy_uri = format!("http://{}", proxy.local_addr()?);
    // Reject all outbound traffic locally, including the final feedback upload.
    let proxy_task = tokio::spawn(async move {
        while let Ok((stream, _)) = proxy.accept().await {
            let mut stream = BufReader::new(stream);
            let mut request = String::new();
            if stream.read_line(&mut request).await.is_ok() {
                let _ = stream.get_mut().write_all(b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await;
            }
        }
    });
    let path = fixture.path.to_string_lossy();
    let marker = fixture.marker.to_string_lossy();
    let home = fixture.root.path().to_string_lossy();
    let package_root = fixture.workspace.join("node_modules/@openai/codex");
    let package_root = package_root.to_string_lossy();
    let mut app_server = TestAppServer::builder()
        .with_program(&fixture.program)
        .with_codex_home(&fixture.home)
        // The CLI does not accept the standalone app-server's test-only flag.
        // Disable plugins through real config; their safety test joins a full sync.
        .with_plugin_startup_tasks()
        .with_args(&["-c", "features.plugins=false", "app-server"])
        .with_env_overrides(&[
            ("PATH", Some(path.as_ref())),
            ("HOME", Some(home.as_ref())),
            ("CODEX_TEST_HELPER_MARKER", Some(marker.as_ref())),
            ("CODEX_MANAGED_BY_NPM", Some("1")),
            ("CODEX_MANAGED_PACKAGE_ROOT", Some(package_root.as_ref())),
            ("ZELLIJ", Some("0")),
            ("ZELLIJ_VERSION", None),
            ("TMUX", None),
            ("TMUX_PANE", None),
            ("HTTP_PROXY", Some(&proxy_uri)),
            ("HTTPS_PROXY", Some(&proxy_uri)),
            ("ALL_PROXY", Some(&proxy_uri)),
            ("http_proxy", Some(&proxy_uri)),
            ("https_proxy", Some(&proxy_uri)),
            ("all_proxy", Some(&proxy_uri)),
            ("NO_PROXY", Some("127.0.0.1,localhost")),
            ("no_proxy", Some("127.0.0.1,localhost")),
        ])
        .build_initialized_with_timeout(Duration::from_secs(/*secs*/ 30))
        .await?;
    let request_id = app_server
        .send_raw_request(
            "feedback/upload",
            Some(json!({
                "classification": "bug", "includeLogs": true
            })),
        )
        .await?;
    let error = timeout(
        Duration::from_secs(/*secs*/ 45),
        app_server.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;
    assert!(error.error.message.contains("failed to upload feedback"));
    assert!(!fixture.marker.exists(), "feedback executed a PATH helper");
    timeout(
        Duration::from_secs(/*secs*/ 10),
        app_server.shutdown_gracefully(),
    )
    .await??;
    proxy_task.abort();
    Ok(())
}
