# Ctrl+C / Ctrl+D behavior history (Codex CLI)

This document explains how `Ctrl+C` and `Ctrl+D` behave in the Codex Rust TUI, how that behavior
changed over time, and why. It draws only from GitHub PR descriptions, PR review comments, and
linked GitHub issues.

## Mental model

`Ctrl+C` / `Ctrl+D` can arrive via different mechanisms:

In a line-buffered program, `Ctrl+C` is usually delivered as a signal (`SIGINT`) and `Ctrl+D` is
usually EOF on stdin. In the TUI, raw mode is enabled and both usually show up as key events
(e.g., `KeyCode::Char('c')` with Control), not as `SIGINT`/EOF.

If Codex is installed via npm, a Node.js wrapper launches the native binary. That wrapper’s signal
forwarding affects how `SIGINT`/`SIGTERM` behave when the parent process receives a signal (it
generally does not affect the TUI’s raw-mode key handling).

## Current behavior (Rust TUI)

As of HEAD:

- `Ctrl+C` is stateful. It may dismiss an active modal, clear the composer, interrupt a running
  task, or trigger shutdown when idle with an empty composer.
- `Ctrl+D` triggers exit only when the composer is empty; otherwise it behaves like normal input.

The main decision point for `Ctrl+C` is `ChatWidget::on_ctrl_c` in
`codex-rs/tui/src/chatwidget.rs:3309`, with consumption/handling in `BottomPane::on_ctrl_c` in
`codex-rs/tui/src/bottom_pane/mod.rs:218`. Exiting is coordinated via `ShutdownComplete`
(`codex-rs/tui/src/chatwidget.rs:1051`) and `AppEvent::Exit(ExitMode::Immediate)`
(`codex-rs/tui/src/app.rs:718`).

## Timeline of behavior changes

### <https://github.com/openai/codex/pull/1402> — Handle Ctrl+C quit when idle

This introduced a deliberate “two-step” quit flow for `Ctrl+C` while idle: the first press shows
a quit hint and a second press exits. It also reset the hint when other input arrived or when work
began.

### <https://github.com/openai/codex/pull/1647> — Introducing shutdown operation

This PR introduced an explicit shutdown operation so background tasks could flush work before the
process exits. A key review concern was that if the Tokio runtime shuts down, still-running tasks
can be dropped before they finish draining queued work, so “fire and forget” spawning is not
enough for correctness.

The review thread is also where “shutdown completion” became an explicit part of the contract:
there is a comment asking whether anything in the TUI waits for shutdown to be processed, and
another calling out a user-visible concern: “Wait, why are we changing Ctrl-C behavior. Did you
manually test this? What is different now?” This is useful context because later `Ctrl+C` quit
behavior relies on the existence and correctness of `ShutdownComplete`.

### <https://github.com/openai/codex/pull/1696> — Fix approval workflow

This PR made approval flows visible and interruptible. Commands requiring approval are written to
chat history before the modal appears, the decision is logged after, and `Ctrl+C` aborts an open
approval modal (effectively acting like Esc for that modal).

The PR review comments are where the “consumed vs not consumed” contract is made explicit. In
particular, one review suggested introducing a small enum (e.g. `CancellationEvent`) rather than a
boolean so it is clear whether `Ctrl+C` was handled by the UI. Another review comment explicitly
describes the intended user flow: after aborting the modal, show the standard quit hint so a
subsequent `Ctrl+C` can exit.

### <https://github.com/openai/codex/pull/1589> — Ctrl+D exits only when composer is empty

This prevented accidental exits from `Ctrl+D` when the composer has typed text. The PR description
frames the design as “exit only when there’s nothing to lose”, and the PR comment summarizes the
mechanics as routing `Ctrl+D` to normal handling unless the composer is empty, plus adding
`is_empty` helpers to keep the check consistent.

### <https://github.com/openai/codex/pull/2691> — single control flow for both Esc and Ctrl+C

This tightened correctness around “interrupt” semantics. The stated intent is that while a task is
running, Esc and `Ctrl+C` should do the same thing, and that certain stuck-history-widget cases
were fixed.

The PR description also highlights a subtle `Ctrl+D` edge case: `Ctrl+D` could quit the app while
an approval modal was showing if the textarea was empty.

There are no PR comment threads to incorporate here, but the commit messages within the PR show
what it tried to make true:

- there should be a single interrupt path (rather than multiple UI components sending interrupts),
- interrupt/error should finalize in-progress UI cells consistently (e.g., replacing a spinner with
  a failed mark), and
- `Ctrl+D` behavior needed an explicit fix.

### <https://github.com/openai/codex/pull/3285> — Clear non-empty prompts with ctrl + c

This PR changed `Ctrl+C` to clear the prompt when the composer is non-empty, which is more aligned
with shell/REPL ergonomics than exiting. The PR description also mentions adjusting the hint text
to distinguish “interrupt” vs “quit” depending on state, and calls out tradeoffs (e.g., overlap
between Esc and `Ctrl+C`, and footer hint text not perfectly matching all states).

This change is important context for “why does Ctrl+C quit immediately now?” because once `Ctrl+C`
is overloaded as “clear the prompt” when non-empty, the remaining idle case (empty composer) is
the one left to map to “quit”.

### <https://github.com/openai/codex/pull/5470> — Recover cleared prompt via history (Up arrow)

This PR was a mitigation for prompt-clearing on `Ctrl+C`: it preserved cleared text so it could be
recovered via history navigation.

The PR review comments show that an “undo clear prompt” shortcut was considered. A review comment
warned that restoring the prompt is tricky when the composer contains attachments: if placeholders
are restored as plain text rather than rebuilt atomically, users can partially delete/edit them and
silently lose the associated image. This is a concrete example of why a narrow undo binding can
have surprising edge cases.

The review also includes implementation-level tightening (e.g., moving “record cleared draft” into
the clear path so clearing and remembering remain in sync, and questioning whether restoration
should reuse existing higher-level methods like `set_text_content`).

### <https://github.com/openai/codex/pull/4627> — Fix false "task complete" state while streaming

This is an example where state tracking affects `Ctrl+C` behavior: the bug caused `Ctrl+C` to quit
instead of canceling the stream during the final agent message. It is a reminder that semantics
like “interrupt when working” depend on correct “working vs idle” state.

### <https://github.com/openai/codex/pull/5078> — ^C resets prompt history navigation cursor

This made `Ctrl+C` in the prompt area behave more like common shells by resetting history
navigation state. It is not an exit behavior change, but it reinforces the trend of making prompt
area `Ctrl+C` do “prompt things” first (clear/reset/interrupt) before “quit”.

## npm Node wrapper (signal path)

When Codex is installed via npm, a Node.js wrapper launches the native binary. Signal handling in
that wrapper affects how `SIGINT`/`SIGTERM` behave when the parent process receives a signal.

### <https://github.com/openai/codex/pull/1590> — npm wrapper: forward signals and exit with child

This PR fixed two problems in the npm wrapper: it now forwards termination signals to the child
and it correctly exits when the child exits. It references reports of runaway processes and links
to <https://github.com/openai/codex/issues/1570>.

## If you change Ctrl+C behavior again

Most “what should `Ctrl+C` do?” questions depend on which state you are in. Before changing
behavior, it helps to make an explicit decision for each:

- idle with empty composer (quit immediately vs require confirmation),
- idle with non-empty composer (clear vs quit, and how recoverable clear should be),
- working/streaming (always interrupt, and whether any state bugs can misclassify as idle),
- modal open (dismiss/abort first, and what the next `Ctrl+C` should do),
- signal path vs key-event path (especially for npm installs).
