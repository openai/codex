//! Fixed transport exit categories and bounded heartbeat progress telemetry.
//!
//! Keep wire values and error text out of these categories: CCA retains this
//! telemetry even when ordinary exec-server diagnostics are filtered out.

pub(super) enum TransportExitReason {
    PongTimeout,
    OutgoingClosed,
    Framing,
    SequenceExhausted,
    Encryption,
    WebsocketWriteTimeout,
    WebsocketWrite,
    WebsocketEnded,
    InvalidFrame,
    ApplicationDelivery,
    InvalidData,
    RelayReset,
    WebsocketClose,
    UnexpectedText,
    WebsocketRead,
}

impl TransportExitReason {
    pub(super) fn as_str(&self) -> &'static str {
        match self {
            Self::PongTimeout => "pong_timeout",
            Self::OutgoingClosed => "outgoing_closed",
            Self::Framing => "framing",
            Self::SequenceExhausted => "sequence_exhausted",
            Self::Encryption => "encryption",
            Self::WebsocketWriteTimeout => "websocket_write_timeout",
            Self::WebsocketWrite => "websocket_write",
            Self::WebsocketEnded => "websocket_ended",
            Self::InvalidFrame => "invalid_frame",
            Self::ApplicationDelivery => "application_delivery",
            Self::InvalidData => "invalid_data",
            Self::RelayReset => "relay_reset",
            Self::WebsocketClose => "websocket_close",
            Self::UnexpectedText => "unexpected_text",
            Self::WebsocketRead => "websocket_read",
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub(super) enum WebsocketWriteError {
    #[error("websocket write timed out")]
    Timeout,
    #[error("{0}")]
    Transport(String),
}

impl WebsocketWriteError {
    pub(super) fn exit_reason(&self) -> TransportExitReason {
        match self {
            Self::Timeout => TransportExitReason::WebsocketWriteTimeout,
            Self::Transport(_) => TransportExitReason::WebsocketWrite,
        }
    }
}

/// WebSocket heartbeat progress only proves reachability of the relay peer, not
/// delivery to the executor behind it. Emit at most once per local Ping or Pong.
pub(super) enum HeartbeatProgress {
    PingWritten,
    PongReceived,
}

#[derive(Default)]
pub(super) struct HeartbeatDiagnostics {
    pub(super) pings_written: u64,
    pub(super) pongs_received: u64,
    last_ping_at: Option<tokio::time::Instant>,
}

impl HeartbeatDiagnostics {
    pub(super) fn record(&mut self, progress: HeartbeatProgress, stream_id: &str) {
        let phase = match progress {
            HeartbeatProgress::PingWritten => {
                self.pings_written += 1;
                self.last_ping_at = Some(tokio::time::Instant::now());
                "ping_written"
            }
            HeartbeatProgress::PongReceived => {
                // Unsolicited Pongs must not create unbounded telemetry volume.
                if self.last_ping_at.is_none() {
                    return;
                }
                self.pongs_received += 1;
                "pong_received"
            }
        };
        tracing::event!(
            name: "codex.exec_server.noise_heartbeat",
            target: "codex_otel.trace_safe",
            tracing::Level::INFO,
            event.name = "codex.exec_server.noise_heartbeat",
            stream_id,
            phase,
            pings_written = self.pings_written,
            pongs_received = self.pongs_received,
            ping_elapsed_ms = self.last_ping_at.map(|at| at.elapsed().as_millis() as u64).unwrap_or(0),
        );
        if matches!(progress, HeartbeatProgress::PongReceived) {
            self.last_ping_at = None;
        }
    }
}
