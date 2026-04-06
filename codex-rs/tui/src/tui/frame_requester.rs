//! Frame draw scheduling utilities for the TUI.
//!
//! This module exposes [`FrameRequester`], a lightweight handle that widgets and
//! background tasks can clone to request future redraws of the TUI.
//!
//! Internally it spawns a [`FrameScheduler`] task that coalesces many requests
//! into a single notification on a broadcast channel used by the main TUI event
//! loop. This keeps animations and status updates smooth without redrawing more
//! often than necessary.
//!
//! This follows the actor-style design from
//! [“Actors with Tokio”](https://ryhl.io/blog/actors-with-tokio/), with a
//! dedicated scheduler task and lightweight request handles.

use std::time::Duration;
use std::time::Instant;

use tokio::sync::broadcast;
use tokio::sync::mpsc;

use super::frame_rate_limiter::FrameRateLimiter;

/// A requester for scheduling future frame draws on the TUI event loop.
///
/// This is the handler side of an actor/handler pair with `FrameScheduler`, which coalesces
/// multiple frame requests into a single draw operation.
///
/// Clones of this type can be freely shared across tasks to make it possible to trigger frame draws
/// from anywhere in the TUI code.
#[derive(Clone, Debug)]
pub struct FrameRequester {
    frame_schedule_tx: mpsc::UnboundedSender<Instant>,
}

impl FrameRequester {
    /// Create a new FrameRequester and spawn its associated FrameScheduler task.
    ///
    /// The provided `draw_tx` is used to notify the TUI event loop of scheduled draws.
    pub fn new(draw_tx: broadcast::Sender<()>) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let scheduler = FrameScheduler::new(rx, draw_tx);
        tokio::spawn(scheduler.run());
        Self {
            frame_schedule_tx: tx,
        }
    }

    /// Schedule a frame draw as soon as possible.
    pub fn schedule_frame(&self) {
        let _ = self.frame_schedule_tx.send(Instant::now());
    }

    /// Schedule a frame draw to occur after the specified duration.
    pub fn schedule_frame_in(&self, dur: Duration) {
        let _ = self.frame_schedule_tx.send(Instant::now() + dur);
    }
}

#[cfg(test)]
impl FrameRequester {
    /// Create a no-op frame requester for tests.
    pub(crate) fn test_dummy() -> Self {
        let (tx, _rx) = mpsc::unbounded_channel();
        FrameRequester {
            frame_schedule_tx: tx,
        }
    }
}

/// A scheduler for coalescing frame draw requests and notifying the TUI event loop.
///
/// This type is internal to `FrameRequester` and is spawned as a task to handle scheduling logic.
///
/// To avoid wasted redraw work, draw notifications are clamped to a maximum of 120 FPS (see
/// [`FrameRateLimiter`]).
struct FrameScheduler {
    receiver: mpsc::UnboundedReceiver<Instant>,
    draw_tx: broadcast::Sender<()>,
    rate_limiter: FrameRateLimiter,
}

impl FrameScheduler {
    /// Create a new FrameScheduler with the provided receiver and draw notification sender.
    fn new(receiver: mpsc::UnboundedReceiver<Instant>, draw_tx: broadcast::Sender<()>) -> Self {
        Self {
            receiver,
            draw_tx,
            rate_limiter: FrameRateLimiter::default(),
        }
    }

    /// Run the scheduling loop, coalescing frame requests and notifying the TUI event loop.
    ///
    /// This method runs indefinitely until all senders are dropped. A single draw notification
    /// is sent for multiple requests scheduled before the next draw deadline.
    async fn run(mut self) {
        const ONE_YEAR: Duration = Duration::from_secs(60 * 60 * 24 * 365);
        let mut next_deadline: Option<Instant> = None;
        loop {
            let target = next_deadline.unwrap_or_else(|| Instant::now() + ONE_YEAR);
            let deadline = tokio::time::sleep_until(target.into());
            tokio::pin!(deadline);

            tokio::select! {
                draw_at = self.receiver.recv() => {
                    let Some(draw_at) = draw_at else {
                        // All senders dropped; exit the scheduler.
                        break
                    };
                    let draw_at = self.rate_limiter.clamp_deadline(draw_at);
                    next_deadline = Some(next_deadline.map_or(draw_at, |cur| cur.min(draw_at)));

                    // Do not send a draw immediately here. By continuing the loop,
                    // we recompute the sleep target so the draw fires once via the
                    // sleep branch, coalescing multiple requests into a single draw.
                    continue;
                }
                _ = &mut deadline => {
                    if next_deadline.is_some() {
                        next_deadline = None;
                        self.rate_limiter.mark_emitted(target);
                        let _ = self.draw_tx.send(());
                    }
                }
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::super::frame_rate_limiter::MIN_FRAME_INTERVAL;
    use super::*;
    use tokio::time;
    use tokio_util::time::FutureExt;

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn test_schedule_frame_immediate_triggers_once() {
        let (draw_tx, mut draw_rx) = broadcast::channel(16);
        let requester = FrameRequester::new(draw_tx);

        requester.schedule_frame();

        // Advance time minimally to let the scheduler process and hit the deadline == now.
        time::advance(Duration::from_millis(1)).await;

        // First draw should arrive.
        let first = draw_rx
            .recv()
            .timeout(Duration::from_millis(50))
            .await
            .expect("timed out waiting for first draw");
        assert!(first.is_ok(), "broadcast closed unexpectedly");

        // No second draw should arrive.
        let second = draw_rx.recv().timeout(Duration::from_millis(20)).await;
        assert!(second.is_err(), "unexpected extra draw received");
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn test_schedule_frame_in_triggers_at_delay() {
        let (draw_tx, mut draw_rx) = broadcast::channel(16);
        let requester = FrameRequester::new(draw_tx);

        requester.schedule_frame_in(Duration::from_millis(50));

        // Advance less than the delay: no draw yet.
        time::advance(Duration::from_millis(30)).await;
        let early = draw_rx.recv().timeout(Duration::from_millis(10)).await;
        assert!(early.is_err(), "draw fired too early");

        // Advance past the deadline: one draw should fire.
        time::advance(Duration::from_millis(25)).await;
        let first = draw_rx
            .recv()
            .timeout(Duration::from_millis(50))
            .await
            .expect("timed out waiting for scheduled draw");
        assert!(first.is_ok(), "broadcast closed unexpectedly");

        // No second draw should arrive.
        let second = draw_rx.recv().timeout(Duration::from_millis(20)).await;
        assert!(second.is_err(), "unexpected extra draw received");
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn test_coalesces_multiple_requests_into_single_draw() {
        let (draw_tx, mut draw_rx) = broadcast::channel(16);
        let requester = FrameRequester::new(draw_tx);

        // Schedule multiple immediate requests close together.
        requester.schedule_frame();
        requester.schedule_frame();
        requester.schedule_frame();

        // Allow the scheduler to process and hit the coalesced deadline.
        time::advance(Duration::from_millis(1)).await;

        // Expect only a single draw notification despite three requests.
        let first = draw_rx
            .recv()
            .timeout(Duration::from_millis(50))
            .await
            .expect("timed out waiting for coalesced draw");
        assert!(first.is_ok(), "broadcast closed unexpectedly");

        // No additional draw should be sent for the same coalesced batch.
        let second = draw_rx.recv().timeout(Duration::from_millis(20)).await;
        assert!(second.is_err(), "unexpected extra draw received");
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn test_coalesces_mixed_immediate_and_delayed_requests() {
        let (draw_tx, mut draw_rx) = broadcast::channel(16);
        let requester = FrameRequester::new(draw_tx);

        // Schedule a delayed draw and then an immediate one; should coalesce and fire at the earliest (immediate).
        requester.schedule_frame_in(Duration::from_millis(100));
        requester.schedule_frame();

        time::advance(Duration::from_millis(1)).await;

        let first = draw_rx
            .recv()
            .timeout(Duration::from_millis(50))
            .await
            .expect("timed out waiting for coalesced immediate draw");
        assert!(first.is_ok(), "broadcast closed unexpectedly");

        // The later delayed request should have been coalesced into the earlier one; no second draw.
        let second = draw_rx.recv().timeout(Duration::from_millis(120)).await;
        assert!(second.is_err(), "unexpected extra draw received");
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn test_limits_draw_notifications_to_120fps() {
        let (draw_tx, mut draw_rx) = broadcast::channel(16);
        let requester = FrameRequester::new(draw_tx);

        requester.schedule_frame();
        time::advance(Duration::from_millis(1)).await;
        let first = draw_rx
            .recv()
            .timeout(Duration::from_millis(50))
            .await
            .expect("timed out waiting for first draw");
        assert!(first.is_ok(), "broadcast closed unexpectedly");

        requester.schedule_frame();
        time::advance(Duration::from_millis(1)).await;
        let early = draw_rx.recv().timeout(Duration::from_millis(1)).await;
        assert!(
            early.is_err(),
            "draw fired too early; expected max 120fps (min interval {MIN_FRAME_INTERVAL:?})"
        );

        time::advance(MIN_FRAME_INTERVAL).await;
        let second = draw_rx
            .recv()
            .timeout(Duration::from_millis(50))
            .await
            .expect("timed out waiting for second draw");
        assert!(second.is_ok(), "broadcast closed unexpectedly");
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn test_rate_limit_clamps_early_delayed_requests() {
        let (draw_tx, mut draw_rx) = broadcast::channel(16);
        let requester = FrameRequester::new(draw_tx);

        requester.schedule_frame();
        time::advance(Duration::from_millis(1)).await;
        let first = draw_rx
            .recv()
            .timeout(Duration::from_millis(50))
            .await
            .expect("timed out waiting for first draw");
        assert!(first.is_ok(), "broadcast closed unexpectedly");

        requester.schedule_frame_in(Duration::from_millis(1));

        time::advance(MIN_FRAME_INTERVAL / 2).await;
        let too_early = draw_rx.recv().timeout(Duration::from_millis(1)).await;
        assert!(
            too_early.is_err(),
            "draw fired too early; expected max 120fps (min interval {MIN_FRAME_INTERVAL:?})"
        );

        time::advance(MIN_FRAME_INTERVAL).await;
        let second = draw_rx
            .recv()
            .timeout(Duration::from_millis(50))
            .await
            .expect("timed out waiting for clamped draw");
        assert!(second.is_ok(), "broadcast closed unexpectedly");
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn test_rate_limit_does_not_delay_future_draws() {
        let (draw_tx, mut draw_rx) = broadcast::channel(16);
        let requester = FrameRequester::new(draw_tx);

        requester.schedule_frame();
        time::advance(Duration::from_millis(1)).await;
        let first = draw_rx
            .recv()
            .timeout(Duration::from_millis(50))
            .await
            .expect("timed out waiting for first draw");
        assert!(first.is_ok(), "broadcast closed unexpectedly");

        requester.schedule_frame_in(Duration::from_millis(50));

        time::advance(Duration::from_millis(49)).await;
        let early = draw_rx.recv().timeout(Duration::from_millis(1)).await;
        assert!(early.is_err(), "draw fired too early");

        time::advance(Duration::from_millis(1)).await;
        let second = draw_rx
            .recv()
            .timeout(Duration::from_millis(50))
            .await
            .expect("timed out waiting for delayed draw");
        assert!(second.is_ok(), "broadcast closed unexpectedly");
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn test_multiple_delayed_requests_coalesce_to_earliest() {
        let (draw_tx, mut draw_rx) = broadcast::channel(16);
        let requester = FrameRequester::new(draw_tx);

        // Schedule multiple delayed draws; they should coalesce to the earliest (10ms).
        requester.schedule_frame_in(Duration::from_millis(100));
        requester.schedule_frame_in(Duration::from_millis(20));
        requester.schedule_frame_in(Duration::from_millis(120));

        // Advance to just before the earliest deadline: no draw yet.
        time::advance(Duration::from_millis(10)).await;
        let early = draw_rx.recv().timeout(Duration::from_millis(10)).await;
        assert!(early.is_err(), "draw fired too early");

        // Advance past the earliest deadline: one draw should fire.
        time::advance(Duration::from_millis(20)).await;
        let first = draw_rx
            .recv()
            .timeout(Duration::from_millis(50))
            .await
            .expect("timed out waiting for earliest coalesced draw");
        assert!(first.is_ok(), "broadcast closed unexpectedly");

        // No additional draw should fire for the later delayed requests.
        let second = draw_rx.recv().timeout(Duration::from_millis(120)).await;
        assert!(second.is_err(), "unexpected extra draw received");
    }
}
