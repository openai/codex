## Overview
`bottom_pane` manages the lower portion of the TUI, including the chat composer, status indicator, and modal overlays (approvals, selection lists, custom prompts). It routes input events, renders the active view, and coordinates task-running UI hints.

## Detailed Behavior
- Main types:
  - `BottomPane` owns a `ChatComposer`, a stack of `BottomPaneView` implementations, status indicator widget, queued user messages, and focus/task state flags.
  - `BottomPaneParams` seeds the pane with event senders, frame scheduler, placeholder text, and configuration toggles (enhanced keys, paste burst).
  - `CancellationEvent` communicates whether `Ctrl-C` handling was consumed by the active view.
- Event handling:
  - `handle_key_event` forwards keys to the active view, handles `Esc` for approvals/status interrupts, manages paste burst scheduling, and returns `InputResult` from the composer when appropriate.
  - `on_ctrl_c` gives modal views a chance to handle `Ctrl-C`; otherwise clears the composer and shows quit hints.
  - `handle_paste`, `insert_str`, `set_composer_text`, `clear_composer_for_ctrl_c`, `composer_text` manipulate the composer when no modal is active.
- Status and hints:
  - `set_task_running` shows/hides the status indicator, updates queued user messages, and coordinates timer pausing when modals appear.
  - Hint methods (`show_ctrl_c_quit_hint`, `show_esc_backtrack_hint`, etc.) synchronize composer tooltips with app-level state.
  - `set_context_window_percent` feeds context metrics into the composer.
- Views and overlays:
  - `push_view`, `show_view`, `show_selection_view`, `push_approval_request` manage modal overlays, pausing status timers as needed and resuming them when views complete.
  - Approval flow tries to let existing modals consume new requests; otherwise it pushes an `ApprovalOverlay`.
- Rendering/layout:
  - `desired_height`, `layout`, and `cursor_pos` compute how much vertical space is needed, splitting between status and composer or dedicating the area to modal views.
  - Implements `WidgetRef` for direct rendering.
- History/attachments:
  - `set_history_metadata`, `on_history_entry_response`, and paste burst helpers coordinate with the composer to display fetched history lines.
  - `on_file_search_result`, `attach_image`, `take_recent_submission_images` expose attachment and file-search integration.
- State queries signal backtrack eligibility (`is_normal_backtrack_mode`), task state, and popup activity to the app layer.

## Broader Context
- `App` directs user input and status updates through `BottomPane`, while other modules (e.g., backtrack, approvals) rely on its view stack to present interactive overlays.

## Technical Debt
- The module centralizes many responsibilities (composer interaction, modal stack, status indicator); separating modal management into its own helper could simplify event routing and future extensions.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Extract a dedicated modal/view manager to reduce the amount of conditional logic in `BottomPane::handle_key_event` and colleagues.
related_specs:
  - ./chat_composer.rs.spec.md
  - ./approval_overlay.rs.spec.md
  - ./command_popup.rs.spec.md
