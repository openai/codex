//! Keep executor writes bounded while processing pongs independently of write backpressure.

use futures::Sink;
use futures::SinkExt;
use tokio::sync::mpsc;
use tokio::time::Instant;
use tokio_tungstenite::tungstenite::Message;
use tracing::warn;

use crate::connection::WEBSOCKET_KEEPALIVE_INTERVAL;
use crate::relay::RendezvousDisconnectReason;
use crate::websocket_pong_watchdog::WEBSOCKET_PONG_TIMEOUT;
use crate::websocket_pong_watchdog::WebSocketPongWatchdog;

pub(super) async fn run<S, E>(
    mut sink: S,
    mut outgoing_rx: mpsc::Receiver<Vec<u8>>,
    mut pong_rx: mpsc::Receiver<()>,
) -> RendezvousDisconnectReason
where
    S: Sink<Message, Error = E> + Unpin,
    E: std::fmt::Display,
{
    let mut keepalive = tokio::time::interval_at(
        Instant::now() + WEBSOCKET_KEEPALIVE_INTERVAL,
        WEBSOCKET_KEEPALIVE_INTERVAL,
    );
    keepalive.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut watchdog = WebSocketPongWatchdog::new(WEBSOCKET_PONG_TIMEOUT);
    loop {
        let message = tokio::select! {
            result = receive_pong(&mut watchdog, &mut pong_rx) => {
                if let Err(reason) = result {
                    return reason;
                }
                continue;
            }
            _ = keepalive.tick(), if watchdog.deadline().is_none() => {
                Message::Ping(Vec::new().into())
            }
            encoded = outgoing_rx.recv() => {
                let Some(encoded) = encoded else {
                    return RendezvousDisconnectReason::LocalShutdown;
                };
                Message::Binary(encoded.into())
            }
        };
        let is_keepalive_ping = matches!(message, Message::Ping(_));
        // A pong clears only its own deadline, never the fixed write budget.
        let write = tokio::time::timeout(WEBSOCKET_PONG_TIMEOUT, sink.send(message));
        tokio::pin!(write);
        loop {
            tokio::select! {
                // Keep the same send future alive when a pong arrives: cancelling
                // and restarting a partially flushed send can duplicate a frame.
                result = receive_pong(&mut watchdog, &mut pong_rx), if !is_keepalive_ping => {
                    if let Err(reason) = result {
                        return reason;
                    }
                }
                result = &mut write => {
                    match result {
                        Ok(Ok(())) => {
                            if is_keepalive_ping {
                                // Pongs received during the ping flush stay queued
                                // until the watchdog is armed here.
                                watchdog.ping_sent(Instant::now());
                            }
                            break;
                        }
                        Ok(Err(error)) => {
                            warn!("Noise multiplexed environment websocket write failed: {error}");
                            return RendezvousDisconnectReason::WriteError;
                        }
                        Err(_) => {
                            warn!("Noise multiplexed environment websocket write timed out");
                            return RendezvousDisconnectReason::WriteError;
                        }
                    }
                }
            }
        }
    }
}

async fn receive_pong(
    watchdog: &mut WebSocketPongWatchdog,
    pong_rx: &mut mpsc::Receiver<()>,
) -> Result<(), RendezvousDisconnectReason> {
    let deadline = watchdog.deadline();
    tokio::select! {
        biased;
        // Honor a pong already read by the reader even if the timer is also ready.
        pong = pong_rx.recv() => {
            pong.ok_or(RendezvousDisconnectReason::LocalShutdown)?;
            watchdog.received_pong();
            Ok(())
        }
        _ = tokio::time::sleep_until(deadline.unwrap_or_else(Instant::now)), if deadline.is_some() => {
            Err(RendezvousDisconnectReason::PongTimeout)
        }
    }
}

#[cfg(test)]
#[path = "relay_writer_tests.rs"]
mod tests;
