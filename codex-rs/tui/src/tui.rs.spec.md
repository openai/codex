## Overview
`codex-tui::tui` encapsulates terminal setup, teardown, and event dispatch for the ratatui frontend. It manages raw mode, alternate screen transitions, keyboard enhancement flags, draw scheduling, and asynchronous event streams consumed by `App`.

## Detailed Behavior
- Terminal lifecycle:
  - `set_modes` enables bracketed paste, raw mode, keyboard enhancement flags, and focus events. It gracefully ignores unsupported enhancement flags.
  - `restore` reverses terminal modes (pop enhancement flags, disable bracketed paste/focus, show cursor).
  - `init` ensures stdout is a terminal, calls `set_modes`, installs a panic hook that restores the terminal on panic, and constructs a `CustomTerminal<CrosstermBackend<Stdout>>`.
- Event abstractions:
  - `TuiEvent` enum covers key presses, paste events, and draw triggers.
  - `Tui` struct stores the terminal, frame scheduler, broadcast channel for draw events, pending history lines, and state for alternate-screen overlays and focus tracking. On Unix it also tracks suspend/resume details for inline viewport realignment.
  - `FrameRequester` schedules draw frames by sending timestamps over an unbounded channel.
- Event processing (`event_stream` detailed in remainder of file):
  - Combines crossterm events (Key, Paste, Focus) with draw scheduling and history line flushes.
  - Supports overlay alt screen toggling (Enter/LeaveAlternateScreen, custom commands for alternate scroll).
  - Tracks terminal focus updates to avoid unnecessary redraws when unfocused.
  - Handles Unix-specific suspend/resume signals (SIGTSTP) by realigning viewports or restoring alternate screens.
- Drawing:
  - `draw_frame` (later in file) renders pending history lines, writes them with ANSI sequences, and instructs ratatui to draw the UI. Uses synchronized updates to reduce flicker.
  - Maintains `pending_history_lines` to avoid duplicate emissions when the terminal is not ready.

## Broader Context
- `run_ratatui_app` creates a `Tui`, passes it to `App::run`, and later restores the terminal. `FrameRequester` is injected into widgets (e.g., chat input) to request redraws after state changes.
- The custom terminal integrates with `custom_terminal::Terminal` wrapper to support inline viewport rendering (preserving scrollback) and alternate screen overlays.
- Context can't yet be determined for cross-platform differences (e.g., Windows support for enhanced keys); the module already guards platform-specific logic.

## Technical Debt
- `Tui` mixes low-level terminal control with history rendering; splitting overlay/scrollback responsibilities into helper structs would simplify reasoning.
- Event stream handling is complex; unit tests or instrumentation for focus/suspend paths would catch regressions.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Refactor event handling into smaller components (keyboard, paste, overlay, suspend) to ease maintenance.
    - Add automated tests or integration harnesses for suspend/resume and focus change handling.
related_specs:
  - ./app.rs.spec.md
  - ./custom_terminal.rs.spec.md
