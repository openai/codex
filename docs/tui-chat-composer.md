# Chat Composer state machine (TUI)

This note documents the `ChatComposer` input state machine and the paste-related behavior added
for Windows terminals.

Primary implementations:

- `codex-rs/tui/src/bottom_pane/chat_composer.rs`

Paste-burst detector:

- `codex-rs/tui/src/bottom_pane/paste_burst.rs`

## What problem is being solved?

On some terminals (notably on Windows via `crossterm`), _bracketed paste_ is not reliably surfaced
as a single paste event. Instead, pasting multi-line content can show up as a rapid sequence of
key events:

- `KeyCode::Char(..)` for text
- `KeyCode::Enter` for newlines

If the composer treats those events as “normal typing”, it can:

- accidentally trigger UI toggles (e.g. `?`) while the paste is still streaming,
- submit the message mid-paste when an `Enter` arrives,
- render a typed prefix, then “reclassify” it as paste once enough chars arrive (flicker).

The solution is to detect paste-like _bursts_ and buffer them into a single explicit
`handle_paste(String)` call.

## High-level state machines

`ChatComposer` effectively combines two small state machines:

1. **UI mode**: which popup (if any) is active.
   - `ActivePopup::None | Command | File | Skill`
2. **Paste burst**: transient detection state for non-bracketed paste.
   - implemented by `PasteBurst`

### Key event routing

`ChatComposer::handle_key_event` dispatches based on `active_popup`:

- If a popup is visible, a popup-specific handler processes the key first (navigation, selection,
  completion).
- Otherwise, `handle_key_event_without_popup` handles higher-level semantics (Enter submit,
  history navigation, etc).
- After handling the key, `sync_popups()` runs so popup visibility/filters stay consistent with the
  latest text + cursor.
- When a slash command name is completed and the user types a space, the `/command` token is
  promoted into a text element so it renders distinctly and edits atomically.

### History navigation (↑/↓)

Up/Down recall is handled by `ChatComposerHistory` and merges two sources:

- **Persistent history** (cross-session, fetched from `~/.codex/history.jsonl`): text-only. It
  does **not** carry text element ranges or local image attachments, so recalling one of these
  entries only restores the text.
- **Local history** (current session): stores the full submission payload, including text
  elements and local image paths. Recalling a local entry rehydrates placeholders and attachments.

This distinction keeps the on-disk history backward compatible and avoids persisting attachments,
while still providing a richer recall experience for in-session edits.

## Config gating for reuse

`ChatComposer` now supports feature gating via `ChatComposerConfig`
(`codex-rs/tui/src/bottom_pane/chat_composer.rs`). The default config preserves current chat
behavior.

Flags:

- `popups_enabled`
- `slash_commands_enabled`
- `image_paste_enabled`

Key effects when disabled:

- When `popups_enabled` is `false`, `sync_popups()` forces `ActivePopup::None`.
- When `slash_commands_enabled` is `false`, the composer does not treat `/...` input as commands.
- When `slash_commands_enabled` is `false`, the composer does not expand custom prompts in
  `prepare_submission_text`.
- When `slash_commands_enabled` is `false`, slash-context paste-burst exceptions are disabled.
- When `image_paste_enabled` is `false`, file-path paste image attachment is skipped.
- `ChatWidget` may toggle `image_paste_enabled` at runtime based on the selected model's
  `input_modalities`; attach and submit paths also re-check support and emit a warning instead of
  dropping the draft.

Built-in slash command availability is centralized in
`codex-rs/tui/src/bottom_pane/slash_commands.rs` and reused by both the composer and the command
popup so gating stays in sync.

## Submission flow (Enter/Tab)

There are multiple submission paths, but they share the same core rules:

### Normal submit/queue path

`handle_submission` calls `prepare_submission_text` for both submit and queue. That method:

1. Expands any pending paste placeholders so element ranges align with the final text.
2. Trims whitespace and rebases element ranges to the trimmed buffer.
3. Expands `/prompts:` custom prompts:
   - Named args use key=value parsing.
   - Numeric args use positional parsing for `$1..$9` and `$ARGUMENTS`.
     The expansion preserves text elements and yields the final submission payload.
4. Prunes attachments so only placeholders that survive expansion are sent.
5. Clears pending pastes on success and suppresses submission if the final text is empty and there
   are no attachments.

The same preparation path is reused for slash commands with arguments (for example `/plan` and
`/review`) so pasted content and text elements are preserved when extracting args.

### Numeric auto-submit path

When the slash popup is open and the first line matches a numeric-only custom prompt with
positional args, Enter auto-submits without calling `prepare_submission_text`. That path still:

- Expands pending pastes before parsing positional args.
- Uses expanded text elements for prompt expansion.
- Prunes attachments based on expanded placeholders.
- Clears pending pastes after a successful auto-submit.

## Paste burst: concepts and assumptions

The burst detector is intentionally conservative: it only processes “plain” character input
(no Ctrl/Alt modifiers). Everything else flushes and/or clears the burst window so shortcuts keep
their normal meaning.

### Conceptual `PasteBurst` states

- **Idle**: no buffer, no pending char.
- **Pending first char** (ASCII only): hold one fast character very briefly to avoid rendering it
  and then immediately removing it if the stream turns out to be a paste.
- **Active buffer**: once a burst is classified as paste-like, accumulate the content into a
  `String` buffer.
- **Enter suppression window**: keep treating `Enter` as “newline” briefly after burst activity so
  multiline pastes remain grouped even if there are tiny gaps.

### ASCII vs non-ASCII (IME) input

Non-ASCII characters frequently come from IMEs and can legitimately arrive in quick bursts. Holding
the first character in that case can feel like dropped input.

The composer therefore distinguishes:

- **ASCII path**: allow holding the first fast char (`PasteBurst::on_plain_char`).
- **non-ASCII path**: never hold the first char (`PasteBurst::on_plain_char_no_hold`), but still
  allow burst detection. When a burst is detected on this path, the already-inserted prefix may be
  retroactively removed from the textarea and moved into the paste buffer.

To avoid misclassifying IME bursts as paste, the non-ASCII retro-capture path runs an additional
heuristic (`PasteBurst::decide_begin_buffer`) to determine whether the retro-grabbed prefix “looks
pastey” (e.g. contains whitespace or is long).

### Disabling burst detection

`ChatComposer` supports `disable_paste_burst` as an escape hatch.

When enabled:

- The burst detector is bypassed for new input (no flicker suppression hold and no burst buffering
  decisions for incoming characters).
- The key stream is treated as normal typing (including normal slash command behavior).
- Enabling the flag flushes any held/buffered burst text through the normal paste path
  (`ChatComposer::handle_paste`) and then clears the burst timing and Enter-suppression windows so
  transient burst state cannot leak into subsequent input.

### Enter handling

When paste-burst buffering is active, Enter is treated as “append `\n` to the burst” rather than
“submit the message”. This prevents mid-paste submission for multiline pastes that are emitted as
`Enter` key events.

The composer also disables burst-based Enter suppression inside slash-command context (popup open
or the first line begins with `/`) so command dispatch is predictable.

## PasteBurst: event-level behavior (cheat sheet)

This section spells out how `ChatComposer` interprets the `PasteBurst` decisions. It’s intended to
make the state transitions reviewable without having to “run the code in your head”.

### Plain ASCII `KeyCode::Char(c)` (no Ctrl/Alt modifiers)

`ChatComposer::handle_input_basic` calls `PasteBurst::on_plain_char(c, now)` and switches on the
returned `CharDecision`:

- `RetainFirstChar`: do **not** insert `c` into the textarea yet. A UI tick later may flush it as a
  normal typed char via `PasteBurst::flush_if_due`.
- `BeginBufferFromPending`: the first ASCII char is already held/buffered; append `c` via
  `PasteBurst::append_char_to_buffer`.
- `BeginBuffer { retro_chars }`: attempt a retro-capture of the already-inserted prefix:
  - call `PasteBurst::decide_begin_buffer(now, before_cursor, retro_chars)`;
  - if it returns `Some(grab)`, delete `grab.start_byte..cursor` from the textarea and then append
    `c` to the buffer;
  - if it returns `None`, fall back to normal insertion.
- `BufferAppend`: append `c` to the active buffer.

### Plain non-ASCII `KeyCode::Char(c)` (no Ctrl/Alt modifiers)

`ChatComposer::handle_non_ascii_char` uses a slightly different flow:

- It first flushes any pending transient ASCII state with `PasteBurst::flush_before_modified_input`
  (which includes a single held ASCII char).
- If a burst is already active, `PasteBurst::try_append_char_if_active(c, now)` appends `c` directly.
- Otherwise it calls `PasteBurst::on_plain_char_no_hold(now)`:
  - `BufferAppend`: append `c` to the active buffer.
  - `BeginBuffer { retro_chars }`: run `decide_begin_buffer(..)` and, if it starts buffering, delete
    the retro-grabbed prefix from the textarea and append `c`.
  - `None`: insert `c` into the textarea normally.

The extra `decide_begin_buffer` heuristic on this path is intentional: IME input can arrive as
quick bursts, so the code only retro-grabs if the prefix “looks pastey” (whitespace, or a long
enough run) to avoid misclassifying IME composition as paste.

### `KeyCode::Enter`: newline vs submit

There are two distinct “Enter becomes newline” mechanisms:

- **While in a burst context** (`paste_burst.is_active()`): `append_newline_if_active(now)` appends
  `\n` into the burst buffer so multi-line pastes stay buffered as one explicit paste.
- **Immediately after burst activity** (enter suppression window):
  `newline_should_insert_instead_of_submit(now)` inserts `\n` into the textarea and calls
  `extend_window(now)` so a slightly-late Enter keeps behaving like “newline” rather than “submit”.

Both are disabled inside slash-command context (command popup is active or the first line begins
with `/`) so Enter keeps its normal “submit/execute” semantics while composing commands.

### Non-char keys / Ctrl+modified input

Non-char input must not leak burst state across unrelated actions:

- If there is buffered burst text, callers should flush it before calling
  `clear_window_after_non_char` (see “Pitfalls worth calling out”), typically via
  `PasteBurst::flush_before_modified_input`.
- `PasteBurst::clear_window_after_non_char` clears the “recent burst” window so the next keystroke
  doesn’t get incorrectly grouped into a previous paste.

### Pitfalls worth calling out

- `PasteBurst::clear_window_after_non_char` clears `last_plain_char_time`. If you call it while
  `buffer` is non-empty and _haven’t already flushed_, `flush_if_due()` no longer has a timestamp
  to time out against, so the buffered text may never flush. Treat `clear_window_after_non_char` as
  “drop classification context after flush”, not “flush”.
- `PasteBurst::flush_if_due` uses a strict `>` comparison, so tests and UI ticks should cross the
  threshold by at least 1ms (see `PasteBurst::recommended_flush_delay`).

## Notable interactions / invariants

- The composer frequently slices `textarea.text()` using the cursor position; all code that
  slices must clamp the cursor to a UTF-8 char boundary first.
- `sync_popups()` must run after any change that can affect popup visibility or filtering:
  inserting, deleting, flushing a burst, applying a paste placeholder, etc.
- Shortcut overlay toggling via `?` is gated on `!is_in_paste_burst()` so pastes cannot flip UI
  modes while streaming.
- Mention popup selection has two payloads: visible `$name` text and hidden
  `mention_paths[name] -> canonical target` linkage. The generic
  `set_text_content` path intentionally clears linkage for fresh drafts; restore
  paths that rehydrate blocked/interrupted submissions must use the
  mention-preserving setter so retry keeps the originally selected target.

## Tests that pin behavior

The `PasteBurst` logic is currently exercised through `ChatComposer` integration tests.

- `codex-rs/tui/src/bottom_pane/chat_composer.rs`
  - `non_ascii_burst_handles_newline`
  - `ascii_burst_treats_enter_as_newline`
  - `question_mark_does_not_toggle_during_paste_burst`
  - `burst_paste_fast_small_buffers_and_flushes_on_stop`
  - `burst_paste_fast_large_inserts_placeholder_on_flush`

This document calls out some additional contracts (like “flush before clearing”) that are not yet
fully pinned by dedicated `PasteBurst` unit tests.
