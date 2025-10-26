## Overview
`bottom_pane::textarea` implements the multi-line text editor used by the chat composer. It handles Unicode-aware wrapping, cursor movements, word-wise editing, kill/yank operations, and integrates with Ratatui for rendering.

## Detailed Behavior
- `TextArea` stores the current text, cursor position, wrap cache (`WrapCache`), tracked text elements (for prompt insertion boundaries), and a kill buffer for yank operations.
- Editing operations:
  - `insert_str`, `replace_range`, `set_text`, `delete_forward/backward`, `delete_backward_word`, `kill_to_beginning_of_line`, `kill_to_end_of_line`, `yank`, etc., update text and cursor while maintaining wrap cache and element boundaries.
  - `clamp_pos_*` helpers ensure insertions respect element ranges and Unicode grapheme boundaries via `unicode_segmentation`.
- Navigation:
  - Cursor movement (`move_cursor_left/right/up/down`, word-wise via Alt/Control combos) respects preferred columns and wraps, using Unicode width for accurate alignment.
  - Helpers compute beginning/end of lines, words, and maintain `preferred_col` for vertical movement.
- Wrapping & rendering:
  - `wrapped_lines(width)` caches line ranges for efficient rendering; `effective_scroll` and `cursor_pos_with_state` map cursor positions to screen coordinates given `TextAreaState`.
  - Implements `WidgetRef` and `StatefulWidgetRef` to draw the textarea with optional selection highlighting, cursor, and placeholder styling.
- Keyboard input:
  - `input(KeyEvent)` handles the full set of expected keybindings, including control chords (`Ctrl-b`, `Ctrl-f`), meta word navigation, kill/yank (`Ctrl-u`, `Ctrl-k`, `Ctrl-y`), and newline insertion.
  - Distinguishes plain text insertion from meta key combos (e.g., prevents Alt-modified characters from inserting control bytes).
- Utilities support integration with higher-level components:
  - `desired_height(width)` returns required rows for current text.
  - Scroll management (`TextAreaState`, `ensure_visible`) keeps the cursor on screen.
  - Public getters/setters expose the text, cursor, and selection for tests and composer logic.

## Broader Context
- `ChatComposer` owns a `TextArea`, relying on its editing semantics to implement paste bursts, history navigation, and slash command expansion while the bottom pane handles rendering and footers.

## Technical Debt
- The textarea handles many responsibilities (wrapping, elements, keybindings). Future refactors could split element boundary management and custom keymaps into separate helpers to ease extension and testing.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Separate element-boundary logic from the core text editor to simplify future features (e.g., inline tokens).
related_specs:
  - ./chat_composer.rs.spec.md
  - ./selection_popup_common.rs.spec.md
