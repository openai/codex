use anyhow::Context;
use clap::Parser;
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use url::Url;

#[derive(Debug, Parser)]
pub struct CloudLinkCli {
    /// The URL of the Span server (e.g., wss://span.example.com)
    #[arg(long, default_value = "ws://localhost:3000")]
    pub server_url: String,

    /// The client ID to identify this machine
    #[arg(long, default_value = "codex-cli")]
    pub client_id: String,

    /// The session ID to link to. If provided, messages will include this ID.
    #[arg(long, env = "CODEX_SESSION_ID")]
    pub session_id: Option<String>,

    /// Authentication token
    #[arg(long, env = "CODEX_CLOUD_TOKEN")]
    pub token: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum ClientEnvelope {
    Hello {
        #[serde(rename = "clientId")]
        client_id: String,
        ts: String,
        payload: serde_json::Value,
    },
    Log {
        #[serde(rename = "clientId")]
        client_id: String,
        #[serde(rename = "sessionId")]
        session_id: Option<String>,
        ts: String,
        payload: LogPayload,
    },
    Status {
        #[serde(rename = "clientId")]
        client_id: String,
        #[serde(rename = "sessionId")]
        session_id: Option<String>,
        ts: String,
        payload: StatusPayload,
    },
    Pong {
        #[serde(rename = "clientId")]
        client_id: String,
        ts: String,
    },
}

#[derive(Debug, Serialize, Deserialize)]
struct LogPayload {
    stream: String, // "stdout" | "stderr"
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    agent: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct StatusPayload {
    state: String, // "running" | "exited" | "error"
    #[serde(skip_serializing_if = "Option::is_none")]
    agent: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum ServerMessage {
    Welcome {
        message: String,
    },
    Control {
        action: String,
        #[serde(rename = "agentId")]
        agent_id: Option<String>,
        payload: Option<ControlPayload>,
    },
    Ping,
    Ack {
        #[allow(dead_code)]
        id: Option<u64>,
    },
}

#[derive(Debug, Deserialize)]
struct ControlPayload {
    prompt: Option<String>,
    args: Option<Vec<String>>,
}

pub async fn run_main(args: CloudLinkCli) -> anyhow::Result<()> {
    let session_id = args.session_id.clone().unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    
    let codex_home = codex_core::config::find_codex_home()?;
    let stored_token = codex_core::auth::load_auth_dot_json(
        &codex_home,
        codex_core::auth::AuthCredentialsStoreMode::Auto
    )?.and_then(|a| a.cloud_token);

    let refresh_token = match args.token.or(stored_token) {
        Some(token) => token,
        None => anyhow::bail!("No cloud token found. Please run 'bracket pair <CODE>' first."),
    };

    // 1. Exchange refresh token for a session token
    let http_client = reqwest::Client::new();
    let server_url_base = args.server_url.replace("ws://", "http://").replace("wss://", "https://");
    let server_url_base = server_url_base.trim_end_matches('/');
    let session_url = format!("{server_url_base}/api/clients/session");
    
    println!("Authenticating with Span...");
    let res = http_client.post(&session_url)
        .header("Authorization", format!("Bearer {refresh_token}"))
        .send()
        .await?;

    if !res.status().is_success() {
        anyhow::bail!("Authentication failed: {}", res.status());
    }

    let data: serde_json::Value = res.json().await?;
    let session_token = data["sessionToken"].as_str().context("No sessionToken in response")?;

    // 2. Connect to WebSocket with the session token
    let mut url = Url::parse(&args.server_url)?;
    if !url.path().ends_with("/api/ws") {
        url.set_path(&format!("{}/api/ws", url.path().trim_end_matches('/')));
    }
    url.query_pairs_mut().append_pair("token", session_token);

    println!("Connecting to {url}...");
    println!("Session ID: {session_id}");

    let (ws_stream, _) = connect_async(url.to_string()).await?;
    println!("Connected!");

    let (mut write, mut read) = ws_stream.split();

    // Send Hello
    let hello = ClientEnvelope::Hello {
        client_id: args.client_id.clone(),
        ts: chrono::Utc::now().to_rfc3339(),
        payload: serde_json::json!({
            "device": hostname::get().unwrap_or_default().to_string_lossy(),
            "platform": std::env::consts::OS,
        }),
    };
    write
        .send(Message::Text(serde_json::to_string(&hello)?.into()))
        .await?;

    // Always send an initial status to register the session in Span
    let status = ClientEnvelope::Status {
        client_id: args.client_id.clone(),
        session_id: Some(session_id.clone()),
        ts: chrono::Utc::now().to_rfc3339(),
        payload: StatusPayload {
            state: "running".to_string(),
            agent: Some("bracket".to_string()),
        },
    };
    write
        .send(Message::Text(serde_json::to_string(&status)?.into()))
        .await?;
    println!("Registered session in Span");

    let (tx, mut rx) = tokio::sync::mpsc::channel::<ClientEnvelope>(100);

    // Spawn a task to handle writing to WebSocket
    let client_id = args.client_id.clone();
    tokio::spawn(async move {
        while let Some(envelope) = rx.recv().await {
            if let Ok(json) = serde_json::to_string(&envelope)
                && let Err(e) = write.send(Message::Text(json.into())).await {
                eprintln!("Failed to send message: {e}");
                break;
            }
        }
    });

    while let Some(msg) = read.next().await {
        let msg = msg?;
        if let Message::Text(text) = msg {
            // Handle Ping explicitly if it's a raw "ping" string (some impls do this)
            if text == "ping" {
                tx.send(ClientEnvelope::Pong {
                    client_id: client_id.clone(),
                    ts: chrono::Utc::now().to_rfc3339(),
                })
                .await?;
                continue;
            }

            match serde_json::from_str::<ServerMessage>(&text) {
                Ok(ServerMessage::Welcome { message }) => {
                    println!("Server: {message}");
                }
                Ok(ServerMessage::Ping) => {
                    tx.send(ClientEnvelope::Pong {
                        client_id: client_id.clone(),
                        ts: chrono::Utc::now().to_rfc3339(),
                    })
                    .await?;
                }
                Ok(ServerMessage::Control {
                    action,
                    agent_id,
                    payload,
                }) => {
                    if action == "spawn"
                        && let (Some(agent_id), Some(payload)) = (agent_id, payload) {
                        spawn_agent(client_id.clone(), agent_id, payload, tx.clone()).await;
                    }
                }
                Ok(ServerMessage::Ack { .. }) => {
                    // Ignore acks for now
                }
                Err(e) => {
                    // Ignore unknown messages or parse errors
                     eprintln!("Failed to parse message: {text} | Error: {e}");
                }
            }
        }
    }

    Ok(())
}

async fn spawn_agent(
    client_id: String,
    agent_id: String,
    payload: ControlPayload,
    tx: tokio::sync::mpsc::Sender<ClientEnvelope>,
) {
    let codex_bin = std::env::current_exe().unwrap_or_else(|_| "bracket".into());
    
    let mut args = vec!["exec".to_string()];
    if let Some(extra_args) = payload.args {
        args.extend(extra_args);
    }
    if let Some(prompt) = payload.prompt {
        args.push(prompt);
    }

    println!("Spawning agent {agent_id}: {codex_bin:?} {args:?}");

    // Notify running
    let _ = tx
        .send(ClientEnvelope::Status {
            client_id: client_id.clone(),
            session_id: Some(agent_id.clone()),
            ts: chrono::Utc::now().to_rfc3339(),
            payload: StatusPayload {
                state: "running".to_string(),
                agent: Some("bracket".to_string()),
            },
        })
        .await;

    let child = Command::new(codex_bin)
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();

    match child {
        Ok(mut child) => {
            let stdout = child.stdout.take();
            let stderr = child.stderr.take();

            if let (Some(stdout), Some(stderr)) = (stdout, stderr) {
                let tx_stdout = tx.clone();
                let agent_id_stdout = agent_id.clone();
                let client_id_stdout = client_id.clone();
                tokio::spawn(async move {
                    let mut reader = BufReader::new(stdout).lines();
                    while let Ok(Some(line)) = reader.next_line().await {
                        let _ = tx_stdout
                            .send(ClientEnvelope::Log {
                                client_id: client_id_stdout.clone(),
                                session_id: Some(agent_id_stdout.clone()),
                                ts: chrono::Utc::now().to_rfc3339(),
                                payload: LogPayload {
                                    stream: "stdout".to_string(),
                                    message: line,
                                    agent: Some("bracket".to_string()),
                                },
                            })
                            .await;
                    }
                });

                let tx_stderr = tx.clone();
                let agent_id_stderr = agent_id.clone();
                let client_id_stderr = client_id.clone();
                tokio::spawn(async move {
                    let mut reader = BufReader::new(stderr).lines();
                    while let Ok(Some(line)) = reader.next_line().await {
                        let _ = tx_stderr
                            .send(ClientEnvelope::Log {
                                client_id: client_id_stderr.clone(),
                                session_id: Some(agent_id_stderr.clone()),
                                ts: chrono::Utc::now().to_rfc3339(),
                                payload: LogPayload {
                                    stream: "stderr".to_string(),
                                    message: line,
                                    agent: Some("bracket".to_string()),
                                },
                            })
                            .await;
                    }
                });

                // Wait for finish
                tokio::spawn(async move {
                    let status = child.wait().await;
                    let _ = tx
                        .send(ClientEnvelope::Status {
                            client_id,
                            session_id: Some(agent_id),
                            ts: chrono::Utc::now().to_rfc3339(),
                            payload: StatusPayload {
                                state: if status.map(|s| s.success()).unwrap_or(false) {
                                    "exited".to_string()
                                } else {
                                    "error".to_string()
                                },
                                agent: Some("bracket".to_string()),
                            },
                        })
                        .await;
                });
            }
        }
        Err(e) => {
            eprintln!("Failed to spawn: {e}");
             let _ = tx
                .send(ClientEnvelope::Status {
                    client_id,
                    session_id: Some(agent_id),
                    ts: chrono::Utc::now().to_rfc3339(),
                    payload: StatusPayload {
                        state: "error".to_string(),
                        agent: Some("bracket".to_string()),
                    },
                })
                .await;
        }
    }
}
