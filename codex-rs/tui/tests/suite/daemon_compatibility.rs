//! Exercises daemon compatibility recovery through real TUI startup.

use super::focus_palette::PtyCodex;
use super::focus_palette::write_test_config_with_bundled_models;
use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use anyhow::ensure;
use codex_app_server_protocol::InitializeResponse;
use codex_app_server_protocol::JSONRPCMessage;
use futures::SinkExt;
use futures::StreamExt;
use serde_json::json;
use tokio::net::UnixListener;
use tokio::net::UnixStream;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::Message;

async fn read_version_probe_message(
    probe: &mut WebSocketStream<UnixStream>,
) -> Result<JSONRPCMessage> {
    let frame = probe
        .next()
        .await
        .context("daemon version probe disconnected")??;
    Ok(serde_json::from_str(frame.to_text()?)?)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn incompatible_daemon_can_run_embedded_for_explicit_features() -> Result<()> {
    for scenario in ["explicit enable", "explicit disable"] {
        let cwd = codex_utils_cargo_bin::repo_root()?;
        let home = tempfile::tempdir_in("/tmp")?;
        write_test_config_with_bundled_models(home.path(), &cwd)?;
        let socket = codex_app_server_client::app_server_control_socket_path(home.path())?;
        std::fs::create_dir_all(socket.parent().unwrap())?;
        let listener = UnixListener::bind(socket.as_path())?;
        let initialize = InitializeResponse {
            user_agent: "daemon-test/0.0.0".into(),
            codex_home: home.path().try_into()?,
            platform_family: std::env::consts::FAMILY.into(),
            platform_os: std::env::consts::OS.into(),
        };
        let server = tokio::spawn(async move {
            let mut socket = loop {
                let (stream, _) = listener.accept().await?;
                if let Ok(socket) = tokio_tungstenite::accept_async(stream).await {
                    break socket;
                }
            };
            while let Some(Ok(Message::Text(text))) = socket.next().await {
                let JSONRPCMessage::Request(request) = serde_json::from_str(&text)? else {
                    continue;
                };
                let response = match request.method.as_str() {
                    "initialize" => {
                        json!({"id": request.id, "result": initialize})
                    }
                    "config/read" => {
                        json!({"id": request.id, "result": {"config": {}, "layers": []}})
                    }
                    "configRequirements/read" => {
                        json!({"id": request.id, "result": {"requirements": null}})
                    }
                    "experimentalFeature/list" => json!({"id": request.id, "result": {"data": [{
                        "name": "api_key_model_discovery", "stage": "stable",
                        "displayName": null, "description": null, "announcement": null,
                        "enabled": scenario == "explicit disable", "defaultEnabled": true,
                    }], "nextCursor": null}}),
                    _ => panic!("unexpected request: {}", request.method),
                };
                socket
                    .send(Message::Text(response.to_string().into()))
                    .await?;
            }
            // Recovery checks the daemon version on a new connection before offering a choice.
            let (stream, _) = listener.accept().await?;
            let mut probe = tokio_tungstenite::accept_async(stream).await?;
            let JSONRPCMessage::Request(request) = read_version_probe_message(&mut probe).await?
            else {
                bail!("expected initialize request from the daemon version probe");
            };
            ensure!(
                request.method == "initialize"
                    && request.params.as_ref().is_some_and(
                        |params| params["clientInfo"]["name"] == "codex_app_server_daemon"
                    ),
                "expected the daemon version probe after the compatibility check",
            );
            let response = json!({"id": request.id, "result": initialize});
            probe
                .send(Message::Text(response.to_string().into()))
                .await?;
            let initialized = read_version_probe_message(&mut probe).await?;
            ensure!(
                matches!(initialized, JSONRPCMessage::Notification(n) if n.method == "initialized"),
                "daemon version probe did not acknowledge initialize",
            );
            Ok::<_, anyhow::Error>(listener.into_std()?)
        });
        let args = if scenario == "explicit disable" {
            vec!["-c", "features.api_key_model_discovery=false"]
        } else {
            vec!["-c", "features.api_key_model_discovery=true"]
        };
        let mut terminal = PtyCodex::start(&cwd, home, &args)?;
        terminal.wait_for_screen("Background server has incompatible feature settings")?;
        let listener = tokio::time::timeout(std::time::Duration::from_secs(/*secs*/ 5), server)
            .await
            .context("recovery prompt appeared without a completed daemon version probe")???;
        terminal.write_input(b"1")?;
        terminal.wait_for_startup()?;
        terminal.wait_for_screen("warning")?;
        terminal.write_input(b"\x14")?;
        let (snapshot, warning_end) = match scenario {
            "explicit enable" => (
                "daemon_feature_mismatch",
                "api_key_model_discovery to be enabled.",
            ),
            "explicit disable" => (
                "daemon_override_mismatch",
                "api_key_model_discovery to be disabled.",
            ),
            _ => unreachable!(),
        };
        terminal.wait_for_screen(warning_end)?;
        let screen = terminal.screen_contents();
        let warning = screen
            .lines()
            .find(|line| line.contains("Running without the shared background server"))
            .context("missing warning line")?;
        insta::assert_snapshot!(snapshot, warning);
        terminal.write_input(b"\x14")?;
        terminal.write_input(b"/status")?;
        terminal.wait_for_screen("show current session configuration")?;
        terminal.write_input(b"\r")?;
        terminal.wait_for_screen("Model:")?;
        ensure!(!terminal.screen_contains("unix://"));
        // No one has accepted since the probe. Even a retry that immediately disconnected
        // and fell back to embedded would still be queued on this nonblocking listener.
        match listener.accept() {
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(err) => return Err(err.into()),
            Ok(_) => {
                bail!("TUI reconnected to the daemon after choosing to run without it ({scenario})")
            }
        }
    }
    Ok(())
}
