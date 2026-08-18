use bytes::Buf;
use bytes::Bytes;
use bytes::BytesMut;
use codex_exec_server_protocol::JSONRPCMessage;

use crate::ExecServerError;

const LENGTH_PREFIX_BYTES: usize = size_of::<u32>();
pub(crate) const MAX_NOISE_JSONRPC_MESSAGE_LEN: usize = 64 * 1024 * 1024;
pub(crate) const NOISE_RECORD_PLAINTEXT_LEN: usize = 60 * 1024;

/// Serialize one JSON-RPC message into the encrypted record byte stream.
///
/// Clatter limits an individual Noise message to 65,535 bytes, while valid
/// exec-server responses can be much larger. A four-byte authenticated length
/// prefix lets the caller split this byte stream into bounded Noise records and
/// lets the receiver reconstruct exact JSON-RPC message boundaries.
pub(crate) fn frame_jsonrpc_message(message: &JSONRPCMessage) -> Result<Vec<u8>, ExecServerError> {
    let mut framed = vec![0; LENGTH_PREFIX_BYTES];
    serde_json::to_writer(&mut framed, message)?;
    let prefix = message_length_prefix(framed.len() - LENGTH_PREFIX_BYTES)?;
    framed[..LENGTH_PREFIX_BYTES].copy_from_slice(&prefix);
    Ok(framed)
}

pub(crate) fn frame_message(message: &[u8]) -> Result<Vec<u8>, ExecServerError> {
    let prefix = message_length_prefix(message.len())?;
    let mut framed = Vec::with_capacity(LENGTH_PREFIX_BYTES + message.len());
    framed.extend_from_slice(&prefix);
    framed.extend_from_slice(message);
    Ok(framed)
}

fn message_length_prefix(message_len: usize) -> Result<[u8; LENGTH_PREFIX_BYTES], ExecServerError> {
    if message_len == 0 || message_len > MAX_NOISE_JSONRPC_MESSAGE_LEN {
        return Err(ExecServerError::Protocol(
            "Noise relay JSON-RPC message exceeds maximum length".to_string(),
        ));
    }
    Ok((message_len as u32).to_be_bytes())
}

/// Incrementally reconstructs authenticated JSON-RPC messages from Noise records.
///
/// The length prefix is encrypted along with the message. It is still bounded
/// here so a bad authenticated peer cannot grow the reassembly buffer forever.
#[derive(Default)]
pub(crate) struct JsonRpcMessageDecoder {
    decoder: MessageDecoder,
}

impl JsonRpcMessageDecoder {
    pub(crate) fn push(
        &mut self,
        plaintext_record: &[u8],
    ) -> Result<Vec<JSONRPCMessage>, ExecServerError> {
        self.decoder
            .push(plaintext_record)?
            .into_iter()
            .map(|message| serde_json::from_slice(&message).map_err(Into::into))
            .collect()
    }
}

/// Reassembles opaque application payloads without interpreting their schema.
#[derive(Default)]
pub(crate) struct MessageDecoder {
    buffered: BytesMut,
}

impl MessageDecoder {
    /// Append one decrypted record and return all complete framed messages.
    pub(crate) fn push(&mut self, plaintext_record: &[u8]) -> Result<Vec<Bytes>, ExecServerError> {
        if plaintext_record.len() > NOISE_RECORD_PLAINTEXT_LEN {
            return Err(ExecServerError::Protocol(
                "Noise relay plaintext record exceeds maximum length".to_string(),
            ));
        }
        self.buffered.extend_from_slice(plaintext_record);

        // One record can finish multiple messages, and one message can span
        // multiple records. Parse only after the authenticated length prefix
        // and the full declared payload are present.
        let mut messages = Vec::new();
        while let Some(prefix) = self.buffered.get(..LENGTH_PREFIX_BYTES) {
            let message_len =
                u32::from_be_bytes([prefix[0], prefix[1], prefix[2], prefix[3]]) as usize;
            // Reject the authenticated length before waiting for its payload.
            if message_len == 0 || message_len > MAX_NOISE_JSONRPC_MESSAGE_LEN {
                return Err(ExecServerError::Protocol(
                    "Noise relay JSON-RPC message has invalid length".to_string(),
                ));
            }
            let framed_len = LENGTH_PREFIX_BYTES + message_len;
            if self.buffered.len() < framed_len {
                break;
            }
            self.buffered.advance(LENGTH_PREFIX_BYTES);
            messages.push(self.buffered.split_to(message_len).freeze());
        }

        // Even before a message is complete, keep reassembly memory bounded.
        if self.buffered.len() > LENGTH_PREFIX_BYTES + MAX_NOISE_JSONRPC_MESSAGE_LEN {
            return Err(ExecServerError::Protocol(
                "Noise relay JSON-RPC reassembly buffer exceeds maximum length".to_string(),
            ));
        }
        Ok(messages)
    }
}

#[cfg(test)]
#[path = "message_framing_tests.rs"]
mod tests;
