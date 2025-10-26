## Overview
`bottom_pane_view` defines the trait implemented by modal overlays displayed in the bottom pane (approval dialog, selection lists, etc.). It extends `Renderable` so views can render themselves while providing lifecycle hooks for keyboard and paste input.

## Detailed Behavior
- `BottomPaneView` inherits from `Renderable` and exposes optional overrides:
  - `handle_key_event` for per-view key handling.
  - `is_complete` to signal when the view should be popped off the stack.
  - `on_ctrl_c` to consume `Ctrl-C` (returning `CancellationEvent::Handled`).
  - `handle_paste` to accept pasted text.
  - `cursor_pos` to expose the cursor location when the view is active.
  - `try_consume_approval_request` so an existing modal can swallow incoming approval prompts instead of spawning a new overlay.

## Broader Context
- `BottomPane` stores `Box<dyn BottomPaneView>` instances on a stack; concrete implementations (e.g., `ApprovalOverlay`, `ListSelectionView`) implement this trait.

## Technical Debt
- Trait remains minimal; no additional debt identified.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./mod.rs.spec.md
  - ./list_selection_view.rs.spec.md
  - ./approval_overlay.rs.spec.md
