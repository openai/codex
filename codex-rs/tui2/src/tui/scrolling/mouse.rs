//! Mouse scroll tuning based on terminal cadence.
//!
//! The mouse wheel and trackpad can look wildly different once their events reach the terminal.
//! This module keeps the logic local to tui2 and lets us adjust line deltas using timing
//! heuristics and terminal-specific defaults.
//!
//! Heuristics:
//! - Fast bursts (sub-5ms) are treated like mouse wheel bursts on some terminals and can be
//!   divided down (Ghostty) so a single notch feels natural. Burst detection uses a short rolling
//!   window (3 intervals) so a single quick event never suppresses scrolling.
//! - Frame-rate cadence (roughly 16-20ms) is used as a proxy for trackpad-like scrolling in
//!   terminals that clamp or batch events.
//! - Slow events are treated as single-event trackpad gestures and map to one line, since a lone
//!   event should not be amplified without stronger evidence of wheel input.
//!
//! The values are intentionally conservative and are meant to be refined as we learn more about
//! each terminal's event patterns.

use codex_core::terminal::TerminalInfo;
use codex_core::terminal::TerminalName;
use std::time::Duration;
use std::time::Instant;
use tracing::trace;

/// Number of inter-event intervals used for burst detection.
const INTERVAL_WINDOW: usize = 3;

/// High-level scroll direction used to sign line deltas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ScrollDirection {
    Up,
    Down,
}

impl ScrollDirection {
    fn signed_lines(self, lines: i32) -> i32 {
        match self {
            ScrollDirection::Up => -lines,
            ScrollDirection::Down => lines,
        }
    }
}

/// Terminal-specific tuning parameters for mouse scroll deltas.
///
/// The tuning values are derived from observed terminal behavior:
/// - `fast_threshold` gates burst detection for wheel-like events.
/// - `frame_threshold` captures frame-rate cadence from clamped event streams.
/// - `fast_lines` and `fast_divisor` determine how burst events are downscaled.
/// - `frame_lines` and `slow_lines` define the line count for frame-rate and slower events
///   respectively (used as trackpad vs. wheel heuristics).
#[derive(Clone, Copy, Debug)]
pub(crate) struct ScrollTuning {
    /// Duration that qualifies as a fast burst.
    fast_threshold: Duration,

    /// Duration threshold for frame-rate cadence detection.
    frame_threshold: Duration,

    /// Line count to accumulate during fast burst handling.
    fast_lines: i32,

    /// Divisor applied to accumulated fast burst lines.
    fast_divisor: i32,

    /// Line count for events within the frame-rate cadence window.
    frame_lines: i32,

    /// Line count for slower events outside the frame-rate cadence window.
    slow_lines: i32,
}

impl ScrollTuning {
    /// Selects terminal-specific tuning based on detected terminal metadata.
    ///
    /// Defaults are used for unknown terminals. The goal is to keep the heuristics
    /// understandable and easy to tweak as new observations come in.
    pub(crate) fn for_terminal(terminal: &TerminalInfo) -> Self {
        match terminal.name {
            // Wheel: ~9 events in ~0.5-3ms bursts.
            // Trackpad: closer to frame cadence.
            // Outcome: divide fast bursts to avoid triple speed.
            TerminalName::Ghostty => Self {
                fast_threshold: Duration::from_millis(5),
                frame_threshold: Duration::from_millis(20),
                fast_lines: 1,
                fast_divisor: 3,
                frame_lines: 1,
                slow_lines: 1,
            },
            // Wheel: clamped to ~16ms cadence.
            // Trackpad: larger gaps than frame cadence.
            // Outcome: upscale frame events; keep slower single events at 1 line.
            TerminalName::VsCode => Self {
                fast_threshold: Duration::from_millis(5),
                frame_threshold: Duration::from_millis(20),
                fast_lines: 1,
                fast_divisor: 1,
                frame_lines: 3,
                slow_lines: 1,
            },
            // Wheel: 3 events per notch.
            // Trackpad: typical cadence.
            // Outcome: defaults yield natural 3-line steps and sane trackpad feel.
            TerminalName::AppleTerminal => Self::default(),
            // Wheel: often single event, but slower than frame cadence.
            // Trackpad: cadence clusters around ~16-17ms.
            // Outcome: frame events stay at 1 line; slower events map to 3 lines when a short
            // sequence is detected, while isolated slow events stay at 1 line.
            TerminalName::Iterm2 => Self {
                fast_threshold: Duration::from_millis(5),
                frame_threshold: Duration::from_millis(20),
                fast_lines: 1,
                fast_divisor: 1,
                frame_lines: 1,
                slow_lines: 3,
            },
            // Wheel: ~3+ event bursts.
            // Trackpad: ~50ms slow, ~2-10ms fast (some effectively simultaneous).
            // Outcome: defaults for now.
            TerminalName::Kitty => Self::default(),
            // Wheel: 2-3 sub-ms events.
            // Trackpad: slow looks normal, fast batches around 7-8ms gaps.
            // Outcome: defaults for now.
            TerminalName::Alacritty => Self::default(),
            // Wheel: single events, fast ~6-10ms.
            // Trackpad: single events, fast ~1-10ms.
            // Outcome: defaults; likely need config or upstream option to disambiguate.
            TerminalName::WezTerm => Self::default(),
            _ => Self::default(),
        }
    }
}

/// Baseline tuning for terminals without specific overrides.
impl Default for ScrollTuning {
    fn default() -> Self {
        Self {
            fast_threshold: Duration::from_millis(5),
            frame_threshold: Duration::from_millis(20),
            fast_lines: 1,
            fast_divisor: 1,
            frame_lines: 1,
            slow_lines: 1,
        }
    }
}

/// Tracks recent mouse scroll events so we can interpret bursts consistently.
///
/// The state stores the last event timestamp and direction so elapsed time is computed only for
/// successive events in the same direction. A remainder is stored for burst division so multiple
/// fast events can accumulate into a single scroll line over time.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct MouseScrollState {
    /// Timestamp of the last scroll event.
    last_event_at: Option<Instant>,

    /// Direction of the last scroll event.
    last_direction: Option<ScrollDirection>,

    /// Remainder lines accumulated while dividing burst events.
    fast_remainder: i32,

    /// Recently observed inter-event intervals used for burst detection.
    intervals: [Duration; INTERVAL_WINDOW],

    /// Number of valid intervals stored in the rolling buffer.
    interval_count: usize,

    /// Next insertion index for the rolling interval buffer.
    interval_index: usize,

    /// Number of consecutive intervals within the sequence window.
    sequence_count: usize,
}

impl MouseScrollState {
    /// Computes a signed line delta using the current time.
    ///
    /// This is the entry point for production code; it timestamps the event,
    /// evaluates burst cadence, and returns a signed line count to apply.
    pub(crate) fn delta_lines(&mut self, direction: ScrollDirection, tuning: ScrollTuning) -> i32 {
        self.delta_lines_at(Instant::now(), direction, tuning)
    }

    /// Computes a signed line delta using an injected timestamp.
    ///
    /// Callers supply the timestamp to simulate event cadence in tests. The method
    /// updates internal state, applies burst division when events are tightly
    /// clustered, and falls back to frame/slow thresholds for single events.
    pub(crate) fn delta_lines_at(
        &mut self,
        now: Instant,
        direction: ScrollDirection,
        tuning: ScrollTuning,
    ) -> i32 {
        let cadence = self.cadence(now, direction, tuning);

        if cadence.is_burst && tuning.fast_divisor > 1 {
            let total = self.fast_remainder + tuning.fast_lines;
            let lines = total / tuning.fast_divisor;
            self.fast_remainder = total % tuning.fast_divisor;
            let signed_lines = if lines == 0 {
                0
            } else {
                direction.signed_lines(lines)
            };
            self.trace_scroll(
                direction,
                tuning,
                cadence,
                total,
                lines,
                signed_lines,
                "burst",
            );
            return signed_lines;
        }

        self.fast_remainder = 0;

        let elapsed = cadence.elapsed.unwrap_or(Duration::MAX);
        let lines = if elapsed <= tuning.frame_threshold {
            tuning.frame_lines
        } else if cadence.has_sequence {
            tuning.slow_lines
        } else {
            1
        };

        let signed_lines = direction.signed_lines(lines);
        let reason = if elapsed <= tuning.frame_threshold {
            "frame"
        } else if cadence.has_sequence {
            "sequence"
        } else {
            "single"
        };
        self.trace_scroll(direction, tuning, cadence, 0, lines, signed_lines, reason);
        signed_lines
    }

    /// Updates the cadence buffer and returns elapsed/burst classification for the new event.
    fn cadence(
        &mut self,
        now: Instant,
        direction: ScrollDirection,
        tuning: ScrollTuning,
    ) -> Cadence {
        let elapsed = match (self.last_event_at, self.last_direction) {
            (Some(last_event_at), Some(last_direction)) if last_direction == direction => {
                Some(now.duration_since(last_event_at))
            }
            _ => {
                self.reset_intervals();
                None
            }
        };

        if let Some(elapsed) = elapsed {
            let sequence_window = tuning.frame_threshold.saturating_mul(4);
            if elapsed <= sequence_window {
                self.sequence_count = self.sequence_count.saturating_add(1);
            } else {
                self.sequence_count = 0;
            }

            if elapsed <= tuning.frame_threshold {
                self.push_interval(elapsed);
            } else {
                self.reset_intervals();
            }
        } else {
            self.sequence_count = 0;
        }

        self.last_event_at = Some(now);
        self.last_direction = Some(direction);

        let is_burst = self.interval_count >= 2
            && self
                .median_interval()
                .is_some_and(|median| median <= tuning.fast_threshold);
        let has_sequence = self.sequence_count >= 2;

        Cadence {
            elapsed,
            is_burst,
            has_sequence,
        }
    }

    fn reset_intervals(&mut self) {
        self.interval_count = 0;
        self.interval_index = 0;
        self.fast_remainder = 0;
    }

    fn push_interval(&mut self, elapsed: Duration) {
        self.intervals[self.interval_index] = elapsed;
        self.interval_index = (self.interval_index + 1) % INTERVAL_WINDOW;
        self.interval_count = self.interval_count.saturating_add(1).min(INTERVAL_WINDOW);
    }

    fn median_interval(&self) -> Option<Duration> {
        let count = self.interval_count;
        if count == 0 {
            return None;
        }

        let mut intervals = self.intervals;
        intervals[..count].sort_unstable();
        Some(intervals[count / 2])
    }

    fn trace_scroll(
        &self,
        direction: ScrollDirection,
        tuning: ScrollTuning,
        cadence: Cadence,
        burst_total: i32,
        lines: i32,
        signed_lines: i32,
        reason: &'static str,
    ) {
        trace!(
            target: "tui2::scrolling",
            direction = ?direction,
            reason,
            elapsed_ms = cadence.elapsed.map(|elapsed| elapsed.as_millis()),
            fast_threshold_ms = tuning.fast_threshold.as_millis(),
            frame_threshold_ms = tuning.frame_threshold.as_millis(),
            interval_count = self.interval_count,
            sequence_count = self.sequence_count,
            is_burst = cadence.is_burst,
            has_sequence = cadence.has_sequence,
            burst_total,
            lines,
            signed_lines,
            "scroll cadence",
        );
    }
}

/// Cadence details for the most recent event.
#[derive(Clone, Copy, Debug)]
struct Cadence {
    /// Duration between the new event and the previous one when the direction matches.
    elapsed: Option<Duration>,
    /// Whether the rolling window indicates a sustained burst.
    is_burst: bool,
    /// Whether recent events form a short sequence that supports wheel inference.
    has_sequence: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn terminal_info_named(name: TerminalName) -> TerminalInfo {
        TerminalInfo {
            name,
            term_program: None,
            version: None,
            term: None,
            multiplexer: None,
        }
    }

    #[test]
    fn ghostty_fast_scroll_divides_bursts() {
        let tuning = ScrollTuning::for_terminal(&terminal_info_named(TerminalName::Ghostty));
        let base = Instant::now();
        let mut state = MouseScrollState {
            last_event_at: Some(base),
            last_direction: Some(ScrollDirection::Up),
            ..MouseScrollState::default()
        };

        let delta_one =
            state.delta_lines_at(base + Duration::from_millis(1), ScrollDirection::Up, tuning);
        let delta_two =
            state.delta_lines_at(base + Duration::from_millis(2), ScrollDirection::Up, tuning);
        let delta_three =
            state.delta_lines_at(base + Duration::from_millis(3), ScrollDirection::Up, tuning);
        let delta_four =
            state.delta_lines_at(base + Duration::from_millis(4), ScrollDirection::Up, tuning);

        assert_eq!(delta_one, -1);
        assert_eq!(delta_two, 0);
        assert_eq!(delta_three, 0);
        assert_eq!(delta_four, -1);
    }

    #[test]
    fn iterm_scroll_uses_frame_and_slow_thresholds() {
        let tuning = ScrollTuning::for_terminal(&terminal_info_named(TerminalName::Iterm2));
        let base = Instant::now();
        let mut state = MouseScrollState {
            last_event_at: Some(base),
            last_direction: Some(ScrollDirection::Down),
            ..MouseScrollState::default()
        };

        let frame_delta = state.delta_lines_at(
            base + Duration::from_millis(16),
            ScrollDirection::Down,
            tuning,
        );
        let slow_delta = state.delta_lines_at(
            base + Duration::from_millis(40),
            ScrollDirection::Down,
            tuning,
        );

        assert_eq!(frame_delta, 1);
        assert_eq!(slow_delta, 3);
    }

    #[test]
    fn iterm_single_slow_event_stays_one_line() {
        let tuning = ScrollTuning::for_terminal(&terminal_info_named(TerminalName::Iterm2));
        let base = Instant::now();
        let mut state = MouseScrollState {
            last_event_at: Some(base),
            last_direction: Some(ScrollDirection::Down),
            ..MouseScrollState::default()
        };

        let slow_delta = state.delta_lines_at(
            base + Duration::from_millis(40),
            ScrollDirection::Down,
            tuning,
        );

        assert_eq!(slow_delta, 1);
    }

    #[test]
    fn vscode_scroll_uses_frame_and_slow_thresholds() {
        let tuning = ScrollTuning::for_terminal(&terminal_info_named(TerminalName::VsCode));
        let base = Instant::now();
        let mut state = MouseScrollState {
            last_event_at: Some(base),
            last_direction: Some(ScrollDirection::Down),
            ..MouseScrollState::default()
        };

        let frame_delta = state.delta_lines_at(
            base + Duration::from_millis(16),
            ScrollDirection::Down,
            tuning,
        );
        let slow_delta = state.delta_lines_at(
            base + Duration::from_millis(40),
            ScrollDirection::Down,
            tuning,
        );

        assert_eq!(frame_delta, 3);
        assert_eq!(slow_delta, 1);
    }
}
