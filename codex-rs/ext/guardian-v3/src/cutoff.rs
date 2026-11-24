//! Share a root history position across queued reviews, then acknowledge each request
//! once its evidence has been routed. Later board writes remain visible.
use std::sync::Arc;
use std::sync::OnceLock;

use anyhow::Context;
use codex_core::guardian_review::PendingReviewWrites;
use codex_core::guardian_review::ReviewPreparationError;
use codex_guardian_reviewer::ReviewerTasks;
use codex_protocol::ThreadId;
use codex_protocol::protocol::HistoryPosition;
use codex_thread_store::OriginalHistoryRequest;
use codex_thread_store::ThreadStore;
use codex_thread_store::ThreadStoreResult;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::sync::watch;

use crate::EvidenceCursor;
use crate::EvidenceSnapshot;

#[cfg(test)]
#[path = "cutoff_tests.rs"]
mod tests;

enum CutoffKind {
    Bind,
    CatchUp,
}

struct CutoffRequest {
    kind: CutoffKind,
    reply: oneshot::Sender<ThreadStoreResult<Option<HistoryPosition>>>,
}

/// Installed on the root thread while its evidence intake is active.
pub struct RootEvidence {
    lifetime: Arc<ReviewerTasks>,
    history: Arc<dyn ThreadStore>,
    cutoffs: mpsc::UnboundedSender<CutoffRequest>,
    cutoff_failure: Arc<OnceLock<ReviewPreparationError>>,
    // Advances on enqueue. The housekeeper independently tracks successful routing.
    enqueued: Mutex<EvidenceCursor>,
    // A rejected durable record keeps its reason for subsequent reviews of this root.
    rejection: OnceLock<String>,
    pending: mpsc::Sender<Capture>,
    routed: watch::Sender<HistoryPosition>,
}

pub(super) struct Capture {
    pub snapshot: EvidenceSnapshot,
    routed: watch::Sender<HistoryPosition>,
}

async fn capture_cutoffs(
    root: ThreadId,
    history: Arc<dyn ThreadStore>,
    writes: Arc<PendingReviewWrites>,
    failure: Arc<OnceLock<ReviewPreparationError>>,
    mut requests: mpsc::UnboundedReceiver<CutoffRequest>,
) {
    loop {
        let mut batch = Vec::new();
        if requests.recv_many(&mut batch, usize::MAX).await == 0 {
            return;
        }
        batch.retain(|request| !request.reply.is_closed());
        if batch.is_empty() {
            continue;
        }
        // Seal the group before inspecting history. A later caller may need a
        // newer record even if the root's authorization hasn't changed.
        let reviewers = batch.len();
        let lone_catch_up = reviewers == 1 && matches!(batch[0].kind, CutoffKind::CatchUp);
        let cancelled = async {
            for request in &mut batch {
                request.reply.closed().await;
            }
        };
        let capture = async {
            // With no position to share, let enqueue read the unread records
            // and their end position together under the cursor lock.
            if lone_catch_up {
                return Ok(None);
            }
            tracing::debug!(name: "guardian_v3.cutoff_batch", reviewers, "Capturing Guardian history position");
            history.flush_thread(root).await?;
            history
                .read_original_history(root, OriginalHistoryRequest::Tail)
                .await
                .map(|history| Some(history.end))
        };
        let result = tokio::select! {
            biased;
            () = cancelled => continue,
            result = async {
                // One wait covers everyone already in this group.
                if let Err(error) = writes.wait().await {
                    // Publish the reason before closing the current and queued replies.
                    let _ = failure.set(error);
                    return None;
                }
                Some(capture.await)
            } => result,
        };
        let Some(result) = result else {
            return;
        };
        for request in batch {
            let _ = request.reply.send(result.clone());
        }
    }
}

fn covers(position: HistoryPosition, cutoff: HistoryPosition) -> anyhow::Result<bool> {
    anyhow::ensure!(
        position.thread_id == cutoff.thread_id,
        "Guardian cutoff belongs to another root"
    );
    if position.end_ordinal_exclusive < cutoff.end_ordinal_exclusive {
        return Ok(false);
    }
    anyhow::ensure!(
        position.end_byte_offset >= cutoff.end_byte_offset,
        "Guardian cutoff is inconsistent"
    );
    Ok(true)
}

async fn wait_until_routed(
    mut routed: watch::Receiver<HistoryPosition>,
    target: HistoryPosition,
) -> anyhow::Result<HistoryPosition> {
    while !covers(*routed.borrow_and_update(), target)? {
        routed
            .changed()
            .await
            .context("Guardian housekeeper stopped")?;
    }
    Ok(target)
}

impl Capture {
    /// Advance only after the sole writer has published all deliveries through this position.
    pub(super) fn routed_through(&self, position: HistoryPosition) {
        self.routed.send_if_modified(|current| {
            if *current == position {
                return false;
            }
            *current = position;
            true
        });
    }

    /// Complete a capture, including one with no selected evidence.
    pub(super) fn finish(self) {
        self.routed_through(self.snapshot.cutoff());
    }
}

impl RootEvidence {
    pub(super) fn new(
        lifetime: Arc<ReviewerTasks>,
        history: Arc<dyn ThreadStore>,
        cursor: EvidenceCursor,
        writes: Arc<PendingReviewWrites>,
    ) -> (Arc<Self>, mpsc::Receiver<Capture>) {
        let (pending, requests) = mpsc::channel(1);
        let routed = watch::Sender::new(cursor.position());
        let (cutoffs, readers) = mpsc::unbounded_channel();
        let cutoff_failure = Arc::new(OnceLock::new());
        let capture = capture_cutoffs(
            cursor.thread_id(),
            Arc::clone(&history),
            writes,
            Arc::clone(&cutoff_failure),
            readers,
        );
        let cancel = lifetime.cancellation.clone();
        lifetime.tasks.spawn(async move {
            let _ = cancel.run_until_cancelled(capture).await;
        });
        (
            Arc::new(Self {
                lifetime,
                history,
                cutoffs,
                cutoff_failure,
                enqueued: Mutex::new(cursor),
                rejection: OnceLock::new(),
                pending,
                routed,
            }),
            requests,
        )
    }

    pub(super) async fn capture_on_activity(self: Arc<Self>, mut wakes: mpsc::Receiver<()>) {
        let capture = async {
            while wakes.recv().await.is_some() {
                if self.enqueue(/*cutoff*/ None).await.is_err() {
                    if self.rejection.get().is_some() {
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

    fn ensure_active(&self) -> anyhow::Result<()> {
        if let Some(reason) = self.rejection.get() {
            anyhow::bail!("{reason}");
        }
        anyhow::ensure!(!self.pending.is_closed(), "Guardian housekeeper stopped");
        Ok(())
    }

    #[expect(
        clippy::await_holding_invalid_type,
        reason = "capture and enqueue must agree on source order across concurrent approvals"
    )]
    async fn enqueue(&self, cutoff: Option<HistoryPosition>) -> anyhow::Result<HistoryPosition> {
        // The writer can reach our cutoff while a newer capture holds the cursor.
        let mut cursor = if let Some(end) = cutoff {
            tokio::select! {
                biased;
                ready = wait_until_routed(self.routed.subscribe(), end) => {
                    self.ensure_active()?;
                    return ready;
                }
                cursor = self.enqueued.lock() => cursor,
            }
        } else {
            self.enqueued.lock().await
        };
        self.ensure_active()?;
        let history = match cutoff {
            Some(end) => {
                let start = cursor.position();
                if covers(start, end)? {
                    return Ok(end);
                }
                self.history
                    .read_original_history(
                        cursor.thread_id(),
                        OriginalHistoryRequest::Range { start, end },
                    )
                    .await?
            }
            None => {
                self.history.flush_thread(cursor.thread_id()).await?;
                cursor.read_history(self.history.as_ref()).await?
            }
        };
        let snapshot = match cursor.snapshot(history) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                tracing::warn!(root_thread_id = %cursor.thread_id(), %error, "Guardian intake rejected stored evidence");
                let _ = self.rejection.set(error.to_string());
                return Err(error);
            }
        };
        let end = snapshot.cutoff();
        if !snapshot.source_advanced() {
            return Ok(end);
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
        permit.send(Capture {
            snapshot,
            routed: self.routed.clone(),
        });
        Ok(end)
    }
}

impl RootEvidence {
    /// Capture only the position. Binding an action does not enqueue any housekeeper work.
    pub(super) async fn capture_cutoff(&self) -> anyhow::Result<HistoryPosition> {
        self.request_cutoff(CutoffKind::Bind)
            .await?
            .context("Guardian binding requires a history position")
    }

    pub(super) fn cutoff_error(&self) -> ReviewPreparationError {
        self.cutoff_failure
            .get()
            .copied()
            .unwrap_or(ReviewPreparationError::EvidenceUnavailable)
    }

    async fn request_cutoff(&self, kind: CutoffKind) -> anyhow::Result<Option<HistoryPosition>> {
        self.ensure_active()?;
        let (reply, position) = oneshot::channel();
        self.cutoffs
            .send(CutoffRequest { kind, reply })
            .map_err(|_| self.cutoff_error())?;
        Ok(position.await.map_err(|_| self.cutoff_error())??)
    }

    /// Submit a catch-up through the current durable tail and return its delivery acknowledgement.
    pub async fn request_catch_up(
        &self,
    ) -> anyhow::Result<impl Future<Output = anyhow::Result<HistoryPosition>> + Send + use<>> {
        self.request_through(/*cutoff*/ None).await
    }

    pub(super) async fn request_through(
        &self,
        cutoff: Option<HistoryPosition>,
    ) -> anyhow::Result<impl Future<Output = anyhow::Result<HistoryPosition>> + Send + use<>> {
        let cutoff = match cutoff {
            Some(end) => Some(end),
            None => self.request_cutoff(CutoffKind::CatchUp).await?,
        };
        let target = tokio::select! { biased;
            _ = self.lifetime.cancellation.cancelled() => anyhow::bail!("Guardian intake stopped"),
            result = self.enqueue(cutoff) => result?,
        };
        let routed = self.routed.subscribe();
        let writer = self.pending.clone();
        let cancel = self.lifetime.cancellation.clone();
        Ok(async move {
            tokio::select! {
                biased;
                _ = cancel.cancelled() => anyhow::bail!("Guardian intake stopped"),
                ready = wait_until_routed(routed, target) => ready,
                _ = writer.closed() => anyhow::bail!("Guardian housekeeper stopped"),
            }
        })
    }
}
