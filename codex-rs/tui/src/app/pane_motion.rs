//! Slide the sidebar from its last painted width, independently of keyboard focus.

use std::time::Duration;
use tokio::time::Instant;

use crate::tui::FrameRequester;
use crate::tui::TARGET_FRAME_INTERVAL;

const SLIDE_DURATION: Duration = Duration::from_millis(180);

#[derive(Clone, Debug, Default)]
pub(super) struct PaneMotion {
    fraction: f64,
    target: f64,
    slide: Option<(Instant, f64)>,
}

impl PaneMotion {
    /// Without a frame requester, snap to the target immediately.
    pub(super) fn advance(&mut self, open: bool, frames: Option<FrameRequester>) -> f64 {
        let target = f64::from(open);
        if self.target != target {
            self.target = target;
            self.slide = Some((Instant::now(), self.fraction));
        }
        if let (Some((start, from)), Some(frames)) = (self.slide, frames) {
            let progress = start.elapsed().as_secs_f64() / SLIDE_DURATION.as_secs_f64();
            if progress < 1.0 && from != target {
                let eased = progress * progress * (3.0 - 2.0 * progress);
                self.fraction = from + (target - from) * eased;
                frames.schedule_frame_in(TARGET_FRAME_INTERVAL);
                return self.fraction;
            }
        }
        self.slide = None;
        self.fraction = target;
        target
    }
}
