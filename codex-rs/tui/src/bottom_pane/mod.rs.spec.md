## Overview
`codex-tui::bottom_pane` renders the lower-half composer area (chat input, popups, approvals). It manages the active view stack, handles keyboard events, and coordinates status indicators while the chat widget occupies the upper transcript.

## Detailed Behavior
- Core struct `BottomPane`:
  - Owns a `ChatComposer` (text input), a stack of modal `BottomPaneView`s (popups, selection lists, approval overlays), and state flags (input focus, task running, hints).
  - Maintains an optional `StatusIndicatorWidget` to show progress above the composer, along with queued user messages and context window usage.
  - Uses `AppEventSender` to push UI actions back to the app loop and `FrameRequester` to request redraws.
- Layout and rendering:
  - `desired_height(width)` computes the total height needed (status indicator + composer or active view) plus margins.
  - `layout(area)` splits the pane into status vs composer sections, or uses the full area for popups.
  - `cursor_pos(area)` hides the cursor when overlays are active; otherwise delegates to the composer.
- Event handling:
  - `handle_key_event` routes key events to the active view or composer. ESC close semantics (`CancellationEvent`) pop overlays when complete.
  - `push_view`, `pop_view`, and `active_view` manage the view stack; completion triggers cleanup via `on_active_view_complete`.
  - `input_submitted`/`input_cancelled` send messages to the app (e.g., submit user prompt, cancel approvals).
- Constructors / helpers:
  - `BottomPane::new(BottomPaneParams)` configures composer focus, placeholder text, paste burst behavior, etc.
  - Re-exports `ApprovalOverlay`, `ApprovalRequest`, `ChatComposer`, `InputResult`, `SelectionViewParams`, `SelectionAction`, `SelectionItem`, and `FeedbackView` for other modules.
- Submodules:
  - `chat_composer`, `textarea`, `paste_burst` manage the text input component.
  - `command_popup`, `file_search_popup`, `custom_prompt_view`, `list_selection_view`, `feedback_view` implement specific modal views.
  - `approval_overlay` handles approval prompts; `prompt_args`, `scroll_state`, and `selection_popup_common` provide shared helpers.
  - `footer` draws hint lines; `popup_consts` defines standard messages.

## Broader Context
- `ChatWidget` uses `BottomPane` to display the composer and pivot into popups based on events (e.g., approvals, diff reviews, custom prompts).
- Status indicators integrate with `status_indicator_widget`; composer history interacts with `chat_composer_history`.
- Context can't yet be determined for multi-column layouts; the current design assumes a single bottom pane across the terminal width.

## Technical Debt
- Extensive responsibilities (composer, popups, status) reside in this module; factoring views into a trait object hierarchy is a start, but additional structure could improve testability.
- Key handling logic has many branches; consider centralizing ESC/backtrack behavior to avoid duplication across views.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Continue isolating modal view logic into dedicated structs to keep `BottomPane` focused on orchestration.
    - Add tests around key handling (especially ESC/backtrack) to prevent regressions.
related_specs:
  - ./chatwidget.rs.spec.md
  - ./status_indicator_widget.rs.spec.md
  - ./bottom_pane/chat_composer.rs.spec.md (to add)
