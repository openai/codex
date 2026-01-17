//! Shared ASCII animation state for popups and onboarding.
//!
//! The animation tracks elapsed time and asks the [`FrameRequester`] to redraw
//! at the next frame boundary. Callers can switch between preloaded variants
//! without resetting the timing model.

use std::convert::TryFrom;
use std::time::Duration;
use std::time::Instant;

use rand::Rng as _;

use crate::frames::ALL_VARIANTS;
use crate::frames::FRAME_TICK_DEFAULT;
use crate::tui::FrameRequester;

/// Drives ASCII art animations shared across popups and onboarding widgets.
pub(crate) struct AsciiAnimation {
    /// Frame scheduler used to request UI redraws.
    request_frame: FrameRequester,

    /// Immutable list of frame variants (each variant is a list of frames).
    variants: &'static [&'static [&'static str]],

    /// Index into `variants` selecting the active animation.
    variant_idx: usize,

    /// Duration between frame advances.
    frame_tick: Duration,

    /// Start time used to compute frame offsets.
    start: Instant,
}

impl AsciiAnimation {
    /// Build an animation using the shared variants and default frame tick.
    pub(crate) fn new(request_frame: FrameRequester) -> Self {
        Self::with_variants(request_frame, ALL_VARIANTS, 0)
    }

    /// Build an animation with the provided variants and initial index.
    ///
    /// The start time is initialized immediately; callers can change variants later without
    /// resetting the timing model.
    pub(crate) fn with_variants(
        request_frame: FrameRequester,
        variants: &'static [&'static [&'static str]],
        variant_idx: usize,
    ) -> Self {
        assert!(
            !variants.is_empty(),
            "AsciiAnimation requires at least one animation variant",
        );
        let clamped_idx = variant_idx.min(variants.len() - 1);
        Self {
            request_frame,
            variants,
            variant_idx: clamped_idx,
            frame_tick: FRAME_TICK_DEFAULT,
            start: Instant::now(),
        }
    }

    /// Schedule a redraw at the next frame boundary based on `frame_tick`.
    pub(crate) fn schedule_next_frame(&self) {
        let tick_ms = self.frame_tick.as_millis();
        if tick_ms == 0 {
            self.request_frame.schedule_frame();
            return;
        }
        let elapsed_ms = self.start.elapsed().as_millis();
        let rem_ms = elapsed_ms % tick_ms;
        let delay_ms = if rem_ms == 0 {
            tick_ms
        } else {
            tick_ms - rem_ms
        };
        if let Ok(delay_ms_u64) = u64::try_from(delay_ms) {
            self.request_frame
                .schedule_frame_in(Duration::from_millis(delay_ms_u64));
        } else {
            self.request_frame.schedule_frame();
        }
    }

    /// Return the current frame string for the active variant.
    pub(crate) fn current_frame(&self) -> &'static str {
        let frames = self.frames();
        if frames.is_empty() {
            return "";
        }
        let tick_ms = self.frame_tick.as_millis();
        if tick_ms == 0 {
            return frames[0];
        }
        let elapsed_ms = self.start.elapsed().as_millis();
        let idx = ((elapsed_ms / tick_ms) % frames.len() as u128) as usize;
        frames[idx]
    }

    /// Pick a new random variant and schedule a redraw if one exists.
    ///
    /// Returns `false` when there is only one variant, leaving state unchanged.
    pub(crate) fn pick_random_variant(&mut self) -> bool {
        if self.variants.len() <= 1 {
            return false;
        }
        let mut rng = rand::rng();
        let mut next = self.variant_idx;
        while next == self.variant_idx {
            next = rng.random_range(0..self.variants.len());
        }
        self.variant_idx = next;
        self.request_frame.schedule_frame();
        true
    }

    /// Request an immediate frame redraw without changing animation state.
    #[allow(dead_code)]
    pub(crate) fn request_frame(&self) {
        self.request_frame.schedule_frame();
    }

    /// Return the frame list for the current variant.
    fn frames(&self) -> &'static [&'static str] {
        self.variants[self.variant_idx]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_tick_must_be_nonzero() {
        assert!(FRAME_TICK_DEFAULT.as_millis() > 0);
    }
}
