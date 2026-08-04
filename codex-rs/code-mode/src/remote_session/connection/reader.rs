use codex_code_mode_protocol::host::TransportLane;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use super::driver::DriverEvent;
use super::transport::ConnectionReader;

pub(super) async fn drive_reader(
    mut reader: ConnectionReader,
    events: mpsc::Sender<DriverEvent>,
    cancellation: CancellationToken,
    lane: Option<TransportLane>,
) -> Result<(), String> {
    loop {
        let message = tokio::select! {
            _ = cancellation.cancelled() => return Ok(()),
            result = reader.read() => result,
        };
        let message = match message {
            Ok(Some(message)) => message,
            Ok(None) => return Err("code-mode host closed its stdout".to_string()),
            Err(err) => return Err(format!("failed to read code-mode host message: {err}")),
        };
        if let Some(lane) = lane
            && !message.allows_transport_lane(lane)
        {
            return Err("code-mode host sent a message on the wrong websocket lane".to_string());
        }
        events
            .send(DriverEvent::HostMessage(message))
            .await
            .map_err(|_| "code-mode connection driver closed".to_string())?;
    }
}
