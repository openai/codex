## Overview
`tui::tui` encapsulates terminal management for the Codex TUI. It configures raw mode, handles bracketed paste/keyboard enhancement flags, manages alternate screen overlays, schedules redraws, and exposes event streams consumed by the application loop.

## Detailed Behavior
- Terminal setup:
  - `set_modes` enables raw mode, bracketed paste, focus events, and (best effort) keyboard enhancement flags.
  - `restore` reverses these changes, showing the cursor and disabling enhancements.
  - `init` verifies stdout is a TTY, sets the panic hook so panics restore the terminal, and constructs a `CustomTerminal<CrosstermBackend>` instance.
- `TuiEvent` enumerates the event stream (`Key`, `Paste`, `Draw`) used by `App`.
- `Tui` holds:
  - Channels for frame scheduling (`frame_schedule_tx`) and draw notifications (`draw_tx`).
  - The Ratatui terminal, pending history lines, and alternate-screen viewport state.
  - Focus/alt-screen flags plus Unix-specific suspend/resume tracking (`ResumeAction`).
- Frame scheduling:
  - `FrameRequester` wraps the scheduling channel, providing `schedule_frame` and `schedule_frame_in`.
  - `frame_stream()` combines scheduled instants with rate limiting to drive redraw cadence.
  - `drain_frame_requests` keeps only the latest pending frame to avoid redundant draws.
- Event loop:
  - `event_stream` merges crossterm events (keys, paste, focus) with draw ticks, normalizing clipboard data and focus changes.
  - `event_loop_task` listens for crossterm events, updates enhanced-key support flags, and forwards focus/blur state via `terminal_focused`.
- History handling:
  - `insert_history_lines` buffers transcript lines until a draw occurs, then flushes them into the inline viewport.
  - Alternate screen helpers (`enter_alt_screen`, `leave_alt_screen`, `restore_inline_viewport`) preserve viewport content when overlays open/close.
- Unix suspend/resume:
  - Uses signal hooks to detect `SIGTSTP`/resume, tracking whether to realign inline mode or restore alt screen.
- Misc utilities expose dimensions, focus state, paste support, and direct access to the underlying `Terminal`.

## Broader Context
- `App` relies on `Tui` for rendering, history insertion, and event delivery. Overlay modules (`pager_overlay`, backtrack) use the alt-screen helpers, while animations/widgets schedule frames via `FrameRequester`.

## Technical Debt
- `Tui` intertwines signal handling, drawing, and event streams in a single type; future refactors could split platform-specific resume logic or render scheduling into dedicated structs.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Consider isolating suspend/resume and frame scheduling into helper modules to simplify testing and future maintenance.
related_specs:
  - ./app.rs.spec.md
  - ./pager_overlay.rs.spec.md
  - ./ascii_animation.rs.spec.md
