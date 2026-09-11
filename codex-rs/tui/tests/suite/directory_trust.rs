//! Exercises folder consent against a connected server through the real terminal event loop.

use super::focus_palette::PtyCodex;
use super::focus_palette::write_test_config;
use anyhow::Result;
use codex_app_server_protocol::JSONRPCMessage;
use futures::SinkExt;
use futures::StreamExt;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tokio::net::UnixListener;
use tokio_tungstenite::tungstenite::Message;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn connected_trust_cancellation_and_acceptance_control_task_creation() -> Result<()> {
    for trust_level in [None, Some("untrusted")] {
        let repo_root = codex_utils_cargo_bin::repo_root()?;
        let codex_home = tempfile::tempdir_in("/tmp")?;
        // The server's trust decision must win over the client's trusted-folder setting.
        write_test_config(codex_home.path(), &repo_root)?;
        let config_path = codex_home.path().join("config.toml");
        let config = std::fs::read_to_string(&config_path)?;
        std::fs::write(
            config_path,
            format!("{config}\n[tui]\nresume_cwd = \"current\"\n"),
        )?;
        let socket = codex_app_server_client::app_server_control_socket_path(codex_home.path())?;
        std::fs::create_dir_all(socket.parent().unwrap())?;
        let listener = UnixListener::bind(socket.as_path())?;
        let cwd = repo_root.clone();
        let methods = Arc::new(Mutex::new(Vec::new()));
        let requests = Arc::clone(&methods);
        let server = tokio::spawn(async move {
            let mut clients = tokio::task::JoinSet::new();
            let trust_reads = Arc::new(AtomicUsize::new(0));
            loop {
                let (stream, _) = listener
                    .accept()
                    .await
                    .expect("accept fake app-server client");
                let cwd = cwd.clone();
                let methods = Arc::clone(&requests);
                let trust_reads = Arc::clone(&trust_reads);
                clients.spawn(async move {
                    let Ok(mut socket) = tokio_tungstenite::accept_async(stream).await else {
                        return Ok::<_, anyhow::Error>(());
                    };
                    let other_cwd = cwd.join("other-folder");
                    let id = "00000000-0000-0000-0000-000000000001";
                    let thread = json!({
                        "id": id, "sessionId": id, "preview": "Untrusted saved task", "ephemeral": false,
                        "modelProvider": "openai", "createdAt": 1, "updatedAt": 2,
                        "status": if trust_level.is_some() { json!({"type": "active", "activeFlags": []}) } else { json!({"type": "notLoaded"}) }, "cwd": other_cwd,
                        "cliVersion": "0.0.0", "source": "cli", "turns": []
                    });
                    let mut launch_thread = thread.clone();
                    launch_thread["id"] = json!("00000000-0000-0000-0000-000000000002");
                    launch_thread["sessionId"] = launch_thread["id"].clone();
                    launch_thread["cwd"] = json!(cwd);
                    launch_thread["preview"] = json!("Launch-folder task");
                    launch_thread["updatedAt"] = json!(1);
                    while let Some(Ok(Message::Text(text))) = socket.next().await {
                        let JSONRPCMessage::Request(request) = serde_json::from_str(&text)? else {
                            continue;
                        };
                        methods.lock().unwrap().push(request.method.clone());
                        let result = match request.method.as_str() {
                            "initialize" => json!({"userAgent": "trust-pty"}),
                            "account/read" => {
                                json!({"account": {"type": "apiKey"}, "requiresOpenaiAuth": false})
                            }
                            "config/read" => {
                                if request
                                    .params
                                    .as_ref()
                                    .is_some_and(|params| params["includeLayers"] == true)
                                {
                                    trust_reads.fetch_add(1, Ordering::SeqCst);
                                }
                                json!({"config": {"model": "gpt-5.6-terra", "projects": {
                                    cwd.to_string_lossy(): {"trust_level": if trust_reads.load(Ordering::SeqCst) == 4 { Some("trusted") } else { trust_level }},
                                    other_cwd.to_string_lossy(): {"trust_level": "untrusted"},
                                    cwd.join("moved-folder").to_string_lossy(): {"trust_level": "untrusted"}
                                }}, "origins": {}, "layers": []})
                            }
                            "config/batchWrite" => json!({"status": "ok", "version": "1", "filePath": cwd.join("config.toml"), "overriddenMetadata": null}),
                            "hooks/list" => json!({"data": if trust_level.is_none() && methods.lock().unwrap().iter().any(|method| method == "config/batchWrite") {
                                vec![json!({"cwd": request.params.as_ref().unwrap()["cwds"][0], "hooks": [{
                                    "key": "project:test", "eventName": "sessionStart", "handlerType": "command",
                                    "command": "echo hook", "async": false, "matcher": null, "timeoutSec": 30,
                                    "statusMessage": null, "additionalContextLimit": null,
                                    "sourcePath": cwd.join(".codex/hooks.json"), "source": "project", "pluginId": null,
                                    "displayOrder": 0, "enabled": false, "isManaged": false,
                                    "currentHash": "sha256:test", "trustStatus": "untrusted"
                                }], "warnings": [], "errors": []})]
                            } else { vec![] }}),
                            "configRequirements/read" => json!({"requirements": null}),
                            "model/list" => json!({"data": [], "nextCursor": null}),
                            "thread/list" => {
                                json!({"data": vec![thread.clone(), launch_thread.clone()], "nextCursor": null})
                            }
                            "thread/read" => {
                                let mut thread = if request.params.as_ref().unwrap()["threadId"] == launch_thread["id"] {
                                    launch_thread.clone()
                                } else {
                                    thread.clone()
                                };
                                if thread["id"] == id && trust_reads.load(Ordering::SeqCst) >= 5 {
                                    thread["cwd"] = json!(cwd.join("moved-folder"));
                                }
                                json!({"thread": thread})
                            } ,
                            "skills/list" | "thread/loaded/list" => json!({"data": []}),
                            _ => {
                                socket
                                    .send(Message::Text(
                                        json!({"id": request.id, "error": {
                                            "code": if request.method == "thread/start" { -32000 } else { -32601 }, "message": if request.method == "thread/start" { "Consent accepted" } else { "method not found" }
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
                    }
                    Ok(())
                });
            }
        });
        let mut terminal = PtyCodex::start(
            &repo_root,
            codex_home,
            &["do not submit this launch prompt"],
        )?;
        let prompt = if trust_level.is_some() {
            "Open restricted"
        } else {
            "Trust and continue"
        };
        for (expected, input) in [
            (prompt, b"\x1b".as_slice()),
            ("create task", b"\x1b"),
            ("Launch-folder task", b"\x1b[B\x0e"),
            ("create task", b"new task"),
            ("› new task", b"\r"),
            (prompt, b"\x1b"),
            ("› new task", b"\r"),
            ("You are in", b"\x1b"),
            ("› new task", b"\x1b"),
            ("ctrl+o resume", b"\x0f"),
            ("Resume a previous session", b"\x1b[C"),
            ("Untrusted saved task", b"\r"),
            ("Open existing task", b"\r"),
            ("moved-folder", b"\x1b"),
            ("ctrl+o resume", b"\x0e"),
            ("› new task", b"\r"),
            (prompt, b"\r"),
        ] {
            let is_consent = expected == prompt
                || matches!(
                    expected,
                    "Open existing task" | "moved-folder" | "You are in"
                );
            terminal.wait_for_screen(expected)?;
            if expected == "You are in" {
                assert_eq!(
                    terminal
                        .screen_contents()
                        .lines()
                        .find(|line| line.contains("You are in")),
                    Some(format!("> You are in {}", repo_root.display()).as_str())
                );
            }
            if is_consent {
                terminal.wait_for_screen("Back to Agent Command Center")?;
            }
            // Confirmations must be fresh input after the protected-screen transition.
            tokio::time::sleep(Duration::from_millis(/*millis*/ 200)).await;
            terminal.write_input(input)?;
            terminal.read_output(Duration::from_millis(/*millis*/ 100))?;
        }
        if trust_level.is_none() {
            terminal.wait_for_screen("Hooks need review")?;
            assert!(
                !methods
                    .lock()
                    .unwrap()
                    .iter()
                    .any(|method| method == "thread/start")
            );
            tokio::time::sleep(Duration::from_millis(/*millis*/ 200)).await;
            terminal.write_input(b"3")?;
        }
        terminal.wait_for_screen("Consent accepted")?;
        drop(terminal);
        server.abort();
        let methods = methods.lock().unwrap();
        let mutations: Vec<_> = methods
            .iter()
            .map(String::as_str)
            .filter(|method| {
                matches!(
                    *method,
                    "thread/start"
                        | "thread/resume"
                        | "thread/fork"
                        | "turn/start"
                        | "config/batchWrite"
                )
            })
            .collect();
        assert_eq!(
            mutations,
            if trust_level.is_some() {
                vec!["thread/start"]
            } else {
                vec!["config/batchWrite", "thread/start"]
            }
        );
    }
    Ok(())
}
