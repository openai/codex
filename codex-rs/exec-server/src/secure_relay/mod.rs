mod environment;
mod harness;
mod message_framing;
mod ordered_ciphertext;

use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;

use crate::ExecServerError;

pub(crate) use environment::HarnessKeyValidator;
pub(crate) use environment::run_secure_multiplexed_environment;
pub(crate) use harness::secure_harness_connection_from_websocket;

// This bounds allocation in tungstenite before protobuf and secure-record
// validation run. It comfortably fits one maximum Noise record plus metadata.
const MAX_SECURE_RELAY_WEBSOCKET_MESSAGE_SIZE: usize = 256 * 1024;

/// Return the websocket limits required by every secure relay endpoint.
pub(crate) fn secure_relay_websocket_config() -> WebSocketConfig {
    WebSocketConfig::default()
        .max_frame_size(Some(MAX_SECURE_RELAY_WEBSOCKET_MESSAGE_SIZE))
        .max_message_size(Some(MAX_SECURE_RELAY_WEBSOCKET_MESSAGE_SIZE))
}

fn take_next_sequence(next_seq: &mut u32) -> Result<u32, ExecServerError> {
    // Never wrap: relay sequence is the explicit ordering key for an implicit
    // Noise nonce. Reusing zero after u32::MAX would be ambiguous and unsafe.
    let seq = *next_seq;
    *next_seq = next_seq.checked_add(1).ok_or_else(|| {
        ExecServerError::Protocol("secure relay sequence number exhausted".to_string())
    })?;
    Ok(seq)
}
