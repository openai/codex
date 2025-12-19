# TUI2 Scroll Input Model (Data-Derived)

This document captures the scroll probe findings and the TUI2 implementation derived from them.
The probe data came from a small harness run across a limited set of terminals and devices; we
need more data from other terminals, operating systems, and input hardware.

## Implementation in codex-tui2 (derived from the probe, plus UX requirements)

TUI2's goal is:
- Mouse wheel: scroll **3 lines per physical wheel tick** (classic feel), regardless of how many raw
  events the terminal emits for that tick.
- Trackpad: remain **higher fidelity**, meaning small movements can accumulate fractionally and
  should not be artificially accelerated to wheel speed.

The implementation (see `codex-rs/tui2/src/tui/scrolling/mouse.rs`) follows this model:
- Streams: scroll input is grouped into short streams separated by `STREAM_GAP_MS` or direction flips.
- Normalization: per-terminal `EVENTS_PER_TICK` factors convert raw events into "tick-equivalents".
  Note: config key `tui.scroll_events_per_line` has a historic name; it is treated as an events-per-tick
  factor for TUI2.
- Wheel vs trackpad: device type is not directly observable from terminal scroll events, so TUI2 uses
  a heuristic in `tui.scroll_mode = "auto"`:
  - Start by treating a stream as trackpad-like (to avoid overshoot).
  - Promote a stream to wheel-like if the first tick-worth of events arrives quickly.
  - For 1-event-per-tick terminals, use an end-of-stream fallback only for very small bursts.
  - Users can force `wheel` or `trackpad` behavior with `tui.scroll_mode` if auto misclassifies.
- Scaling:
  - Wheel-like: each event contributes `scroll_wheel_lines / events_per_tick` lines.
  - Trackpad-like: each event contributes `scroll_trackpad_lines / events_per_tick` lines, with
    fractional accumulation carried across streams.
- Redraw coalescing: apply whole-line deltas at `REDRAW_CADENCE_MS` (~60 Hz) while input is active.
- Direction: use raw event direction; user inversion is controlled by `tui.scroll_invert` rather than inferred.
- Horizontal events: ignored for vertical scrolling (TUI2 currently receives only vertical scroll in the main app event loop).
- Guard rails: cap events per stream and clamp accumulated lines to avoid floods.

## Differences from the prior cadence-based approach
- Previous approach relied on rolling inter-event timing thresholds (fast burst, frame cadence, slow events)
  plus per-terminal tuning to guess wheel vs trackpad behavior.
- The stream approach groups related events into bounded streams and redraws at a fixed cadence.
- Normalization is explicit: `events_per_tick` is a data-derived per-terminal factor, and wheel/trackpad
  scaling is expressed directly as lines-per-tick.
- Wheel speed is guaranteed for multi-tick wheel bursts (not just "wheel_single") by applying wheel scaling
  to wheel-like streams regardless of stream length.
- Trackpad overshoot is reduced by (a) defaulting to trackpad-like behavior until a wheel signal is observed,
  and (b) not forcing a minimum +/- 1 line on trackpad-like stream finalization.

## Follow-up analysis (latest log per terminal; 2025-12-20)

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
- ghostty: 9
- iTerm: 1
- VS Code: 1
- Kitty: 3

## Scroll probe findings (authoritative)
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

Terminal | Scenario | Median Dt (ms) | P95 Dt (ms) | Typical burst | Notes
---|---|---:|---:|---:|---
Apple_Terminal 455.1 | wheel_single | 0.14 | 97.68 | 3 |
Apple_Terminal 455.1 | wheel_small | 0.12 | 23.81 | 19 |
Apple_Terminal 455.1 | wheel_long | 0.03 | 15.93 | 48 |
Apple_Terminal 455.1 | trackpad_single | 92.35 | 213.15 | 2 |
Apple_Terminal 455.1 | trackpad_slow | 11.30 | 75.46 | 14 |
Apple_Terminal 455.1 | trackpad_fast | 0.13 | 8.92 | 96 |
WarpTerminal v0.2025.12.17.17.stable_02 | wheel_single | 0.07 | 0.34 | 9 |
WarpTerminal v0.2025.12.17.17.stable_02 | wheel_small | 0.05 | 5.04 | 65 |
WarpTerminal v0.2025.12.17.17.stable_02 | wheel_long | 0.01 | 0.42 | 166 |
WarpTerminal v0.2025.12.17.17.stable_02 | trackpad_single | 9.77 | 32.64 | 10 |
WarpTerminal v0.2025.12.17.17.stable_02 | trackpad_slow | 7.93 | 16.44 | 74 |
WarpTerminal v0.2025.12.17.17.stable_02 | trackpad_fast | 5.40 | 10.04 | 74 |
WezTerm 20240203-110809-5046fc22 | wheel_single | 416.07 | 719.64 | 1 |
WezTerm 20240203-110809-5046fc22 | wheel_small | 19.41 | 50.19 | 6 |
WezTerm 20240203-110809-5046fc22 | wheel_long | 13.19 | 29.96 | 10 |
WezTerm 20240203-110809-5046fc22 | trackpad_single | 237.56 | 237.56 | 1 |
WezTerm 20240203-110809-5046fc22 | trackpad_slow | 23.54 | 76.10 | 10 | 12.5% horiz
WezTerm 20240203-110809-5046fc22 | trackpad_fast | 7.10 | 24.86 | 32 | 12.6% horiz
alacritty | wheel_single | 0.09 | 0.33 | 3 |
alacritty | wheel_small | 0.11 | 37.24 | 24 |
alacritty | wheel_long | 0.01 | 15.96 | 56 |
alacritty | trackpad_single | n/a | n/a | 1 |
alacritty | trackpad_slow | 41.90 | 97.36 | 11 |
alacritty | trackpad_fast | 3.07 | 25.13 | 62 |
ghostty 1.2.3 | wheel_single | 0.05 | 0.20 | 9 |
ghostty 1.2.3 | wheel_small | 0.05 | 7.18 | 52 |
ghostty 1.2.3 | wheel_long | 0.02 | 1.16 | 146 |
ghostty 1.2.3 | trackpad_single | 61.28 | 124.28 | 3 | 23.5% horiz
ghostty 1.2.3 | trackpad_slow | 23.10 | 76.30 | 14 | 34.7% horiz
ghostty 1.2.3 | trackpad_fast | 3.84 | 37.72 | 47 | 23.4% horiz
iTerm.app 3.6.6 | wheel_single | 74.96 | 80.61 | 1 |
iTerm.app 3.6.6 | wheel_small | 20.79 | 84.83 | 6 |
iTerm.app 3.6.6 | wheel_long | 16.70 | 50.91 | 9 |
iTerm.app 3.6.6 | trackpad_single | n/a | n/a | 1 |
iTerm.app 3.6.6 | trackpad_slow | 17.25 | 94.05 | 9 |
iTerm.app 3.6.6 | trackpad_fast | 7.12 | 24.54 | 33 |
vscode 1.107.1 | wheel_single | 58.01 | 58.01 | 1 |
vscode 1.107.1 | wheel_small | 16.76 | 66.79 | 5 |
vscode 1.107.1 | wheel_long | 9.86 | 32.12 | 8 |
vscode 1.107.1 | trackpad_single | n/a | n/a | 1 |
vscode 1.107.1 | trackpad_slow | 164.19 | 266.90 | 3 |
vscode 1.107.1 | trackpad_fast | 16.78 | 61.05 | 11 |
xterm-kitty | wheel_single | 0.16 | 51.74 | 3 |
xterm-kitty | wheel_small | 0.10 | 24.12 | 26 |
xterm-kitty | wheel_long | 0.01 | 16.10 | 56 |
xterm-kitty | trackpad_single | 155.65 | 289.87 | 1 | 12.5% horiz
xterm-kitty | trackpad_slow | 16.89 | 67.04 | 16 | 30.4% horiz
xterm-kitty | trackpad_fast | 0.23 | 16.37 | 78 | 20.6% horiz

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
struct ScrollStream {
    start: Instant,
    last: Instant,
    last_dir: i32,
    event_count: usize,
    accumulated_events: i32,
    accumulated_lines: f32,
}

fn on_scroll_event(dir: i32, now: Instant, state: &mut State) {
    if let Some(stream) = &mut state.stream {
        let gap_ms = now.duration_since(stream.last).as_millis() as u64;
        if gap_ms > STREAM_GAP_MS || dir != stream.last_dir {
            finalize_stream(state);
            state.stream = None;
        }
    }

    let stream = state.stream.get_or_insert_with(|| ScrollStream {
        start: now,
        last: now,
        last_dir: dir,
        event_count: 0,
        accumulated_events: 0,
        accumulated_lines: 0.0,
    });

    stream.last = now;
    stream.last_dir = dir;
    stream.event_count = stream.event_count.saturating_add(1).min(MAX_EVENTS_PER_STREAM);
    stream.accumulated_events += dir;

    let epl = state.events_per_line as f32;
    stream.accumulated_lines += (dir as f32) / epl;

    if state.last_redraw.elapsed().as_millis() as u64 >= REDRAW_CADENCE_MS {
        flush_lines(state, stream, false);
    }
}

fn on_tick(now: Instant, state: &mut State) {
    if let Some(stream) = &mut state.stream {
        let gap_ms = now.duration_since(stream.last).as_millis() as u64;
        if gap_ms > STREAM_GAP_MS {
            finalize_stream(state);
            state.stream = None;
        }
    }
}

fn finalize_stream(state: &mut State) {
    if let Some(stream) = &mut state.stream {
        let duration_ms = stream.last.duration_since(stream.start).as_millis() as u64;
        let discrete = stream.event_count <= DISCRETE_MAX_EVENTS
            && duration_ms <= DISCRETE_MAX_DURATION_MS;
        flush_lines(state, stream, discrete);
    }
}

fn flush_lines(state: &mut State, stream: &mut ScrollStream, discrete: bool) {
    let mut lines = stream.accumulated_lines.trunc() as i32;
    if discrete && lines == 0 && stream.accumulated_events != 0 {
        lines = stream.accumulated_events.signum() * MIN_LINES_PER_DISCRETE_STREAM;
    }

    if lines != 0 {
        apply_scroll(lines.clamp(-MAX_ACCUMULATED_LINES, MAX_ACCUMULATED_LINES));
        stream.accumulated_lines -= lines as f32;
        state.last_redraw = Instant::now();
    }
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

## 10. Implementation checklist
- [ ] introduce constants above and wire them into TUI2 scroll handling
- [ ] implement stream detection with `STREAM_GAP_MS` and direction-change breaks
- [ ] normalize events-per-line using per-terminal overrides
- [ ] apply discrete vs continuous handling based on event count + duration
- [ ] coalesce redraws to `REDRAW_CADENCE_MS`
- [ ] ignore horizontal events for vertical scrolling
- [ ] clamp accumulated lines and event counts
- [ ] add a minimal config hook for `EVENTS_PER_LINE`, wheel lines, and invert direction
