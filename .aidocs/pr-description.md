# Problem

Slash commands are handled outside the normal message submission path, so accepted commands could clear the composer without becoming part of the local Up-arrow recall list. That made command-heavy workflows awkward: after running `/diff`, `/rename Better title`, or `/plan investigate this`, users had to retype the command instead of recalling and editing it like a normal prompt.

# Mental model

The composer owns draft state and local recall, but it does not know whether a slash command actually succeeded at the application layer. This change introduces a two-phase handoff: the composer stages the slash-command draft when it converts input into `InputResult::Command` or `InputResult::CommandWithArgs`, and `ChatWidget` either records or discards that staged history entry after dispatch decides whether the command was accepted.

Bare commands recalled from typed text use the trimmed draft. Commands selected from the popup record the canonical command text, such as `/diff`, rather than the partial filter text the user typed. Inline commands with arguments keep the original command invocation available locally even when their arguments are later prepared through the normal submission pipeline.

# Non-goals

This does not persist slash commands to cross-session history. It only extends the local, in-session recall list that already preserves rich composer state such as text elements, attachments, mention bindings, and pending paste placeholders.

This does not change command availability, command side effects, popup filtering, or the semantics of unsupported commands. Rejected commands are intentionally discarded from the staged recall slot so Up-arrow does not surface commands that the application refused to run.

# Tradeoffs

The main tradeoff is that dispatch now returns an acceptance boolean for slash commands. That keeps the history decision close to the command result, but it also means new command branches must choose `true` or `false` deliberately. Treating that boolean as part of the contract is preferable to recording every parsed command immediately, because parse success is not the same thing as a command being useful or accepted.

Inline command handling now avoids double-recording by preparing inline arguments without using the normal message-submission history path. The staged slash-command entry becomes the single source of local recall for the command invocation.

# Architecture

`ChatComposer` records a pending `HistoryEntry` when slash-command input is promoted into an `InputResult`. The pending entry mirrors the existing local history payload shape so recall can restore text elements, local images, remote images, mention bindings, and pending paste state when those are present.

`BottomPane` exposes the pending-history operations as narrow forwarding methods because it owns the composer. `ChatWidget` wraps command dispatch and commits the pending entry only when `dispatch_command` or `dispatch_command_with_args` returns `true`; otherwise it discards the staged entry.

The design preserves the existing ownership split: the composer owns editing and recall state, while `ChatWidget` owns application-level command acceptance. The contract between them is that every slash-command dispatch path must resolve the staged entry exactly once.

# Observability

There is no new logging because this is a local UI recall behavior and the acceptance decision is already visible through the command outcome. The practical debug path is to trace an Enter key through `ChatComposer::try_dispatch_bare_slash_command`, `ChatComposer::try_dispatch_slash_command_with_args`, or popup Enter/Tab handling, then check whether `ChatWidget` records or discards the pending entry after dispatch.

If a command unexpectedly appears in recall, inspect the relevant `dispatch_command` branch and confirm it returns `false` on the rejection path. If a command unexpectedly does not appear in recall, confirm the composer staged the pending entry before clearing the textarea and that the `ChatWidget` wrapper is used instead of calling dispatch directly from the input-result match.

# Tests

Composer-level tests cover staging and recording for a bare typed slash command, a popup-selected command, and an inline command with arguments.

Chat-widget tests cover the end-to-end behavior that accepted bare and inline slash commands can be recalled locally with Up-arrow after dispatch.
