## Overview
Provides a deterministic `VT100Backend` for testing Ratatui widgets. It wraps `CrosstermBackend<vt100::Parser>` so rendering code can execute without touching stdout, while exposing the parsed screen contents for assertions and snapshots.

## Detailed Behavior
- Constructor `new(width, height)` seeds a `vt100::Parser` with the desired dimensions and builds a `CrosstermBackend`.
- `vt100()` exposes the parser so tests can inspect the virtual terminal screen (e.g., colors, cursor position).
- Implements `std::io::Write`, forwarding writes/flushes to the parser.
- `Display` renders the screen’s text content, enabling concise snapshot comparisons (`format!("{backend}")`).
- Implements Ratatui `Backend`:
  - Delegates drawing, cursor control, clear operations, and scrolling to the underlying `CrosstermBackend`.
  - `get_cursor_position`, `size`, and `window_size` read from the `vt100` screen instead of issuing terminal queries, ensuring tests remain pure.
  - `window_size` returns consistent pixel dimensions (arbitrary 640×480) because tests only need column/row counts.

## Broader Context
- Used across TUI tests (chat widgets, status overlays, history insertion) to snapshot widget output without relying on actual terminal I/O.
- Complements the custom terminal wrapper (`custom_terminal.rs`) by providing a fully in-memory backend for verifying ANSI behavior.

## Technical Debt
- Pixel dimensions are hard-coded; tests relying on real DPI metrics would need enhancements.
- Backend does not simulate terminal behaviors like resize events; tests requiring dynamic geometry must mock those separately.

---
tech_debt:
  severity: low
  highest_priority_items:
    - Extend the backend to simulate resize events if future tests need to verify responsive layouts.
related_specs:
  - custom_terminal.rs.spec.md
  - insert_history.rs.spec.md
