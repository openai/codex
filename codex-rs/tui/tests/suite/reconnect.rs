//! Drives automatic reconnect through the real binary and terminal event loop.

use super::focus_palette::PtyCodex;
use super::focus_palette::write_test_config;
use anyhow::Result;
use anyhow::ensure;
use codex_app_server_protocol::JSONRPCMessage;
use futures::SinkExt;
use futures::StreamExt;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::time::Duration;
use std::time::Instant;
use tokio::net::UnixListener;
use tokio_tungstenite::tungstenite::Message;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn automatic_reconnect_restores_draft_and_routes_new_notifications() -> Result<()> {
    let repo_root = codex_utils_cargo_bin::repo_root()?;
    // macOS's default temporary directory leaves too little room for the control socket path.
    let codex_home = tempfile::tempdir_in("/tmp")?;
    write_test_config(codex_home.path(), &repo_root)?;
    let socket = codex_app_server_client::app_server_control_socket_path(codex_home.path())?;
    std::fs::create_dir_all(socket.parent().unwrap())?;
    let listener = UnixListener::bind(socket.as_path())?;
    let (disconnect_tx, mut disconnect_rx) = tokio::sync::oneshot::channel();
    let (restore_tx, restore_rx) = tokio::sync::oneshot::channel();
    let server_cwd = repo_root.clone();
    let server = tokio::spawn(async move {
        let mut methods = Vec::new();
        let mut restore_rx = Some(restore_rx);
        let id = "00000000-0000-0000-0000-000000000001";
        let thread = json!({
            "id": id, "sessionId": id, "preview": "", "ephemeral": false,
            "modelProvider": "openai", "createdAt": 1, "updatedAt": 2,
            "status": {"type": "active", "activeFlags": []}, "cwd": server_cwd,
            "cliVersion": "0.0.0", "source": "cli", "turns": [{"id": "running", "items": [], "status": "inProgress", "error": null}]
        });
        for connection in 0..3 {
            let mut socket = loop {
                let (stream, _) = listener.accept().await?;
                // Startup probes the default daemon socket before opening its WebSocket.
                if let Ok(socket) = tokio_tungstenite::accept_async(stream).await {
                    break socket;
                }
            };
            loop {
                let frame = tokio::select! {
                    _ = &mut disconnect_rx, if connection == 0 => {
                        socket.close(/*msg*/ None).await?;
                        break;
                    }
                    frame = socket.next() => frame,
                };
                let Some(Ok(Message::Text(text))) = frame else {
                    break;
                };
                let JSONRPCMessage::Request(request) = serde_json::from_str(&text)? else {
                    continue;
                };
                methods.push(request.method.clone());
                if connection == 1 && request.method == "initialize" {
                    restore_rx.take().unwrap().await?;
                }
                if connection == 1 && request.method == "thread/resume" {
                    socket
                        .send(Message::Text(
                            json!({"id": request.id, "error": {
                                "code": -32600,
                                "message": format!("thread {id} is closing; retry after the thread is closed")
                            }})
                            .to_string()
                            .into(),
                        ))
                        .await?;
                    continue;
                }
                let result = match request.method.as_str() {
                    "initialize" => json!({"userAgent": "reconnect-pty"}),
                    "account/read" => {
                        json!({"account": {"type": "apiKey"}, "requiresOpenaiAuth": false})
                    }
                    "model/list" => json!({"data": [], "nextCursor": null}),
                    "config/read" => {
                        json!({"config": {"model": "gpt-5.6-terra", "model_provider": "openai", "projects": {
                        server_cwd.to_string_lossy(): {"trust_level": "trusted"}
                    }}, "origins": {}, "layers": []})
                    }
                    "configRequirements/read" => json!({"requirements": null}),
                    "thread/start" | "thread/resume" => {
                        json!({"thread": thread, "model": "gpt-5.6-terra", "modelProvider": "openai",
                            "cwd": server_cwd, "approvalPolicy": "never", "approvalsReviewer": "user",
                            "sandbox": {"type": "readOnly"}, "reasoningEffort": null})
                    }
                    "thread/read" => json!({"thread": thread}),
                    "thread/goal/get" => json!({"goal": null}),
                    "skills/list" => json!({"data": []}),
                    _ => {
                        socket
                            .send(Message::Text(
                                json!({"id": request.id, "error": {
                                    "code": -32601, "message": "method not found"
                                }})
                                .to_string()
                                .into(),
                            ))
                            .await?;
                        continue;
                    }
                };
                socket
                    .send(Message::Text(
                        json!({"id": request.id, "result": result})
                            .to_string()
                            .into(),
                    ))
                    .await?;
                if connection == 2 && request.method == "thread/resume" {
                    // Keep the recovered turn running: its output must appear without waiting
                    // for turn/completed or rebuilding the transcript.
                    socket
                        .send(Message::Text(
                            json!({
                                "method": "item/agentMessage/delta", "params": {
                                    "threadId": id, "turnId": "running", "itemId": "live-item",
                                    "delta": "fresh-notification-after-reconnect\n"
                                }
                            })
                            .to_string()
                            .into(),
                        ))
                        .await?;
                }
            }
        }
        Ok::<_, anyhow::Error>(methods)
    });
    let mut terminal = PtyCodex::start(&repo_root, codex_home, &[])?;
    terminal.wait_for_startup()?;
    let mut disconnect_tx = Some(disconnect_tx);
    let mut restore_tx = Some(restore_tx);
    for expected in [
        "gpt-5.6-terra",
        "preserved-draft",
        "Reconnecting",
        "preserved-draft!",
        "fresh-notification-after-reconnect",
    ] {
        let deadline = Instant::now() + Duration::from_secs(/*secs*/ 30);
        while !terminal.screen_contains(expected) && Instant::now() < deadline {
            terminal.read_output(Duration::from_millis(/*millis*/ 20))?;
        }
        ensure!(
            terminal.screen_contains(expected),
            "missing {expected}; screen:\n{}",
            terminal.screen_contents()
        );
        match expected {
            "gpt-5.6-terra" => terminal.write_input(b"preserved-draft")?,
            "preserved-draft" => {
                disconnect_tx.take().unwrap().send(()).unwrap();
            }
            "Reconnecting" => terminal.write_input(b"!")?,
            "preserved-draft!" => {
                restore_tx.take().unwrap().send(()).unwrap();
            }
            _ => {}
        }
    }
    // A PTY read can expose the history write before the same terminal update redraws the
    // composer. Wait for both pieces in the same parsed screen before checking recovery.
    let deadline = Instant::now() + Duration::from_secs(/*secs*/ 30);
    while !(terminal.screen_contains("preserved-draft!")
        && terminal.screen_contains("fresh-notification-after-reconnect"))
        && Instant::now() < deadline
    {
        terminal.read_output(Duration::from_millis(/*millis*/ 20))?;
    }
    ensure!(
        terminal.screen_contains("preserved-draft!")
            && terminal.screen_contains("fresh-notification-after-reconnect"),
        "draft and notification did not remain visible after recovery; screen:\n{}",
        terminal.screen_contents()
    );
    drop(terminal);
    let methods = tokio::time::timeout(Duration::from_secs(/*secs*/ 5), server).await???;
    assert_eq!(
        methods
            .iter()
            .filter(|method| *method == "thread/resume")
            .count(),
        2
    );
    assert!(!methods.iter().any(|method| method == "turn/start"));
    Ok(())
}
