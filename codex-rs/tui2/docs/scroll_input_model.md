# TUI2 Scroll Input: Model and Implementation

This is the single "scrolling doc of record" for TUI2.

It describes what we implemented, why it works, and what we tried before this approach.
It also preserves the scroll-probe findings (see Appendix) that motivated the model.

Code reference: `codex-rs/tui2/src/tui/scrolling/mouse.rs`.

## Goals and constraints

Goals:

- Mouse wheel: scroll about **3 transcript lines per physical wheel tick** regardless of terminal
  event density (classic feel).
- Trackpad: remain **higher fidelity**, meaning small movements can accumulate fractionally and
  should not be forced into wheel behavior.
- Work across terminals where a single wheel tick may produce 1, 3, 9, or more raw events.

Constraints:

- Terminals typically encode both wheels and trackpads as the same "scroll up/down" mouse button
  events without a magnitude. We cannot reliably observe device type directly.
- Timing alone is not a reliable discriminator (wheel and trackpad bursts overlap).

## Current implementation (stream-based; data-driven)

TUI2 uses a stream model: scroll events are grouped into short streams separated by silence.
Within a stream, we normalize by a per-terminal "events per tick" factor and then apply either
wheel-like (fixed lines per tick) or trackpad-like (fractional) semantics.

### 1. Stream detection

- A stream begins on the first scroll event.
- A stream ends when the gap since the last event exceeds `STREAM_GAP_MS` or when direction flips.
- Direction flips always close the current stream and start a new one, so we never blend "up" and
  "down" into a single accumulator.

This makes behavior stable across:

- Dense bursts (Warp/Ghostty-style sub-ms intervals).
- Sparse bursts (single events separated by tens or hundreds of ms).
- Mixed wheel + trackpad input where direction changes quickly.

### 2. Normalization: events-per-tick

Different terminals emit different numbers of raw events per physical wheel notch.
We normalize by converting raw events into tick-equivalents:

`tick_equivalents = raw_events / events_per_tick`

Per-terminal defaults come from the probe logs (Appendix), and users can override them.

Config key: `tui.scroll_events_per_tick`.

### 3. Wheel vs trackpad behavior (and why it is heuristic)

Because device type is not directly observable, the implementation provides a mode setting:

- `tui.scroll_mode = "auto"` (default): infer wheel-like vs trackpad-like behavior per stream.
- `tui.scroll_mode = "wheel"`: always treat streams as wheel-like.
- `tui.scroll_mode = "trackpad"`: always treat streams as trackpad-like.

In auto mode:

- Streams start trackpad-like (safer: avoids overshoot when we guess wrong).
- Streams promote to wheel-like when the first tick-worth of events arrives quickly.
- For 1-event-per-tick terminals, "first tick completion time" is not observable, so there is a
  conservative end-of-stream fallback for very small bursts.

This design assumes that auto classification is a best-effort heuristic and must be overridable.

### 4. Applying scroll: wheel-like streams

Wheel-like streams target the "classic feel" requirement.

- Each raw event contributes `tui.scroll_wheel_lines / events_per_tick` lines.
- Deltas flush immediately (not cadence-gated) so wheels feel snappy even on dense streams.
- Wheel-like streams apply a minimum +/- 1 line when events were received but rounding would yield 0.

Defaults:

- `tui.scroll_wheel_lines = 3`

### 5. Applying scroll: trackpad-like streams

Trackpad-like streams are designed for fidelity first.

- Each raw event contributes `tui.scroll_trackpad_lines / trackpad_events_per_tick` lines.
- Fractional remainder is carried across streams, so tiny gestures accumulate instead of being lost.
- Trackpad deltas are cadence-gated to ~60 Hz (`REDRAW_CADENCE_MS`) to avoid redraw floods and to
  reduce "stop lag" / overshoot.
- Trackpad streams intentionally do not apply a minimum +/- 1 line at stream end; if a gesture is
  small enough to round to 0, it should feel like "no movement", not a forced jump.

Dense wheel terminals (e.g. Ghostty/Warp) can emit trackpad streams with high event density.
Using a wheel-derived `events_per_tick = 9` for trackpad would make trackpads feel slow, so we use
a capped divisor for trackpad normalization:

- `trackpad_events_per_tick = min(events_per_tick, 3)`

Additionally, to keep small gestures precise while making large/fast swipes cover more content,
trackpad-like streams apply bounded acceleration based on event count:

- `tui.scroll_trackpad_accel_events`: how many events correspond to +1x multiplier.
- `tui.scroll_trackpad_accel_max`: maximum multiplier.

### 6. Guard rails and axis handling

- Horizontal scroll events are ignored for vertical scrolling.
- Streams clamp event counts and accumulated line deltas to avoid floods.

## Terminal defaults and per-terminal tuning

Defaults are keyed by `TerminalName` (terminal family), not exact version.
Probe data is version-specific, so defaults should be revalidated as more logs arrive.

Events-per-tick defaults derived from `wheel_single` medians:

- AppleTerminal: 3
- WarpTerminal: 9
- WezTerm: 1
- Alacritty: 3
- Ghostty: 3
- Iterm2: 1
- VsCode: 1
- Kitty: 3
- Unknown: 3

Note: probe logs measured Ghostty at ~9 events per tick, but we default to 3 because an upstream
Ghostty change is expected to reduce wheel event density. Users can override with
`tui.scroll_events_per_tick`.

Auto-mode wheel promotion thresholds can also be tuned per terminal if needed (see config below).

## Configuration knobs (TUI2)

These are user-facing knobs in `config.toml` under `[tui]`:

In this repo, "tick" always refers to a physical mouse wheel notch. Trackpads do not have ticks, so
trackpad settings are expressed in terms of "tick-equivalents" (raw events normalized to a common
scale).

The core normalization formulas are:

- Wheel-like streams:
  - `lines_per_event = scroll_wheel_lines / scroll_events_per_tick`
- Trackpad-like streams:
  - `lines_per_event = scroll_trackpad_lines / min(scroll_events_per_tick, 3)`
  - (plus bounded acceleration from `scroll_trackpad_accel_*` and fractional carry across streams)

Keys:

- `scroll_events_per_tick` (number):
  - Raw vertical scroll events per physical wheel notch in your terminal (normalization input).
  - Affects wheel-like scroll speed and auto-mode wheel promotion timing.
  - Trackpad-like mode uses `min(..., 3)` as the divisor so dense wheel ticks (e.g. 9 events per
    notch) do not make trackpads feel artificially slow.
- `scroll_wheel_lines` (number):
  - Lines per physical wheel notch (default 3).
  - Change this if you want "classic" wheel scrolling to be more/less aggressive globally.
- `scroll_trackpad_lines` (number):
  - Baseline trackpad sensitivity in trackpad-like mode (default 1).
  - Change this if your trackpad feels consistently too slow/fast for small motions.
- `scroll_trackpad_accel_events` (number):
  - Trackpad acceleration tuning (default 30). Smaller values accelerate earlier.
  - Trackpad-like streams compute a multiplier:
    - `multiplier = clamp(1 + abs(events) / scroll_trackpad_accel_events, 1..scroll_trackpad_accel_max)`
  - The multiplier is applied to the trackpad stream’s computed line delta (including any carried
    fractional remainder).
- `scroll_trackpad_accel_max` (number):
  - Trackpad acceleration cap (default 3). Set to 1 to effectively disable acceleration.
- `scroll_mode` (`auto` | `wheel` | `trackpad`):
  - `auto` (default): infer wheel-like vs trackpad-like per stream.
  - `wheel`: always wheel-like (good for wheel-only setups; trackpads will feel jumpy).
  - `trackpad`: always trackpad-like (good if auto misclassifies; wheels may feel slow).
- `scroll_wheel_tick_detect_max_ms` (number):
  - Auto-mode promotion threshold: how quickly the first tick-worth of events must arrive to
    consider the stream wheel-like.
  - If wheel feels slow in a dense-wheel terminal, increasing this is usually better than changing
    `scroll_events_per_tick`.
- `scroll_wheel_like_max_duration_ms` (number):
  - Auto-mode fallback for 1-event-per-tick terminals (WezTerm/iTerm/VS Code).
  - If wheel feels like trackpad (too slow) in those terminals, increasing this can help.
- `scroll_invert` (bool):
  - Invert direction after terminal detection; applies consistently to wheel and trackpad.

## Previous approaches tried (and why they were replaced)

1. Cadence-based inference (rolling inter-event thresholds)

- Approach: infer wheel vs trackpad using inter-event timing thresholds (burst vs frame cadence vs slow),
  with terminal-specific tuning.
- Problem: terminals differ more in event density and batching than in timing; timing overlaps heavily
  between wheel and trackpad. Small threshold changes had outsized, terminal-specific effects.

2. Pure event-count or pure duration classification

- Approach: classify wheel-like vs trackpad-like by event count <= N or duration <= M.
- Problem: burst length overlaps heavily across devices/terminals; duration is more separable but still
  not strong enough to be authoritative.

3. Why streams + normalization won

- Streams give a stable unit ("what did the user do in one gesture?") that we can bound and reason about.
- Normalization directly addresses the main cross-terminal source of variation: raw event density.
- Classification remains heuristic, but is isolated and configurable.

## Appendix A: Follow-up analysis (latest log per terminal; 2025-12-20)

This section is derived from a "latest log per terminal" subset analysis. The exact event count is
not significant; it is included only as a note about which subset was used.

Key takeaways:

- Burst length overlaps heavily between wheel and trackpad. Simple "event count <= N" classifiers perform poorly.
- Burst span (duration) is more separable: wheel bursts typically complete in < ~180-200 ms, while trackpad
  bursts are often hundreds of milliseconds.
- Conclusion: explicit wheel vs trackpad classification is inherently weak from these events; prefer a
  stream model, plus a small heuristic and a config override (`tui.scroll_mode`) for edge cases.

Data notes (latest per terminal label):

- Logs used (one per terminal, by filename timestamp):
  - mouse_scroll_log_Apple_Terminal_2025-12-19T19-53-54Z.jsonl
  - mouse_scroll_log_WarpTerminal_2025-12-19T19-59-38Z.jsonl
  - mouse_scroll_log_WezTerm_2025-12-19T20-00-36Z.jsonl
  - mouse_scroll_log_alacritty_2025-12-19T19-56-45Z.jsonl
  - mouse_scroll_log_ghostty_2025-12-19T19-52-44Z.jsonl
  - mouse_scroll_log_iTerm_app_2025-12-19T19-55-08Z.jsonl
  - mouse_scroll_log_vscode_2025-12-19T19-51-20Z.jsonl
  - mouse_scroll_log_xterm-kitty_2025-12-19T19-58-19Z.jsonl

Per-terminal burst separability (wheel vs trackpad), summarized as median and p90:

- Apple Terminal:
  - Wheel: length median 9.5 (p90 49), span median 94 ms (p90 136)
  - Trackpad: length median 13.5 (p90 104), span median 238 ms (p90 616)
- Warp:
  - Wheel: length median 43 (p90 169), span median 88 ms (p90 178)
  - Trackpad: length median 60 (p90 82), span median 358 ms (p90 721)
- WezTerm:
  - Wheel: length median 4 (p90 10), span median 91 ms (p90 156)
  - Trackpad: length median 10.5 (p90 36), span median 270 ms (p90 348)
- alacritty:
  - Wheel: length median 14 (p90 63), span median 109 ms (p90 158)
  - Trackpad: length median 12.5 (p90 63), span median 372 ms (p90 883)
- ghostty:
  - Wheel: length median 32.5 (p90 163), span median 99 ms (p90 157)
  - Trackpad: length median 14.5 (p90 60), span median 366 ms (p90 719)
- iTerm:
  - Wheel: length median 4 (p90 9), span median 91 ms (p90 230)
  - Trackpad: length median 9 (p90 36), span median 223 ms (p90 540)
- VS Code:
  - Wheel: length median 3 (p90 9), span median 94 ms (p90 120)
  - Trackpad: length median 3 (p90 12), span median 192 ms (p90 468)
- Kitty:
  - Wheel: length median 15.5 (p90 59), span median 87 ms (p90 233)
  - Trackpad: length median 15.5 (p90 68), span median 292 ms (p90 563)

Wheel_single medians (events per tick) in the latest logs:

- Apple: 3
- Warp: 9
- WezTerm: 1
- alacritty: 3
- ghostty: 9 (measured); TUI2 defaults use 3 because an upstream Ghostty change is expected to
  reduce wheel event density. If your Ghostty build still emits ~9 events per wheel tick, set
  `tui.scroll_events_per_tick = 9`.
- iTerm: 1
- VS Code: 1
- Kitty: 3

## Appendix B: Scroll probe findings (authoritative; preserved verbatim)

The remainder of this document is preserved from the original scroll-probe spec.
It is intentionally not rewritten so the data and rationale remain auditable.

Note: the original text uses "events per line" terminology; the implementation treats this as an
events-per-wheel-tick normalization factor (see "Normalization: events-per-tick").

Note: the pseudocode in the preserved spec is not the exact current implementation; it is kept as
historical context for how the probe data originally mapped into an algorithm. The current
implementation is described in the sections above.

## 1. TL;DR

Analysis of 16 scroll-probe logs (13,734 events) across 8 terminals shows large per-terminal variation in how many raw events are emitted per physical wheel tick (1-9+ events). Timing alone does not distinguish wheel vs trackpad; event counts and burst duration are more reliable. The algorithm below treats scroll input as short streams separated by gaps, normalizes events into line deltas using a per-terminal events-per-line factor, coalesces redraws at 60 Hz, and applies a minimum 1-line delta for discrete bursts. This yields stable behavior across dense streams, sparse bursts, and terminals that emit horizontal events.

## 2. Data overview

- Logs analyzed: 16
- Total events: 13,734
- Terminals covered:
  - Apple_Terminal 455.1
  - WarpTerminal v0.2025.12.17.17.stable_02
  - WezTerm 20240203-110809-5046fc22
  - alacritty
  - ghostty 1.2.3
  - iTerm.app 3.6.6
  - vscode 1.107.1
  - xterm-kitty
- Scenarios captured: `wheel_single`, `wheel_small`, `wheel_long`, `trackpad_single`, `trackpad_slow`, `trackpad_fast` (directional up/down variants treated as distinct bursts).
- Legacy `wheel_scroll_*` logs are mapped to `wheel_small` in analysis.

## 3. Cross-terminal comparison table

| Terminal                                | Scenario        | Median Dt (ms) | P95 Dt (ms) | Typical burst | Notes       |
| --------------------------------------- | --------------- | -------------: | ----------: | ------------: | ----------- |
| Apple_Terminal 455.1                    | wheel_single    |           0.14 |       97.68 |             3 |
| Apple_Terminal 455.1                    | wheel_small     |           0.12 |       23.81 |            19 |
| Apple_Terminal 455.1                    | wheel_long      |           0.03 |       15.93 |            48 |
| Apple_Terminal 455.1                    | trackpad_single |          92.35 |      213.15 |             2 |
| Apple_Terminal 455.1                    | trackpad_slow   |          11.30 |       75.46 |            14 |
| Apple_Terminal 455.1                    | trackpad_fast   |           0.13 |        8.92 |            96 |
| WarpTerminal v0.2025.12.17.17.stable_02 | wheel_single    |           0.07 |        0.34 |             9 |
| WarpTerminal v0.2025.12.17.17.stable_02 | wheel_small     |           0.05 |        5.04 |            65 |
| WarpTerminal v0.2025.12.17.17.stable_02 | wheel_long      |           0.01 |        0.42 |           166 |
| WarpTerminal v0.2025.12.17.17.stable_02 | trackpad_single |           9.77 |       32.64 |            10 |
| WarpTerminal v0.2025.12.17.17.stable_02 | trackpad_slow   |           7.93 |       16.44 |            74 |
| WarpTerminal v0.2025.12.17.17.stable_02 | trackpad_fast   |           5.40 |       10.04 |            74 |
| WezTerm 20240203-110809-5046fc22        | wheel_single    |         416.07 |      719.64 |             1 |
| WezTerm 20240203-110809-5046fc22        | wheel_small     |          19.41 |       50.19 |             6 |
| WezTerm 20240203-110809-5046fc22        | wheel_long      |          13.19 |       29.96 |            10 |
| WezTerm 20240203-110809-5046fc22        | trackpad_single |         237.56 |      237.56 |             1 |
| WezTerm 20240203-110809-5046fc22        | trackpad_slow   |          23.54 |       76.10 |            10 | 12.5% horiz |
| WezTerm 20240203-110809-5046fc22        | trackpad_fast   |           7.10 |       24.86 |            32 | 12.6% horiz |
| alacritty                               | wheel_single    |           0.09 |        0.33 |             3 |
| alacritty                               | wheel_small     |           0.11 |       37.24 |            24 |
| alacritty                               | wheel_long      |           0.01 |       15.96 |            56 |
| alacritty                               | trackpad_single |            n/a |         n/a |             1 |
| alacritty                               | trackpad_slow   |          41.90 |       97.36 |            11 |
| alacritty                               | trackpad_fast   |           3.07 |       25.13 |            62 |
| ghostty 1.2.3                           | wheel_single    |           0.05 |        0.20 |             9 |
| ghostty 1.2.3                           | wheel_small     |           0.05 |        7.18 |            52 |
| ghostty 1.2.3                           | wheel_long      |           0.02 |        1.16 |           146 |
| ghostty 1.2.3                           | trackpad_single |          61.28 |      124.28 |             3 | 23.5% horiz |
| ghostty 1.2.3                           | trackpad_slow   |          23.10 |       76.30 |            14 | 34.7% horiz |
| ghostty 1.2.3                           | trackpad_fast   |           3.84 |       37.72 |            47 | 23.4% horiz |
| iTerm.app 3.6.6                         | wheel_single    |          74.96 |       80.61 |             1 |
| iTerm.app 3.6.6                         | wheel_small     |          20.79 |       84.83 |             6 |
| iTerm.app 3.6.6                         | wheel_long      |          16.70 |       50.91 |             9 |
| iTerm.app 3.6.6                         | trackpad_single |            n/a |         n/a |             1 |
| iTerm.app 3.6.6                         | trackpad_slow   |          17.25 |       94.05 |             9 |
| iTerm.app 3.6.6                         | trackpad_fast   |           7.12 |       24.54 |            33 |
| vscode 1.107.1                          | wheel_single    |          58.01 |       58.01 |             1 |
| vscode 1.107.1                          | wheel_small     |          16.76 |       66.79 |             5 |
| vscode 1.107.1                          | wheel_long      |           9.86 |       32.12 |             8 |
| vscode 1.107.1                          | trackpad_single |            n/a |         n/a |             1 |
| vscode 1.107.1                          | trackpad_slow   |         164.19 |      266.90 |             3 |
| vscode 1.107.1                          | trackpad_fast   |          16.78 |       61.05 |            11 |
| xterm-kitty                             | wheel_single    |           0.16 |       51.74 |             3 |
| xterm-kitty                             | wheel_small     |           0.10 |       24.12 |            26 |
| xterm-kitty                             | wheel_long      |           0.01 |       16.10 |            56 |
| xterm-kitty                             | trackpad_single |         155.65 |      289.87 |             1 | 12.5% horiz |
| xterm-kitty                             | trackpad_slow   |          16.89 |       67.04 |            16 | 30.4% horiz |
| xterm-kitty                             | trackpad_fast   |           0.23 |       16.37 |            78 | 20.6% horiz |

## 4. Key findings

- Raw wheel ticks vary by terminal: median events per tick are 1 (WezTerm/iTerm/vscode), 3 (Apple/alacritty/kitty), and 9 (Warp/ghostty).
- Trackpad bursts are longer than wheel ticks but overlap in timing; inter-event timing alone does not distinguish device type.
- Continuous streams have short gaps: overall inter-event p99 is 70.67 ms; trackpad_slow p95 is 66.98 ms.
- Horizontal events appear only in trackpad scenarios and only in WezTerm/ghostty/kitty; ignore horizontal events for vertical scrolling.
- Burst duration is a reliable discrete/continuous signal:
  - wheel_single median 0.15 ms (p95 80.61 ms)
  - trackpad_single median 0 ms (p95 237.56 ms)
  - wheel_small median 96.88 ms (p95 182.90 ms)
  - trackpad_slow median 320.69 ms (p95 812.10 ms)

## 5. Scrolling model (authoritative)

**Stream detection.** Treat scroll input as short streams separated by silence. A stream begins on the first scroll event and ends when the gap since the last event exceeds `STREAM_GAP_MS` or the direction flips. Direction flip immediately closes the current stream and starts a new one.

**Normalization.** Convert raw events into line deltas using a per-terminal `EVENTS_PER_LINE` factor derived from the terminal's median `wheel_single` burst length. If no terminal override matches, use the global default (`3`).

**Discrete vs continuous.** Classify the stream after it ends:

- If `event_count <= DISCRETE_MAX_EVENTS` **and** `duration_ms <= DISCRETE_MAX_DURATION_MS`, treat as discrete.
- Otherwise treat as continuous.

**Discrete streams.** Apply the accumulated line delta immediately. If the stream's accumulated lines rounds to 0 but events were received, apply a minimum +/-1 line (respecting direction).

**Continuous streams.** Accumulate fractional lines and coalesce redraws to `REDRAW_CADENCE_MS`. Flush any remaining fractional lines on stream end (with the same +/-1 minimum if the stream had events but rounded to 0).

**Direction.** Always use the raw event direction. Provide a separate user-level invert option if needed; do not infer inversion from timing.

**Horizontal events.** Ignore horizontal events in vertical scroll logic.

## 6. Concrete constants (data-derived)

```text
STREAM_GAP_MS                 = 80
DISCRETE_MAX_EVENTS           = 10
DISCRETE_MAX_DURATION_MS      = 250
REDRAW_CADENCE_MS             = 16
DEFAULT_EVENTS_PER_LINE       = 3
MAX_EVENTS_PER_STREAM         = 256
MAX_ACCUMULATED_LINES         = 256
MIN_LINES_PER_DISCRETE_STREAM = 1
DEFAULT_WHEEL_LINES_PER_TICK  = 3
```

Why these values:

- `STREAM_GAP_MS=80`: overall p99 inter-event gap is 70.67 ms; trackpad_slow p95 is 66.98 ms. 80 ms ends streams without splitting most continuous input.
- `DISCRETE_MAX_EVENTS=10`: wheel_single p95 burst = 9; trackpad_single p95 burst = 10.
- `DISCRETE_MAX_DURATION_MS=250`: trackpad_single p95 duration = 237.56 ms.
- `REDRAW_CADENCE_MS=16`: coalesces dense streams to ~60 Hz; trackpad_fast p95 Dt = 19.83 ms.
- `DEFAULT_EVENTS_PER_LINE=3`: global median wheel_single burst length.
- `MAX_EVENTS_PER_STREAM=256` and `MAX_ACCUMULATED_LINES=256`: highest observed burst is 206; cap to avoid floods.
- `DEFAULT_WHEEL_LINES_PER_TICK=3`: restores classic wheel speed; this is a UX choice rather than a data-derived constant.

## 7. Pseudocode (Rust-oriented)

```rust
// This is intentionally a simplified sketch of the current implementation.
// For the authoritative behavior, see `codex-rs/tui2/src/tui/scrolling/mouse.rs`.

enum StreamKind {
    Unknown,
    Wheel,
    Trackpad,
}

struct Stream {
    start: Instant,
    last: Instant,
    dir: i32,
    event_count: usize,
    accumulated_events: i32,
    applied_lines: i32,
    kind: StreamKind,
    just_promoted: bool,
}

struct State {
    stream: Option<Stream>,
    carry_lines: f32,
    last_redraw_at: Instant,
    cfg: Config,
}

struct Config {
    events_per_tick: u16,
    wheel_lines_per_tick: u16,
    trackpad_lines_per_tick: u16,
    trackpad_accel_events: u16,
    trackpad_accel_max: u16,
    wheel_tick_detect_max: Duration,
}

fn on_scroll_event(dir: i32, now: Instant, st: &mut State) -> i32 {
    // Close stream on idle gap or direction flip.
    if let Some(stream) = st.stream.as_ref() {
        let gap = now.duration_since(stream.last);
        if gap > STREAM_GAP || stream.dir != dir {
            finalize_stream(now, st);
            st.stream = None;
        }
    }

    let stream = st.stream.get_or_insert_with(|| Stream {
        start: now,
        last: now,
        dir,
        event_count: 0,
        accumulated_events: 0,
        applied_lines: 0,
        kind: StreamKind::Unknown,
        just_promoted: false,
    });

    stream.last = now;
    stream.dir = dir;
    stream.event_count = (stream.event_count + 1).min(MAX_EVENTS_PER_STREAM);
    stream.accumulated_events =
        (stream.accumulated_events + dir).clamp(-(MAX_EVENTS_PER_STREAM as i32), MAX_EVENTS_PER_STREAM as i32);

    // Auto-mode promotion: promote to wheel-like when the first tick-worth of events arrives quickly.
    if matches!(stream.kind, StreamKind::Unknown) {
        let ept = st.cfg.events_per_tick.max(1) as usize;
        if ept >= 2 && stream.event_count >= ept && now.duration_since(stream.start) <= st.cfg.wheel_tick_detect_max {
            stream.kind = StreamKind::Wheel;
            stream.just_promoted = true;
        }
    }

    flush_lines(now, st)
}

fn on_tick(now: Instant, st: &mut State) -> i32 {
    if let Some(stream) = st.stream.as_ref() {
        let gap = now.duration_since(stream.last);
        if gap > STREAM_GAP {
            return finalize_stream(now, st);
        }
    }
    flush_lines(now, st)
}

fn finalize_stream(now: Instant, st: &mut State) -> i32 {
    // In auto mode, any stream that isn't wheel-like by promotion stays trackpad-like.
    if let Some(stream) = st.stream.as_mut() {
        if matches!(stream.kind, StreamKind::Unknown) {
            stream.kind = StreamKind::Trackpad;
        }
    }

    let lines = flush_lines(now, st);

    // Carry fractional remainder across streams for trackpad-like input.
    if let Some(stream) = st.stream.as_ref() {
        if matches!(stream.kind, StreamKind::Trackpad) {
            st.carry_lines = desired_lines_f32(st, stream) - stream.applied_lines as f32;
        } else {
            st.carry_lines = 0.0;
        }
    }

    lines
}

fn flush_lines(now: Instant, st: &mut State) -> i32 {
    let Some(stream) = st.stream.as_mut() else { return 0; };

    let wheel_like = matches!(stream.kind, StreamKind::Wheel);
    let cadence_elapsed = now.duration_since(st.last_redraw_at) >= REDRAW_CADENCE;
    let should_flush = wheel_like || cadence_elapsed || stream.just_promoted;
    if !should_flush {
        return 0;
    }

    let desired_total = desired_lines_f32(st, stream);
    let mut desired_lines = desired_total.trunc() as i32;

    // Wheel guardrail: ensure we never produce a "dead tick" for non-zero input.
    if wheel_like && desired_lines == 0 && stream.accumulated_events != 0 {
        desired_lines = stream.accumulated_events.signum() * MIN_LINES_PER_DISCRETE_STREAM;
    }

    let mut delta = desired_lines - stream.applied_lines;
    if delta == 0 {
        return 0;
    }

    delta = delta.clamp(-MAX_ACCUMULATED_LINES, MAX_ACCUMULATED_LINES);
    stream.applied_lines += delta;
    stream.just_promoted = false;
    st.last_redraw_at = now;
    delta
}

fn desired_lines_f32(st: &State, stream: &Stream) -> f32 {
    let wheel_like = matches!(stream.kind, StreamKind::Wheel);

    let events_per_tick = if wheel_like {
        st.cfg.events_per_tick.max(1) as f32
    } else {
        // Trackpad divisor is capped so dense wheel terminals don't feel slow for trackpads.
        st.cfg.events_per_tick.clamp(1, DEFAULT_EVENTS_PER_LINE).max(1) as f32
    };

    let lines_per_tick = if wheel_like {
        st.cfg.wheel_lines_per_tick.max(1) as f32
    } else {
        st.cfg.trackpad_lines_per_tick.max(1) as f32
    };

    let mut total = (stream.accumulated_events as f32 * (lines_per_tick / events_per_tick))
        .clamp(-(MAX_ACCUMULATED_LINES as f32), MAX_ACCUMULATED_LINES as f32);

    if !wheel_like {
        total = (total + st.carry_lines).clamp(-(MAX_ACCUMULATED_LINES as f32), MAX_ACCUMULATED_LINES as f32);

        // Bounded acceleration for large swipes (keep small swipes precise).
        let event_count = stream.accumulated_events.abs() as f32;
        let accel = (1.0 + (event_count / st.cfg.trackpad_accel_events.max(1) as f32))
            .clamp(1.0, st.cfg.trackpad_accel_max.max(1) as f32);
        total = (total * accel).clamp(-(MAX_ACCUMULATED_LINES as f32), MAX_ACCUMULATED_LINES as f32);
    }

    total
}
```

## 8. Terminal-specific adjustments (minimal)

Use per-terminal `EVENTS_PER_LINE` overrides derived from median `wheel_single` bursts:

```text
Apple_Terminal 455.1                     = 3
WarpTerminal v0.2025.12.17.17.stable_02  = 9
WezTerm 20240203-110809-5046fc22         = 1
alacritty                                 = 3
ghostty 1.2.3                             = 9
iTerm.app 3.6.6                           = 1
vscode 1.107.1                            = 1
xterm-kitty                               = 3
```

If terminal is not matched, use `DEFAULT_EVENTS_PER_LINE = 3`.

## 9. Known weird cases and guardrails

- Extremely dense streams (sub-ms Dt) occur in Warp/ghostty/kitty; redraw coalescing is mandatory.
- Sparse bursts (hundreds of ms between events) occur in trackpad_single; do not merge them into long streams.
- Horizontal scroll events (12-35% of trackpad events in some terminals) must be ignored for vertical scrolling.
- Direction inversion is user-configurable in terminals; always use event direction and expose an application-level invert setting.
- Guard against floods: cap event counts and accumulated line deltas per stream.
