# Tmux Scroll Region Handling Design

## Goal
Prevent tmux pane boundary artifacts during history insertion while keeping
inline rendering behavior consistent. Disable delayed-frame scheduling in tmux
to avoid long-running animation pressure.

## Non-Goals
- Rewrite the TUI rendering pipeline.
- Change the default (non-tmux) scrollback behavior.
- Add new user-facing configuration flags.

## Current Behavior Summary
`insert_history_lines` uses scroll-region ANSI sequences (DECSTBM) plus
Reverse Index (`ESC M`) to insert history lines above the viewport. In tmux
this can affect the pane boundary and produce visible artifacts (pane divider
disappears, spacing changes).

## Proposed Changes
1. Add a scroll-region mode to `insert_history_lines`.
   - Default mode keeps existing behavior.
   - A tmux-safe mode avoids scroll-region sequences entirely and instead
     clears the viewport area, appends history lines at the bottom, and then
     re-anchors the viewport for the next redraw.
2. In `Tui`, detect tmux (already available) and route history insertion
   through the tmux-safe mode.
3. Keep tmux deferred-frame scheduling disabled by selecting a large
   `min_frame_interval` (existing behavior) and add test coverage for the
   scroll-region mode to prevent regressions.

## User-Visible Impact
- tmux: same content and interaction, but without pane boundary artifacts.
- non-tmux: unchanged.

## Testing Strategy
- Add unit tests in `tui/src/insert_history.rs` to assert:
  - Scroll-region sequences are emitted in default mode.
  - Scroll-region sequences are absent in tmux-safe mode.

## Risks
- tmux-safe mode may scroll the screen differently; mitigate by clearing the
  viewport before insertion and re-anchoring the viewport for redraw.

