//! Unicode mascot rendering primitives for the startup session header.
//!
//! This module owns the small amount of runtime state needed to animate the generated frame
//! tables in `codex_logo_frames`. It does not decide where the mascot is rendered or schedule
//! redraws; the session-header cell renders the selected frame and `ChatWidget` drives the
//! lifecycle while the header remains active.
//!
//! A startup animation is intentionally copyable. The placeholder header created before session
//! configuration and the configured session header must share the same selected motion and
//! `Instant`, otherwise the mascot can restart or switch motions when session metadata arrives.
//! Once the fixed startup window expires, callers render `STATIC_FRAME` and stop scheduling
//! mascot redraws.

use crate::codex_logo_frames::BLINK_FRAMES;
use crate::codex_logo_frames::READ_BELOW_FRAMES;
use crate::codex_logo_frames::THINKING_FRAMES;
use crate::codex_logo_frames::WORKING_FRAMES;
use crate::color;
use rand::Rng;
use std::time::Duration;
use std::time::Instant;

pub(crate) use crate::codex_logo_frames::HEIGHT;
pub(crate) use crate::codex_logo_frames::LogoFrame;
pub(crate) use crate::codex_logo_frames::WIDTH;

/// Number of columns reserved between the mascot and session text.
pub(crate) const GAP_WIDTH: usize = 1;

const ANIMATION_FRAME_MILLIS: u64 = 200;
const STARTUP_ANIMATION_LOOPS: u64 = 2;
const FRAME_COUNT: usize = 8;
const HIGHLIGHT_PERIOD_FRAMES: u64 = HEIGHT as u64 + 4;

const BRIGHT_GRADIENT: [(u8, u8, u8); HEIGHT] = [
    (153, 161, 255),
    (136, 157, 255),
    (119, 148, 255),
    (100, 130, 255),
    (72, 92, 253),
    (61, 78, 249),
];

const DARK_GRADIENT: [(u8, u8, u8); HEIGHT] = [
    (70, 84, 202),
    (58, 96, 214),
    (47, 108, 222),
    (40, 101, 218),
    (30, 72, 194),
    (27, 57, 176),
];

/// Motion sequences eligible for the one-shot startup animation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum StartupAnimationKind {
    ReadBelow,
    Thinking,
    Working,
}

/// Selected startup motion and the clock origin shared across header handoffs.
///
/// Keep this value intact when replacing the placeholder header with configured session
/// information. Constructing another value during that handoff would restart the animation and
/// could choose a different motion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct StartupAnimation {
    kind: StartupAnimationKind,
    start: Instant,
}

const STARTUP_ANIMATION_KINDS: [StartupAnimationKind; 3] = [
    StartupAnimationKind::Working,
    StartupAnimationKind::Thinking,
    StartupAnimationKind::ReadBelow,
];

/// Reference frame rendered after startup motion settles or when animations are disabled.
pub(crate) const STATIC_FRAME: LogoFrame = BLINK_FRAMES[0];

/// Starts one randomly selected mascot motion at the current instant.
pub(crate) fn startup_animation() -> StartupAnimation {
    let mut rng = rand::rng();
    StartupAnimation {
        kind: STARTUP_ANIMATION_KINDS[rng.random_range(0..STARTUP_ANIMATION_KINDS.len())],
        start: Instant::now(),
    }
}

/// Returns the total lifetime of the one-shot startup motion.
pub(crate) fn startup_animation_duration() -> Duration {
    Duration::from_millis(ANIMATION_FRAME_MILLIS * FRAME_COUNT as u64 * STARTUP_ANIMATION_LOOPS)
}

/// Returns the redraw cadence expected while startup motion is active.
pub(crate) fn animation_frame_interval() -> Duration {
    Duration::from_millis(ANIMATION_FRAME_MILLIS)
}

/// Returns the current frame tick while startup motion remains active.
///
/// `None` means the caller should settle the header to `STATIC_FRAME` and stop scheduling redraws.
pub(crate) fn animation_tick(animation: StartupAnimation) -> Option<u64> {
    let elapsed = animation.start.elapsed();
    if elapsed >= startup_animation_duration() {
        None
    } else {
        Some(elapsed.as_millis() as u64 / ANIMATION_FRAME_MILLIS)
    }
}

/// Resolves the generated mascot frame for an active animation tick.
pub(crate) fn frame_for_tick(animation: StartupAnimation, tick: u64) -> &'static LogoFrame {
    &animation_frames(animation.kind)[tick as usize % FRAME_COUNT]
}

/// Selects a readable static mascot gradient for the terminal background.
pub(crate) fn gradient_for_bg(bg: Option<(u8, u8, u8)>) -> [(u8, u8, u8); HEIGHT] {
    if bg.is_some_and(color::is_light) {
        DARK_GRADIENT
    } else {
        BRIGHT_GRADIENT
    }
}

/// Selects the mascot gradient for an active tick, including the moving highlight row.
pub(crate) fn gradient_for_animation_tick(
    bg: Option<(u8, u8, u8)>,
    tick: u64,
) -> [(u8, u8, u8); HEIGHT] {
    let mut gradient = gradient_for_bg(bg);
    let highlight_row = tick % HIGHLIGHT_PERIOD_FRAMES;
    for (row, rgb) in gradient.iter_mut().enumerate() {
        let distance = row.abs_diff(highlight_row as usize);
        if distance <= 1 {
            *rgb = brighten(
                *rgb,
                if distance == 0 {
                    /*amount*/
                    36
                } else {
                    /*amount*/
                    16
                },
            );
        }
    }
    gradient
}

fn animation_frames(kind: StartupAnimationKind) -> &'static [LogoFrame; FRAME_COUNT] {
    match kind {
        StartupAnimationKind::ReadBelow => &READ_BELOW_FRAMES,
        StartupAnimationKind::Thinking => &THINKING_FRAMES,
        StartupAnimationKind::Working => &WORKING_FRAMES,
    }
}

fn brighten((r, g, b): (u8, u8, u8), amount: u8) -> (u8, u8, u8) {
    (
        r.saturating_add(amount),
        g.saturating_add(amount),
        b.saturating_add(amount),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use unicode_width::UnicodeWidthStr;

    #[test]
    fn frames_keep_14_by_6_geometry() {
        for kind in STARTUP_ANIMATION_KINDS {
            for frame in animation_frames(kind) {
                assert_eq!(frame.len(), HEIGHT);
                for line in frame {
                    assert_eq!(UnicodeWidthStr::width(*line), WIDTH);
                }
            }
        }
        for line in STATIC_FRAME {
            assert_eq!(UnicodeWidthStr::width(line), WIDTH);
        }
    }

    #[test]
    fn source_animations_have_distinct_frames() {
        for kind in STARTUP_ANIMATION_KINDS {
            let frames = animation_frames(kind);
            assert!(frames.windows(2).any(|frames| frames[0] != frames[1]));
        }
    }

    #[test]
    fn animation_wraps_and_settles() {
        let animation = StartupAnimation {
            kind: StartupAnimationKind::Working,
            start: Instant::now(),
        };
        assert_eq!(
            frame_for_tick(animation, /*tick*/ 0),
            frame_for_tick(animation, FRAME_COUNT as u64)
        );
        let completed_animation = StartupAnimation {
            kind: StartupAnimationKind::Working,
            start: Instant::now()
                .checked_sub(startup_animation_duration())
                .expect("duration should fit"),
        };
        assert_eq!(animation_tick(completed_animation), None);
    }

    #[test]
    fn copied_animation_preserves_kind_and_origin() {
        let animation = StartupAnimation {
            kind: StartupAnimationKind::Thinking,
            start: Instant::now(),
        };
        let copied_animation = animation;

        assert_eq!(copied_animation, animation);
    }

    #[test]
    fn animation_tick_changes_gradient() {
        assert_ne!(
            gradient_for_animation_tick(/*bg*/ None, /*tick*/ 0),
            gradient_for_animation_tick(/*bg*/ None, /*tick*/ 1)
        );
    }
}
