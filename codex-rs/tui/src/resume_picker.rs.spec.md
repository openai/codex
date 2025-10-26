## Overview
Implements the fullscreen “resume previous session” picker shown at startup. It renders an alternate-screen UI that lists recorded rollout logs, supports incremental search, paginates through history via background tasks, and lets users resume, start fresh, or exit entirely.

## Detailed Behavior
- **Entry point (`run_resume_picker`)**:
  - Enters the terminal’s alternate screen via `AltScreenGuard`, spins up an unbounded channel for background page loads, and builds a `PageLoader` closure that invokes `RolloutRecorder::list_conversations` on Tokio tasks.
  - Initializes `PickerState`, loads the first page, and requests a draw. The main loop selects over TUI events and background responses; key handling updates state and may return a `ResumeSelection`. Draw events recompute list metrics before calling `draw_picker`. Background events feed new pages back into the state machine.
  - If the loop ends unexpectedly, defaults to `StartFresh`.
- **State management (`PickerState`)**:
  - Tracks raw rows (`all_rows`), filtered rows (`filtered_rows`), seen paths (deduplication), pagination cursors, search status, selected index, scroll position, and viewport size (row capacity).
  - `handle_key` supports navigation (↑/↓, PgUp/PgDn respecting viewport height, Home/End via helper methods), escape to start fresh, `Ctrl+C` to exit, Enter to resume the highlighted session, and inline search typing. Search updates call `set_query`, which filters in-memory rows and, if empty, initiates asynchronous page loads until matches appear or the scan cap is reached.
  - Pagination states:
    - `PaginationState` keeps `next_cursor`, total scanned files, cap reached flag, and a `LoadingState` (`Idle` vs `Pending` with request/search tokens). `SearchState` tracks active search tokens to ignore stale responses.
    - `load_more_if_needed` issues page requests when nearing the bottom (`LOAD_NEAR_THRESHOLD`) or when searches need more data. `ensure_minimum_rows_for_view` prefetches when the viewport shows fewer rows than available space.
  - `ingest_page` converts `ConversationItem`s into `Row`s (path, preview, created/updated timestamps) and appends them in backend order while deduplicating by path.
  - Filtering uses lowercase substring matching on the preview snippet. Scroll coordination ensures the selected row stays within the visible window; `ensure_selected_visible` clamps `scroll_top` appropriately.
- **Row construction and metrics**:
  - `head_to_row` extracts timestamps from item metadata (preferring explicit `created_at`/`updated_at`, then JSON timestamps in the head) and finds the first user message text via `preview_from_head`.
  - Column metrics calculate width for “Created”/“Updated” columns to align relative time strings, defaulting to `-` when timestamps are absent. `human_time_ago` formats times into friendly labels (seconds/minutes/hours/days).
- **Rendering (`draw_picker`)**:
  - Layout: header, search prompt, column headers, scrollable list, footer hints. Column headers and rows use stylized spans, truncating previews with `truncate_text` to fit the viewport width. Loading indicators and empty-state messages adapt to search state, pagination progress, and scan limits.
  - Footer hints reuse `key_hint` spans for consistent keyboard legend styling.
- **Utilities**:
  - `AltScreenGuard` ensures the alternate screen is exited on drop.
  - `render_offset_content` allows partially visible renderables to copy only the on-screen subset into the main buffer (used in pager overlays but included here for completeness).
- **Tests**:
  - Cover preview extraction, deduplication order, timestamp handling, pagination/search behavior (including request token management and scan caps), scroll math, page navigation step sizes, rendering snapshots (table layout), and empty-state logic.

## Broader Context
- Part of the TUI startup flow; invoked before the main chat interface loads so users can continue previous interactive sessions. Relies on core `RolloutRecorder` metadata and integrates with existing history cell infrastructure for previews.
- Shares styling with other TUI components (`key_hint`, `text_formatting`, `FrameRequester`) and uses the same terminal abstraction (`tui::Tui`) employed elsewhere.

## Technical Debt
- Search performs simple case-insensitive substring matching; richer filtering (e.g., fuzzy search, path-based filters) would require reworking query handling.
- The UI preloads pages sequentially and assumes the backend returns small batches; large histories could benefit from batching or a background prefetch strategy to reduce latency.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Explore fuzzy search or field-specific filters to improve discoverability of past sessions.
    - Prefetch additional pages opportunistically to avoid repeated waits when navigating long histories.
related_specs:
  - tui.rs.spec.md
  - key_hint.rs.spec.md
  - text_formatting.rs.spec.md
