# TUI2 Scroll Input Plan and Status

This status update is meant to be re-read in a future Codex session. It summarizes the work
completed, the data used, and what is still pending. Full probe findings and the derived algorithm
are preserved in `codex-rs/tui2/docs/scroll_input_model.md`.

## Goal
Replace the prior cadence-based scroll heuristics with a stream-based normalization model that is
stable across terminals and devices, using the data-derived constants and per-terminal overrides
from the scroll-probe logs.

Additionally, enforce the UX requirement that a mouse wheel scrolls ~3 lines per physical wheel
tick (classic feel), while trackpads remain higher fidelity.

## Data findings (from probe logs)
- Logs analyzed: 16 (13,734 events) across Apple Terminal, Warp, WezTerm, Alacritty, Ghostty,
  iTerm2, VS Code, Kitty.
- Raw event counts per wheel tick vary widely by terminal (1, 3, 9+). Timing alone cannot reliably
  distinguish wheel vs trackpad; burst duration and event counts are more reliable.
- Horizontal scroll events appear in trackpad scenarios only (WezTerm/Ghostty/Kitty) and must be
  ignored for vertical scrolling.
- Discrete vs continuous is best classified by event count + burst duration.

Data-derived constants (implemented in TUI2, with minor UX-driven additions):
- STREAM_GAP_MS = 80
- REDRAW_CADENCE_MS = 16
- DEFAULT_EVENTS_PER_TICK = 3
- DEFAULT_WHEEL_LINES_PER_TICK = 3 (classic wheel feel; UX choice)
- DEFAULT_TRACKPAD_LINES_PER_TICK = 1 (trackpad fidelity; UX choice)
- DEFAULT_WHEEL_TICK_DETECT_MAX_MS = 12 (heuristic tuning; can be overridden)
- DEFAULT_WHEEL_LIKE_MAX_DURATION_MS = 200 (heuristic fallback for 1-event-per-tick terminals)
- MAX_EVENTS_PER_STREAM = 256
- MAX_ACCUMULATED_LINES = 256
- MIN_LINES_PER_WHEEL_STREAM = 1 (guardrail; trackpad streams do not use a minimum)

Per-terminal events-per-line overrides (implemented in TUI2, keyed by TerminalName):
- AppleTerminal = 3
- WarpTerminal = 9
- WezTerm = 1
- Alacritty = 3
- Ghostty = 9
- Iterm2 = 1
- VsCode = 1
- Kitty = 3

Note: overrides are keyed by terminal family, not exact version. Probe data is version-specific,
so we should re-validate as more logs arrive.

Full TL;DR, cross-terminal comparison table, and pseudocode are preserved verbatim in
`codex-rs/tui2/docs/scroll_input_model.md`.

## Implementation status (done)
- Replaced cadence-based scroll tuning with stream-based normalization in
  `codex-rs/tui2/src/tui/scrolling/mouse.rs`.
  - Streams close on gap > 80 ms or direction flip.
  - Wheel speed: wheel-like streams scroll `tui.scroll_wheel_lines` lines per physical wheel tick,
    independent of terminal event density.
  - Trackpad fidelity: trackpad-like streams scroll `tui.scroll_trackpad_lines` lines per
    tick-equivalent and carry fractional remainders across streams.
  - Auto device inference: streams begin trackpad-like and are promoted to wheel-like if the first
    tick-worth of events arrives quickly. There is an end-of-stream fallback for 1-event-per-tick
    terminals (applied only to very small bursts).
  - Guard rails: event count and accumulated lines are clamped.
  - Horizontal scroll events are ignored.
- App integration in `codex-rs/tui2/src/app.rs`:
  - Scroll events now produce a `ScrollUpdate` and schedule a follow-up draw to close streams.
  - Draw ticks call `handle_scroll_tick` to finalize streams without extra frame requests.
- New config hooks (TUI2-only):
  - `tui.scroll_events_per_line` (override normalization factor).
  - `tui.scroll_wheel_lines` (override lines applied per wheel tick).
  - `tui.scroll_trackpad_lines` (override trackpad sensitivity).
  - `tui.scroll_mode` (`auto`/`wheel`/`trackpad`).
  - `tui.scroll_wheel_tick_detect_max_ms` (auto-mode heuristic tuning).
  - `tui.scroll_wheel_like_max_duration_ms` (auto-mode heuristic tuning).
  - `tui.scroll_invert` (invert direction).
  - Wiring in `codex-rs/core/src/config/types.rs` and `codex-rs/core/src/config/mod.rs`.
- Docs:
  - Added `codex-rs/tui2/docs/scroll_input_model.md` with full probe findings and derived model.
  - Linked from `codex-rs/tui2/docs/tui_viewport_and_history.md`.
  - Added config documentation in `docs/config.md`.
- Tests:
  - New scroll stream tests in `codex-rs/tui2/src/tui/scrolling/mouse.rs`.
  - Config default tests updated for new fields in `codex-rs/core/src/config/mod.rs`.

## Current status
- Baseline: stream-based scrolling is implemented and wired into TUI2.
- Current work ("take into account some better statistics") is implemented in the working tree.
  It addresses the two primary UX complaints:
  - Mouse wheel is too slow (often ~3x; ~9x in Warp/Ghostty): fixed by guaranteeing a wheel tick maps
    to `tui.scroll_wheel_lines` lines (default 3) independent of terminal event density, and applying
    that scaling to multi-tick wheel bursts (wheel_small/wheel_long), not just single-tick bursts.
  - Trackpad has a stop-lag / overshoot (notably VS Code + Terminal.app): reduced by removing the
    "minimum +/-1 line" behavior for trackpad-like streams and carrying fractional scroll remainder
    across stream boundaries.

Implementation highlights (current working tree):
- `codex-rs/tui2/src/tui/scrolling/mouse.rs`
  - Normalization is now expressed as events-per-tick (still configured via the historic
    `tui.scroll_events_per_line` key).
  - Auto wheel-vs-trackpad inference is heuristic:
    - Streams start trackpad-like by default (safer: avoids end-of-stream jumps).
    - Promote a stream to wheel-like when the first tick-worth of events arrives quickly
      (`tui.scroll_wheel_tick_detect_max_ms`, default 12ms).
    - Note: the built-in default for `scroll_wheel_tick_detect_max_ms` is now *per-terminal*
      (e.g., Ghostty uses a larger threshold) because Ghostty wheel ticks may arrive spread out
      enough to miss a tight global threshold and feel slow.
    - For terminals that emit ~1 event per tick (WezTerm/iTerm/VS Code), there is no "tick completion"
      signal, so we use a small end-of-stream fallback for very small bursts
      (`tui.scroll_wheel_like_max_duration_ms`, default 200ms).
    - Users can force behavior with `tui.scroll_mode` when the heuristic is wrong.
  - Wheel-like streams flush immediately (not cadence-gated) so the wheel feels snappy.
  - Trackpad-like streams still coalesce redraw to ~60Hz and carry fractional remainder across streams.
  - Trackpad normalization uses a capped "events per tick" value (max 3) instead of the wheel-derived
    `events_per_tick` so Ghostty/Warp trackpad does not become artificially slow.
- `codex-rs/core/src/config/types.rs`, `codex-rs/core/src/config/mod.rs`, `docs/config.md`
  - Added config knobs (see below) and updated documentation.

## Tests and tooling run
- `cargo check -p codex-tui2` (passed).
- `just fmt` (runs with stable warnings about `imports_granularity`; formatting still applied).
- `just fix -p codex-tui2 --allow-no-vcs` (passed).
- `cargo test -p codex-tui2`:
  - New/updated scroll tests pass.
  - Remaining failures are VT100 color expectations in `insert_history` (see below); these are the
    same class of flaky failures previously observed and explicitly deprioritized for this task.

VT100 test failures (flaky; ignored for this task):
- `insert_history::tests::vt100_blockquote_line_emits_green_fg`
- `insert_history::tests::vt100_blockquote_wrap_preserves_color_on_all_wrapped_lines`
- `insert_history::tests::vt100_colored_prefix_then_plain_text_resets_color`
- `insert_history::tests::vt100_deep_nested_mixed_list_third_level_marker_is_colored`

Each failure reports missing non-default foreground colors in VT100 output. No fix was attempted.

## Remaining tasks / next steps
- Run `just fmt` in `codex-rs/` after code changes.
- Run `cargo check -p codex-tui2` and `cargo test -p codex-tui2` to validate.
- Run `just fix -p codex-tui2 --allow-no-vcs` before landing to address clippy/lints.
- Validate feel in at least: Terminal.app + VS Code (trackpad overshoot), and Warp/Ghostty (wheel speed).
- Collect more scroll probe data from additional terminals/versions (and non-macOS) and update overrides
  or heuristics as needed.

Config knobs (TUI2-only):
- `tui.scroll_events_per_line`: override events-per-tick normalization factor (historic name).
- `tui.scroll_wheel_lines`: lines per wheel tick (classic feel; default 3).
- `tui.scroll_trackpad_lines`: trackpad sensitivity (lines per tick-equivalent; default 1).
- `tui.scroll_trackpad_accel_events`: trackpad acceleration events per +1x speed (default 30).
- `tui.scroll_trackpad_accel_max`: trackpad acceleration max multiplier (default 3).
- `tui.scroll_mode`: `auto`/`wheel`/`trackpad`.
- `tui.scroll_wheel_tick_detect_max_ms`: auto-mode promotion threshold (default 12ms).
- `tui.scroll_wheel_like_max_duration_ms`: auto-mode fallback for 1-event-per-tick terminals (default 200ms).
- `tui.scroll_invert`: invert direction.
