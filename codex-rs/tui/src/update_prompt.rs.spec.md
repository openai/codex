## Overview
Displays the update prompt modal (in release builds) when a newer Codex version is available. Allows the user to run the suggested update command, skip once, or suppress reminders until the next version.

## Detailed Behavior
- `run_update_prompt_if_needed`:
  - Uses `updates::get_upgrade_version_for_popup` and `get_update_action` to determine whether to show the modal. If either returns `None`, returns `Continue`.
  - Instantiates `UpdatePromptScreen`, renders it once, then processes TUI events until the user selects an option or the stream ends.
  - On completion:
    - `UpdateNow`: clears the terminal and returns `RunUpdate(update_action)`.
    - `DontRemind`: persists the dismissal via `updates::dismiss_version`.
    - `NotNow` or no selection: returns `Continue`.
- `UpdatePromptScreen`:
  - Keeps track of highlight state and final selection.
  - `handle_key` supports arrow-key and vim-style navigation, numeric shortcuts (1/2/3), Enter, Esc, and `Ctrl+C`/`Ctrl+D` to skip. Key releases are ignored.
  - `render_ref` draws the modal using `ColumnRenderable`: header with emoji and version delta, release notes link, selectable rows produced by `selection_option_row`, and an Enter hint with `key_hint`.
- `UpdateSelection` implements `next`/`prev` for wrapping navigation.
- Tests snapshot the modal rendering and verify keyboard interactions select the expected outcome (update, skip, do-not-remind) including `Ctrl+C`.

## Broader Context
- Invoked during startup or CLI invocation when the updater detects a newer version. Integrates with the general updates subsystem for command selection (`UpdateAction`).
- Uses the same selection and key hint utilities as other dialogs, ensuring consistent UX.

## Technical Debt
- Modal assumes a blocking event loop; in future, integrating with asynchronous flows may require refactoring to support background prompts.
- Release notes link is hard-coded to GitHub; providing channel-specific links (e.g., self-hosted) may need configuration hooks.

---
tech_debt:
  severity: low
  highest_priority_items:
    - Parameterize the release notes URL so alternate distribution channels can customize destinations.
related_specs:
  - selection_list.rs.spec.md
  - updates/mod.rs.spec.md
