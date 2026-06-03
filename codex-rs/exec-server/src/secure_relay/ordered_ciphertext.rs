use std::collections::BTreeMap;

use crate::ExecServerError;

const MAX_REORDER_DISTANCE: u32 = 64;
const MAX_PENDING_FRAMES: usize = 64;
const MAX_PENDING_BYTES: usize = 1024 * 1024;

/// Bounded pre-decryption reorder buffer for Noise transport records.
///
/// Relay delivery can be duplicated or reordered, but Noise transport nonces
/// are strictly ordered. This type absorbs only a small reliable-delivery
/// window and releases ciphertexts exactly once in nonce order. It must sit
/// before `SecureTransport::decrypt`; attempting to decrypt a future record
/// would advance or desynchronize cryptographic state.
#[derive(Default)]
pub(crate) struct OrderedCiphertextFrames {
    next_seq: u32,
    pending: BTreeMap<u32, Vec<u8>>,
    pending_bytes: usize,
}

impl OrderedCiphertextFrames {
    /// Accept one relay record and return the newly contiguous ciphertext run.
    pub(crate) fn push(
        &mut self,
        seq: u32,
        payload: Vec<u8>,
    ) -> Result<Vec<Vec<u8>>, ExecServerError> {
        // Already-delivered and already-buffered frames are retries. Keep the
        // first buffered ciphertext for a sequence so a duplicate cannot
        // replace it before authentication.
        if seq < self.next_seq || self.pending.contains_key(&seq) {
            return Ok(Vec::new());
        }
        if seq > self.next_seq {
            // Bound both sequence distance and actual buffered memory. Without
            // both limits, an authenticated peer could hold the stream open
            // while forcing unbounded pre-decryption state.
            if seq - self.next_seq > MAX_REORDER_DISTANCE {
                return Err(ExecServerError::Protocol(
                    "secure relay ciphertext exceeds reorder window".to_string(),
                ));
            }
            let pending_bytes = self
                .pending_bytes
                .checked_add(payload.len())
                .ok_or_else(|| {
                    ExecServerError::Protocol(
                        "secure relay pending ciphertext byte count overflowed".to_string(),
                    )
                })?;
            if self.pending.len() >= MAX_PENDING_FRAMES || pending_bytes > MAX_PENDING_BYTES {
                return Err(ExecServerError::Protocol(
                    "secure relay pending ciphertext buffer is full".to_string(),
                ));
            }
            self.pending.insert(seq, payload);
            self.pending_bytes = pending_bytes;
            return Ok(Vec::new());
        }

        // The expected record closes the current gap. Release it and every
        // contiguous buffered successor so Noise sees exactly nonce order.
        let mut ready = vec![payload];
        self.advance()?;
        while let Some(payload) = self.pending.remove(&self.next_seq) {
            self.pending_bytes -= payload.len();
            ready.push(payload);
            self.advance()?;
        }
        Ok(ready)
    }

    fn advance(&mut self) -> Result<(), ExecServerError> {
        self.next_seq = self.next_seq.checked_add(1).ok_or_else(|| {
            ExecServerError::Protocol("secure relay sequence number exhausted".to_string())
        })?;
        Ok(())
    }
}

#[cfg(test)]
#[path = "ordered_ciphertext_tests.rs"]
mod tests;
