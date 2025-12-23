//! Scroll normalization for mouse wheel/trackpad input.
//!
//! Terminal scroll events vary widely in event counts per wheel tick, and inter-event timing
//! overlaps heavily between wheel and trackpad input. We normalize scroll input by treating
//! events as short streams separated by gaps, converting events into line deltas with a
//! per-terminal events-per-tick factor, and coalescing redraw to a fixed cadence.
//!
//! A mouse wheel "tick" (one notch) is expected to scroll by a fixed number of lines (default: 3)
//! regardless of the terminal's raw event density. Trackpad scrolling should remain higher
//! fidelity (small movements can result in sub-line accumulation that only scrolls once whole
//! lines are reached).
//!
//! Because terminal mouse scroll events do not encode magnitude (only direction), wheel-vs-trackpad
//! detection is heuristic. We bias toward treating input as trackpad-like (to avoid overshoot) and
//! "promote" to wheel-like when the first tick-worth of events arrives quickly. A user can always
//! force wheel/trackpad behavior via config if the heuristic is wrong for their setup.
//!
//! See `codex-rs/tui2/docs/scroll_input_model.md` for the data-derived constants and analysis.

use codex_core::config::types::ScrollInputMode;
use codex_core::terminal::TerminalInfo;
use codex_core::terminal::TerminalName;
use std::time::Duration;
use std::time::Instant;

const STREAM_GAP_MS: u64 = 80;
const STREAM_GAP: Duration = Duration::from_millis(STREAM_GAP_MS);
const REDRAW_CADENCE_MS: u64 = 16;
const REDRAW_CADENCE: Duration = Duration::from_millis(REDRAW_CADENCE_MS);
const DEFAULT_EVENTS_PER_TICK: u16 = 3;
const DEFAULT_WHEEL_LINES_PER_TICK: u16 = 3;
const DEFAULT_TRACKPAD_LINES_PER_TICK: u16 = 1;
const DEFAULT_SCROLL_MODE: ScrollInputMode = ScrollInputMode::Auto;
const DEFAULT_WHEEL_TICK_DETECT_MAX_MS: u64 = 12;
const DEFAULT_WHEEL_LIKE_MAX_DURATION_MS: u64 = 200;
const DEFAULT_TRACKPAD_ACCEL_EVENTS: u16 = 30;
const DEFAULT_TRACKPAD_ACCEL_MAX: u16 = 3;
const MAX_EVENTS_PER_STREAM: usize = 256;
const MAX_ACCUMULATED_LINES: i32 = 256;
const MIN_LINES_PER_WHEEL_STREAM: i32 = 1;

fn default_wheel_tick_detect_max_ms_for_terminal(name: TerminalName) -> u64 {
    // This threshold is only used for the "promote to wheel-like" fast path in auto mode.
    // We keep it per-terminal because some terminals emit wheel ticks spread over tens of
    // milliseconds; a tight global threshold causes those wheel ticks to be misclassified as
    // trackpad-like and feel too slow.
    match name {
        TerminalName::WarpTerminal => 20,
        _ => DEFAULT_WHEEL_TICK_DETECT_MAX_MS,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ScrollStreamKind {
    Unknown,
    Wheel,
    Trackpad,
}

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
/// - `tui.scroll_events_per_tick`
/// - `tui.scroll_wheel_lines`
/// - `tui.scroll_invert`
#[derive(Clone, Copy, Debug)]
pub(crate) struct ScrollConfig {
    /// Per-terminal normalization factor ("events per wheel tick").
    ///
    /// Terminals can emit anywhere from ~1 to ~9+ raw events for the same physical wheel notch.
    /// We use this factor to convert raw event counts into a "ticks" estimate.
    ///
    /// Each raw scroll event contributes `1 / events_per_tick` ticks. That tick value is then
    /// scaled to lines depending on the active scroll mode (wheel vs trackpad).
    ///
    /// User-facing name: `tui.scroll_events_per_tick`.
    events_per_tick: u16,

    /// Lines applied per mouse wheel tick.
    ///
    /// When the input is interpreted as wheel-like, one physical wheel notch maps to this many
    /// transcript lines. Default is 3 to match typical "classic terminal" scrolling.
    wheel_lines_per_tick: u16,

    /// Lines applied per tick-equivalent for trackpad scrolling.
    ///
    /// Trackpads do not have discrete "ticks", but terminals still emit discrete up/down events.
    /// We interpret trackpad-like streams as `trackpad_lines_per_tick / events_per_tick` lines per
    /// event and accumulate fractions until they cross a whole line.
    trackpad_lines_per_tick: u16,

    /// Trackpad acceleration: the approximate number of events required to gain +1x speed.
    ///
    /// This is a pragmatic UX knob: in some terminals the vertical event density for trackpad
    /// input can be relatively low, which makes large/faster swipes feel sluggish even when small
    /// swipes feel correct.
    trackpad_accel_events: u16,

    /// Trackpad acceleration: maximum multiplier applied to trackpad-like streams.
    ///
    /// Set to 1 to effectively disable acceleration.
    trackpad_accel_max: u16,

    /// Force wheel/trackpad behavior, or infer it per stream.
    mode: ScrollInputMode,

    /// Auto-mode threshold: how quickly the first wheel tick must complete to be considered wheel.
    ///
    /// This uses the time between the first event of a stream and the moment we have seen
    /// `events_per_tick` events. If the first tick completes faster than this, we promote the
    /// stream to wheel-like. If not, we keep treating it as trackpad-like.
    wheel_tick_detect_max: Duration,

    /// Auto-mode fallback: maximum duration that is still considered "wheel-like".
    ///
    /// If a stream ends before this duration and we couldn't confidently classify it, we treat it
    /// as wheel-like so wheel notches in 1-event-per-tick terminals (WezTerm/iTerm/VS Code) still
    /// get classic multi-line behavior.
    wheel_like_max_duration: Duration,

    /// Invert the sign of vertical scroll direction.
    ///
    /// We do not attempt to infer terminal-level inversion settings; this is an explicit
    /// application-level toggle.
    invert_direction: bool,
}

/// Optional user overrides for scroll configuration.
///
/// Most callers should construct this from the merged [`codex_core::config::Config`] fields so
/// TUI2 inherits terminal defaults and only overrides what the user configured.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ScrollConfigOverrides {
    pub(crate) events_per_tick: Option<u16>,
    pub(crate) wheel_lines_per_tick: Option<u16>,
    pub(crate) trackpad_lines_per_tick: Option<u16>,
    pub(crate) trackpad_accel_events: Option<u16>,
    pub(crate) trackpad_accel_max: Option<u16>,
    pub(crate) mode: Option<ScrollInputMode>,
    pub(crate) wheel_tick_detect_max_ms: Option<u64>,
    pub(crate) wheel_like_max_duration_ms: Option<u64>,
    pub(crate) invert_direction: bool,
}

impl ScrollConfig {
    /// Derive scroll normalization defaults from detected terminal metadata.
    ///
    /// This uses [`TerminalInfo`] (in particular [`TerminalName`]) to pick an empirically derived
    /// `events_per_line` default. Users can override both `events_per_line` and the per-wheel-tick
    /// multiplier via `config.toml` (see [`ScrollConfig`] docs).
    pub(crate) fn from_terminal(terminal: &TerminalInfo, overrides: ScrollConfigOverrides) -> Self {
        let mut events_per_tick = match terminal.name {
            TerminalName::AppleTerminal => 3,
            TerminalName::WarpTerminal => 9,
            TerminalName::WezTerm => 1,
            TerminalName::Alacritty => 3,
            TerminalName::Ghostty => 3,
            TerminalName::Iterm2 => 1,
            TerminalName::VsCode => 1,
            TerminalName::Kitty => 3,
            _ => DEFAULT_EVENTS_PER_TICK,
        };

        if let Some(override_value) = overrides.events_per_tick {
            events_per_tick = override_value.max(1);
        }

        let mut wheel_lines_per_tick = DEFAULT_WHEEL_LINES_PER_TICK;
        if let Some(override_value) = overrides.wheel_lines_per_tick {
            wheel_lines_per_tick = override_value.max(1);
        }

        let mut trackpad_lines_per_tick = DEFAULT_TRACKPAD_LINES_PER_TICK;
        if let Some(override_value) = overrides.trackpad_lines_per_tick {
            trackpad_lines_per_tick = override_value.max(1);
        }

        let mut trackpad_accel_events = DEFAULT_TRACKPAD_ACCEL_EVENTS;
        if let Some(override_value) = overrides.trackpad_accel_events {
            trackpad_accel_events = override_value.max(1);
        }

        let mut trackpad_accel_max = DEFAULT_TRACKPAD_ACCEL_MAX;
        if let Some(override_value) = overrides.trackpad_accel_max {
            trackpad_accel_max = override_value.max(1);
        }

        let wheel_tick_detect_max_ms = overrides
            .wheel_tick_detect_max_ms
            .unwrap_or_else(|| default_wheel_tick_detect_max_ms_for_terminal(terminal.name));
        let wheel_tick_detect_max = Duration::from_millis(wheel_tick_detect_max_ms);
        let wheel_like_max_duration = Duration::from_millis(
            overrides
                .wheel_like_max_duration_ms
                .unwrap_or(DEFAULT_WHEEL_LIKE_MAX_DURATION_MS),
        );

        Self {
            events_per_tick,
            wheel_lines_per_tick,
            trackpad_lines_per_tick,
            trackpad_accel_events,
            trackpad_accel_max,
            mode: overrides.mode.unwrap_or(DEFAULT_SCROLL_MODE),
            wheel_tick_detect_max,
            wheel_like_max_duration,
            invert_direction: overrides.invert_direction,
        }
    }

    fn events_per_tick_f32(self) -> f32 {
        self.events_per_tick.max(1) as f32
    }

    fn wheel_lines_per_tick_f32(self) -> f32 {
        self.wheel_lines_per_tick.max(1) as f32
    }

    fn trackpad_lines_per_tick_f32(self) -> f32 {
        self.trackpad_lines_per_tick.max(1) as f32
    }

    fn trackpad_events_per_tick_f32(self) -> f32 {
        // `events_per_tick` is derived from wheel behavior and can be much larger than the actual
        // trackpad event density for the same physical movement. If we use it directly for
        // trackpads, terminals like Ghostty/Warp can feel artificially slow.
        //
        // We cap at the global "typical" wheel tick size (3) which produces more consistent
        // trackpad feel across terminals while keeping wheel normalization intact.
        self.events_per_tick.clamp(1, DEFAULT_EVENTS_PER_TICK) as f32
    }

    fn trackpad_accel_events_f32(self) -> f32 {
        self.trackpad_accel_events.max(1) as f32
    }

    fn trackpad_accel_max_f32(self) -> f32 {
        self.trackpad_accel_max.max(1) as f32
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
            events_per_tick: DEFAULT_EVENTS_PER_TICK,
            wheel_lines_per_tick: DEFAULT_WHEEL_LINES_PER_TICK,
            trackpad_lines_per_tick: DEFAULT_TRACKPAD_LINES_PER_TICK,
            trackpad_accel_events: DEFAULT_TRACKPAD_ACCEL_EVENTS,
            trackpad_accel_max: DEFAULT_TRACKPAD_ACCEL_MAX,
            mode: DEFAULT_SCROLL_MODE,
            wheel_tick_detect_max: Duration::from_millis(DEFAULT_WHEEL_TICK_DETECT_MAX_MS),
            wheel_like_max_duration: Duration::from_millis(DEFAULT_WHEEL_LIKE_MAX_DURATION_MS),
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

/// Tracks mouse scroll input streams and coalesces redraws.
///
/// This is the state machine that turns discrete terminal scroll events (`ScrollUp`/`ScrollDown`)
/// into viewport line deltas. It implements the stream-based model described in
/// `codex-rs/tui2/docs/scroll_input_model.md`:
///
/// - **Streams**: a sequence of events is treated as one user gesture until a gap larger than
///   [`STREAM_GAP`] or a direction flip closes the stream.
/// - **Normalization**: streams are converted to line deltas using [`ScrollConfig`] (per-terminal
///   `events_per_tick`, per-mode lines-per-tick, and optional invert).
/// - **Coalescing**: trackpad-like streams are flushed at most every [`REDRAW_CADENCE`] to avoid
///   floods in very dense terminals; wheel-like streams flush immediately to feel responsive.
/// - **Follow-up ticks**: because stream closure is defined by a *time gap*, callers must schedule
///   periodic ticks while a stream is active. The returned [`ScrollUpdate::next_tick_in`] provides
///   the next suggested wake-up.
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
    carry_lines: f32,
    carry_direction: Option<ScrollDirection>,
}

impl MouseScrollState {
    /// Create a new scroll state with a deterministic time origin.
    ///
    /// This is primarily used by unit tests so they can control the coalescing and stream-gap
    /// behavior by choosing `now` values. Production code generally uses [`Default`] and the
    /// `Instant::now()`-based entrypoints.
    fn new_at(now: Instant) -> Self {
        Self {
            stream: None,
            last_redraw_at: now,
            carry_lines: 0.0,
            carry_direction: None,
        }
    }

    /// Handle a scroll event using the current time.
    ///
    /// This is the normal production entrypoint used by the TUI event loop. It forwards to
    /// [`MouseScrollState::on_scroll_event_at`] using `Instant::now()`.
    ///
    /// If the returned [`ScrollUpdate::next_tick_in`] is `Some`, callers should schedule a future
    /// tick (typically by requesting a frame) and call [`MouseScrollState::on_tick`] (or
    /// [`MouseScrollState::on_tick_at`] in tests) so we can close the stream after it goes idle.
    /// Without those ticks, streams would only close when a *new* scroll event arrives, which can
    /// leave fractional trackpad scroll unflushed and make stop behavior feel laggy.
    pub(crate) fn on_scroll_event(
        &mut self,
        direction: ScrollDirection,
        config: ScrollConfig,
    ) -> ScrollUpdate {
        self.on_scroll_event_at(Instant::now(), direction, config)
    }

    /// Handle a scroll event at a specific time.
    ///
    /// This is the deterministic entrypoint for the scroll stream state machine. It exists so we
    /// can write unit tests that exercise stream splitting, coalesced redraw, and end-of-stream
    /// flushing without depending on wall-clock time.
    ///
    /// Behavior is identical to [`MouseScrollState::on_scroll_event`], except the caller provides
    /// the timestamp (`now`). In the real app, the timestamp comes from `Instant::now()`.
    ///
    /// Key details (see `codex-rs/tui2/docs/scroll_input_model.md` for the full model):
    ///
    /// - **Stream boundaries**: a gap larger than [`STREAM_GAP`] or a direction flip closes the
    ///   previous stream and starts a new one.
    /// - **Wheel vs trackpad**: the stream kind may be promoted to wheel-like in auto mode when a
    ///   tick-worth of events arrives quickly; otherwise it remains trackpad-like.
    /// - **Redraw coalescing**: wheel-like streams flush immediately; trackpad-like streams flush
    ///   at most every [`REDRAW_CADENCE`].
    /// - **Follow-up ticks**: the returned [`ScrollUpdate::next_tick_in`] tells the caller when it
    ///   should call [`MouseScrollState::on_tick_at`] to close idle streams and flush any remaining
    ///   whole lines. In TUI2 this is wired through the app’s frame scheduler.
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

        if self.stream.is_none() {
            if self.carry_direction != Some(direction) {
                self.carry_lines = 0.0;
                self.carry_direction = Some(direction);
            }
            self.stream = Some(ScrollStream::new(now, direction, config));
        }
        let carry_lines = self.carry_lines;
        let Some(stream) = self.stream.as_mut() else {
            unreachable!("stream inserted above");
        };
        stream.push_event(now, direction);
        stream.maybe_promote_kind(now);

        // Wheel-like scrolling should feel immediate; trackpad-like streams are coalesced to a
        // fixed redraw cadence to avoid floods in very dense terminals.
        if stream.is_wheel_like()
            || now.duration_since(self.last_redraw_at) >= REDRAW_CADENCE
            || stream.just_promoted
        {
            lines += Self::flush_lines_at(&mut self.last_redraw_at, carry_lines, now, stream);
            stream.just_promoted = false;
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
    ///
    /// This should be called even when no new scroll events are arriving, while a stream is still
    /// considered active. It has two roles:
    ///
    /// - **Stream closure**: if the stream has been idle for longer than [`STREAM_GAP`], we close
    ///   it and flush any remaining whole-line scroll.
    /// - **Coalesced flush**: for trackpad-like streams, we also flush on [`REDRAW_CADENCE`] even
    ///   without new events. This avoids a perceived "late jump" when the stream finally closes
    ///   (users interpret that as overshoot).
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
                    lines = Self::flush_lines_at(
                        &mut self.last_redraw_at,
                        self.carry_lines,
                        now,
                        &mut stream,
                    );
                }
                self.stream = Some(stream);
            }
        }

        ScrollUpdate {
            lines,
            next_tick_in: self.next_tick_in(now),
        }
    }

    /// Finalize a stream and update the trackpad carry state.
    ///
    /// Callers invoke this when a stream is known to have ended (gap/direction flip). It forces
    /// a final wheel/trackpad classification for auto mode, flushes any whole-line deltas, and
    /// persists any remaining fractional scroll for trackpad-like streams so the next stream
    /// continues smoothly.
    fn finalize_stream_at(&mut self, now: Instant, stream: &mut ScrollStream) -> i32 {
        stream.finalize_kind();
        let lines = Self::flush_lines_at(&mut self.last_redraw_at, self.carry_lines, now, stream);

        // Preserve sub-line fractional scroll for trackpad-like streams across stream boundaries.
        if stream.kind != ScrollStreamKind::Wheel && stream.config.mode != ScrollInputMode::Wheel {
            self.carry_lines =
                stream.desired_lines_f32(self.carry_lines) - stream.applied_lines as f32;
        } else {
            self.carry_lines = 0.0;
        }

        lines
    }

    /// Compute and apply any newly-reached whole-line deltas for the active stream.
    ///
    /// This converts the stream’s accumulated events to a *desired total line position*,
    /// truncates to whole lines, and returns the delta relative to what has already been applied
    /// for this stream.
    ///
    /// For wheel-like streams we also apply a minimum of ±1 line for any non-zero input so wheel
    /// notches never become "dead" due to rounding or mis-detection.
    fn flush_lines_at(
        last_redraw_at: &mut Instant,
        carry_lines: f32,
        now: Instant,
        stream: &mut ScrollStream,
    ) -> i32 {
        let desired_total = stream.desired_lines_f32(carry_lines);
        let mut desired_lines = desired_total.trunc() as i32;

        // For wheel-mode (or wheel-like streams), ensure at least one line for any non-zero input.
        // This avoids "dead" wheel ticks when `events_per_tick` is mis-detected or overridden.
        if stream.is_wheel_like() && desired_lines == 0 && stream.accumulated_events != 0 {
            desired_lines = stream.accumulated_events.signum() * MIN_LINES_PER_WHEEL_STREAM;
        }

        let mut delta = desired_lines - stream.applied_lines;
        if delta == 0 {
            return 0;
        }

        delta = delta.clamp(-MAX_ACCUMULATED_LINES, MAX_ACCUMULATED_LINES);
        stream.applied_lines = stream.applied_lines.saturating_add(delta);
        *last_redraw_at = now;
        delta
    }

    /// Determine when the caller should next call [`MouseScrollState::on_tick_at`].
    ///
    /// While a stream is active, we need follow-up ticks for two reasons:
    ///
    /// - **Stream closure**: once idle for [`STREAM_GAP`], we finalize the stream.
    /// - **Trackpad coalescing**: if whole lines are pending but we haven't hit
    ///   [`REDRAW_CADENCE`] yet, we schedule an earlier tick so the viewport updates promptly.
    ///
    /// Returning `None` means no stream is active (or it is already past the gap threshold).
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
        let desired_lines = stream.desired_lines_f32(self.carry_lines).trunc() as i32;
        if desired_lines != stream.applied_lines {
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
/// Per-stream state accumulated while the user performs one scroll gesture.
///
/// A "stream" corresponds to one contiguous gesture as defined by [`STREAM_GAP`] (silence) and
/// direction changes. The stream accumulates raw event counts and converts them into a desired
/// total line position via [`ScrollConfig`]. The outer [`MouseScrollState`] then applies only the
/// delta between `desired_total` and `applied_lines` so callers can treat scroll updates as
/// incremental line deltas.
///
/// This type is intentionally not exposed outside this module. The public API is the pair of
/// entrypoints:
///
/// - [`MouseScrollState::on_scroll_event_at`] for new events.
/// - [`MouseScrollState::on_tick_at`] for idle-gap closure and coalesced flush.
///
/// See `codex-rs/tui2/docs/scroll_input_model.md` for the full rationale and probe-derived
/// constants.
struct ScrollStream {
    start: Instant,
    last: Instant,
    direction: ScrollDirection,
    event_count: usize,
    accumulated_events: i32,
    applied_lines: i32,
    config: ScrollConfig,
    kind: ScrollStreamKind,
    first_tick_completed_at: Option<Instant>,
    just_promoted: bool,
}

impl ScrollStream {
    /// Start a new stream at `now`.
    ///
    /// The initial `kind` is [`ScrollStreamKind::Unknown`]. In auto mode, streams begin behaving
    /// like trackpads (to avoid overshoot) until [`ScrollStream::maybe_promote_kind`] promotes the
    /// stream to wheel-like.
    fn new(now: Instant, direction: ScrollDirection, config: ScrollConfig) -> Self {
        Self {
            start: now,
            last: now,
            direction,
            event_count: 0,
            accumulated_events: 0,
            applied_lines: 0,
            config,
            kind: ScrollStreamKind::Unknown,
            first_tick_completed_at: None,
            just_promoted: false,
        }
    }

    /// Record one raw event in the stream.
    ///
    /// This updates the stream's last-seen timestamp, direction, and counters. Counters are
    /// clamped to avoid floods and numeric blowups when terminals emit extremely dense streams.
    fn push_event(&mut self, now: Instant, direction: ScrollDirection) {
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
    }

    /// Promote an auto-mode stream to wheel-like if the first tick completes quickly.
    ///
    /// Terminals often batch a wheel notch into a short burst of `events_per_tick` raw events.
    /// When we observe at least that many events and they arrived within
    /// [`ScrollConfig::wheel_tick_detect_max`], we treat the stream as wheel-like so a notch
    /// scrolls a fixed multi-line amount (classic feel).
    ///
    /// We only attempt this when `events_per_tick >= 2`. In 1-event-per-tick terminals there is
    /// no "tick completion time" signal; auto-mode handles those via
    /// [`ScrollStream::finalize_kind`]'s end-of-stream fallback.
    fn maybe_promote_kind(&mut self, now: Instant) {
        if self.config.mode != ScrollInputMode::Auto {
            return;
        }
        if self.kind != ScrollStreamKind::Unknown {
            return;
        }

        let events_per_tick = self.config.events_per_tick.max(1) as usize;
        if events_per_tick >= 2 && self.event_count >= events_per_tick {
            self.first_tick_completed_at.get_or_insert(now);
            let elapsed = now.duration_since(self.start);
            if elapsed <= self.config.wheel_tick_detect_max {
                self.kind = ScrollStreamKind::Wheel;
                self.just_promoted = true;
            }
        }
    }

    /// Finalize wheel/trackpad classification for the stream.
    ///
    /// In forced modes (`wheel`/`trackpad`), this simply sets the stream kind.
    ///
    /// In auto mode, streams that were not promoted to wheel-like remain trackpad-like, except
    /// for a small end-of-stream fallback for 1-event-per-tick terminals. That fallback treats a
    /// very small, short-lived stream as wheel-like so wheels in WezTerm/iTerm/VS Code still get
    /// the expected multi-line notch behavior.
    fn finalize_kind(&mut self) {
        match self.config.mode {
            ScrollInputMode::Wheel => self.kind = ScrollStreamKind::Wheel,
            ScrollInputMode::Trackpad => self.kind = ScrollStreamKind::Trackpad,
            ScrollInputMode::Auto => {
                if self.kind != ScrollStreamKind::Unknown {
                    return;
                }
                // If we didn't see a fast-completing first tick, we keep treating the stream as
                // trackpad-like. The only exception is terminals that emit 1 event per wheel tick:
                // we can't observe a "tick completion time" there, so we use a conservative
                // end-of-stream fallback for *very small* bursts.
                let duration = self.last.duration_since(self.start);
                if self.config.events_per_tick <= 1
                    && self.event_count <= 2
                    && duration <= self.config.wheel_like_max_duration
                {
                    self.kind = ScrollStreamKind::Wheel;
                } else {
                    self.kind = ScrollStreamKind::Trackpad;
                }
            }
        }
    }

    /// Whether this stream should currently behave like a wheel.
    ///
    /// In auto mode, streams are wheel-like only after we promote them (or after the 1-event
    /// fallback triggers on finalization). While `kind` is still unknown, we treat the stream as
    /// trackpad-like to avoid overshooting.
    fn is_wheel_like(&self) -> bool {
        match self.config.mode {
            ScrollInputMode::Wheel => true,
            ScrollInputMode::Trackpad => false,
            ScrollInputMode::Auto => matches!(self.kind, ScrollStreamKind::Wheel),
        }
    }

    /// The per-mode lines-per-tick scaling factor.
    ///
    /// In auto mode, unknown streams use the trackpad factor until promoted.
    fn effective_lines_per_tick_f32(&self) -> f32 {
        match self.config.mode {
            ScrollInputMode::Wheel => self.config.wheel_lines_per_tick_f32(),
            ScrollInputMode::Trackpad => self.config.trackpad_lines_per_tick_f32(),
            ScrollInputMode::Auto => match self.kind {
                ScrollStreamKind::Wheel => self.config.wheel_lines_per_tick_f32(),
                ScrollStreamKind::Trackpad | ScrollStreamKind::Unknown => {
                    self.config.trackpad_lines_per_tick_f32()
                }
            },
        }
    }

    /// Compute the desired total line position for this stream (including trackpad carry).
    ///
    /// This converts raw event counts into line units using the appropriate divisor and scaling:
    ///
    /// - Wheel-like: `lines = events * (wheel_lines_per_tick / events_per_tick)`
    /// - Trackpad-like: `lines = events * (trackpad_lines_per_tick / min(events_per_tick, 3))`
    ///
    /// For trackpad-like streams we also add `carry_lines` (fractional remainder from previous
    /// streams) and then apply bounded acceleration. The returned value is clamped as a guardrail.
    fn desired_lines_f32(&self, carry_lines: f32) -> f32 {
        let events_per_tick = if self.is_wheel_like() {
            self.config.events_per_tick_f32()
        } else {
            self.config.trackpad_events_per_tick_f32()
        };
        let lines_per_tick = self.effective_lines_per_tick_f32();

        // Note: clamping here is a guardrail; the primary protection is limiting event_count.
        let mut total = (self.accumulated_events as f32 * (lines_per_tick / events_per_tick))
            .clamp(
                -(MAX_ACCUMULATED_LINES as f32),
                MAX_ACCUMULATED_LINES as f32,
            );
        if !self.is_wheel_like() {
            total = (total + carry_lines).clamp(
                -(MAX_ACCUMULATED_LINES as f32),
                MAX_ACCUMULATED_LINES as f32,
            );

            // Trackpad acceleration: keep small swipes precise, but speed up large/fast swipes so
            // they can cover more content. This is intentionally simple and bounded.
            let event_count = self.accumulated_events.abs() as f32;
            let accel = (1.0 + (event_count / self.config.trackpad_accel_events_f32()))
                .clamp(1.0, self.config.trackpad_accel_max_f32());
            total = (total * accel).clamp(
                -(MAX_ACCUMULATED_LINES as f32),
                MAX_ACCUMULATED_LINES as f32,
            );
        }
        total
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
    fn terminal_overrides_match_current_defaults() {
        let wezterm = ScrollConfig::from_terminal(
            &terminal_info_named(TerminalName::WezTerm),
            ScrollConfigOverrides::default(),
        );
        let warp = ScrollConfig::from_terminal(
            &terminal_info_named(TerminalName::WarpTerminal),
            ScrollConfigOverrides::default(),
        );
        let ghostty = ScrollConfig::from_terminal(
            &terminal_info_named(TerminalName::Ghostty),
            ScrollConfigOverrides::default(),
        );
        let unknown = ScrollConfig::from_terminal(
            &terminal_info_named(TerminalName::Unknown),
            ScrollConfigOverrides::default(),
        );

        assert_eq!(wezterm.events_per_tick, 1);
        assert_eq!(wezterm.wheel_lines_per_tick, DEFAULT_WHEEL_LINES_PER_TICK);
        assert_eq!(warp.events_per_tick, 9);
        assert_eq!(ghostty.events_per_tick, 3);
        assert_eq!(unknown.events_per_tick, DEFAULT_EVENTS_PER_TICK);
    }

    #[test]
    fn wheel_tick_scrolls_three_lines_even_when_terminal_emits_three_events() {
        let config = ScrollConfig::from_terminal(
            &terminal_info_named(TerminalName::AppleTerminal),
            ScrollConfigOverrides {
                events_per_tick: Some(3),
                mode: Some(ScrollInputMode::Auto),
                ..ScrollConfigOverrides::default()
            },
        );
        let base = Instant::now();
        let mut state = MouseScrollState::new_at(base);

        // Simulate a single wheel notch in terminals that emit 3 raw events per tick.
        let _ = state.on_scroll_event_at(
            base + Duration::from_millis(1),
            ScrollDirection::Down,
            config,
        );
        let _ = state.on_scroll_event_at(
            base + Duration::from_millis(2),
            ScrollDirection::Down,
            config,
        );
        let update = state.on_scroll_event_at(
            base + Duration::from_millis(3),
            ScrollDirection::Down,
            config,
        );

        assert_eq!(
            update,
            ScrollUpdate {
                lines: 3,
                next_tick_in: Some(Duration::from_millis(STREAM_GAP_MS)),
            }
        );
    }

    #[test]
    fn wheel_tick_scrolls_three_lines_when_terminal_emits_nine_events() {
        let config = ScrollConfig::from_terminal(
            &terminal_info_named(TerminalName::WarpTerminal),
            ScrollConfigOverrides {
                events_per_tick: Some(9),
                mode: Some(ScrollInputMode::Auto),
                ..ScrollConfigOverrides::default()
            },
        );
        let base = Instant::now();
        let mut state = MouseScrollState::new_at(base);

        let mut update = ScrollUpdate::default();
        for idx in 0..9u64 {
            update = state.on_scroll_event_at(
                base + Duration::from_millis(idx + 1),
                ScrollDirection::Down,
                config,
            );
        }
        assert_eq!(update.lines, 3);
    }

    #[test]
    fn wheel_lines_override_scales_wheel_ticks() {
        let config = ScrollConfig::from_terminal(
            &terminal_info_named(TerminalName::AppleTerminal),
            ScrollConfigOverrides {
                events_per_tick: Some(3),
                wheel_lines_per_tick: Some(2),
                mode: Some(ScrollInputMode::Wheel),
                ..ScrollConfigOverrides::default()
            },
        );
        let base = Instant::now();
        let mut state = MouseScrollState::new_at(base);

        let first = state.on_scroll_event_at(
            base + Duration::from_millis(1),
            ScrollDirection::Down,
            config,
        );
        let second = state.on_scroll_event_at(
            base + Duration::from_millis(2),
            ScrollDirection::Down,
            config,
        );
        let third = state.on_scroll_event_at(
            base + Duration::from_millis(3),
            ScrollDirection::Down,
            config,
        );

        assert_eq!(first.lines + second.lines + third.lines, 2);
    }

    #[test]
    fn ghostty_trackpad_is_not_penalized_by_wheel_event_density() {
        let config = ScrollConfig::from_terminal(
            &terminal_info_named(TerminalName::Ghostty),
            ScrollConfigOverrides {
                events_per_tick: Some(9),
                mode: Some(ScrollInputMode::Trackpad),
                ..ScrollConfigOverrides::default()
            },
        );
        let base = Instant::now();
        let mut state = MouseScrollState::new_at(base);

        let _ = state.on_scroll_event_at(
            base + Duration::from_millis(1),
            ScrollDirection::Down,
            config,
        );
        let _ = state.on_scroll_event_at(
            base + Duration::from_millis(2),
            ScrollDirection::Down,
            config,
        );
        let update = state.on_scroll_event_at(
            base + Duration::from_millis(REDRAW_CADENCE_MS + 1),
            ScrollDirection::Down,
            config,
        );

        // Trackpad mode uses a capped events-per-tick for normalization, so 3 events should
        // produce at least one line even when the wheel tick size is 9.
        assert_eq!(update.lines, 1);
    }

    #[test]
    fn trackpad_acceleration_speeds_up_large_swipes_without_affecting_small_swipes_too_much() {
        let config = ScrollConfig::from_terminal(
            &terminal_info_named(TerminalName::Ghostty),
            ScrollConfigOverrides {
                events_per_tick: Some(9),
                trackpad_accel_events: Some(30),
                trackpad_accel_max: Some(3),
                mode: Some(ScrollInputMode::Trackpad),
                ..ScrollConfigOverrides::default()
            },
        );
        let base = Instant::now();
        let mut state = MouseScrollState::new_at(base);

        let mut total_lines = 0;
        for idx in 0..60u64 {
            let update = state.on_scroll_event_at(
                base + Duration::from_millis((idx + 1) * (REDRAW_CADENCE_MS + 1)),
                ScrollDirection::Down,
                config,
            );
            total_lines += update.lines;
        }
        total_lines += state
            .on_tick_at(base + Duration::from_millis(60 * (REDRAW_CADENCE_MS + 1)) + STREAM_GAP)
            .lines;

        // Without acceleration, 60 events at 1/3 line each would be ~20 lines. With acceleration,
        // we should be meaningfully faster.
        assert!(total_lines >= 30, "total_lines={total_lines}");
    }

    #[test]
    fn direction_flip_closes_previous_stream() {
        let config = ScrollConfig::from_terminal(
            &terminal_info_named(TerminalName::AppleTerminal),
            ScrollConfigOverrides {
                events_per_tick: Some(3),
                mode: Some(ScrollInputMode::Auto),
                ..ScrollConfigOverrides::default()
            },
        );
        let base = Instant::now();
        let mut state = MouseScrollState::new_at(base);

        let _ =
            state.on_scroll_event_at(base + Duration::from_millis(1), ScrollDirection::Up, config);
        let _ =
            state.on_scroll_event_at(base + Duration::from_millis(2), ScrollDirection::Up, config);
        let up =
            state.on_scroll_event_at(base + Duration::from_millis(3), ScrollDirection::Up, config);
        let down = state.on_scroll_event_at(
            base + Duration::from_millis(4),
            ScrollDirection::Down,
            config,
        );

        assert_eq!(
            up,
            ScrollUpdate {
                lines: -3,
                next_tick_in: Some(Duration::from_millis(STREAM_GAP_MS)),
            }
        );
        assert_eq!(
            down,
            ScrollUpdate {
                lines: 0,
                next_tick_in: Some(Duration::from_millis(STREAM_GAP_MS)),
            }
        );
    }

    #[test]
    fn continuous_stream_coalesces_redraws() {
        let config = ScrollConfig::from_terminal(
            &terminal_info_named(TerminalName::AppleTerminal),
            ScrollConfigOverrides {
                events_per_tick: Some(1),
                mode: Some(ScrollInputMode::Trackpad),
                ..ScrollConfigOverrides::default()
            },
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
            ScrollConfigOverrides {
                events_per_tick: Some(1),
                invert_direction: true,
                ..ScrollConfigOverrides::default()
            },
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
