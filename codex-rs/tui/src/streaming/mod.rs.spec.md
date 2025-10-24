## Overview
`codex-tui::streaming` manages streamed Markdown output from the agent. It packages a `MarkdownStreamCollector` alongside a queue of completed lines and exposes a controller (in `controller.rs`) that emits history cells incrementally.

## Detailed Behavior
- `StreamState`:
  - Holds a `MarkdownStreamCollector` configured with optional wrap width.
  - Maintains a `VecDeque<Line<'static>>` for queued completed lines.
  - Tracks whether any delta has been seen (`has_seen_delta`) to detect empty streams.
  - Methods:
    - `new(width)`, `clear()` reset collector and queue.
    - `step()` pops the next queued line (used for animation ticks).
    - `drain_all()` drains the queue entirely (used when finishing).
    - `is_idle()` checks whether there are lines pending.
    - `enqueue(lines)` appends rendered lines.
- `controller` (see `controller.rs.spec.md`) builds on `StreamState` to handle newline-gated commits, header emission, and animation scheduling.

## Broader Context
- `ChatWidget` instantiates a `StreamController` to stream agent messages, ensuring partial lines are only displayed when complete.
- Markdown rendering relies on `markdown_stream` to convert partial deltas into formatted lines.
- Context can't yet be determined for multi-stream scenarios (e.g., separate reasoning vs message streams); the state struct provides the building blocks for additional controllers if needed.

## Technical Debt
- None; the module’s responsibilities are narrow.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./controller.rs.spec.md
  - ../markdown_stream.rs.spec.md
