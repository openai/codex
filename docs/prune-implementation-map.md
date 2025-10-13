# Prune Implementation Map (TUI + Core)

This document maps every moving piece of the (re)introduced prune feature so it’s easy to re‑apply after syncing with upstream. All paths below are workspace‑relative.

Goals:
- Advanced prune works end‑to‑end: include/exclude (non‑destructive) + delete (destructive).
- Changes take effect immediately in core (next turn uses the filtered context).
- On shutdown, the TUI rewrites the rollout (best‑effort) to persist the current inclusion state.

---

## High‑Level Data Flow

1) TUI (Advanced) builds a plan (to_include, to_exclude, to_delete).
2) On confirm:
   - Send `Op::SetContextInclusion { indices: to_include, included: true }`.
   - Send `Op::SetContextInclusion { indices: to_exclude, included: false }`.
   - Send `Op::PruneContextByIndices { indices: to_delete }`.
3) Core updates its in‑memory session state and emits an updated `ContextItems` event.
4) Next turn input is built from the filtered history (respecting the inclusion mask).
5) On graceful exit (shutdown): TUI rewrites the rollout JSONL using the current inclusion set and saves a `.bak` alongside.
6) “Restore full context” reopens the session from the `.bak` after validating the conversation id.

---

## Core Changes (codex‑rs/core)

### Session state (include mask + helpers)

- File: `core/src/state/session.rs`

- Added field:
  - `include_mask: Option<BTreeSet<usize>>` on `SessionState`.
  - None → all items included; Some(set) → only indices in `set` are included.

- Added/modified methods:
  - `filtered_history(&self) -> Vec<ResponseItem>`
    - Returns a view of history after applying the `include_mask`.
  - `record_items(&mut self, items: I)` (SessionState wrapper)
    - After appending to history, automatically inserts new indices into `include_mask` (when present) so fresh items remain included by default.
  - `ensure_mask_all_included(&mut self)`
    - Initializes `include_mask` as 0..len when first needed.
  - `set_context_inclusion(&mut self, indices: &[usize], included: bool)`
    - Non‑destructive include/exclude; ignores out‑of‑range indices.
  - `prune_by_indices(&mut self, indices: Vec<usize>)`
    - Destructive delete; removes items in descending index order; shifts `include_mask` safely.
  - `prune_by_categories(&mut self, categories: &[PruneCategory], range: &PruneRange)`
    - Marks matching indices as excluded (range is effectively `All`).
  - `build_context_items_event(&self) -> ContextItemsEvent`
    - Produces `{ index, category, preview, included }` for the TUI.
  - `replace_history(&mut self, items: Vec<ResponseItem>)`
    - Resets the `include_mask` (indices changed).

- Categorization/preview helpers (same file):
  - `categorize(item: &ResponseItem) -> Option<PruneCategory>`
    - Maps messages to User/Assistant/Reasoning/ToolCall/ToolOutput; detects XML blocks
      (`<environment_context>`, `<user_instructions>`) to EnvironmentContext/UserInstructions.
  - `preview_for(item: &ResponseItem) -> String`
    - Single‑line short preview; handles `WebSearchAction::Other` exhaustively.

### Prompt input respects inclusion

- File: `core/src/codex.rs`
  - Method: `turn_input_with_history(&self, extra: Vec<ResponseItem>) -> Vec<ResponseItem>`
  - Change: use `state.filtered_history()` instead of `state.history_snapshot()`.
  - Effect: immediate impact of Advanced prune on the next turn.

### Reintroduced Op handlers

- File: `core/src/codex.rs` (inside `submission_loop` match over `Op`)
  - `Op::GetContextItems` → emit `EventMsg::ContextItems(state.build_context_items_event())`.
  - `Op::SetContextInclusion { indices, included }` → `state.set_context_inclusion(...)` then emit updated `ContextItems`.
  - `Op::PruneContextByIndices { indices }` → `state.prune_by_indices(indices)` then emit updated `ContextItems`.
  - `Op::PruneContext { categories, range }` → `state.prune_by_categories(...)` then emit updated `ContextItems`.

Notes:
- All mutations finish by emitting `ContextItems` so UIs that don’t keep local toggles can refresh.

### Rollout persistence policy (noise control)

- File: `core/src/rollout/policy.rs`
  - `should_persist_event_msg(ev: &EventMsg) -> bool` excludes prune‑info events from rollout:
    - `EventMsg::ContextItems(_)` → not persisted
    - `EventMsg::ConversationUsage(_)` → not persisted
  - Rationale: these are ephemeral UI/support events and would bloat rollout files; the functional state is captured by the conversation items and, on exit, the rewritten rollout.

---

## TUI Changes (codex‑rs/tui)

### Advanced apply → signal core

- File: `tui/src/chatwidget.rs`
  - Method: `apply_advanced_prune()`
  - Behavior:
    - For include set → `Op::SetContextInclusion{ included: true }`.
    - For exclude set → `Op::SetContextInclusion{ included: false }`.
    - For delete set → `Op::PruneContextByIndices`.
  - Rationale: prior code only deleted; include/exclude did nothing in core (regression fixed).

### Menu and views

- File: `tui/src/chatwidget.rs`
  - Advanced view (space=toggle keep, del=toggle delete, enter=apply).
  - Root menu `/prune`: "Advanced prune", "Manual prune", "Restore full context".
  - Manual prune by category: calls `Op::PruneContext{ categories, range: All }`.
  - `open_prune_advanced()` renders immediately when a recent `last_context_items` cache exists, and concurrently refreshes via `Op::GetContextItems` (prevents the UI from feeling frozen if the core response is delayed).

- File: `tui/src/app.rs`
  - Event wiring added:
    - `AppEvent::PruneRootClosed` → `chatwidget.on_prune_root_closed()`
    - `AppEvent::OpenPruneManualConfirm { category, label }` → `chatwidget.open_prune_manual_confirm(...)`
  - Rationale: ensures the manual‑confirm popup and root‑closed bookkeeping actually execute (previously defined but not routed → dead code warnings).

### Persistence & Restore (TUI‑only)

- File: `tui/src/app.rs`
  - `finalize_prune_on_shutdown()`
    - Backs up rollout to `.bak`, rewrites JSONL keeping only included `ResponseItem`s (+ preserves first `SessionMeta` and `TurnContext`). Never blocks exit.
  - `restore_context_from_backup()`
    - Validates conversation id in `.bak` and resumes the session from it; posts info/error messages accordingly.

---

## Documentation Notes

- File: `codex-rs/README.md`
  - Describes Advanced prune applying immediately in core, manual prune by category, and the “Restore full context” option.

- File: `docs/advanced.md`
  - Documents Advanced controls and “Restore full context” from `.bak`.

---

## Re‑Apply Checklist After Upstream Sync

1) Core state & prompt:
   - Re‑add `include_mask` to `SessionState` with helper methods above.
   - Ensure `turn_input_with_history()` uses `filtered_history()`.

2) Core Op handlers in `submission_loop`:
   - Re‑add match arms for `GetContextItems`, `SetContextInclusion`, `PruneContextByIndices`, `PruneContext`.
   - Emit updated `ContextItems` after each mutation.

3) TUI:
   - `apply_advanced_prune()` must send `SetContextInclusion` (include/exclude) and `PruneContextByIndices` (delete).
   - Root menu includes "Advanced prune", "Manual prune" e "Restore full context".

4) Persistence & Restore (optional, but recommended):
   - Keep `finalize_prune_on_shutdown()` and `restore_context_from_backup()` aligned with protocol types (RolloutItem layout).

5) Docs:
   - Keep README and `docs/advanced.md` synchronized with the behavior above.

---

## Grep Anchors (fast navigation)

- Core Ops block: `core/src/codex.rs` → `// --- Context prune (experimental) ---`
- Include mask & helpers: `core/src/state/session.rs` → `include_mask`, `filtered_history`, `build_context_items_event`.
- Prompt filter hook: `core/src/codex.rs` → `turn_input_with_history`.
- TUI apply: `tui/src/chatwidget.rs` → `apply_advanced_prune`.
- TUI event wiring: `tui/src/app.rs` → `OpenPruneManualConfirm`, `PruneRootClosed`.
- TUI persistence: `tui/src/app.rs` → `finalize_prune_on_shutdown`, `restore_context_from_backup`.

---

## Invariants & Safety

- Indices:
  - Always ignore out‑of‑range indices.
  - Delete in descending order and then shift mask indices safely.
- Exhaustive matches:
  - Handle new `ResponseItem`/`WebSearchAction` variants to keep `categorize`/`preview` total.
- Rollout rewrite:
  - Best‑effort; never block shutdown; `.bak` is always created first.

---

## Notes / Limitations

- `ContextItems` is a lightweight snapshot (approximate preview, not a structured diff).

---

## Quick Testing Tips

- TUI manual sanity:
  - Open Advanced, toggle a few items, confirm.
  - Run a new turn; check the model context reflects the changes (e.g., removed tool outputs don’t reappear).
  - Try delete; ensure items disappear; try “Restore full context” and verify it recovers from `.bak`.

- Unit targets (suggested):
  - `set_context_inclusion`, `prune_by_indices` (mask shifting), `build_context_items_event` (included flag & categories).
