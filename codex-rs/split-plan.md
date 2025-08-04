Split plan: break the combined feature branch into three focused changesets

Goal
- The current branch introduces three user-facing features at once: scrollable slash commands, model changing (with a warning dialog), and approval mode changing. To ease review and reduce risk, we will split this work into three sequential changesets, landed in this order: 1) scrollable slash commands, 2) model changing + warning dialog, 3) approval mode changing.

General guidelines for all three changesets
- Keep each PR minimal and focused; avoid carrying unrelated refactors across changesets.
- Preserve behavior and tests unrelated to the target feature of each PR.
- Update or add tests and snapshots that directly validate the feature in scope.
- Document any new user-facing behavior in README/help or inline usage text.
- Run: `just fmt`, `just fix`, and `cargo test --all-features` with CODEX_SANDBOX_NETWORK_DISABLED=1 before submitting each PR.
- If necessary, use small, self-contained preparatory commits at the start of a PR to reduce diff noise (e.g., moving shared utilities without changing behavior).

Source of truth for scoping files
- Use `git diff --name-only origin/main...HEAD` to identify which files contain changes to be partitioned.
- From the current comparison, the relevant files (all under codex-rs/) include:
  - common: `common/src/fuzzy_match.rs`, `common/src/lib.rs`, `common/Cargo.toml`
  - core: `core/src/codex.rs`, `core/src/config.rs`, `core/src/lib.rs`, `core/src/openai_model_info.rs`
  - tui: `tui/src/app.rs`, `tui/src/app_event.rs`, `tui/src/bottom_pane/chat_composer.rs`, `tui/src/bottom_pane/command_popup.rs`, `tui/src/bottom_pane/file_search_popup.rs`, `tui/src/bottom_pane/mod.rs`, `tui/src/bottom_pane/popup_consts.rs`, `tui/src/bottom_pane/scroll_state.rs`, `tui/src/bottom_pane/selection_list.rs`, `tui/src/bottom_pane/selection_popup.rs`, `tui/src/bottom_pane/selection_popup_common.rs`, `tui/src/chatwidget.rs`, `tui/src/command_utils.rs`, `tui/src/danger_warning_screen.rs`, `tui/src/history_cell.rs`, `tui/src/lib.rs`, `tui/src/slash_command.rs`, `tui/Cargo.toml`
  - file-search: `file-search/src/lib.rs`, `file-search/Cargo.toml`
  - docs/metadata: `README.md`, `Cargo.lock`

We will assign only the necessary subset of these files to each changeset. When a file contains mixed feature changes, split the hunks with `git add -p` so only the relevant lines ship in the targeted PR. If a file must be touched by multiple features, ship the minimal interfaces first and layer feature-specific logic in the later PRs.

Changeset 1: Scrollable slash commands (foundation)
Objective
- Introduce scrolling and list navigation for the slash command popup, without changing models or approval behavior.

Scope (files and typical changes that belong here)
- tui: scrolling infrastructure and command popup rendering
  - `tui/src/bottom_pane/scroll_state.rs`: new or updated scrolling state management for lists/popups.
  - `tui/src/bottom_pane/selection_list.rs`: list widget behavior for selection + scrolling.
  - `tui/src/bottom_pane/selection_popup_common.rs`: shared popup utilities/hooks to support scrollable lists.
  - `tui/src/bottom_pane/selection_popup.rs`: generic selection popup rendering and input handling with scroll.
  - `tui/src/bottom_pane/command_popup.rs`: the slash command popup, wired to the scrolling list.
  - `tui/src/bottom_pane/popup_consts.rs`: constants for sizes/margins/limits that the popup uses.
  - `tui/src/bottom_pane/mod.rs`, `tui/src/bottom_pane/chat_composer.rs`, `tui/src/command_utils.rs`, `tui/src/slash_command.rs`: only the minimal changes required to hook up the scrollable popup to existing slash commands. Do not add new commands yet; keep the existing command set and semantics.
- common: only changes strictly required by the popup mechanics (e.g., minor fuzzy-match tweaks used by the list), if any.
  - `common/src/fuzzy_match.rs` (limit to neutral refactors or bug fixes that the popup depends on).

Out of scope for this PR
- No changes to model selection, warning dialogs, or approval mode.
- Do not introduce new slash commands for model/approval in this PR.

Tests and validation
- Unit tests (if present) for selection/scroll state and popup navigation.
- Snapshot tests for the popup UI if the project uses them; otherwise, add a small set of focused tests.

Docs
- Update UI help or README with note that slash command lists are now scrollable (if user-facing docs exist for this area).

Landing order notes
- This PR should land first to provide the infrastructure leveraged by later features.

Changeset 2: Model changing with warning dialog
Objective
- Add the ability to switch models from the UI and present a warning dialog when switching to a non-default or potentially risky/expensive model.

Scope (files and typical changes that belong here)
- core/common: model metadata and selection plumbing
  - `core/src/openai_model_info.rs`: model metadata, pricing/perf characteristics used in the warning.
  - `core/src/codex.rs`, `core/src/lib.rs`, `core/src/config.rs`: wiring to apply the selected model for new requests and propagate to the UI.
  - `common/src/lib.rs`: shared types or flags if needed for representing the selected model.
- tui: UI for issuing model change and showing the warning dialog
  - `tui/src/danger_warning_screen.rs`: warning dialog implementation for model change.
  - `tui/src/app.rs`, `tui/src/app_event.rs`: route events to open/confirm/cancel model switching; persist selection.
  - `tui/src/chatwidget.rs`, `tui/src/history_cell.rs`: reflect current model in the UI if applicable.
  - `tui/src/command_utils.rs`, `tui/src/slash_command.rs`, `tui/src/bottom_pane/command_popup.rs`: add a slash command (e.g., `/model`) and hook it into the selection popup to choose among available models. Reuse the scrolling infrastructure from Changeset 1.
  - `tui/src/lib.rs`: any necessary app-wide enum/state updates to track the pending warning dialog.
- file-search: only include changes if they are strictly required by the new model selection flow; otherwise keep them out.

Out of scope for this PR
- Approval mode UI/behavior changes.
- Unrelated refactors to fuzzy matching or popups beyond what is required for model selection.

Tests and validation
- Unit tests for mapping selected model -> runtime config and for the warning dialog flow (open, confirm, cancel).
- If applicable, snapshot tests for the warning dialog.
- Manual QA:
  - Trigger the model selection slash command, pick a model, see the warning dialog, confirm to apply.
  - Cancel from the warning dialog and verify the model remains unchanged.
  - Verify the selected model persists as intended across turns and does not break message sending.

Docs
- Document how to change models, explain the warning dialog and when it appears, and list supported models.

Landing order notes
- This PR should land after the scrollable popup; it reuses the selection UI for model lists.

Changeset 3: Approval mode changing
Objective
- Provide a way to change the approval mode from the UI (e.g., via a slash command or a dedicated popup) and correctly apply it to the underlying workflow.

Scope (files and typical changes that belong here)
- core/common: approval mode representation and application
  - `core/src/codex.rs`, `core/src/config.rs`: accept and apply approval mode changes to the engine.
  - `common/src/lib.rs`: types and utilities related to approval mode, if any.
- tui: UI for selecting approval mode
  - `tui/src/app_event.rs`, `tui/src/app.rs`: event and state handling for choosing approval mode.
  - `tui/src/command_utils.rs`, `tui/src/slash_command.rs`, `tui/src/bottom_pane/command_popup.rs`: introduce a slash command (e.g., `/approval`) and wire it into the selection popup (from Changeset 1) to choose the mode.
  - If an approval-specific modal or banner exists, include the minimal changes to display the active mode; exclude model warning dialog logic.

Out of scope for this PR
- Any model switching logic or warning dialogs.
- Structural refactors to the popup/list infrastructure (those shipped in PR 1).

Tests and validation
- Unit tests asserting that the selected approval mode is propagated to the core and affects behavior as intended.

Docs
- Update help/README to describe the available approval modes and how to switch between them.

Landing order notes
- This PR should land last; it relies on the scrollable command UI from PR 1 and may mirror the event/state patterns introduced in PR 2.

Practical extraction steps
1) Create a new branch off `origin/main` for PR 1 (Scrollable commands).
   - Use `git add -p` to stage only the scroll-related hunks in the files listed under Changeset 1. Leave model/approval changes unstaged.
   - Build locally with `just fmt`, `just fix`, and `cargo test --all-features` (with CODEX_SANDBOX_NETWORK_DISABLED=1) and adjust as needed.
   - Wait for the user to commit changes
2) After PR 1 is opened, create a new branch off `origin/main` (or off the merged commit) for PR 2 (Model switching + warning).
   - Stage only hunks related to model selection and the warning dialog in the files listed under Changeset 2.
   - Ensure the UI relies on the scrollable list utilities from PR 1.
   - Format, lint, and test.
3) Finally, create a new branch off `origin/main` (or the merged sequence) for PR 3 (Approval mode).
   - Stage only approval-mode-related hunks in the files listed under Changeset 3.
   - Format, lint, and test.

Additional notes to reduce review friction
- Where files contain mixed concerns, split functions or introduce minimal adapters in PR 1 to make later diffs cleaner without changing behavior.
- Keep variable/function names stable across PRs to minimize churn; avoid renames until the PR where they are indispensable.
- If you added new slash commands for model/approval in the combined branch, exclude those from PR 1 and introduce them in PRs 2 and 3 respectively.

Deliverables
- Three PRs in this order:
  1) Scrollable slash commands (UI mechanics only)
  2) Model changing with warning dialog
  3) Approval mode changing
- Each PR has passing CI, clear description of scope, QA steps, and links to any relevant screenshots.

