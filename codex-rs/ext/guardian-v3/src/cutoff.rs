//! Capture and enqueue each history suffix in one order. The sole board writer opens
//! each requested view after routing that suffix and before processing its successor.
use std::sync::Arc;

use anyhow::Context;
use codex_guardian_reviewer::ReviewerTasks;
use codex_message_board::AgentMessageBoard;
use codex_protocol::AgentPath;
use codex_protocol::ThreadId;
use codex_protocol::protocol::HistoryPosition;
use codex_thread_store::ThreadStore;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use tokio::sync::oneshot;

use crate::EvidenceCursor;
use crate::EvidenceSnapshot;
use crate::ReviewerSubscriptions;

#[cfg(test)]
#[path = "cutoff_tests.rs"]
mod tests;

/// One approval's fixed source boundary and matching discovery, retrieval and delivery view.
pub struct ReviewCutoff {
    pub source: HistoryPosition,
    pub subscriptions: ReviewerSubscriptions,
}

/// Installed on the root thread while its evidence intake is active.
pub struct RootEvidence {
    lifetime: Arc<ReviewerTasks>,
    history: Arc<dyn ThreadStore>,
    // Advances on enqueue. The housekeeper independently tracks successful routing.
    // A rejected durable record keeps its reason for subsequent reviews of this root.
    enqueued: Mutex<Result<EvidenceCursor, String>>,
    pending: mpsc::Sender<Capture>,
}

struct Freeze {
    caller: ThreadId,
    path: AgentPath,
    reply: oneshot::Sender<anyhow::Result<ReviewCutoff>>,
}

pub(super) struct Capture {
    pub snapshot: EvidenceSnapshot,
    reviewer: Option<Freeze>,
}

impl Capture {
    pub async fn finish(self, reader: &Arc<dyn AgentMessageBoard>) {
        if let Some(Freeze {
            caller,
            path,
            reply,
        }) = self.reviewer
            && !reply.is_closed()
        {
            let source = self.snapshot.cutoff();
            let result = ReviewerSubscriptions::capture(reader.clone(), caller, path)
                .await
                .map(|subscriptions| ReviewCutoff {
                    source,
                    subscriptions,
                });
            let _ = reply.send(result);
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
        (
            Arc::new(Self {
                lifetime,
                history,
                enqueued: Mutex::new(Ok(cursor)),
                pending,
            }),
            requests,
        )
    }

    pub(super) async fn capture_on_activity(self: Arc<Self>, mut wakes: mpsc::Receiver<()>) {
        let capture = async {
            while wakes.recv().await.is_some() {
                if self.enqueue(/*reviewer*/ None).await.is_err() {
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
    async fn enqueue(&self, reviewer: Option<Freeze>) -> anyhow::Result<()> {
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
        // A freeze must wait behind earlier writes even when the source has not advanced.
        if reviewer.is_none() && !snapshot.source_advanced() {
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
        permit.send(Capture { snapshot, reviewer });
        Ok(())
    }
}

impl RootEvidence {
    /// Freeze once per review, using caller identity and path from the acting thread's host.
    pub async fn freeze(&self, caller: ThreadId, path: AgentPath) -> anyhow::Result<ReviewCutoff> {
        let _task = self.lifetime.tasks.token();
        let request = async {
            let (reply, ready) = oneshot::channel();
            self.enqueue(Some(Freeze {
                caller,
                path,
                reply,
            }))
            .await?;
            ready
                .await
                .context("Guardian stopped before the review cutoff")?
        };
        tokio::select! { biased;
            _ = self.lifetime.cancellation.cancelled() => anyhow::bail!("Guardian intake stopped"),
            result = request => result,
        }
    }
}
