## Overview
`approval_overlay` presents modal dialogs when Codex requests user approval for shell commands or apply-patch operations. It queues multiple requests, renders summaries (diffs/commands), and dispatches decisions back to the app.

## Detailed Behavior
- `ApprovalRequest` variants represent exec and apply-patch approvals, capturing IDs, commands, reasons, working directory, and file changes.
- `ApprovalOverlay` maintains:
  - `current_request`/`current_variant`: the active request and derived display data (`ApprovalVariant`).
  - `queue`: pending requests awaiting user attention.
  - `list`: a `ListSelectionView` with approval options (`Approve`, `Deny`, etc.).
  - `options`: metadata for each list entry, including keyboard shortcuts and review decisions.
- Workflow:
  - `new(request, app_event_tx)` initializes the overlay, building the first set of options and header content (command highlight or diff summary) via `build_options`.
  - `enqueue_request` adds subsequent requests to the queue; `advance_queue` rotates to the next item when the current one completes.
  - `handle_key_event` delegates to the selection view and applies decisions on Enter; `on_ctrl_c` dismisses the overlay when allowed.
  - `apply_selection(actual_idx)` triggers `handle_exec_decision` or `handle_patch_decision`, inserting history cells and sending `AppEvent::CodexOp` with the chosen `ReviewDecision`.
  - Special handling for `Ctrl-A` opens a fullscreen approval view via `AppEvent::FullScreenApprovalRequest`.
- Rendering:
  - Implements `BottomPaneView`, delegating rendering to the internal `ListSelectionView` and exposing `cursor_pos`.
  - Cleanly handles multiple approvals by resetting state (`set_current`) after each decision.

## Broader Context
- `BottomPane` pushes `ApprovalOverlay` when it receives `ApprovalRequest`s from the core; the overlay pauses status timers and resumes them once the modal completes.

## Technical Debt
- Exec and patch variants share similar option handling; consider abstracting shared decision logic to reduce duplication and simplify adding new approval types.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Consolidate shared option-building/decision dispatch code between exec and patch variants.
related_specs:
  - ./mod.rs.spec.md
  - ./list_selection_view.rs.spec.md
