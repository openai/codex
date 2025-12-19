//! Scroll normalization for mouse wheel/trackpad input.
//!
//! Terminal scroll events vary widely in event counts per wheel tick, and inter-event timing
//! overlaps heavily between wheel and trackpad input. We normalize scroll input by treating
//! events as short streams separated by gaps, converting events into line deltas with a
//! per-terminal events-per-line factor, and classifying streams as discrete or continuous
//! after they end.
//!
//! Discrete streams apply their total line delta when the stream closes (with a minimum of
//! one line when rounding to zero), scaled by a per-tick wheel multiplier so a single
//! wheel notch retains the classic multi-line feel. Continuous streams accumulate fractional
//! lines and flush them at a ~60 Hz cadence while the stream is active.
//!
//! See `codex-rs/tui2/docs/scroll_input_model.md` for the data-derived constants and analysis.

use codex_core::terminal::TerminalInfo;
use codex_core::terminal::TerminalName;
use std::time::Duration;
use std::time::Instant;

const STREAM_GAP_MS: u64 = 80;
const STREAM_GAP: Duration = Duration::from_millis(STREAM_GAP_MS);
const DISCRETE_MAX_EVENTS: usize = 10;
const DISCRETE_MAX_DURATION_MS: u64 = 250;
const REDRAW_CADENCE_MS: u64 = 16;
const REDRAW_CADENCE: Duration = Duration::from_millis(REDRAW_CADENCE_MS);
const DEFAULT_EVENTS_PER_LINE: u16 = 3;
const DEFAULT_WHEEL_LINES_PER_TICK: u16 = 3;
const MAX_EVENTS_PER_STREAM: usize = 256;
const MAX_ACCUMULATED_LINES: i32 = 256;
const MIN_LINES_PER_DISCRETE_STREAM: i32 = 1;

/// High-level scroll direction used to sign line deltas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ScrollDirection {
    Up,
    Down,
}

impl ScrollDirection {
    fn sign(self) -> i32 {
        match self {
            ScrollDirection::Up => -1,
            ScrollDirection::Down => 1,
        }
    }

    fn inverted(self) -> Self {
        match self {
            ScrollDirection::Up => ScrollDirection::Down,
            ScrollDirection::Down => ScrollDirection::Up,
        }
    }
}

/// Scroll normalization settings derived from terminal metadata and user overrides.
///
/// These are the knobs used by [`MouseScrollState`] to translate raw `ScrollUp`/`ScrollDown`
/// events into deltas in *visual lines* for the transcript viewport.
///
/// - `events_per_line` normalizes per-terminal "event density" (how many raw events correspond to
///   one unit of scroll movement).
/// - `wheel_lines_per_tick` scales short, discrete streams so a single mouse wheel notch retains
///   the classic multi-line feel.
///
/// See `codex-rs/tui2/docs/scroll_input_model.md` for the probe data and rationale.
/// User-facing overrides are exposed via `config.toml` as:
/// - `tui.scroll_events_per_line`
/// - `tui.scroll_wheel_lines`
/// - `tui.scroll_invert`
#[derive(Clone, Copy, Debug)]
pub(crate) struct ScrollConfig {
    /// Per-terminal normalization factor ("events per line").
    ///
    /// Each raw scroll event contributes `1 / events_per_line` visual lines before any other
    /// scaling. Larger values make scrolling slower; smaller values make it faster.
    events_per_line: u16,
    /// Lines applied per mouse wheel tick for discrete streams.
    ///
    /// This multiplier is only applied when a stream is classified as *discrete* (wheel-like burst)
    /// to avoid accelerating continuous scrolling (trackpads).
    ///
    /// Note: very small trackpad gestures that look like discrete bursts may also be affected.
    wheel_lines_per_tick: u16,
    /// Invert the sign of vertical scroll direction.
    ///
    /// We do not attempt to infer terminal-level inversion settings; this is an explicit
    /// application-level toggle.
    invert_direction: bool,
}

impl ScrollConfig {
    /// Derive scroll normalization defaults from detected terminal metadata.
    ///
    /// This uses [`TerminalInfo`] (in particular [`TerminalName`]) to pick an empirically derived
    /// `events_per_line` default. Users can override both `events_per_line` and the per-wheel-tick
    /// multiplier via `config.toml` (see [`ScrollConfig`] docs).
    pub(crate) fn from_terminal(
        terminal: &TerminalInfo,
        events_per_line_override: Option<u16>,
        wheel_lines_override: Option<u16>,
        invert_direction: bool,
    ) -> Self {
        let mut events_per_line = match terminal.name {
            TerminalName::AppleTerminal => 3,
            TerminalName::WarpTerminal => 9,
            TerminalName::WezTerm => 1,
            TerminalName::Alacritty => 3,
            TerminalName::Ghostty => 9,
            TerminalName::Iterm2 => 1,
            TerminalName::VsCode => 1,
            TerminalName::Kitty => 3,
            _ => DEFAULT_EVENTS_PER_LINE,
        };

        if let Some(override_value) = events_per_line_override {
            events_per_line = override_value.max(1);
        }

        let mut wheel_lines_per_tick = DEFAULT_WHEEL_LINES_PER_TICK;
        if let Some(override_value) = wheel_lines_override {
            wheel_lines_per_tick = override_value.max(1);
        }

        Self {
            events_per_line,
            wheel_lines_per_tick,
            invert_direction,
        }
    }

    fn events_per_line_f32(self) -> f32 {
        self.events_per_line.max(1) as f32
    }

    fn wheel_lines_per_tick_i32(self) -> i32 {
        self.wheel_lines_per_tick.max(1) as i32
    }

    fn apply_direction(self, direction: ScrollDirection) -> ScrollDirection {
        if self.invert_direction {
            direction.inverted()
        } else {
            direction
        }
    }
}

impl Default for ScrollConfig {
    fn default() -> Self {
        Self {
            events_per_line: DEFAULT_EVENTS_PER_LINE,
            wheel_lines_per_tick: DEFAULT_WHEEL_LINES_PER_TICK,
            invert_direction: false,
        }
    }
}

/// Output from scroll handling: lines to apply plus when to check for stream end.
///
/// The caller should apply `lines` immediately. If `next_tick_in` is `Some`, schedule a follow-up
/// tick (typically by requesting a frame) so [`MouseScrollState::on_tick`] can close the stream
/// after a period of silence.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ScrollUpdate {
    pub(crate) lines: i32,
    pub(crate) next_tick_in: Option<Duration>,
}

/// Tracks active scroll input streams and coalesces redraws to a fixed cadence.
///
/// Typical usage:
/// - Call [`MouseScrollState::on_scroll_event`] for each vertical scroll event.
/// - Apply the returned [`ScrollUpdate::lines`] to the transcript scroll state.
/// - If [`ScrollUpdate::next_tick_in`] is present, schedule a delayed tick and call
///   [`MouseScrollState::on_tick`] to close the stream after it goes idle.
#[derive(Clone, Debug)]
pub(crate) struct MouseScrollState {
    stream: Option<ScrollStream>,
    last_redraw_at: Instant,
}

impl MouseScrollState {
    fn new_at(now: Instant) -> Self {
        Self {
            stream: None,
            last_redraw_at: now,
        }
    }

    /// Handle a scroll event using the current time.
    pub(crate) fn on_scroll_event(
        &mut self,
        direction: ScrollDirection,
        config: ScrollConfig,
    ) -> ScrollUpdate {
        self.on_scroll_event_at(Instant::now(), direction, config)
    }

    /// Handle a scroll event at a specific time (for tests).
    pub(crate) fn on_scroll_event_at(
        &mut self,
        now: Instant,
        direction: ScrollDirection,
        config: ScrollConfig,
    ) -> ScrollUpdate {
        let direction = config.apply_direction(direction);
        let mut lines = 0;

        if let Some(mut stream) = self.stream.take() {
            let gap = now.duration_since(stream.last);
            if gap > STREAM_GAP || stream.direction != direction {
                lines += self.finalize_stream_at(now, &mut stream);
            } else {
                self.stream = Some(stream);
            }
        }

        let stream = self.stream.get_or_insert_with(|| {
            ScrollStream::new(now, direction, config.wheel_lines_per_tick_i32())
        });
        stream.push_event(now, direction, config.events_per_line_f32());

        if now.duration_since(self.last_redraw_at) >= REDRAW_CADENCE {
            lines += Self::flush_lines_at(&mut self.last_redraw_at, now, stream);
        }

        ScrollUpdate {
            lines,
            next_tick_in: self.next_tick_in(now),
        }
    }

    /// Check whether an active stream has ended based on the current time.
    pub(crate) fn on_tick(&mut self) -> ScrollUpdate {
        self.on_tick_at(Instant::now())
    }

    /// Check whether an active stream has ended at a specific time (for tests).
    pub(crate) fn on_tick_at(&mut self, now: Instant) -> ScrollUpdate {
        let mut lines = 0;
        if let Some(mut stream) = self.stream.take() {
            let gap = now.duration_since(stream.last);
            if gap > STREAM_GAP {
                lines = self.finalize_stream_at(now, &mut stream);
            } else {
                // No new events, but we may still have accumulated enough fractional scroll to
                // apply additional whole lines. Flushing on a fixed cadence prevents a "late jump"
                // when the stream finally closes (which users perceive as overshoot).
                if now.duration_since(self.last_redraw_at) >= REDRAW_CADENCE {
                    lines = Self::flush_lines_at(&mut self.last_redraw_at, now, &mut stream);
                }
                self.stream = Some(stream);
            }
        }

        ScrollUpdate {
            lines,
            next_tick_in: self.next_tick_in(now),
        }
    }

    fn finalize_stream_at(&mut self, now: Instant, stream: &mut ScrollStream) -> i32 {
        let duration_ms = stream.last.duration_since(stream.start).as_millis() as u64;
        let discrete =
            stream.event_count <= DISCRETE_MAX_EVENTS && duration_ms <= DISCRETE_MAX_DURATION_MS;
        if discrete {
            Self::flush_discrete_at(&mut self.last_redraw_at, now, stream)
        } else {
            Self::flush_lines_at(&mut self.last_redraw_at, now, stream)
        }
    }

    fn flush_lines_at(
        last_redraw_at: &mut Instant,
        now: Instant,
        stream: &mut ScrollStream,
    ) -> i32 {
        let mut lines = stream.accumulated_lines.trunc() as i32;
        if lines == 0 {
            return 0;
        }

        let clamped = lines.clamp(-MAX_ACCUMULATED_LINES, MAX_ACCUMULATED_LINES);
        stream.applied_lines = stream.applied_lines.saturating_add(clamped);
        stream.accumulated_lines -= clamped as f32;
        *last_redraw_at = now;
        clamped
    }

    fn flush_discrete_at(
        last_redraw_at: &mut Instant,
        now: Instant,
        stream: &mut ScrollStream,
    ) -> i32 {
        let mut total_lines = stream.applied_lines + stream.accumulated_lines.trunc() as i32;
        if total_lines == 0 && stream.accumulated_events != 0 {
            total_lines = stream.accumulated_events.signum() * MIN_LINES_PER_DISCRETE_STREAM;
        }

        let scaled_total = total_lines.saturating_mul(stream.wheel_lines_per_tick);
        let mut delta = scaled_total - stream.applied_lines;
        if delta == 0 {
            return 0;
        }

        delta = delta.clamp(-MAX_ACCUMULATED_LINES, MAX_ACCUMULATED_LINES);
        stream.applied_lines = stream.applied_lines.saturating_add(delta);
        stream.accumulated_lines = 0.0;
        *last_redraw_at = now;
        delta
    }

    fn next_tick_in(&self, now: Instant) -> Option<Duration> {
        let stream = self.stream.as_ref()?;
        let gap = now.duration_since(stream.last);
        if gap > STREAM_GAP {
            return None;
        }

        let mut next = STREAM_GAP.saturating_sub(gap);

        // If we've accumulated at least one whole line but haven't flushed yet (because the last
        // event arrived before the redraw cadence elapsed), schedule an earlier tick so we can
        // flush promptly.
        if stream.accumulated_lines.trunc() as i32 != 0 {
            let since_redraw = now.duration_since(self.last_redraw_at);
            let until_redraw = if since_redraw >= REDRAW_CADENCE {
                Duration::from_millis(0)
            } else {
                REDRAW_CADENCE.saturating_sub(since_redraw)
            };
            next = next.min(until_redraw);
        }

        Some(next)
    }
}

impl Default for MouseScrollState {
    fn default() -> Self {
        Self::new_at(Instant::now())
    }
}

#[derive(Clone, Debug)]
struct ScrollStream {
    start: Instant,
    last: Instant,
    direction: ScrollDirection,
    event_count: usize,
    accumulated_events: i32,
    accumulated_lines: f32,
    applied_lines: i32,
    wheel_lines_per_tick: i32,
}

impl ScrollStream {
    fn new(now: Instant, direction: ScrollDirection, wheel_lines_per_tick: i32) -> Self {
        Self {
            start: now,
            last: now,
            direction,
            event_count: 0,
            accumulated_events: 0,
            accumulated_lines: 0.0,
            applied_lines: 0,
            wheel_lines_per_tick: wheel_lines_per_tick.max(1),
        }
    }

    fn push_event(&mut self, now: Instant, direction: ScrollDirection, events_per_line: f32) {
        self.last = now;
        self.direction = direction;
        self.event_count = self
            .event_count
            .saturating_add(1)
            .min(MAX_EVENTS_PER_STREAM);
        self.accumulated_events = (self.accumulated_events + direction.sign()).clamp(
            -(MAX_EVENTS_PER_STREAM as i32),
            MAX_EVENTS_PER_STREAM as i32,
        );
        self.accumulated_lines =
            (self.accumulated_lines + (direction.sign() as f32 / events_per_line)).clamp(
                -(MAX_ACCUMULATED_LINES as f32),
                MAX_ACCUMULATED_LINES as f32,
            );
    }
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
    fn terminal_overrides_match_probe_defaults() {
        let wezterm = ScrollConfig::from_terminal(
            &terminal_info_named(TerminalName::WezTerm),
            None,
            None,
            false,
        );
        let warp = ScrollConfig::from_terminal(
            &terminal_info_named(TerminalName::WarpTerminal),
            None,
            None,
            false,
        );
        let unknown = ScrollConfig::from_terminal(
            &terminal_info_named(TerminalName::Unknown),
            None,
            None,
            false,
        );

        assert_eq!(wezterm.events_per_line, 1);
        assert_eq!(wezterm.wheel_lines_per_tick, DEFAULT_WHEEL_LINES_PER_TICK);
        assert_eq!(warp.events_per_line, 9);
        assert_eq!(unknown.events_per_line, DEFAULT_EVENTS_PER_LINE);
    }

    #[test]
    fn discrete_stream_applies_min_line_after_gap() {
        let config = ScrollConfig::from_terminal(
            &terminal_info_named(TerminalName::AppleTerminal),
            Some(3),
            None,
            false,
        );
        let base = Instant::now();
        let mut state = MouseScrollState::new_at(base);

        let update = state.on_scroll_event_at(
            base + Duration::from_millis(1),
            ScrollDirection::Down,
            config,
        );
        assert_eq!(
            update,
            ScrollUpdate {
                lines: 0,
                next_tick_in: Some(Duration::from_millis(STREAM_GAP_MS)),
            }
        );

        let update = state.on_tick_at(base + Duration::from_millis(STREAM_GAP_MS + 2));
        assert_eq!(
            update,
            ScrollUpdate {
                lines: 3,
                next_tick_in: None,
            }
        );
    }

    #[test]
    fn wheel_lines_override_scales_discrete_stream() {
        let config = ScrollConfig::from_terminal(
            &terminal_info_named(TerminalName::AppleTerminal),
            Some(3),
            Some(2),
            false,
        );
        let base = Instant::now();
        let mut state = MouseScrollState::new_at(base);

        let update = state.on_scroll_event_at(
            base + Duration::from_millis(1),
            ScrollDirection::Down,
            config,
        );
        assert_eq!(
            update,
            ScrollUpdate {
                lines: 0,
                next_tick_in: Some(Duration::from_millis(STREAM_GAP_MS)),
            }
        );

        let update = state.on_tick_at(base + Duration::from_millis(STREAM_GAP_MS + 2));
        assert_eq!(
            update,
            ScrollUpdate {
                lines: 2,
                next_tick_in: None,
            }
        );
    }

    #[test]
    fn direction_flip_closes_previous_stream() {
        let config = ScrollConfig::from_terminal(
            &terminal_info_named(TerminalName::AppleTerminal),
            Some(3),
            None,
            false,
        );
        let base = Instant::now();
        let mut state = MouseScrollState::new_at(base);

        let _ =
            state.on_scroll_event_at(base + Duration::from_millis(1), ScrollDirection::Up, config);
        let update = state.on_scroll_event_at(
            base + Duration::from_millis(2),
            ScrollDirection::Down,
            config,
        );

        assert_eq!(
            update,
            ScrollUpdate {
                lines: -3,
                next_tick_in: Some(Duration::from_millis(STREAM_GAP_MS)),
            }
        );
    }

    #[test]
    fn continuous_stream_coalesces_redraws() {
        let config = ScrollConfig::from_terminal(
            &terminal_info_named(TerminalName::AppleTerminal),
            Some(1),
            None,
            false,
        );
        let base = Instant::now();
        let mut state = MouseScrollState::new_at(base);

        let first = state.on_scroll_event_at(
            base + Duration::from_millis(1),
            ScrollDirection::Down,
            config,
        );
        let second = state.on_scroll_event_at(
            base + Duration::from_millis(10),
            ScrollDirection::Down,
            config,
        );
        let third = state.on_scroll_event_at(
            base + Duration::from_millis(20),
            ScrollDirection::Down,
            config,
        );

        assert_eq!(
            first,
            ScrollUpdate {
                lines: 0,
                next_tick_in: Some(Duration::from_millis(REDRAW_CADENCE_MS - 1)),
            }
        );
        assert_eq!(
            second,
            ScrollUpdate {
                lines: 0,
                next_tick_in: Some(Duration::from_millis(REDRAW_CADENCE_MS - 10)),
            }
        );
        assert_eq!(
            third,
            ScrollUpdate {
                lines: 3,
                next_tick_in: Some(Duration::from_millis(STREAM_GAP_MS)),
            }
        );
    }

    #[test]
    fn invert_direction_flips_sign() {
        let config = ScrollConfig::from_terminal(
            &terminal_info_named(TerminalName::AppleTerminal),
            Some(1),
            None,
            true,
        );
        let base = Instant::now();
        let mut state = MouseScrollState::new_at(base);

        let update = state.on_scroll_event_at(
            base + Duration::from_millis(REDRAW_CADENCE_MS + 1),
            ScrollDirection::Up,
            config,
        );

        assert_eq!(
            update,
            ScrollUpdate {
                lines: 1,
                next_tick_in: Some(Duration::from_millis(STREAM_GAP_MS)),
            }
        );
    }
}
