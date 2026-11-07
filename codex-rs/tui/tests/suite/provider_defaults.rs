//! Resume and fork picker and --last provider defaults through the real CLI and app server.

use super::focus_palette::PtyCodex;
use super::focus_palette::write_test_config;
use anyhow::Context;
use anyhow::Result;
use anyhow::ensure;
use app_test_support::create_fake_rollout;
use std::process::Stdio;
use std::time::Duration;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn resume_lists_all_providers_and_fork_keeps_provider_filter() -> Result<()> {
    let codex = codex_utils_cargo_bin::cargo_bin("codex")?;
    let cwd = codex_utils_cargo_bin::repo_root()?;
    for action in ["resume", "fork"] {
        for (embedded, last, explicit) in [
            (false, false, false),
            (true, false, false),
            (false, true, false),
            (true, true, false),
            (false, true, true),
            (false, false, true),
        ] {
            let home = tempfile::tempdir_in("/tmp")?;
            write_test_config(home.path(), &cwd)?;
            let config_path = home.path().join("config.toml");
            let config = std::fs::read_to_string(&config_path)?;
            std::fs::write(
                config_path,
                format!(
                    "{config}\n[model_providers.server-provider]\nname = 'Server provider'\nbase_url = 'http://127.0.0.1:9/v1'\nwire_api = 'responses'\nrequires_openai_auth = false\n[projects.'/']\ntrust_level = 'trusted'\n"
                ),
            )?;
            for (hour, provider, preview) in [
                ("10", "openai", "Client provider history"),
                ("11", "server-provider", "Server provider history"),
            ] {
                create_fake_rollout(
                    home.path(),
                    &format!("2025-01-02T{hour}-00-00"),
                    &format!("2025-01-02T{hour}:00:00Z"),
                    preview,
                    Some(provider),
                    /*git_info*/ None,
                )?;
            }
            let mut daemon = tokio::process::Command::new(&codex)
                .args([
                    "app-server",
                    "--listen",
                    "unix://",
                    "-c",
                    "model_provider=\"server-provider\"",
                ])
                .env("CODEX_HOME", home.path())
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::inherit())
                .kill_on_drop(true)
                .spawn()?;
            let socket = codex_app_server_client::app_server_control_socket_path(home.path())?;
            tokio::time::timeout(Duration::from_secs(/*secs*/ 30), async {
                loop {
                    if tokio::net::UnixStream::connect(socket.as_path())
                        .await
                        .is_ok()
                    {
                        return Ok::<(), anyhow::Error>(());
                    }
                    ensure!(
                        daemon.try_wait()?.is_none(),
                        "app server exited before opening its socket"
                    );
                    tokio::time::sleep(Duration::from_millis(/*millis*/ 20)).await;
                }
            })
            .await??;
            let remote_endpoint = format!("unix://{}", socket.as_path().display());
            let mut args = vec![action, "--all"];
            if embedded {
                args.push("--no-daemon");
            }
            if last {
                args.push("--last");
            }
            if explicit {
                args.extend([
                    "--remote",
                    remote_endpoint.as_str(),
                    "-c",
                    "model_provider=\"openai\"",
                ]);
            }
            let case = format!("{action}: embedded={embedded}, last={last}, explicit={explicit}");
            let mut terminal = PtyCodex::start_cli(&cwd, home, &args)?;
            if action == "resume" && last && explicit {
                // --last finds the newest thread across providers. An explicit override
                // cannot switch that thread from server-provider to openai.
                terminal
                    .wait_for_screen("Cannot change this thread's model provider")
                    .with_context(|| format!("{case}; screen:\n{}", terminal.screen_contents()))?;
            } else if action == "resume" {
                terminal
                    .wait_for_screen("Server provider history")
                    .with_context(|| format!("{case}; screen:\n{}", terminal.screen_contents()))?;
                if !last {
                    terminal
                        .wait_for_screen("Client provider history")
                        .with_context(|| {
                            format!("{case}; screen:\n{}", terminal.screen_contents())
                        })?;
                }
            } else {
                let (expected, excluded) = if embedded || explicit {
                    ("Client provider history", "Server provider history")
                } else {
                    ("Server provider history", "Client provider history")
                };
                terminal
                    .wait_for_screen(expected)
                    .with_context(|| format!("{case}; screen:\n{}", terminal.screen_contents()))?;
                ensure!(
                    !terminal.screen_contains(excluded),
                    "{case}: unexpected provider history"
                );
            }
            drop(terminal);
            daemon.kill().await?;
            daemon.wait().await?;
        }
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn selected_gateway_authenticates_before_its_first_chat() -> Result<()> {
    use core_test_support::responses;

    let server = responses::start_mock_server().await;
    let reply = responses::mount_sse_once_match(
        &server,
        wiremock::matchers::body_string_contains(r#"\"thread_source\":\"user\""#),
        responses::sse(vec![
            responses::ev_assistant_message("msg", "gateway handoff verified"),
            responses::ev_completed("resp"),
        ]),
    )
    .await;
    let workspace = tempfile::tempdir()?;
    let home = tempfile::tempdir()?;
    let cwd = workspace.path().canonicalize()?;
    let profile = home.path().join("work.config.toml");
    let auth = home.path().join("auth.json");
    let url = server.uri();
    let trusted = toml::Value::String(cwd.display().to_string());
    std::fs::write(
        &profile,
        format!(
            r#"
model_provider = "openai"
model = "gpt-5.1"
forced_login_method = "api"
cli_auth_credentials_store = "file"
suppress_unstable_features_warning = true
[features]
gateway_setup_wizard = true
bedrock_setup_wizard = false
gcp_setup_wizard = false
api_key_model_discovery = false
[projects.{trusted}]
trust_level = "trusted"
[model_providers.office]
name = "Office gateway"
base_url = "{url}/v1"
wire_api = "responses"
supports_websockets = false
requires_openai_auth = true
"#
        ),
    )?;
    let mut terminal = PtyCodex::start(
        &cwd,
        home,
        &["--profile", "work", "--no-daemon", "verify the gateway"],
    )?;
    terminal
        .wait_for_screen("Sign in another way")
        .with_context(|| terminal.screen_contents())?;
    // Startup drains input after its first draw. Retry the menu shortcut until it is accepted.
    let deadline = std::time::Instant::now() + Duration::from_secs(/*secs*/ 30);
    while !terminal.screen_contains("Use a custom provider or gateway") {
        ensure!(
            std::time::Instant::now() < deadline,
            "gateway option missing: {}",
            terminal.screen_contents()
        );
        terminal.write_input(b"2")?;
        terminal.read_output(Duration::from_millis(/*millis*/ 50))?;
    }
    // Arrow navigation avoids carrying the menu shortcut across two screens.
    terminal.write_input(b"\x1b[B\r")?;
    terminal.wait_for_screen("Office gateway")?;
    terminal.write_input(b"\x1b[H\r")?;
    terminal.wait_for_screen("Paste or type your API key")?;
    let deadline = std::time::Instant::now() + Duration::from_secs(/*secs*/ 30);
    while !auth.is_file() && terminal.screen_contains("Paste or type your API key") {
        ensure!(
            std::time::Instant::now() < deadline,
            "API key was not saved: {}",
            terminal.screen_contents()
        );
        terminal.write_input(b"\r")?;
        terminal.read_output(Duration::from_millis(/*millis*/ 50))?;
    }
    terminal.wait_for_screen("gateway handoff verified")?;
    let first_chat = reply
        .requests()
        .into_iter()
        .find(|request| {
            request
                .message_input_texts("user")
                .iter()
                .any(|text| text == "verify the gateway")
        })
        .context("the selected gateway received the first chat")?;
    assert_eq!(
        first_chat.header("authorization").as_deref(),
        Some("Bearer focus-palette-test")
    );
    Ok(())
}
