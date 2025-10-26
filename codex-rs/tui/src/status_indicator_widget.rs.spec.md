## Overview
Displays the live status indicator during long-running operations. Shows a spinner, animated header, elapsed time, an interrupt hint, and a list of queued user messages. Also manages timing state so it can pause/resume when the agent stops or resumes work.

## Detailed Behavior
- Constructor `new` takes an `AppEventSender` and `FrameRequester`, initializing the header to “Working”, zero elapsed time, and scheduling frames via the requester.
- Timer controls:
  - `pause_timer_at` and `resume_timer_at` adjust `elapsed_running` and `last_resume_at`, guarding against double pause/resume.
  - `elapsed_duration_at`/`elapsed_seconds_at` compute elapsed wall-clock time, respecting the paused state. `fmt_elapsed_compact` converts seconds into strings like `1m 05s` or `2h 03m 09s`.
- Interaction:
  - `interrupt` sends a `CodexOp::Interrupt` via `AppEvent`.
  - `update_header`, `set_queued_messages`, `pause_timer`, and `resume_timer` mutate state and schedule redraws when needed.
  - `desired_height` estimates the vertical footprint, accounting for wrapped queued messages (up to three lines plus ellipsis per message) and key hints.
- Rendering (`WidgetRef` impl):
  - Schedules the next animation frame every 32 ms.
  - Builds the first line with a spinner icon, shimmered header (using `shimmer_spans`), elapsed time, and an `Esc` interrupt hint via `key_hint`.
  - For queued messages, wraps text using `textwrap::wrap`, prefixes the first line with `↳`, dims/italicizes content, and appends ellipses when truncated. Adds an `Alt+Up edit` hint if messages are present.
  - Renders via a simple `Paragraph` without borders to blend with terminal scrollback.
- Tests cover elapsed time formatting, rendering under different widths (with snapshots), message wrapping, and timer pause/resume behavior.

## Broader Context
- Used while Codex executes plans, providing at-a-glance status without leaving the chat view.
- Interacts with the same spinner and shimmer utilities as other animated widgets to maintain consistent visuals.

## Technical Debt
- `textwrap` wrapping runs on every render; caching wrapped output could reduce allocations when messages change infrequently.
- Animation cadence is fixed at ~30 FPS; adaptive scheduling could reduce CPU usage when static.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Cache wrapped queued messages to avoid recomputation every frame, especially for long queues.
related_specs:
  - shimmer.rs.spec.md
  - key_hint.rs.spec.md
