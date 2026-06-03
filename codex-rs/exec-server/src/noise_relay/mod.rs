mod harness;
mod message_framing;
mod ordered_ciphertext;

use crate::ExecServerError;

pub(crate) use harness::noise_harness_connection_from_websocket;

fn take_next_sequence(next_seq: &mut u32) -> Result<u32, ExecServerError> {
    // Never wrap: relay sequence is the explicit ordering key for an implicit
    // Noise nonce. Reusing zero after u32::MAX would be ambiguous and unsafe.
    let seq = *next_seq;
    *next_seq = next_seq.checked_add(1).ok_or_else(|| {
        ExecServerError::Protocol("Noise relay sequence number exhausted".to_string())
    })?;
    Ok(seq)
}
