use std::io::Result;
use std::io::Write;

use crossterm::SynchronizedUpdate;

#[derive(Debug, Default)]
pub(super) struct ReplayClearState {
    pending: bool,
}

impl ReplayClearState {
    pub(super) fn request(&mut self) {
        self.pending = true;
    }

    pub(super) fn is_pending(&self) -> bool {
        self.pending
    }

    pub(super) fn begin_draw(&self) -> ReplayClearDraw {
        ReplayClearDraw {
            requested: self.pending,
            replay_committed: false,
        }
    }

    pub(super) fn finish_draw(&mut self, draw: ReplayClearDraw) {
        if draw.replay_committed {
            self.pending = false;
        }
    }
}

pub(super) struct ReplayClearDraw {
    requested: bool,
    replay_committed: bool,
}

impl ReplayClearDraw {
    pub(super) fn requested(&self) -> bool {
        self.requested
    }

    pub(super) fn commit_replay(&mut self) {
        if self.requested {
            self.replay_committed = true;
        }
    }
}

pub(super) fn run_synchronized_draw<W, T>(
    writer: &mut W,
    operations: impl FnOnce(&mut W) -> Result<T>,
) -> Result<T>
where
    W: Write,
{
    writer.sync_update(operations)?
}

#[cfg(test)]
#[path = "replay_clear_tests.rs"]
mod tests;
