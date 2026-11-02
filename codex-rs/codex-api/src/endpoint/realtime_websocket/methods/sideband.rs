//! Adapts caller-owned, bounded JSON channels to the existing realtime parser and writer.
//!
//! ```text
//! Caller -- incoming JSON --> read loop  -- Message::Text --> Core event parser
//! Caller <-- outgoing JSON -- write loop <-- WsCommand ----- Core writer
//! ```
//!
//! Each channel item carries one JSON message of up to 1 MiB. The loops run
//! independently so Core can still receive events while a command waits for
//! space in the caller's queue. Shutdown seals incoming messages and drains accepted
//! input before closing both directions, including a frame awaiting parser capacity.

use super::*;
use codex_protocol::protocol::RealtimeSidebandTransport;

const MAX_MESSAGE_BYTES: usize = 1024 * 1024;

impl RealtimeWebsocketConnection {
    /// Uses an already authenticated, caller-owned stream without opening or reconnecting a socket.
    pub fn from_sideband(
        transport: RealtimeSidebandTransport,
        event_parser: RealtimeEventParser,
    ) -> Self {
        let RealtimeSidebandTransport {
            mut incoming,
            outgoing,
        } = transport;

        let (tx_command, mut rx_command) = mpsc::channel::<WsCommand>(32);
        let (tx_message, rx_message) = async_channel::bounded(32);
        let (_, terminal_policy_error) = tokio::sync::watch::channel(None);

        let (input_stop, mut input_stopped) = tokio::sync::watch::channel(false);
        let transport_input_stop = input_stop.clone();
        let transport_task = tokio::spawn(async move {
            let read = async {
                let mut is_draining = false;
                loop {
                    let text = if is_draining {
                        // A reserved but unsent permit must not hold shutdown open.
                        // try_recv() will drain messages already queued and exit when emptied.
                        incoming.try_recv().ok()
                    } else {
                        tokio::select! {
                            biased;
                            // Core requested shutdown, or the transport's write loop ended.
                            _ = input_stopped.changed() => {
                                incoming.close();
                                is_draining = true;
                                continue;
                            }
                            // The caller dropped its receiver for Core commands.
                            // Detect this even when the writer is idle, then drain accepted input.
                            _ = outgoing.closed() => {
                                incoming.close();
                                is_draining = true;
                                continue;
                            }
                            text = incoming.recv() => text,
                        }
                    };
                    let Some(text) = text else {
                        break;
                    };
                    if text.len() > MAX_MESSAGE_BYTES {
                        break;
                    }
                    if tx_message
                        .send(Ok(Message::Text(text.into())))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
            };

            let write = async {
                while let Some(command) = rx_command.recv().await {
                    // These commands are still called WsCommand for legacy reasons.
                    match command {
                        WsCommand::Send {
                            message: Message::Text(text),
                            tx_result,
                        } => {
                            let result = if text.len() > MAX_MESSAGE_BYTES {
                                Err(WsError::Io(std::io::Error::other(
                                    "sideband message too large",
                                )))
                            } else {
                                outgoing
                                    .send(text.to_string())
                                    .await
                                    .map_err(|_| WsError::ConnectionClosed)
                            };
                            let failed = result.is_err();
                            // Tell Core's writer whether the caller's channel accepted the command.
                            let _ = tx_result.send(result);
                            if failed {
                                break;
                            }
                        }
                        WsCommand::Close {
                            frame: _,
                            tx_result,
                        } => {
                            let _ = tx_result.send(Ok(()));
                            break;
                        }
                        WsCommand::Send { tx_result, .. } => {
                            let _ = tx_result.send(Err(WsError::Io(std::io::Error::other(
                                "sideband requires JSON text",
                            ))));
                            break;
                        }
                    }
                }
            };

            tokio::pin!(read);
            tokio::select! {
                _ = &mut read => {},
                _ = write => {
                    transport_input_stop.send_replace(true);
                    // Preserve the read future: it may already own a frame that
                    // has not yet fit into the parser queue.
                    read.await;
                },
            }
        });

        Self::new(
            WsStream {
                tx_command,
                transport_task,
                sideband_input_stop: Some(input_stop),
                terminal_policy_error,
            },
            rx_message,
            event_parser,
            RealtimeTranscriptState::default(),
        )
    }
}

impl RealtimeWebsocketWriter {
    /// Seal caller-owned input so its accepted events can drain without waiting for producers.
    /// Socket-backed connections are unaffected.
    pub fn close_sideband_input(&self) {
        if let Some(input_stop) = &self.stream.sideband_input_stop {
            input_stop.send_replace(true);
        }
    }
}

#[cfg(test)]
#[path = "sideband_tests.rs"]
mod tests;
