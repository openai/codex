# Codex TUI – Feature-Parity Checklist

This document tracks the gaps between the **TypeScript Ink** terminal UI (`codex-cli`) and the
current **Rust `ratatui`** implementation (`codex-rs`).  Each unchecked item represents a
feature that exists in the TS version but is **missing or incomplete** in the Rust port.

Use the checklist to coordinate future work and to avoid duplicated effort.  
When you start working on a task, mark it in-progress by adding a code span `in-progress|<session-id>` immediately after the `[ ]` checkbox.  
When a feature is fully implemented, change the checkbox to `[x]`.

> How to read the list
>
> • **Area** – rough subsystem the item belongs to.  
> • **Task** – one-line summary.  
> • **Explanation** – _why_ we need it / reference to the TS behaviour.

### Contributing Guidelines

1. Pick an unchecked box (`- [ ]`).  
2. Mark your item `in-progress|<session-id>` while working.  
3. Once complete, change the box to `[x]` and remove the in-progress tag.

---

## UI & Interaction

- [ ] Header bar at top of screen – show active model, sandbox status and key-hints (Ink `TerminalHeader`).
- [ ] Help overlay (`/help`) – modal that lists slash-commands & shortcuts.
- [ ] Model switch overlay (`/model`) – change the LLM in-session.
- [ ] Approval-mode overlay (`/approval`) – toggle ask-always / on-fail / never.
- [ ] History overlay (`/history`) – scrollable list of executed commands & patches.
- [ ] Git diff overlay (`/diff`) – colourised working-tree diff viewer.
- [ ] First-run onboarding overlay.
- [ ] Image preview in chat history for attached screenshots.

## Chat Rendering

- [ ] Token streaming – render assistant response incrementally while streaming.
- [ ] Group successive auto-approved tool calls into a collapsible batch.
- [ ] Inline diff preview prior to `patch.apply` approval.
- [ ] Syntax highlighting for fenced code blocks.

## Prompt Input

- [ ] Reverse-i-search prompt history (`Ctrl-R`).
- [ ] Slash-command autocomplete & suggestions.
- [ ] External editor integration (`Ctrl-X`/`Ctrl-E`) to edit prompt in `$EDITOR`.
- [ ] Simple image attachment workflow (drag-and-drop or path completion).
- [ ] File-system path completions for `@path` tokens with inline suggestion list.
- [ ] Live context remaining indicator (% tokens left) while typing.

## Execution / Tool Calls

- [ ] Real-time stdout/stderr streaming for long-running `exec` commands.
- [ ] Abort / retry controls for active tool calls.
- [ ] Post-apply file-change summary after `patch.apply`.

## Miscellaneous

- [ ] Theme support (light / dark / high-contrast).
- [ ] Mouse support for scrolling and link activation.
- [ ] Accessibility review (screen-reader focus order, alt text).
- [ ] Windows terminal compatibility testing.

## Slash Commands

- [ ] `/clear` – clear conversation history and free up context.
- [ ] `/clearhistory` – wipe stored command history.
- [ ] `/compact` – summarise conversation into a condensed context block.
- [ ] `/bug` – open a pre-filled GitHub issue with current session log.
- [ ] `/diff` – shortcut to open the git diff overlay.
- [ ] `/help` – quick toggle for help overlay.
- [ ] `/model` – open model selection panel.
- [ ] `/approval` – open approval mode panel.
- [ ] `/history` – open command/file history view.

## Configuration & Settings

- [ ] Load and persist user configuration file (`$XDG_CONFIG_HOME/codex/config.toml`).
- [ ] In-session settings overlay for toggling provider, model, theme, etc.
- [ ] Hot-reload configuration when the file changes on disk.

## Memory & Context Management

- [ ] Show live token usage indicator in the status bar.
- [ ] Automatic context compaction when nearing model context limit.
- [ ] Persistent memory store across sessions (summary + vector store).
- [ ] Scan repo docs on start-up and cache short summaries in memory.

## Updates & Telemetry

- [ ] Check for new CLI releases on start-up and display upgrade hint.
- [ ] Opt-in anonymised usage telemetry (match TS implementation).
- [ ] Toggle verbose logging / trace mode from within the UI.

## Sandbox & Security

- [ ] Enforce directory sandbox identical to TS behaviour (deny parent writes).
- [ ] Network access toggle with allow/deny list per `exec` command.
- [ ] Prompt user before writing files outside the repo root.

## Non-interactive / CI Mode

- [ ] Support `--singlepass` run that executes a prompt then exits without TUI.
- [ ] Machine-parsable JSON output option for scripting.

