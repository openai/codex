## Overview
Defines built-in slash commands for the TUI composer, including ordering, descriptions, and availability rules. The enum also drives the popup list that appears when users type `/`.

## Detailed Behavior
- `SlashCommand` enum:
  - Derives `EnumString`, `EnumIter`, `AsRefStr`, and `IntoStaticStr` (kebab-case). Order matters: commands near the top are surfaced first in the UI.
  - Includes standard actions (`/model`, `/approvals`, `/review`, `/new`, `/undo`, etc.) plus `/test-approval` behind `debug_assertions`.
- Methods:
  - `description` returns the user-facing sentence for the popup and help overlays.
  - `command` returns the string form (without `/`) via the auto-generated static str conversion.
  - `available_during_task` indicates whether the command can run while Codex is executing a task; booking commands like `/diff`, `/status`, and `/quit` are allowed mid-task, while state-altering commands (model selection, approvals) are blocked.
- `built_in_slash_commands` iterates over the enum, filtering `/undo` behind the `BETA_FEATURE` environment flag, and returns pairs of `(command_str, SlashCommand)` for quick lookup.
- `beta_features_enabled` simply checks for the `BETA_FEATURE` env var.

## Broader Context
- The composer UI leverages this list to populate the slash-command palette and validate user input.
- Other systems (like history replay or context compaction) rely on `available_during_task` to decide whether to queue or reject commands while actions run.

## Technical Debt
- Command metadata (descriptions, availability) is hard-coded here, requiring code changes to update text or add flags. A data-driven registry could simplify future additions.
- Environment flag gating for `/undo` is minimal; extending feature gating would benefit from a centralized capability system.

---
tech_debt:
  severity: low
  highest_priority_items:
    - Consider moving command metadata into a shared registry so new commands can be added without touching multiple methods.
related_specs:
  - bottom_pane/chat_composer.rs.spec.md
  - palette/slash_command_popup.rs.spec.md
