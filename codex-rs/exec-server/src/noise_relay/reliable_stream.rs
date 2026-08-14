use std::collections::BTreeMap;
use std::time::Duration;

use tokio::time::Instant;

use crate::ExecServerError;
use crate::relay::RelayAckState;

/// Maximum number of encrypted records retained awaiting peer acknowledgement.
pub(crate) const MAX_UNACKED_SEGMENTS: usize = 32;
/// Maximum encrypted bytes retained awaiting peer acknowledgement.
pub(crate) const MAX_UNACKED_BYTES: usize = 2 * 1024 * 1024;
/// How long an encrypted record may remain unacknowledged before it is retried.
pub(crate) const RESEND_AFTER: Duration = Duration::from_millis(500);

/// One encrypted record ready for an initial send or retry.
///
/// The payload is already Noise-encrypted. Retries clone these exact bytes
/// rather than asking Noise to encrypt the logical record again.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OutboundCiphertext {
    pub(crate) seq: u32,
    pub(crate) payload: Vec<u8>,
}

#[derive(Debug)]
struct UnackedCiphertext {
    payload: Vec<u8>,
    last_sent_at: Instant,
}

/// Sender-side reliability state for one logical Noise relay stream.
///
/// The receive frontier stays in `OrderedCiphertextFrames`, which already owns
/// the bounded reorder buffer needed before Noise decryption. This type only
/// allocates send sequences, applies peer acknowledgement state, and retains
/// exact ciphertext for retries.
#[derive(Debug)]
pub(crate) struct ReliableSender {
    next_seq: u32,
    highest_sent_seq: u32,
    peer_cumulative_ack: u32,
    resend_cursor: u32,
    unacked: BTreeMap<u32, UnackedCiphertext>,
    unacked_bytes: usize,
}

impl Default for ReliableSender {
    fn default() -> Self {
        Self {
            // Sequence zero is reserved so ack = 0 unambiguously means that
            // nothing has been received contiguously yet.
            next_seq: 1,
            highest_sent_seq: 0,
            peer_cumulative_ack: 0,
            resend_cursor: 1,
            unacked: BTreeMap::new(),
            unacked_bytes: 0,
        }
    }
}

impl ReliableSender {
    fn send_window_has_space(&self) -> bool {
        self.next_seq
            .checked_sub(self.peer_cumulative_ack)
            .is_some_and(|span| span <= MAX_UNACKED_SEGMENTS as u32)
    }

    /// Whether one newly encrypted payload can be retained in the send window.
    ///
    /// Callers must check this before consuming another Noise send nonce.
    pub(crate) fn can_admit_ciphertext(&self, ciphertext_len: usize) -> bool {
        ciphertext_len > 0
            && ciphertext_len <= MAX_UNACKED_BYTES
            && self.unacked.len() < MAX_UNACKED_SEGMENTS
            && self.send_window_has_space()
            && self
                .unacked_bytes
                .checked_add(ciphertext_len)
                .is_some_and(|bytes| bytes <= MAX_UNACKED_BYTES)
    }

    /// Allocate the next sequence number and retain an already-encrypted record.
    pub(crate) fn admit_ciphertext(
        &mut self,
        payload: Vec<u8>,
        now: Instant,
    ) -> Result<OutboundCiphertext, ExecServerError> {
        if payload.is_empty() {
            return Err(ExecServerError::Protocol(
                "Noise reliable ciphertext payload is empty".to_string(),
            ));
        }
        if payload.len() > MAX_UNACKED_BYTES {
            return Err(ExecServerError::Protocol(format!(
                "Noise reliable ciphertext exceeds unacked byte limit: {} > {MAX_UNACKED_BYTES}",
                payload.len()
            )));
        }
        if self.unacked.len() >= MAX_UNACKED_SEGMENTS {
            return Err(ExecServerError::Protocol(
                "Noise reliable segment send window is full".to_string(),
            ));
        }
        if !self.send_window_has_space() {
            return Err(ExecServerError::Protocol(
                "Noise reliable cumulative send window is full".to_string(),
            ));
        }
        let unacked_bytes = self
            .unacked_bytes
            .checked_add(payload.len())
            .filter(|bytes| *bytes <= MAX_UNACKED_BYTES)
            .ok_or_else(|| {
                ExecServerError::Protocol("Noise reliable byte send window is full".to_string())
            })?;
        let seq = self.next_seq;
        self.next_seq = self.next_seq.checked_add(1).ok_or_else(|| {
            ExecServerError::Protocol("Noise reliable sequence number exhausted".to_string())
        })?;
        self.highest_sent_seq = seq;
        self.unacked_bytes = unacked_bytes;
        self.unacked.insert(
            seq,
            UnackedCiphertext {
                payload: payload.clone(),
                last_sent_at: now,
            },
        );
        Ok(OutboundCiphertext { seq, payload })
    }

    /// Apply cumulative and selective peer acknowledgement metadata.
    ///
    /// Selective acknowledgement frees cached ciphertext for retry and byte
    /// accounting, but only the cumulative frontier slides the sequence window.
    pub(crate) fn process_peer_ack(
        &mut self,
        ack_state: RelayAckState,
    ) -> Result<(), ExecServerError> {
        let RelayAckState { ack, ack_bits } = ack_state;
        if ack > self.highest_sent_seq {
            return Err(ExecServerError::Protocol(format!(
                "Noise reliable peer ack {ack} exceeds highest sent sequence {}",
                self.highest_sent_seq
            )));
        }
        if ack_bits & 1 != 0 {
            return Err(ExecServerError::Protocol(
                "Noise reliable selective ack bit zero is inconsistent with cumulative ack"
                    .to_string(),
            ));
        }

        let mut selective_acks = Vec::new();
        let mut remaining_ack_bits = ack_bits;
        while remaining_ack_bits != 0 {
            let bit = remaining_ack_bits.trailing_zeros();
            let seq = ack
                .checked_add(1)
                .and_then(|seq| seq.checked_add(bit))
                .ok_or_else(|| {
                    ExecServerError::Protocol(
                        "Noise reliable selective ack sequence overflow".to_string(),
                    )
                })?;
            if seq > self.highest_sent_seq {
                return Err(ExecServerError::Protocol(format!(
                    "Noise reliable selective ack seq {seq} exceeds highest sent sequence {}",
                    self.highest_sent_seq
                )));
            }
            selective_acks.push(seq);
            remaining_ack_bits &= remaining_ack_bits - 1;
        }

        self.peer_cumulative_ack = self.peer_cumulative_ack.max(ack);
        let acknowledged = self
            .unacked
            .range(..=ack)
            .map(|(seq, _pending)| *seq)
            .collect::<Vec<_>>();
        for seq in acknowledged.into_iter().chain(selective_acks) {
            if let Some(pending) = self.unacked.remove(&seq) {
                self.unacked_bytes -= pending.payload.len();
            }
        }
        Ok(())
    }

    /// Return the next cached record whose retry deadline has elapsed.
    ///
    /// Returning one record at a time preserves a scheduling point between
    /// retries so websocket control traffic cannot sit behind a full-window
    /// resend burst. The cursor gives every retained record a chance before
    /// wrapping to an older record that became due again.
    pub(crate) fn next_retry_due(&mut self, now: Instant) -> Option<OutboundCiphertext> {
        let due_seq = self
            .unacked
            .range(self.resend_cursor..)
            .chain(self.unacked.range(..self.resend_cursor))
            .find_map(|(seq, pending)| {
                now.checked_duration_since(pending.last_sent_at)
                    .is_some_and(|elapsed| elapsed >= RESEND_AFTER)
                    .then_some(*seq)
            })?;
        let payload = {
            let pending = self.unacked.get_mut(&due_seq)?;
            pending.last_sent_at = now;
            pending.payload.clone()
        };
        self.resend_cursor = due_seq.checked_add(1).unwrap_or(1);
        Some(OutboundCiphertext {
            seq: due_seq,
            payload,
        })
    }
}

#[cfg(test)]
#[path = "reliable_stream_tests.rs"]
mod tests;
