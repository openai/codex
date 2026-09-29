//! Resume and fork picker and --last provider defaults through the real CLI and app server.

use super::focus_palette::PtyCodex;
use super::focus_palette::write_test_config;
use anyhow::Result;
use anyhow::ensure;
use app_test_support::create_fake_rollout;
use std::process::Stdio;
use std::time::Duration;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn history_lookup_uses_server_provider_with_local_and_embedded_servers() -> Result<()> {
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
            let mut terminal = PtyCodex::start_cli(&cwd, home, &args)?;
            let (expected, excluded) = if embedded || explicit {
                ("Client provider history", "Server provider history")
            } else {
                ("Server provider history", "Client provider history")
            };
            terminal.wait_for_screen(expected)?;
            ensure!(
                !terminal.screen_contains(excluded),
                "{action}: unexpected provider history"
            );
            drop(terminal);
            daemon.kill().await?;
            daemon.wait().await?;
        }
    }
    Ok(())
}
