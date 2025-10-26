## Overview
`streaming::controller` manages incremental message streaming in the TUI. It buffers deltas, commits full lines as they arrive, and emits `HistoryCell`s that animate agent messages with headers.

## Detailed Behavior
- `StreamController` wraps a `StreamState`, tracking whether headers have been emitted and whether finalization should trigger post-drain cleanup.
- `new(width)` seeds the underlying collector with an optional wrap width, enabling consistent line breaking with markdown rendering.
- `push(delta)` appends text to the collector, marking that a delta was seen. When the delta contains `\n`, it commits complete lines and enqueues them for animation, returning `true` when new content is ready.
- `on_commit_tick()` advances the animation by one step: drains at most one queued line and returns the resulting `HistoryCell` plus a boolean indicating whether the controller is idle.
- `finalize()` drains any remaining buffered content, clears state, and returns a single `AgentMessageCell` (or `None` when empty), ensuring headers are only emitted when content exists.
- `emit(lines)` wraps lines in `AgentMessageCell`, toggling the header flag so only the first cell renders the agent header.
- Unit tests confirm streaming output matches full markdown rendering when commit ticks fire between each delta.

## Broader Context
- Used by the streaming controller in `App` to animate model messages as they arrive from codex-core, coordinating with history cells and bottom-pane widgets.

## Technical Debt
- The controller depends on `StreamState` for queue management; documenting that type (in `streaming/mod.rs`) alongside this module helps maintain a complete picture of streaming behavior.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ../../history_cell.rs.spec.md
  - ../../markdown.rs.spec.md
