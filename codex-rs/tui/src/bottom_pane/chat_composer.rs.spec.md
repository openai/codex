## Overview
`chat_composer` implements the interactive text area at the bottom of the TUI. It handles multiline input, slash commands, custom prompts, file search popups, paste bursts, image attachments, and keyboard shortcuts.

## Detailed Behavior
- Core structure:
  - `ChatComposer` wraps a `TextArea` plus state for popups (`ActivePopup`), history navigation (`ChatComposerHistory`), hints, paste burst tracking, and attached images.
  - `InputResult` reports whether the user submitted text, triggered a slash command, or produced no actionable event.
- Rendering & layout:
  - `desired_height` computes space for the composer, footer hints, and any active popup.
  - `layout_areas` splits the available rect into the textarea region and popup/footer area, accounting for margins and hint spacing.
  - Implements `WidgetRef` and `StatefulWidgetRef` so the composer and popups render within the bottom pane, including live prefix columns.
- Input handling:
  - `handle_key_event` processes key presses, including:
    - Enter/Shift+Enter submission logic.
    - Slash command parsing, prompt expansion (custom prompts, positional arguments), and command selection popups.
    - File search popup toggles, navigation, and selection via `FileSearchPopup`.
    - Paste burst detection via `PasteBurst` and scheduled flushes.
  - `handle_paste` decides whether to open the file popup or insert text/images based on clipboard content (`pasted_image_format`, `normalize_pasted_path`).
  - `flush_paste_burst_if_due`, `is_in_paste_burst`, `recommended_paste_flush_delay` support the bottom pane’s scheduling.
- History and metadata:
  - `set_history_metadata`, `on_history_entry_response` coordinate completion of history lookups.
  - `history` object navigates previous submissions; `TextAreaState` ensures cursor positions persist across renders.
- Popups and prompts:
  - Slash command helpers (`prompt_argument_names`, `expand_custom_prompt`, etc.) fill in placeholders, support numeric prompt arguments, and submit commands when appropriate.
  - `ActivePopup` enum tracks whether a command or file popup is visible; `dismissed_file_popup_token` prevents re-triggering on the same paste.
  - `footer_mode`, `footer_hint_override`, and `FooterProps` inform the footer renderer about available shortcuts or hints (Esc backtrack, Ctrl+C quit, etc.).
- Attachments & context:
  - `attach_image`, `take_recent_submission_images` manage inline image metadata for the next submission.
  - `set_context_window_percent` updates token usage hints and refreshes the footer.

## Broader Context
- The bottom pane delegates key handling, paste events, and rendering to `ChatComposer`. Other modules (status indicator, approval overlays) call into the composer to update hints or disable interactions during tasks.

## Technical Debt
- The composer consolidates numerous behaviors; splitting command/file popups, paste burst logic, and prompt expansion into dedicated components could simplify future maintenance and testing.

---
tech_debt:
  severity: high
  highest_priority_items:
    - Factor slash-command handling, file search popups, and paste burst management into composable helpers to reduce the size and complexity of `ChatComposer`.
related_specs:
  - ./command_popup.rs.spec.md
  - ./file_search_popup.rs.spec.md
  - ./textarea.rs.spec.md
  - ../prompt_args.rs.spec.md
