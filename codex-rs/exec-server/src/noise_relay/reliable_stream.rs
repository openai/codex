use std::collections::BTreeMap;
use std::time::Duration;

use tokio::time::Instant;

use crate::ExecServerError;

/// Maximum number of encrypted records retained until the peer cumulatively
/// acknowledges them.
pub(crate) const MAX_UNACKED_SEGMENTS: usize = 32;
/// Maximum encrypted bytes retained until the peer cumulatively acknowledges
/// them.
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
/// allocates send sequences, applies peer cumulative acks, and retains exact
/// ciphertext for retries.
#[derive(Debug)]
pub(crate) struct ReliableSender {
    next_seq: u32,
    highest_sent_seq: u32,
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
            resend_cursor: 1,
            unacked: BTreeMap::new(),
            unacked_bytes: 0,
        }
    }
}

impl ReliableSender {
    /// Whether one newly encrypted payload can be retained in the send window.
    ///
    /// Callers must check this before consuming another Noise send nonce.
    pub(crate) fn can_admit_ciphertext(&self, ciphertext_len: usize) -> bool {
        ciphertext_len > 0
            && ciphertext_len <= MAX_UNACKED_BYTES
            && self.unacked.len() < MAX_UNACKED_SEGMENTS
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

    /// Apply cumulative peer acknowledgement metadata.
    ///
    /// ack = 0 is the reserved empty state. PR1 rejects nonzero selective bits
    /// so they cannot silently alter delivery semantics before PR2.
    pub(crate) fn process_peer_ack(
        &mut self,
        ack: u32,
        ack_bits: u32,
    ) -> Result<(), ExecServerError> {
        if ack_bits != 0 {
            return Err(ExecServerError::Protocol(format!(
                "Noise reliable selective ack bits are unsupported in PR1: {ack_bits}"
            )));
        }
        if ack == 0 {
            return Ok(());
        }
        if ack > self.highest_sent_seq {
            return Err(ExecServerError::Protocol(format!(
                "Noise reliable peer ack {ack} exceeds highest sent sequence {}",
                self.highest_sent_seq
            )));
        }

        let acknowledged = self
            .unacked
            .range(..=ack)
            .map(|(seq, _pending)| *seq)
            .collect::<Vec<_>>();
        for seq in acknowledged {
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
