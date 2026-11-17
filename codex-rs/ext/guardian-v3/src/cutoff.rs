//! Order intake requests by source position and acknowledge a request once its history has
//! been routed. The acknowledgement is a lower bound; subsequent board writes remain visible.
use std::sync::Arc;

use anyhow::Context;
use codex_guardian_reviewer::ReviewerTasks;
use codex_protocol::protocol::HistoryPosition;
use codex_thread_store::ThreadStore;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use tokio::sync::oneshot;

use crate::EvidenceCursor;
use crate::EvidenceSnapshot;
use crate::inbox::LiveDelivery;

#[cfg(test)]
#[path = "cutoff_tests.rs"]
mod tests;

/// Installed on the root thread while its evidence intake is active.
pub struct RootEvidence {
    lifetime: Arc<ReviewerTasks>,
    history: Arc<dyn ThreadStore>,
    // Advances on enqueue. The housekeeper independently tracks successful routing.
    // A rejected durable record keeps its reason for subsequent reviews of this root.
    enqueued: Mutex<Result<EvidenceCursor, String>>,
    pending: mpsc::Sender<Capture>,
    pub(super) delivery: Arc<LiveDelivery>,
}

pub(super) struct Capture {
    pub snapshot: EvidenceSnapshot,
    routed: Option<oneshot::Sender<HistoryPosition>>,
}

impl Capture {
    /// Called by the sole writer only after it published the whole capture to subscriber inboxes.
    pub(super) fn finish(self) {
        if let Some(reply) = self.routed {
            let _ = reply.send(self.snapshot.cutoff());
        }
    }
}

impl RootEvidence {
    pub(super) fn new(
        lifetime: Arc<ReviewerTasks>,
        history: Arc<dyn ThreadStore>,
        cursor: EvidenceCursor,
    ) -> (Arc<Self>, mpsc::Receiver<Capture>) {
        let (pending, requests) = mpsc::channel(1);
        let delivery = Arc::new(LiveDelivery::new(cursor.thread_id()));
        (
            Arc::new(Self {
                lifetime,
                history,
                enqueued: Mutex::new(Ok(cursor)),
                pending,
                delivery,
            }),
            requests,
        )
    }

    pub(super) async fn capture_on_activity(self: Arc<Self>, mut wakes: mpsc::Receiver<()>) {
        let capture = async {
            while wakes.recv().await.is_some() {
                if self.enqueue(/*routed*/ None).await.is_err() {
                    if self.enqueued.lock().await.is_err() {
                        break;
                    }
                    tracing::warn!("Guardian background capture failed; waiting for the next wake");
                }
            }
        };
        tokio::select! {
            biased;
            _ = self.lifetime.cancellation.cancelled() => (),
            _ = self.pending.closed() => (),
            () = capture => (),
        }
    }

    #[expect(
        clippy::await_holding_invalid_type,
        reason = "capture and enqueue must agree on source order across concurrent approvals"
    )]
    async fn enqueue(
        &self,
        routed: Option<oneshot::Sender<HistoryPosition>>,
    ) -> anyhow::Result<()> {
        let mut enqueued = self.enqueued.lock().await;
        let cursor = enqueued
            .as_mut()
            .map_err(|reason| anyhow::anyhow!("{reason}"))?;
        anyhow::ensure!(!self.pending.is_closed(), "Guardian housekeeper stopped");
        self.history.flush_thread(cursor.thread_id()).await?;
        let history = cursor.read_history(self.history.as_ref()).await?;
        let snapshot = match cursor.snapshot(history) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                tracing::warn!(root_thread_id = %cursor.thread_id(), %error, "Guardian intake rejected stored evidence");
                *enqueued = Err(error.to_string());
                return Err(error);
            }
        };
        // A review must wait for earlier writes even when this capture has no new originals.
        if routed.is_none() && !snapshot.source_advanced() {
            return Ok(());
        }
        // Cancellation before reserving leaves the cursor unchanged. After reservation,
        // admission and advancing the captured range cannot be interrupted.
        let permit = self
            .pending
            .reserve()
            .await
            .context("Guardian housekeeper stopped")?;
        cursor.acknowledge(&snapshot, snapshot.records().len())?;
        for &source_ordinal in &snapshot.skipped_incomplete_deliveries {
            tracing::warn!(
                name: "guardian_v3.evidence_skipped",
                root_thread_id = %cursor.thread_id(),
                source_ordinal,
                reason = "incomplete_delivered_message",
                "Skipped incomplete Guardian evidence"
            );
        }
        permit.send(Capture { snapshot, routed });
        Ok(())
    }
}

impl RootEvidence {
    /// Admit the current source boundary while the action is being submitted. A successful
    /// reply means all matching deliveries through this point are already in actor inboxes.
    pub async fn request_catch_up(&self) -> anyhow::Result<oneshot::Receiver<HistoryPosition>> {
        let (routed, ready) = oneshot::channel();
        tokio::select! { biased;
            _ = self.lifetime.cancellation.cancelled() => anyhow::bail!("Guardian intake stopped"),
            result = self.enqueue(Some(routed)) => result?,
        }
        Ok(ready)
    }
}
