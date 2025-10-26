## Overview
Coordinates debounced `@`-triggered file searches for the TUI composer. `FileSearchManager` captures rapid query edits, ensures only one filesystem search runs at a time, and cancels obsolete searches when the user keeps typing. It delivers eventual results back to the main app thread through `AppEventSender`.

## Detailed Behavior
- Constants cap search behavior: `MAX_FILE_SEARCH_RESULTS` (8) bounds UI payload size, `NUM_FILE_SEARCH_THREADS` (2) feeds the Codex file-search crate, `FILE_SEARCH_DEBOUNCE` (100 ms) delays the first run after a keystroke, and `ACTIVE_SEARCH_COMPLETE_POLL_INTERVAL` (20 ms) polls for in-flight completion before launching the next run.
- `FileSearchManager` holds shared `SearchState` behind a mutex, the workspace root (`search_dir`), and the outbound `AppEventSender`.
- `on_user_query` acquires the state:
  - Ignores identical queries to avoid redundant timers.
  - Cancels the active search when the new query is no longer a prefix of the in-flight query, flipping the cancellation token and clearing `active_search`.
  - Schedules a debounce when no timer is pending by setting `is_search_scheduled`, then spawns a thread that sleeps for the debounce window, waits for any active search to clear, and finally copies the latest query while installing a fresh `ActiveSearch`.
- `spawn_file_search` runs the heavy `codex_file_search::run` in a worker thread, configured with the limit and thread count, and re-uses the cancellation token so the search implementation can short-circuit. On success it emits `AppEvent::FileSearchResult { query, matches }` unless cancellation fired. Afterward it clears `active_search` only if the pointer identity matches, preventing stale workers from stomping newer state.

## Broader Context
- `AppEvent::StartFileSearch` events originate from the composer (`chat_composer.rs`), and `App` consumes the resulting `FileSearchResult` to populate suggestion lists.
- The search implementation comes from the `codex-file-search` crate, shared with backend tooling, so this manager acts purely as an orchestration layer around debounce and cancellation.

## Technical Debt
- Uses raw threads rather than async tasks; moving to an async executor or shared thread pool could simplify lifecycle management.
- Debounce polling waits for `active_search` to clear with a busy sleep; wiring cancellation or completion callbacks would reduce latency and CPU churn.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Replace the polling loop with a completion notification so the manager can react immediately when searches finish.
    - Consider integrating with an async runtime or a shared thread pool to reduce thread spawning overhead.
related_specs:
  - mod.spec.md
  - app_event.rs.spec.md
