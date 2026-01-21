# TUI chat composer state machine

This document describes the bottom-pane chat composer behavior used by both
`codex-rs/tui` and `codex-rs/tui2`. It is intentionally aligned with the
module docs in `tui/src/bottom_pane/chat_composer.rs` and
`tui2/src/bottom_pane/chat_composer.rs`.

## Responsibilities

- Maintain the input buffer (`TextArea`) and its text elements (attachment and
  large-paste placeholders).
- Route key events to popups or to the core editor.
- Decide whether Enter submits or inserts a newline; Tab can queue.
- Convert rapid key bursts into explicit pastes when terminals do not provide
  bracketed paste.

## Key event routing

All key handling starts in `ChatComposer::handle_key_event`.

- If a popup is active (slash commands, file search, skill mentions), the
  popup-specific handler runs.
- Otherwise, `handle_key_event_without_popup` processes the input.
- After every handled key, `sync_popups` refreshes popup state to match the
  current buffer and cursor.

## Submission flow (Enter/Tab)

There are multiple submission paths, but they converge on the same core rules.

### Normal submit/queue path

`handle_submission` calls `prepare_submission_text` for both submit and queue.
That method performs the following steps:

1. Snapshot the original input (text, elements, attachments, pending pastes) so
   the composer can restore state if submission is suppressed.
2. If there are pending paste placeholders, expand them to their full text and
   rebuild element ranges so elements stay aligned with the new buffer.
3. Trim whitespace and rebase element ranges against the trimmed text.
4. If the line is a slash command, verify it is built-in or a known prompt name;
   unknown commands report an error and restore the original state.
5. If the line is a `/prompts:` custom prompt, expand it:
   - Named args use key=value parsing.
   - Numeric args use positional parsing for $1..$9 and $ARGUMENTS.
   The expansion returns both text and text elements, which become the new
   submission payload.
6. Prune attachments so only placeholders that survive the expanded text and
   text elements are sent.
7. If the final text is empty and there are no attachments, suppress submission.
8. Clear pending pastes on success and return the prepared text/elements.

### Numeric auto-submit path

When the slash popup is open and the first line looks like a numeric-only
custom prompt with positional arguments, Enter triggers an early submit that
bypasses `prepare_submission_text`.

That path still applies the same rules:

- Expand pending pastes before parsing positional args.
- Use the expanded text elements for parsing and prompt expansion.
- Prune attachments based on the expanded placeholders.
- Clear pending pastes after a successful auto-submit.

### Queueing

When steer is disabled, Enter queues instead of submitting, but it uses the
same `prepare_submission_text` flow, so expansion and pruning are identical.

## Attachments and text elements

- Images are inserted as atomic elements (for example `[Image #1]`) and tracked
  in `attached_images`.
- Editing that removes a placeholder will drop the corresponding attachment.
- On submission, attachments are pruned based on the expanded text elements so
  only placeholders that remain after prompt expansion are sent.

## Pending pastes

Large pastes are inserted as placeholder elements and stored in `pending_pastes`.
On submission (including numeric auto-submit), those placeholders are expanded
back to full text and then cleared so they cannot leak into later submissions.

## Paste-burst state machine

When terminals do not provide bracketed paste (notably Windows), the composer
buffers rapid key bursts and flushes them into `handle_paste`. The burst
behavior is documented in `tui/src/bottom_pane/paste_burst.rs` and
`tui2/src/bottom_pane/paste_burst.rs`.

Key points:

- ASCII bursts may hold the first character briefly (flicker suppression).
- Non-ASCII input does not hold the first character, but can still form a burst.
- The burst detector can be disabled, which flushes or clears any in-flight
  burst state to prevent it from affecting later input.
