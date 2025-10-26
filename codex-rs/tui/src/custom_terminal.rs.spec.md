## Overview
This module rehosts `ratatui::Terminal` so Codex can control how screen diffs are flushed when the UI runs in an inline viewport. It tracks both the visible region and the last cursor position, allowing the TUI to redraw only the necessary cells while keeping scrollback intact. A custom diff routine emits fewer control sequences than the upstream implementation, which reduces flicker when wide characters or stylized spans change near the right edge.

## Detailed Behavior
- **Frame façade**: `Frame` owns a mutable view of the active `Buffer`, exposes the viewport `Rect`, and forwards widget rendering through `render_widget_ref`. Callers set `cursor_position` on the frame so the terminal can restore the caret after flushing.
- **Lifecycle**: `Terminal::with_options` captures the backend, seeds two off-screen buffers, and records the initial viewport anchored to the current cursor row. `get_frame` returns a wrapper over the active buffer; `try_draw` (and `draw`) resize the buffers if the terminal grew or shrank, run the render closure, flush, then either hide or reposition the cursor before swapping buffers and flushing the backend.
- **Viewport & clearing**: `set_viewport_area` resizes both buffers to match the inline viewport so history remains in the host terminal’s scrollback. `clear` and `swap_buffers` reset the inactive buffer so the next frame repaints the entire area. `autoresize` compares the backend size with `last_known_screen_size` to detect when `set_viewport_area` should be re-run.
- **Cursor management**: `hide_cursor`, `show_cursor`, and `set_cursor_position` track `hidden_cursor` and `last_known_cursor_pos`; `Drop` attempts to show the cursor again if rendering exits unexpectedly.
- **Custom diff pipeline**: `flush` calls `diff_buffers` when `use_custom_flush` is true. The diff scanner walks each row to find the last meaningful column (considering glyphs, background colors, and modifiers) and emits a `ClearToEnd` command for trailing whitespace. Remaining cells become `Put` commands, taking care to invalidate areas affected by multi-width symbols. `draw` batches those commands, only sending attribute changes (`ModifierDiff`) when the foreground, background, or modifiers change.
- **Tests**: The inline tests assert that diffing does not erase characters at the edge of the viewport and that wide-character truncation clears the correct region, which protects the custom algorithm from regressions.

## Broader Context
- The TUI runtime ([`tui.rs`](tui.rs.spec.md)) initializes this terminal so UI widgets can render into the inline viewport while Codex’s shell history stays scrollable.
- Preview flows such as `resume_picker` and history insertion utilities render into this terminal during background operations, relying on the custom diff to avoid repaint storms.
- Integration tests under `chatwidget/tests.rs` construct the terminal with VT100 backends to validate layout and wrapping behavior.

## Technical Debt
- Maintaining a fork of `ratatui::Terminal` means upstream bug fixes (for example, Unicode handling or cursor semantics) will not arrive automatically; we must periodically audit upstream changes and port relevant fixes.
- The diff algorithm is guarded by a couple of regression tests, but it lacks coverage for zero-width joiners or emoji sequences; extending tests to those cases would harden future changes.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Periodically diff against upstream `ratatui::Terminal` to pull in fixes, especially around Unicode edge cases.
    - Expand diff tests to cover zero-width joiners and emoji sequences so multi-codepoint glyphs stay safe.
related_specs:
  - ../mod.spec.md
  - tui.rs.spec.md
