//! Centralized motion primitives for the TUI.
//!
//! Callers choose an explicit reduced-motion fallback here instead of reaching
//! directly for time-varying spinner or shimmer helpers.

use std::time::Instant;

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
    if supports_color::on_cached(supports_color::Stream::Stdout)
        .map(|level| level.has_16m)
        .unwrap_or(false)
    {
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
