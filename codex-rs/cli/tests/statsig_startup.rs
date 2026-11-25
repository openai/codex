//! Checks which endpoint the shipped harness uses to initialize Statsig.

use anyhow::Result;
use app_test_support::ChatGptAuthFixture;
use app_test_support::mount_workspace_routing;
use app_test_support::write_chatgpt_auth;
use codex_config::types::AuthCredentialsStoreMode;
use codex_utils_cargo_bin::cargo_bin;
use codex_utils_pty::ChildFds;
use codex_utils_pty::TerminalSize;
use codex_utils_pty::spawn_pipe_process;
use codex_utils_pty::spawn_pty_process;
use core_test_support::runtime_auth::with_harness_proxy;
use serde_json::json;
use std::time::Duration;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::body_partial_json;
use wiremock::matchers::header;
use wiremock::matchers::header_regex;
use wiremock::matchers::method;
use wiremock::matchers::path;

#[tokio::test]
async fn harness_startup_requests_statsig_evaluations() -> Result<()> {
    if option_env!("CODEX_BUNDLED_STATSIG_CLIENT_KEY").is_none_or(str::is_empty) {
        eprintln!(
            "Skipping Statsig launch coverage. Set CODEX_BUNDLED_STATSIG_CLIENT_KEY at build time."
        );
        return Ok(());
    }
    let binary = cargo_bin("codex")?;
    let server = MockServer::start().await;
    mount_workspace_routing(&server).await;
    Mock::given(method("POST"))
        .and(path("/agent/register"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "runtime_agent_instance_id": "fixture-runtime", "agent_host_id": "fixture-host",
            "actor": {"kind": "chatgpt", "user_id": "fixture-user", "account_id": "fixture-account"},
        })))
        .mount(&server)
        .await;
    let registry = server.uri();
    Mock::given(method("POST"))
        .and(path("/cloud/environment/fixture/register"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "environment_id": "fixture",
            "url": format!("ws://{}/relay?role=environment", server.address()),
            "security_profile": "noise_hybrid_ik_v1",
            "executor_registration_id": "fixture",
        })))
        .mount(&server)
        .await;

    with_harness_proxy(&server, /*realtime_target*/ None, async |mut command| {
        for key in [
            "CODEX_ACCESS_TOKEN",
            "CODEX_API_KEY",
            "CODEX_EXEC_SERVER_URL",
            "CODEX_SQLITE_HOME",
            "OPENAI_API_KEY",
            "OPENAI_FEDERATION_RULE_ID",
            "OPENAI_IDENTITY_TOKEN_FILE",
            "OPENAI_WORKLOAD_IDENTITY_CONTEXT",
        ] {
            command.env_remove(key);
        }
        command.env("CODEX_INTERNAL_APP_SERVER_REMOTE_CONTROL_DISABLED", "1");
        for (mode, signed_in) in [
            ("app-server", true),
            ("app-server-runtime", true),
            #[cfg(unix)]
            ("daemon", true),
            ("exec", true),
            ("tui", true),
            #[cfg(unix)]
            ("tui-daemon", true),
            ("sandbox", true),
            ("sandbox", false),
            ("exec-server", true),
            ("exec-server", false),
            ("remote", true),
        ] {
            let tui = matches!(mode, "tui" | "tui-daemon");
            // Keep the managed daemon's Unix socket path short.
            #[cfg(unix)]
            let home = tempfile::tempdir_in("/tmp")?;
            #[cfg(not(unix))]
            let home = tempfile::tempdir()?;
            let cwd = home.path().canonicalize()?;
            let statsig_config = if mode == "app-server-runtime" {
                "features.runtime_agent_instances = { enabled = true, mode = 'assertion_only' }\n"
            } else if mode == "tui-daemon" {
                // CLI feature flags are checked against the daemon's settings.
                "features.statsig_gates = true\n"
            } else {
                ""
            };
            std::fs::write(
                cwd.join("config.toml"),
                format!(
                    "{statsig_config}cli_auth_credentials_store = 'file'\ncheck_for_update_on_startup = false\n[analytics]\nenabled = false\n[projects.{}]\ntrust_level = 'trusted'\n",
                    toml::Value::String(cwd.to_string_lossy().into_owned()),
                ),
            )?;
            if signed_in {
                write_chatgpt_auth(
                    &cwd,
                    ChatGptAuthFixture::new("fixture-token")
                        .account_id("fixture-account")
                        .chatgpt_account_id("fixture-account")
                        .chatgpt_user_id("fixture-user")
                        .plan_type("plus"),
                    AuthCredentialsStoreMode::File,
                )?;
            }
            let listen = format!("unix://{}/app-server.sock", cwd.display());
            let args = match mode {
                "app-server" | "app-server-runtime" => vec!["app-server"],
                "daemon" => vec!["app-server", "--managed-daemon", "--listen", &listen],
                "exec" => vec!["exec", "--skip-git-repo-check", "fixture prompt"],
                "tui" => vec!["--no-daemon", "--no-alt-screen"],
                "tui-daemon" => vec!["--remote", "unix://", "--no-alt-screen"],
                "sandbox" => {
                    // Keep managed requirements enabled without depending on a host sandbox.
                    let mut args = vec!["-c", "default_permissions=':danger-full-access'", "sandbox", "--"];
                    if cfg!(windows) {
                        args.extend(["powershell.exe", "-Command", "Start-Sleep -Seconds 20"]);
                    } else {
                        args.extend(["sleep", "20"]);
                    }
                    args
                }
                "exec-server" => vec!["exec-server", "--listen", "ws://127.0.0.1:0"],
                "remote" => vec!["exec-server", "--remote", &registry, "--environment-id", "fixture"],
                _ => unreachable!("unknown launch mode {mode}"),
            };
            let flags: &[&str] = if mode == "tui-daemon" { &[] } else { &["--enable", "statsig_gates"] };
            let args: Vec<_> = flags.iter().copied().chain(args).map(str::to_owned).collect();
            let (host, endpoint) = if signed_in && mode != "remote" {
                ("chatgpt.com", "/backend-api/wham/statsig/bootstrap")
            } else {
                ("ab.chatgpt.com", "/v1/initialize")
            };
            let mut request = Mock::given(method("POST"))
                .and(header("host", host))
                .and(path(endpoint));
            if mode == "app-server-runtime" {
                request = request.and(header_regex("authorization", "^AgentAssertion "));
            } else if mode == "tui-daemon" {
                request = request.and(body_partial_json(json!({"window_type": "tui"})));
            }
            let request = request
                // The SDK test covers handling the response. Reject here to avoid a retry.
                .respond_with(ResponseTemplate::new(400))
                .expect(1)
                .named(format!("{mode} startup"))
                .mount_as_scoped(&server)
                .await;

            command.env("CODEX_HOME", &cwd);
            if tui {
                command.env("TERM", "xterm-256color");
            }
            let env = command
                .as_std()
                .get_envs()
                .filter_map(|(key, value)| Some((key.to_str()?.to_owned(), value?.to_str()?.to_owned())))
                .collect();
            let daemon = if mode == "tui-daemon" {
                let daemon_args = ["--disable", "statsig_gates", "app-server", "--listen", "unix://"]
                    .map(str::to_owned);
                let daemon = spawn_pipe_process(&binary, &daemon_args, &cwd, &env, /*arg0*/ &None, &[]).await?;
                let socket = codex_app_server::app_server_control_socket_path(&cwd)?;
                tokio::time::timeout(Duration::from_secs(30), async {
                    while !socket.as_path().try_exists()? {
                        tokio::time::sleep(Duration::from_millis(10)).await;
                    }
                    Ok::<_, anyhow::Error>(())
                }).await??;
                Some(daemon)
            } else {
                None
            };
            let mut child = if tui {
                spawn_pty_process(
                    &binary, &args, &cwd, &env, /*arg0*/ &None,
                    TerminalSize::default(), ChildFds::Inherited(&[]),
                ).await?
            } else {
                spawn_pipe_process(&binary, &args, &cwd, &env, /*arg0*/ &None, &[]).await?
            };
            if mode == "exec" {
                child.session.close_stdin();
            }
            let mut output = Vec::new();
            let wait = async {
                loop {
                    tokio::select! {
                        biased;
                        () = request.wait_until_satisfied() => break,
                        status = &mut child.exit_rx => panic!("{mode} exited with {status:?} before POST {endpoint}\n{}", String::from_utf8_lossy(&output)),
                        Some(chunk) = child.stdout_rx.recv() => output.extend(chunk),
                        Some(chunk) = child.stderr_rx.recv() => output.extend(chunk),
                    }
                }
            };
            assert!(
                tokio::time::timeout(Duration::from_secs(15), wait).await.is_ok(),
                "{mode} did not POST {endpoint}\n{}",
                String::from_utf8_lossy(&output),
            );
            child.session.terminate();
            let _ = child.exit_rx.await;
            if let Some(daemon) = daemon {
                daemon.session.signal(codex_utils_pty::ProcessSignal::Interrupt)?;
                tokio::time::timeout(Duration::from_secs(15), daemon.exit_rx).await??;
            }
        }
        Ok(())
    })
    .await
}
