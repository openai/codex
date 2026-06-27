pub(crate) mod executor_stream;
mod harness;
mod message_framing;
mod ordered_ciphertext;

use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;

use crate::ExecServerError;

pub(crate) use harness::NoiseHarnessConnectionArgs;
pub(crate) use harness::noise_harness_connection_from_websocket;

pub(crate) const NOISE_RELAY_RESET_REASON: &str = "noise_relay_protocol_error";

// This bounds allocation in tungstenite before protobuf and Noise record
// validation run. It comfortably fits one maximum Noise record plus metadata.
const MAX_NOISE_RELAY_WEBSOCKET_MESSAGE_SIZE: usize = 256 * 1024;

/// Return the websocket limits required by every Noise relay endpoint.
pub(crate) fn noise_relay_websocket_config() -> WebSocketConfig {
    WebSocketConfig::default()
        .max_frame_size(Some(MAX_NOISE_RELAY_WEBSOCKET_MESSAGE_SIZE))
        .max_message_size(Some(MAX_NOISE_RELAY_WEBSOCKET_MESSAGE_SIZE))
}

fn take_next_sequence(next_seq: &mut u32) -> Result<u32, ExecServerError> {
    // Never wrap: relay sequence is the explicit ordering key for an implicit
    // Noise nonce. Reusing zero after u32::MAX would be ambiguous and unsafe.
    let seq = *next_seq;
    *next_seq = next_seq.checked_add(1).ok_or_else(|| {
        ExecServerError::Protocol("Noise relay sequence number exhausted".to_string())
    })?;
    Ok(seq)
}
