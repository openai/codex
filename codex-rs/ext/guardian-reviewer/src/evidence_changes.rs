//! Coalesces committed Guardian evidence changes for a runtime's host publisher.

use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use tokio::sync::watch;

/// Monotonic, runtime-local changes to the sources used in Guardian root evidence.
/// Subscribers receive the latest committed sequence rather than one event per mutation.
#[derive(Debug, Default)]
pub struct GuardianEvidenceChanges {
    sequence: AtomicU64,
    latest: watch::Sender<u64>,
}

impl GuardianEvidenceChanges {
    /// Returns the latest committed source sequence.
    pub fn sequence(&self) -> u64 {
        self.sequence.load(Ordering::Acquire)
    }

    /// Observes source changes, including a mutation concurrent with subscription.
    pub fn subscribe(&self) -> watch::Receiver<u64> {
        self.publish(self.sequence());
        let rx = self.latest.subscribe();
        // Pair with notify's increment so either we see the change or it sees the receiver.
        self.publish(self.sequence.fetch_add(0, Ordering::AcqRel));
        rx
    }

    /// Marks a committed mutation while its source lock is still held.
    pub fn notify(&self) {
        let sequence = self.sequence.fetch_add(1, Ordering::AcqRel) + 1;
        if self.latest.receiver_count() != 0 {
            self.publish(sequence);
        }
    }

    fn publish(&self, sequence: u64) {
        self.latest.send_if_modified(|latest| {
            if sequence > *latest {
                *latest = sequence;
                true
            } else {
                false
            }
        });
    }
}

#[cfg(test)]
#[path = "evidence_changes_tests.rs"]
mod tests;
