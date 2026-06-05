use super::CHANNEL_CAPACITY;
use super::ConnectionOrigin;
use super::InitializeClientMetadata;
use super::TransportEvent;
use super::forward_incoming_message;
use super::next_connection_id;
use super::serialize_outgoing_message;
use crate::outgoing_message::QueuedOutgoingMessage;
use codex_app_server_protocol::InitializeParams;
use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::JSONRPCRequest;
use std::io::ErrorKind;
use std::io::Result as IoResult;
use tokio::io;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tracing::debug;
use tracing::error;
use tracing::info;

pub async fn start_stdio_connection(
    transport_event_tx: mpsc::Sender<TransportEvent>,
    stdio_handles: &mut Vec<JoinHandle<()>>,
    initialize_client_metadata_tx: oneshot::Sender<InitializeClientMetadata>,
) -> IoResult<()> {
    let connection_id = next_connection_id();
    let (writer_tx, mut writer_rx) = mpsc::channel::<QueuedOutgoingMessage>(CHANNEL_CAPACITY);
    let writer_tx_for_reader = writer_tx.clone();
    transport_event_tx
        .send(TransportEvent::ConnectionOpened {
            connection_id,
            origin: ConnectionOrigin::Stdio,
            writer: writer_tx,
            disconnect_sender: None,
        })
        .await
        .map_err(|_| std::io::Error::new(ErrorKind::BrokenPipe, "processor unavailable"))?;

    let transport_event_tx_for_reader = transport_event_tx.clone();
    stdio_handles.push(tokio::spawn(async move {
        let stdin = io::stdin();
        let reader = BufReader::new(stdin);
        let mut lines = reader.lines();
        let mut initialize_client_metadata_tx = Some(initialize_client_metadata_tx);

        loop {
            match lines.next_line().await {
                Ok(Some(line)) => {
                    if let Some(metadata) = stdio_initialize_client_metadata(&line)
                        && let Some(initialize_client_metadata_tx) =
                            initialize_client_metadata_tx.take()
                    {
                        let _ = initialize_client_metadata_tx.send(metadata);
                    }
                    if !forward_incoming_message(
                        &transport_event_tx_for_reader,
                        &writer_tx_for_reader,
                        connection_id,
                        &line,
                    )
                    .await
                    {
                        break;
                    }
                }
                Ok(None) => break,
                Err(err) => {
                    error!("Failed reading stdin: {err}");
                    break;
                }
            }
        }

        let _ = transport_event_tx_for_reader
            .send(TransportEvent::ConnectionClosed { connection_id })
            .await;
        debug!("stdin reader finished (EOF)");
    }));

    stdio_handles.push(tokio::spawn(async move {
        let mut stdout = io::stdout();
        while let Some(queued_message) = writer_rx.recv().await {
            let Some(mut json) = serialize_outgoing_message(queued_message.message) else {
                continue;
            };
            json.push('\n');
            if let Err(err) = stdout.write_all(json.as_bytes()).await {
                error!("Failed to write to stdout: {err}");
                break;
            }
            if let Some(write_complete_tx) = queued_message.write_complete_tx {
                let _ = write_complete_tx.send(());
            }
        }
        info!("stdout writer exited (channel closed)");
    }));

    Ok(())
}

fn stdio_initialize_client_metadata(line: &str) -> Option<InitializeClientMetadata> {
    let message = serde_json::from_str::<JSONRPCMessage>(line).ok()?;
    let JSONRPCMessage::Request(JSONRPCRequest { method, params, .. }) = message else {
        return None;
    };
    if method != "initialize" {
        return None;
    }
    let params = params?;
    let initialize_params = serde_json::from_value::<InitializeParams>(params.clone()).ok()?;
    let remote_control_apns_registration = params
        .get("capabilities")
        .and_then(|capabilities| capabilities.get("extensions"))
        .and_then(|extensions| extensions.get("com.openai.codex.remote-control"))
        .and_then(|remote_control| remote_control.get("apns"))
        .and_then(|apns| serde_json::from_value(apns.clone()).ok());
    Some(InitializeClientMetadata {
        client_name: initialize_params.client_info.name,
        remote_control_apns_registration,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::RemoteControlApnsEnvironment;
    use crate::transport::RemoteControlApnsRegistration;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    #[test]
    fn extracts_remote_control_apns_registration_from_initialize_extensions() {
        let line = json!({
            "id": 1,
            "method": "initialize",
            "params": {
                "clientInfo": {
                    "name": "codex_desktop",
                    "title": "Codex Desktop",
                    "version": "1.2.3"
                },
                "capabilities": {
                    "extensions": {
                        "com.openai.codex.remote-control": {
                            "apns": {
                                "deviceToken": "device-token",
                                "topic": "com.openai.codex.alpha",
                                "environment": "development"
                            }
                        }
                    }
                }
            }
        })
        .to_string();

        assert_eq!(
            stdio_initialize_client_metadata(&line),
            Some(InitializeClientMetadata {
                client_name: "codex_desktop".to_string(),
                remote_control_apns_registration: Some(RemoteControlApnsRegistration {
                    device_token: "device-token".to_string(),
                    topic: "com.openai.codex.alpha".to_string(),
                    environment: RemoteControlApnsEnvironment::Development,
                }),
            })
        );
    }
}
