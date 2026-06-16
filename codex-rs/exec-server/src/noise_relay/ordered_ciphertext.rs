use std::collections::BTreeMap;

use crate::ExecServerError;

const MAX_REORDER_DISTANCE: u32 = 64;
const MAX_PENDING_BYTES: usize = 1024 * 1024;

/// Reorders relay records before they reach Noise's implicit receive nonce.
/// The window is bounded, and each sequence number is released at most once.
#[derive(Default)]
pub(crate) struct OrderedCiphertextFrames {
    next_seq: u32,
    pending: BTreeMap<u32, Vec<u8>>,
    pending_bytes: usize,
}

impl OrderedCiphertextFrames {
    /// Accept one relay record and return the newly contiguous ciphertext run.
    ///
    /// Returns nothing for duplicates or while a gap remains. Closing a gap also
    /// releases any buffered records that now follow it contiguously.
    pub(crate) fn push(
        &mut self,
        seq: u32,
        payload: Vec<u8>,
    ) -> Result<Vec<Vec<u8>>, ExecServerError> {
        // Keep the first ciphertext for a sequence. Later copies are duplicates.
        if seq < self.next_seq || self.pending.contains_key(&seq) {
            return Ok(Vec::new());
        }
        if seq > self.next_seq {
            // Bound both the sequence gap and buffered bytes.
            if seq - self.next_seq > MAX_REORDER_DISTANCE {
                return Err(ExecServerError::Protocol(
                    "Noise relay ciphertext exceeds reorder window".to_string(),
                ));
            }
            let pending_bytes = self.pending_bytes + payload.len();
            if pending_bytes > MAX_PENDING_BYTES {
                return Err(ExecServerError::Protocol(
                    "Noise relay pending ciphertext buffer is full".to_string(),
                ));
            }
            self.pending.insert(seq, payload);
            self.pending_bytes = pending_bytes;
            return Ok(Vec::new());
        }

        // Release the expected record and anything now contiguous behind it.
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
            ExecServerError::Protocol("Noise relay sequence number exhausted".to_string())
        })?;
        Ok(())
    }
}

#[cfg(test)]
#[path = "ordered_ciphertext_tests.rs"]
mod tests;
