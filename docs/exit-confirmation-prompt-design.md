# Exit and shutdown flow (tui + tui2)

This document describes how exit, shutdown, and interruption work in the Rust TUIs
(`codex-rs/tui` and `codex-rs/tui2`). It is intended for Codex developers and
Codex itself when reasoning about future exit/shutdown changes.

This doc replaces earlier separate history and design notes. High-level history is
summarized below; full details are captured in PR #8936.

## Terms

- **Exit**: end the UI event loop and terminate the process.
- **Shutdown**: request a graceful agent/core shutdown (`Op::Shutdown`) and wait
  for `ShutdownComplete` so cleanup can run.
- **Interrupt**: cancel a running operation (`Op::Interrupt`).

## Event model (AppEvent)

Exit is coordinated via a single event with explicit modes:

- `AppEvent::Exit(ExitMode::ShutdownFirst { confirm })`
  - Prefer this for user-initiated quits so cleanup runs.
- `AppEvent::Exit(ExitMode::Immediate)`
  - Escape hatch for immediate exit. This bypasses shutdown and can drop
    in-flight work (e.g., tasks, rollout flush, child process cleanup).

`App` is the coordinator: it decides whether to open the confirmation prompt or
submit `Op::Shutdown`, and it exits the UI loop only when `ExitMode::Immediate`
arrives (typically after `ShutdownComplete`).

## User-triggered quit flows

### Ctrl+C

Priority order in the UI layer:

1. Active modal/view gets the first chance to consume (`BottomPane::on_ctrl_c`).
   - If the modal handles it, the quit flow stops.
2. If cancellable work is active (streaming/tools/review), send `Op::Interrupt`.
3. If composer has draft input, clear the draft and show the quit hint.
4. If idle + empty, request shutdown-first quit with confirmation.

### Ctrl+D

- Only triggers quit when the composer is empty **and** no modal is active.
- With any modal/popup open, key events are routed to the view and Ctrl+D does
  not attempt to quit.

### Slash commands

- `/quit`, `/exit`, `/logout` request shutdown-first quit **without** a prompt,
  because slash commands are harder to trigger accidentally and imply clear
  intent to quit.

### /new

- Uses shutdown without exit (suppresses `ShutdownComplete`) so the app can
  start a fresh session without terminating.

## Shutdown completion and suppression

`ShutdownComplete` is the signal that core cleanup has finished. The UI treats
it as the boundary for exit:

- `ChatWidget` requests `Exit(Immediate)` on `ShutdownComplete`.
- `App` can suppress a single `ShutdownComplete` when shutdown is used as a
  cleanup step (e.g., `/new`).

## Exit confirmation prompt

The confirmation prompt is a safety net for idle quits. When shown, it provides:

- Quit now (shutdown-first).
- Quit and don't ask again (persists the notice, then shutdown-first).
- Cancel (stay in the app).

The prompt is a bottom-pane selection view, so it does not appear if another
modal is already active.

## Configuration

The prompt can be suppressed via:

```toml
[notice]
hide_exit_confirmation_prompt = true
```

This flag is updated and persisted via `UpdateExitConfirmationPromptHidden` and
`PersistExitConfirmationPromptHidden`.

## Edge cases and invariants

- **Review mode** counts as cancellable work. Ctrl+C should interrupt review, not
  quit.
- **Modal open** means Ctrl+C/Ctrl+D should not quit unless the modal explicitly
  declines to handle Ctrl+C.
- **Immediate exit** is not a normal user path; it is a fallback for shutdown
  completion or an emergency exit. Use it sparingly because it skips cleanup.

## Testing expectations

At a minimum, we want coverage for:

- Ctrl+C while working interrupts, does not quit.
- Ctrl+C while idle and empty shows confirmation, then shutdown-first quit.
- Ctrl+D with modal open does not quit.
- `/quit` / `/exit` / `/logout` quit without prompt, but still shutdown-first.
- "Don't ask again" persists the notice and suppresses future prompts.

## History (high level)

Codex has historically mixed "exit immediately" and "shutdown-first" across
quit gestures, largely due to incremental changes and regressions in state
tracking. This doc reflects the current unified, shutdown-first approach. See
PR #8936 for the detailed history and rationale.
