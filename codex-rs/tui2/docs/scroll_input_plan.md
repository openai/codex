# TUI2 Scroll Input Plan and Status

This status update is meant to be re-read in a future Codex session. It summarizes the work
completed, the data used, and what is still pending. Full probe findings and the derived algorithm
are preserved in `codex-rs/tui2/docs/scroll_input_model.md`.

## Goal
Replace the prior cadence-based scroll heuristics with a stream-based normalization model that is
stable across terminals and devices, using the data-derived constants and per-terminal overrides
from the scroll-probe logs.

## Data findings (from probe logs)
- Logs analyzed: 16 (13,734 events) across Apple Terminal, Warp, WezTerm, Alacritty, Ghostty,
  iTerm2, VS Code, Kitty.
- Raw event counts per wheel tick vary widely by terminal (1, 3, 9+). Timing alone cannot reliably
  distinguish wheel vs trackpad; burst duration and event counts are more reliable.
- Horizontal scroll events appear in trackpad scenarios only (WezTerm/Ghostty/Kitty) and must be
  ignored for vertical scrolling.
- Discrete vs continuous is best classified by event count + burst duration.

Data-derived constants (implemented in TUI2):
- STREAM_GAP_MS = 80
- DISCRETE_MAX_EVENTS = 10
- DISCRETE_MAX_DURATION_MS = 250
- REDRAW_CADENCE_MS = 16
- DEFAULT_EVENTS_PER_LINE = 3
- DEFAULT_WHEEL_LINES_PER_TICK = 3 (classic wheel feel; UX choice)
- MAX_EVENTS_PER_STREAM = 256
- MAX_ACCUMULATED_LINES = 256
- MIN_LINES_PER_DISCRETE_STREAM = 1

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
  - Discrete streams apply accumulated lines on close; minimum +/-1 line when rounding to zero.
  - Discrete streams are scaled by `DEFAULT_WHEEL_LINES_PER_TICK` (default 3) to restore classic
    wheel speed; configurable via `tui.scroll_wheel_lines`.
  - Continuous streams accumulate fractional lines and flush at 60 Hz cadence.
  - Guard rails: event count and accumulated lines are clamped.
  - Horizontal scroll events are ignored.
- App integration in `codex-rs/tui2/src/app.rs`:
  - Scroll events now produce a `ScrollUpdate` and schedule a follow-up draw to close streams.
  - Draw ticks call `handle_scroll_tick` to finalize streams without extra frame requests.
- New config hooks (TUI2-only):
  - `tui.scroll_events_per_line` (override normalization factor).
  - `tui.scroll_wheel_lines` (override lines applied per wheel tick).
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
- Working copy commit: `feat(tui2): implement stream-based scrolling` (jj @).
- Changes are in the files listed in the previous section.

## Tests and tooling run
- `just fmt` (warnings about `imports_granularity` on stable; formatting still applied).
- `cargo check -p codex-tui2` (passed).
- `just fix -p codex-tui2 --allow-no-vcs` (passed; required because `jj` has no VCS metadata).
- `just fmt` (re-run after `just fix`).
- `cargo test -p codex-tui2` (failed; see below).
- `cargo test --all-features` (failed; see below).

Test failures (unchanged files, likely unrelated to scroll changes; user flagged as flaky to ignore):
- `insert_history::tests::vt100_blockquote_line_emits_green_fg`
- `insert_history::tests::vt100_blockquote_wrap_preserves_color_on_all_wrapped_lines`
- `insert_history::tests::vt100_colored_prefix_then_plain_text_resets_color`
- `insert_history::tests::vt100_deep_nested_mixed_list_third_level_marker_is_colored`

Each failure reports missing non-default foreground colors in VT100 output. The user indicated these
are flaky; no fix was attempted.

Additional full-suite failure (likely unrelated to scroll changes):
- `suite::send_message::test_send_message_raw_notifications_opt_in` in `codex-app-server` failed.
  The test expected the developer instruction message, but received the environment context message
  first. Error from `app-server/tests/suite/send_message.rs:324`.

## Remaining tasks / next steps
- Optionally re-run `cargo test -p codex-tui2` if we need to confirm the flaky VT100 tests.
- Decide whether to re-run or adjust for the `codex-app-server` raw notification test failure.
- Re-run `cargo check -p codex-tui2` if additional fixes are made.
- Collect more scroll probe data from additional terminals and versions; update overrides as needed.
