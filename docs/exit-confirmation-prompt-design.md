# Exit confirmation prompt (Ctrl+C / Ctrl+D) — design for tui + tui2

This document proposes a unified implementation for an exit confirmation prompt that triggers on
`Ctrl+C` / `Ctrl+D` in both Rust TUIs (`codex-rs/tui` and `codex-rs/tui2`).

It is grounded in `docs/ctrl-c-quit-history.md`, which captures prior design intent and regressions
around cancellation vs quitting.

## Background and motivation

In Codex’s TUIs, `Ctrl+C` and `Ctrl+D` arrive as key events (raw mode), not as `SIGINT` / stdin
EOF. Historically, Codex has used these keys for multiple user intents:

- cancel/interrupt an in-flight operation,
- dismiss a modal / popup,
- clear draft input,
- quit the application.

This overload is intentional, but it is fragile: any bug in “are we working?” state tracking can
turn a would-be cancellation into an immediate quit. `docs/ctrl-c-quit-history.md` includes an
example (`PR #4627`) where state incorrectly flipped to “idle” while streaming, causing `Ctrl+C`
to quit instead of interrupt.

We also have a related bug: pressing `Ctrl+C` during a review task can quit immediately, but the
intended behavior is “cancel the review”.

The exit confirmation prompt is primarily a safety feature to prevent accidental exits when the key
press would otherwise quit. It must not mask or replace the primary expectation: `Ctrl+C` cancels
active work whenever possible.

## Terminology: exit vs shutdown vs interrupt

Codex distinguishes between:

- **Exit**: end the UI event loop and terminate the process (e.g., `AppEvent::ExitRequest`
  returning `Ok(false)`).
- **Shutdown**: request a graceful agent/core shutdown (e.g., `Op::Shutdown`), often waiting for
  `ShutdownComplete` before exiting so background work can flush cleanly.
- **Interrupt**: cancel a running operation (e.g., `Op::Interrupt`), used for streaming turns,
  long-running tools, and other cancellable tasks.

The design must be explicit about which action a “quit gesture” triggers. In particular: if today
idle `Ctrl+C` results in `Op::Shutdown`, a confirmation prompt must preserve that semantic
(prompting should not silently convert “shutdown” into “exit immediately”).

### Lifecycle model (what runs where)

The UI and the agent/core have distinct lifecycles:

- The **UI event loop** owns input handling and rendering. “Exit” means the event loop stops and
  the process typically terminates shortly after.
- The **agent/core** runs work that can outlive a single frame or keypress: streaming turns,
  tool execution, review tasks, background state updates, etc.

This matters because exiting the UI can abruptly end the runtime and drop background tasks.
`docs/ctrl-c-quit-history.md` calls this out as a correctness concern (see `PR #1647`): a graceful
shutdown handshake exists so the core can finish draining/flush work before the process exits.

In this design:

- **Exit immediately** means “exit the UI loop now” (without a shutdown handshake).
- **Shutdown then exit** means “request core shutdown, then exit the UI after shutdown completes”.
  The completion signal is `ShutdownComplete` (event name varies per crate, but the contract is the
  same: “core has finished shutting down”).

Today, the TUIs mix “exit immediately” and “shutdown first” depending on how the user tries to
leave.

### Examples of core/UI state (what users see)

- **Idle**: no streaming output, no running tools, no active review task; the composer may be empty
  or contain a draft.
- **Waiting for the turn to start**: the user pressed Enter, and the UI is waiting for the first
  events for the new turn (this typically transitions quickly into “working/streaming”).
- **Streaming**: partial agent output is arriving and being rendered.
- **Running commands/tools**: the agent is executing local commands or tools and streaming their
  progress/output into the transcript.
- **Review in progress**: a review request has been issued and the UI is in review mode, awaiting
  results.
- **Modal open**: approvals, pickers, selection views, etc. are on screen.

In the current implementation, the key state driving `Ctrl+C` behavior is whether the bottom pane
considers a task “running”. That state is toggled as turns start/finish, while some other
long-running flows (like review) have separate flags (e.g., `is_review_mode`). Regressions happen
when those states disagree (for example, when streaming is happening but the UI momentarily thinks
it is idle).

### Current trigger mapping (tui and tui2)

| Trigger                        | What it does today                                            |
| ------------------------------ | ------------------------------------------------------------- |
| `Ctrl+C` while work is running | Interrupt (`Op::Interrupt`)                                   |
| `Ctrl+C` while idle            | Shutdown (`Op::Shutdown`), exit on `ShutdownComplete`         |
| `Ctrl+D` with empty composer   | Exit immediately (`ExitRequest`, bypasses `Op::Shutdown`)      |
| `/quit`, `/exit`, `/logout`    | Exit immediately (`ExitRequest`, bypasses `Op::Shutdown`)      |
| `/new` (NewSession)            | Shutdown current conversation, then stay running              |

Notes:

- The “work is running” case depends on “task running” state being accurate. If it is wrong, a
  `Ctrl+C` can take the idle path and start shutdown instead of interrupting.
- `ShutdownComplete` is generally treated as “exit now” by the widget layer, but the app layer can
  suppress it when shutdown is used for cleanup rather than quitting.

### What `ShutdownComplete` does today

`ShutdownComplete` is a sentinel event with no payload. It is used as a lifecycle boundary:

- In core, it is emitted at the end of the shutdown handler after aborting tasks, terminating
  processes, and shutting down the rollout recorder.
- In the TUIs, `ChatWidget` treats it as “exit now” by calling `request_exit()`.
- In `App`, a one-shot suppression flag can ignore the next `ShutdownComplete` when shutdown is
  used for cleanup (e.g., stopping the current conversation during `/new`) rather than quitting.

The key point: `ShutdownComplete` is not “cleanup itself”; it is the signal that cleanup has
already happened (or that the core believes shutdown is complete).

### Scenario walkthrough (how shutdown vs exit shows up)

These examples describe what happens today and why it matters for the confirmation prompt design:

- **Idle + composer empty**: `Ctrl+C` sends `Op::Shutdown`, and the UI exits once `ShutdownComplete`
  arrives. `Ctrl+D` exits immediately without sending `Op::Shutdown`. `/quit` and `/exit` also exit
  immediately.
- **Idle + composer has a draft**: `Ctrl+C` clears the draft (and shows the quit hint). It does not
  exit or shutdown on that press. A subsequent `Ctrl+C` may now hit “idle + empty” and trigger
  shutdown.
- **Working/streaming output**: `Ctrl+C` sends `Op::Interrupt` (cancel) rather than quitting. This
  is the primary “don’t lose my session” behavior, and it relies on “task running” being true.
- **Running local tools/commands**: this is still “working”; `Ctrl+C` should interrupt rather than
  quit. If “task running” is false by mistake, `Ctrl+C` can incorrectly start shutdown.
- **Review in progress**: intended behavior is the same as “working”: `Ctrl+C` cancels the review.
  The reported bug (“quit immediately during review”) indicates the UI is misclassifying review as
  idle for `Ctrl+C` handling.
- **Modal open**: `Ctrl+C` is first offered to the active modal/view to dismiss/abort. It should
  not trigger shutdown/exit unless the modal declines to handle it.

### Why `/quit`, `/exit`, `/logout`, and `Ctrl+D` don’t call shutdown today

Today these are implemented as explicit “leave the UI now” actions:

- `/quit`, `/exit`, and `/logout` dispatch `AppEvent::ExitRequest` directly.
- `Ctrl+D` exits only when the composer is empty (a guard added to reduce accidental exits), but it
  still exits via `ExitRequest` rather than `Op::Shutdown`.

This split does not have a principled/rational basis documented anywhere, and it may simply be an
accidental divergence in how different quit triggers were implemented over time.

Either way, bypassing `Op::Shutdown` is the riskier option: it relies on runtime teardown and
dropping to clean up in-flight work, which can leave behind “leftovers” (e.g., unified exec child
processes, unflushed rollout/session tail, or other background tasks that would normally be fenced
by `ShutdownComplete`). Where possible, Codex should prefer the shutdown handshake and exit only
after `ShutdownComplete`, regardless of whether the quit was initiated via keys or slash commands.

## Problem statement

1. **Accidental closure**: users sometimes press `Ctrl+C` / `Ctrl+D` out of habit
   and unexpectedly lose their session.
2. **State misclassification regressions**: if the UI incorrectly believes it is
   idle (streaming, review, etc.), `Ctrl+C` can incorrectly quit.
3. **Review cancellation bug**: `Ctrl+C` during an in-progress review should
   cancel the review, not quit.
4. **Modal edge cases**: `Ctrl+D` (composer empty) must not quit while a modal/popup is open (see
   history doc for prior regressions).

## Goals

- Prompt only when `Ctrl+C` / `Ctrl+D` would otherwise quit.
- Keep `/quit`, `/exit`, `/logout` as intentional exits (no prompt).
- Prefer shutdown+exit (graceful) for all quit paths where possible.
- Ensure `Ctrl+C` cancels review (and other active work) reliably.
- Keep behavior consistent across `tui` and `tui2`, including prompt copy and config key.
- Persist “don’t ask again” in `~/.codex/config.toml` under `[notice]`.
- Add tests that encode the rationale and prevent regressions.

## Non-goals

- Changing npm wrapper signal forwarding behavior (signal path).
- Redesigning all footer hint text (keep changes focused to quit confirmation).
- Introducing a shared UI crate between `tui` and `tui2` (we align by convention, not by code
  sharing).

## Proposed user-visible behavior

Use a single config key:

```toml
[notice]
hide_exit_confirmation_prompt = true
```

`[notice]` is the right section for this: it already stores similar acknowledgement/NUX flags like
`hide_full_access_warning` and `hide_rate_limit_model_nudge`.

When unset/false, show a confirmation prompt before quitting via `Ctrl+C` or `Ctrl+D` (the
accidental-prone exit gestures). The prompt must offer:

- A primary quit option labeled like other confirmations (recommended: `Yes, quit Codex`).
- A cancel option (recommended: `No, stay in Codex`).
- A “remember” option that matches existing wording used elsewhere in the UI (recommended:
  `Yes, and don't ask again`), which sets the config key and persists it.

### State-based behavior table

The key is to treat `Ctrl+C` primarily as “cancel”, and only as “quit” when there is nothing to
cancel.

| State                        | `Ctrl+C`                      | `Ctrl+D`              |
| ---------------------------- | ----------------------------- | --------------------- |
| Modal/popup open             | Dismiss/abort the modal       | Must not quit         |
| Task/review/streaming active | Interrupt/cancel (never quit) | Do nothing            |
| Composer has draft content   | Clear draft                   | Normal input          |
| Idle, composer empty         | Quit (shutdown+exit)          | Quit (shutdown+exit)  |

Notes:

- “Task active” must include review mode. If review is not represented as “task running” today, it
  must be wired in as part of this change.
- `Ctrl+D` should be treated as a quit gesture only when there is nothing to lose (composer empty)
  and no modal is open.

## Proposed implementation (unified design)

### Policy: quit should be shutdown-first

Since there is no clear rationale for having some quit triggers bypass shutdown, this design
assumes a single default: quitting Codex should request `Op::Shutdown` and exit only after
`ShutdownComplete`.

An “exit immediately” path can remain as a fallback (e.g., if shutdown hangs), but it should not
be the normal route for user-initiated quit gestures or commands.

### Add an app-level event that means “quit with confirmation”

Keep `AppEvent::ExitRequest` meaning “exit immediately”.

Add:

```rust
AppEvent::QuitRequest { confirm: bool }
```

Handling lives in the app layer (`App`), because:

- `App` is already the coordinator for exit (`ExitRequest`) and for reacting to `ShutdownComplete`.
- `App` owns the current configuration and is the right place to gate “prompt vs no prompt”.

Pseudo-flow:

1. A key handler decides “this key press would quit” and sends
   `QuitRequest { confirm: true }`.
2. `App` checks `config.notices.hide_exit_confirmation_prompt.unwrap_or(false)`:
   - if `true`, initiate shutdown+exit immediately (no prompt).
   - if `false`, ask `ChatWidget` to open the confirmation prompt.

Slash commands like `/quit`, `/exit`, and `/logout` can dispatch `QuitRequest { confirm: false }`
to preserve the “no prompt” behavior while still taking the shutdown+exit path.

### Action handling details (shutdown+exit)

To keep behavior correct and consistent, `App` must treat quitting as a two-step
sequence:

1. Send `Op::Shutdown` to the core.
2. Exit the UI loop only after observing the shutdown completion event.

This is the core invariant: the confirmation prompt should only gate whether we ask the user.

Codex already uses `Op::Shutdown` for non-exit reasons (e.g., stopping a conversation/thread before
starting a new one). In the current TUIs, `ChatWidget` always exits on `ShutdownComplete`, and
`App` decides whether to forward that event using a suppression flag (see
`codex-rs/tui/src/app.rs:314` and `codex-rs/tui2/src/app.rs:378`). Any new quit confirmation flow
should preserve that separation: “shutdown” does not inherently mean “exit the app”.

This is also why `/new` works the way it does today: it shuts down the current conversation/thread
to avoid leaking work, but it suppresses `ShutdownComplete` so the app can create a new session and
keep running instead of quitting.

### Policy change: make exit paths shutdown-first

Historically, Codex has had multiple exit paths. Some of them request `Op::Shutdown` first (idle
`Ctrl+C`), and some exit the UI immediately (`Ctrl+D` when the composer is empty, plus `/quit`,
`/exit`, and `/logout`). That split is easy to miss and it can skip meaningful cleanup.

This design proposes to move all application exit paths to shutdown-first, so exiting Codex is
consistent and performs the same cleanup regardless of which “quit” gesture or command the user
uses. In practice, that means:

- `Ctrl+C` (idle quit) continues to be shutdown-first.
- `Ctrl+D` (empty composer quit) should become shutdown-first.
- `/quit`, `/exit`, and `/logout` should become shutdown-first.

If shutdown latency or hangs are a concern, this policy should be paired with:

- the logging described in the “Observability and hang diagnosis” section,
- a bounded timeout with a clearly logged fallback to “exit immediately” (optional, but recommended
  if hangs are observed in practice).

### What `Op::Shutdown` cleans up in core (today)

In `codex-rs/core`, shutdown performs a small set of concrete cleanup steps before emitting
`ShutdownComplete`:

- Abort active tasks and turns (cancels tasks, runs per-task abort hooks, emits `TurnAborted`):
  `codex-rs/core/src/tasks/mod.rs:158`.
- Terminate all unified exec processes (kills any long-running child processes created via unified
  exec): `codex-rs/core/src/unified_exec/process_manager.rs:653`.
- Shut down the rollout recorder (acts as a barrier so the writer task has processed all prior
  queued items; each rollout line is flushed as it is written): `codex-rs/core/src/codex.rs:2109`
  and `codex-rs/core/src/rollout/recorder.rs:368`.
- Emit `ShutdownComplete`: `codex-rs/core/src/codex.rs:2137`.

This is the work that can be skipped or cut short if the UI exits immediately without requesting
shutdown: tasks may be dropped by runtime teardown, rollout items can be lost if still queued, and
child processes may survive past the UI exit.

### What it means to drop in-flight work (effects users can see)

If the UI exits immediately (without `Op::Shutdown`), the process can terminate while work is still
in flight. The concrete consequences depend on which state Codex is in:

- **Streaming/working**: partial output may be visible in the transcript, but the turn may not reach
  its normal “end” path (e.g., no final `TaskComplete`, no finalization for in-progress UI cells).
- **Tool execution / unified exec**: child processes may outlive the UI if not explicitly
  terminated, depending on how they were spawned. A shutdown path explicitly terminates them.
- **Rollout/session persistence**: the rollout writer is asynchronous. If the process exits while
  items are queued but not yet processed, the session file may miss its tail. That can affect
  replay/resume fidelity and any tooling/tests that inspect the rollout file.
- **Review in progress**: review uses separate state and can be misclassified as idle. If the UI
  takes an exit path instead of an interrupt path, review can stop abruptly rather than being
  cleanly cancelled.

This is why the design prioritizes correctness over convenience for accidental-prone quit gestures:
we want to prefer “interrupt when working” and “shutdown+exit when idle” rather than “exit now”.

### Rollout recorder shutdown: why this is written as “shutdown”

The rollout shutdown mechanism can look surprising at first glance because it sends a “shutdown”
command to the writer task, then drops the recorder.

Based on the current code:

- The rollout writer task is single-threaded and processes commands FIFO.
- Each rollout line is written and flushed immediately when processed.
- Sending `RolloutCmd::Shutdown` and waiting for its ack works as a barrier: the ack cannot be sent
  until all earlier queued `AddItems`/`Flush` commands have been processed by that task.
- Dropping the recorder then closes the channel, so the writer task can exit and the file handle is
  closed.

This appears to be the intended design (the comment in `codex-rs/core/src/codex.rs:2117` calls out
avoiding races with the background writer, especially in tests). If this shutdown barrier has
user-facing downsides (e.g., exit latency when the queue is backlogged), it would be worth
confirming intent and expectations with code owners while implementing the prompt flow.

## Developer notes (footguns and invariants)

This section is a checklist of context that is easy to miss when changing quit behavior.

### Key-event path vs signal path

In the TUIs, `Ctrl+C` and `Ctrl+D` typically arrive as key events (raw mode), not as `SIGINT` or
stdin EOF. Signal forwarding in the npm wrapper can affect non-TUI usage, but it usually does not
drive the in-TUI `Ctrl+C` handling. The quit confirmation prompt is about the key-event path.

### “Consumed vs not consumed” is the quit boundary

`Ctrl+C` is first offered to the bottom pane (`BottomPane::on_ctrl_c`), which may:

- dismiss an active view/modal,
- clear a non-empty composer draft (and show the quit hint),
- or return “not handled” when the composer is empty and no view is active.

The quit confirmation prompt should only be reachable when `Ctrl+C` was not consumed by the UI
layer (and `Ctrl+D` should be disabled while a modal is open).

### “Task running” is a policy input, not a source of truth

The current `Ctrl+C` decision uses `bottom_pane.is_task_running()` as the proxy for “working vs
idle” (see `ChatWidget::on_ctrl_c` in both TUIs). This state is toggled by task lifecycle events
and can be wrong transiently if state updates lag behind UI input.

Implications:

- Any regression that makes “task running” false while the agent is still active can turn `Ctrl+C`
  into a quit (shutdown) path instead of an interrupt path.
- Review mode (`is_review_mode`) is tracked separately from “task running” today. The reported bug
  (“quit during review”) strongly suggests these states can disagree. The implementation should
  treat review as cancellable work and ensure the `Ctrl+C` path hits interrupt/cancel, not quit.

### Interrupt vs shutdown clean up different things

- `Op::Interrupt` is the “cancel current work” path.
- `Op::Shutdown` is the “stop everything and clean up” path. It aborts tasks, terminates unified
  exec processes, and fences rollout persistence before emitting `ShutdownComplete`.

The quit confirmation prompt must preserve this distinction; it should not replace a shutdown-first
quit path with an immediate exit.

### Shutdown completion is wired to exit today

In both TUIs, `ChatWidget` treats `ShutdownComplete` as “exit now” via `request_exit()`. The app
layer can ignore one `ShutdownComplete` using `suppress_shutdown_complete` when shutdown is used as
cleanup (e.g., `/new`).

When adding a confirmation prompt, keep this separation intact:

- “shutdown” is a core lifecycle action, and it may be used without quitting (conversation reset),
- “exit” is a UI lifecycle action, and it may happen without shutdown (a fallback for shutdown
  hangs, but not the preferred/normal path).

### `/new` already uses shutdown without quitting

`/new` (NewSession) shuts down the current conversation/thread, suppresses the next
`ShutdownComplete`, removes the thread from the manager, and then creates a new session. This is a
useful example of why shutdown intent must be explicit: “shutdown” does not always mean “quit the
app”.

### Consider “shutdown stuck” behavior

If the quit confirmation prompt encourages more shutdown-first paths, it increases the chance
users wait on shutdown. The current UI largely assumes shutdown completes. If shutdown can hang in
practice, the design should decide whether to:

- show a status line while shutting down,
- apply a timeout and then force-exit,
- or provide an escape hatch (e.g., a second confirm to exit immediately).

This doc does not pick a policy yet, but implementers should consider it when wiring the flow.

### Observability and hang diagnosis

There are known cases where exit feels “stuck” to users. The shutdown/exit split makes this harder
to debug unless we log the decision points and the time spent waiting.

The goal of logging here is: given a single log excerpt, it should be obvious:

- what initiated the exit attempt (key vs slash command vs internal reset),
- whether we chose interrupt, shutdown+exit, or exit-immediately,
- what cleanup steps we started and finished,
- what timed out or errored,
- and whether `ShutdownComplete` was suppressed (conversation reset) or acted upon (quit).

Recommended logging points (high signal, low volume):

- **UI quit initiation** (tui/tui2): log once when we decide a quit path is being taken. Include:
  - trigger (`ctrl_c`, `ctrl_d`, `/quit`, `/exit`, `/logout`, `/new`, etc.),
  - action (`interrupt`, `shutdown_then_exit`, `exit_immediately`),
  - identifiers (thread/conversation id if available),
  - and whether a confirmation prompt was shown or skipped due to config.
- **Shutdown request sent** (tui/tui2): log when `Op::Shutdown` is submitted and record a start
  timestamp for latency measurement.
- **Shutdown complete observed** (tui/tui2): log when `ShutdownComplete` is received, including:
  - elapsed time since shutdown request (if known),
  - whether it was suppressed (`suppress_shutdown_complete`),
  - and the resulting action (exit UI vs continue running).
- **Core shutdown handler** (core): log the start and end of each cleanup step with durations:
  - abort tasks,
  - terminate unified exec processes,
  - rollout recorder shutdown barrier,
  - emit `ShutdownComplete`.

Timeout guidance:

- `abort_all_tasks` already includes a bounded “graceful interrupt” window per task, but the overall
  shutdown path can still stall on I/O (e.g., rollout file flush) or unexpected blocking. If hangs
  are a problem, add bounded timeouts around the most likely culprits (rollout shutdown barrier and
  unified exec termination) and emit a warning that includes which step timed out and what cleanup
  may have been skipped.

Developer ergonomics:

- Prefer structured logs with stable fields (trigger, action, ids, elapsed_ms, step) so we can grep
  and so future telemetry can be added without rewriting messages.
- Use one-line summaries at INFO and keep deeper detail at DEBUG to avoid spamming normal runs.

### Render the prompt UI in ChatWidget (both tui and tui2)

Add a method like:

```rust
fn open_exit_confirmation_prompt(&mut self)
```

Implementation requirements:

- Do not open the prompt if another bottom-pane view/popup is already active.
- “Quit now” initiates shutdown+exit.
- “Quit and don’t ask again” triggers, in order:
  1) update in-memory config (`UpdateExitConfirmationPromptHidden(true)`),
  2) persist it (`PersistExitConfirmationPromptHidden`),
  3) initiate shutdown+exit.
- Keep copy consistent between `tui` and `tui2` and mention `Ctrl+C`/`Ctrl+D` explicitly.

### Centralize “can cancel?” logic (fixes the review bug)

The related bug (“Ctrl+C quits during review”) is fundamentally a missing/incorrect state signal.

As part of this work, define a single predicate used by the `Ctrl+C` handler:

- `is_cancellable_work_active()` (name may vary), returning true for:
  - agent turn streaming,
  - review task running,
  - any other operation that should treat `Ctrl+C` as interrupt.

Then:

- If `is_cancellable_work_active()` is true, `Ctrl+C` must send `Op::Interrupt` (or the
  appropriate review-cancel op) and must not open the quit prompt.
- Ensure the review workflow flips whatever state drives this predicate early and keeps it true
  until the review is fully finished or cancelled.

This is the correctness fix; the confirmation prompt is the safety net.

### Persist config

In `codex-rs/core`:

- Add `Notice::hide_exit_confirmation_prompt: Option<bool>`.
- Add a `ConfigEdit` variant and a `ConfigEditsBuilder` helper:
  - `SetNoticeHideExitConfirmationPrompt(bool)`
  - `set_hide_exit_confirmation_prompt(bool) -> Self`
- Add a unit test ensuring setting this flag preserves existing `[notice]` table contents
  (mirroring existing tests for other notice keys).

In docs:

- Update `docs/config.md` to mention the new `[notice]` key and describe the behavior in one short
  paragraph.

## Testing plan

### Snapshot tests (both tui and tui2)

Add an `insta` snapshot test that renders the exit confirmation prompt and asserts the full popup
text. This locks:

- the exact prompt copy (grounded in the rationale),
- the presence of the “don’t ask again” option,
- line wrapping behavior at typical widths (e.g., 80 columns).

Keep snapshot names aligned between crates, e.g.:

- `exit_confirmation_popup` (tui)
- `exit_confirmation_popup` (tui2)

### Behavioral tests (both tui and tui2)

Add targeted tests that encode the correctness boundaries:

- `ctrl_c_during_review_cancels_review_not_exit`
  - Assert: no `ExitRequest`/exit event emitted; interrupt/cancel path is taken.
- `ctrl_c_when_task_running_interrupts_not_exit`
- `ctrl_d_does_not_exit_when_modal_open`
- `ctrl_d_does_not_exit_when_composer_non_empty`
- `dont_ask_again_sets_notice_and_persists`
  - Assert the `Update...Hidden(true)` and `Persist...Hidden` events are emitted in sequence (or
    equivalent).

### Manual testing (both tui and tui2)

Run the same checklist in `codex-rs/tui` and `codex-rs/tui2`.

- **Idle, composer empty**
  - Press `Ctrl+C` → confirmation prompt appears → confirm quit → app performs shutdown+exit.
  - Press `Ctrl+D` → confirmation prompt appears → confirm quit → app performs shutdown+exit.
- **Idle, composer has draft**
  - Press `Ctrl+C` → clears draft (no prompt).
  - Press `Ctrl+D` → does not quit (normal input behavior; no prompt).
- **While streaming / tool running**
  - Start a turn that streams for a bit (or run a tool that takes time, e.g. something that sleeps).
  - Press `Ctrl+C` → interrupts/cancels work (no prompt, no quit).
- **Review in progress**
  - Start a review flow.
  - Press `Ctrl+C` → cancels the review (no prompt, no quit).
- **Modal/popup open**
  - Open any modal/popup (approvals/picker/etc.).
  - Press `Ctrl+D` → must not quit.
  - Press `Ctrl+C` → dismisses/aborts the modal (must not quit).
- **Slash commands**
  - Run `/quit` (and separately `/exit`, `/logout`) → exits without prompting, but still uses the
    shutdown+exit path.
- **Don’t ask again**
  - Trigger the prompt and choose “don’t ask again”.
  - Repeat `Ctrl+C` / `Ctrl+D` on idle+empty → no prompt; still shutdown+exit.
  - Verify the flag persisted in `~/.codex/config.toml` under `[notice]`.
- **Shutdown completion handling**
  - Trigger quit and verify the UI waits for shutdown completion (via logs/visible delay if any).
  - Run `/new` and verify shutdown completion is suppressed (app stays running).

### Core persistence unit test (codex-core)

Add a blocking config edit test analogous to existing notice tests:

- seed a config file with `[notice]\nexisting = "value"\n`
- apply the edit
- assert the file preserves `existing` and appends `hide_exit_confirmation_prompt = true`

## Implementation notes (code documentation)

Add short doc comments in code where they prevent re-introducing the same regressions:

- On `Notice::hide_exit_confirmation_prompt`:
  - explain that it suppresses the `Ctrl+C`/`Ctrl+D` quit confirmation prompt.
- Near the `Ctrl+C` handler:
  - explicitly document the priority order: modal dismissal → interrupt/cancel → clear draft → quit
    flow.
- On the “review cancellation” predicate:
  - explicitly mention that review must count as cancellable work, to prevent quitting mid-review.

These comments should be brief and focused on intent (the “why”), not on re-stating code.

## Open questions / decisions to confirm

1. **Should quit always be shutdown-first?**
   - Recommendation: yes for all quit paths (keys and slash commands), keeping “exit immediately”
     only as a logged fallback for shutdown hangs/timeouts.
2. **Should the quit prompt appear on the first `Ctrl+C` press or retain the historical “two-step
   hint” behavior?**
   - Recommendation: the prompt replaces the two-step hint for the idle-empty case (clearer and
     more explicit).
3. **Should `Ctrl+D` show the same prompt as `Ctrl+C`?**
   - Recommendation: yes, but only when composer is empty and no modal is open.

## Rollout considerations

- Default behavior should be safe: prompt enabled unless explicitly disabled.
- The “don’t ask again” option must persist reliably (core config edit test).
- Ensure we do not prompt during active work: the review cancellation fix is a prerequisite for
  the prompt to feel correct rather than annoying.
