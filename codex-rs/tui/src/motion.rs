//! Centralized motion primitives for the TUI.
//!
//! Callers choose an explicit reduced-motion fallback here instead of reaching
//! directly for time-varying spinner or shimmer helpers.

use std::time::Duration;
use std::time::Instant;

use crate::terminal_palette::StdoutColorLevel;
use crate::terminal_palette::stdout_color_level;
use crate::text_formatting::contains_rtl_or_arabic_number;
use crate::tui::FrameRequester;
use ratatui::style::Stylize;
use ratatui::text::Span;

#[path = "shimmer.rs"]
mod shimmer;

use shimmer::shimmer_spans;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MotionMode {
    Animated,
    Reduced,
}

impl MotionMode {
    pub(crate) fn from_animations_enabled(animations_enabled: bool) -> Self {
        if animations_enabled {
            Self::Animated
        } else {
            Self::Reduced
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ReducedMotionIndicator {
    Hidden,
    StaticBullet,
}

pub(crate) const LOADING_FRAME_INTERVAL: Duration = Duration::from_millis(100);

/// Show the current loading glyph immediately and schedule the next frame.
pub(crate) fn loading_glyph(
    started_at: Instant,
    mode: MotionMode,
    requester: &FrameRequester,
) -> &'static str {
    loading_glyph_with_delay(started_at, Duration::ZERO, mode, requester)
}

/// Show nothing during the animation delay, or a static glyph in reduced motion.
/// Schedule the frame that makes the spinner visible and each animation frame.
pub(crate) fn loading_glyph_with_delay(
    started_at: Instant,
    delay: Duration,
    mode: MotionMode,
    requester: &FrameRequester,
) -> &'static str {
    const FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    if mode == MotionMode::Reduced {
        return "◌";
    }
    let elapsed = started_at.elapsed();
    if elapsed < delay {
        requester.schedule_frame_in(delay - elapsed);
        return "";
    }
    requester.schedule_frame_in(LOADING_FRAME_INTERVAL);
    let frame = elapsed.as_millis() / LOADING_FRAME_INTERVAL.as_millis();
    FRAMES[usize::try_from(frame).unwrap_or_default() % FRAMES.len()]
}

pub(crate) fn activity_indicator(
    start_time: Option<Instant>,
    motion_mode: MotionMode,
    reduced_motion_indicator: ReducedMotionIndicator,
) -> Option<Span<'static>> {
    match motion_mode {
        MotionMode::Animated => Some(animated_activity_indicator(start_time)),
        MotionMode::Reduced => match reduced_motion_indicator {
            ReducedMotionIndicator::Hidden => None,
            ReducedMotionIndicator::StaticBullet => Some("•".dim()),
        },
    }
}

pub(crate) fn shimmer_text(text: &str, motion_mode: MotionMode) -> Vec<Span<'static>> {
    match motion_mode {
        MotionMode::Animated if contains_rtl_or_arabic_number(text) => vec![text.to_owned().dim()],
        MotionMode::Animated => shimmer_spans(text),
        MotionMode::Reduced => {
            if text.is_empty() {
                Vec::new()
            } else {
                vec![text.to_string().into()]
            }
        }
    }
}

fn animated_activity_indicator(start_time: Option<Instant>) -> Span<'static> {
    let elapsed = start_time.map(|st| st.elapsed()).unwrap_or_default();
    if stdout_color_level() == StdoutColorLevel::TrueColor {
        shimmer_spans("•")
            .into_iter()
            .next()
            .unwrap_or_else(|| "•".into())
    } else {
        let blink_on = (elapsed.as_millis() / 600).is_multiple_of(2);
        if blink_on { "•".into() } else { "◦".dim() }
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn reduced_motion_activity_indicator_uses_explicit_fallback() {
        assert_eq!(
            activity_indicator(
                /*start_time*/ None,
                MotionMode::Reduced,
                ReducedMotionIndicator::Hidden,
            ),
            None
        );
        assert_eq!(
            activity_indicator(
                /*start_time*/ None,
                MotionMode::Reduced,
                ReducedMotionIndicator::StaticBullet,
            ),
            Some("•".dim())
        );
    }

    #[test]
    fn reduced_motion_shimmer_text_is_plain_text() {
        assert_eq!(
            shimmer_text("Loading", MotionMode::Reduced),
            vec!["Loading".into()]
        );
        assert_eq!(
            shimmer_text("", MotionMode::Reduced),
            Vec::<Span<'static>>::new()
        );
    }
}
