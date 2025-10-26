## Overview
Manages the full-screen pager overlay used for transcript review and static documents. It encapsulates navigation, rendering, and keyboard handling for both scrollable transcripts and ad-hoc static content overlays while displaying consistent key hints.

## Detailed Behavior
- `Overlay` enum switches between `TranscriptOverlay` and `StaticOverlay`, delegating event handling and completion checks.
- Shared key bindings (↑/↓, PgUp/PgDn, Home/End, Space, `q`, `esc`, `enter`, `Ctrl+C`, `Ctrl+T`) power navigation and dismissal. `render_key_hints` prints these as dimmed spans on footer lines using `key_hint::KeyBinding`.
- `PagerView` renders a list of `Renderable` chunks with a top header slash motif, scrollable content area, and bottom status bar:
  - Maintains `scroll_offset`, caches the last measured content height, and tracks pending “scroll chunk into view” requests for focus jumps.
  - `render` clears the area, draws the header, adjusts scroll bounds, renders visible chunks (using `render_offset_content` for partially visible items), fills blank rows with `~`, and writes a bottom bar showing percentage scrolled.
  - Keyboard events adjust `scroll_offset` by rows or pages; scheduling a frame via `FrameRequester` keeps the overlay responsive.
  - `is_scrolled_to_bottom` checks offsets against total height to decide whether to auto-follow new content.
- `CachedRenderable` memoizes height per width so repeated `desired_height` calls avoid recomputation.
- `CellRenderable` adapts a `HistoryCell` into a `Renderable`, applying user message styling and optional highlight (reversed) for editing cues. Non-stream-continuation cells receive top padding via `InsetRenderable`.
- `TranscriptOverlay`:
  - Builds the pager view with transcript cells and default scroll offset `usize::MAX` to start at the bottom.
  - `render` splits the screen into content and footer, rendering key hints (`quit`, `edit prev`, conditional `enter` hint when highlighting).
  - `insert_cell` appends history, respecting the bottom-follow state to keep the view pinned unless the user scrolled away.
  - `set_highlight_cell` updates styles and ensures the highlighted chunk scrolls into view.
  - Event handler listens for navigation keys and termination keys (`q`, `Ctrl+C`, `Ctrl+T`), and triggers full redraws on `TuiEvent::Draw`.
- `StaticOverlay` mirrors the structure but renders a supplied set of lines or renderables and shows a simpler footer (`q` to quit).
- `render_offset_content` supports partial rendering by drawing into a temporary buffer and copying the visible slice, allowing smooth scrolling within tall renderables.
- Tests snapshot pager layout, verify hints, ensure scroll retention when new cells arrive, and confirm chunk focusing logic plus percentage calculations. Transcript-specific tests cover patch events, approval decisions, and exec output to guarantee consistent rendering.

## Broader Context
- The pager overlay is invoked when users view prior conversation transcripts or inspect static documents (e.g., release notes). It ties together history cells, renderable infrastructure, and global TUI event routing.
- Integrates with key hint styling (`key_hint.rs`) and user message theming (`style.rs`), ensuring overlays remain visually aligned with the rest of the interface.

## Technical Debt
- Footer hints are hard-coded; dynamic action contexts (e.g., different keys for static overlays) require manual updates.
- Rendering large transcripts loads all cells into memory and renders them sequentially; incremental virtualization could improve performance for extremely long histories.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Consider virtualizing transcript rendering for very large conversations to avoid quadratic rendering costs.
    - Centralize footer hint configuration so overlays can declaratively specify available actions.
related_specs:
  - history_cell/mod.rs.spec.md
  - render/renderable.rs.spec.md
  - key_hint.rs.spec.md
