# Request user input overlay (TUI)

This note documents the TUI overlay used to gather answers for
`RequestUserInputEvent`.

## Overview

The overlay renders one question at a time and collects:

- A single selected option (when options exist).
- Freeform notes (always available).

When options are present, notes are stored per selected option and the first
option is selected by default, so every option question has an answer. If a
question has no options and no notes are provided, the answer is submitted as
`skipped`.

## Focus and input routing

The overlay tracks a small focus state:

- **Options**: Up/Down move the selection and Space selects.
- **Notes**: Text input edits notes for the currently selected option.

Typing while focused on options switches into notes automatically to reduce
friction for freeform input.

## Navigation

- Enter advances to the next question.
- Enter on the last question submits all answers.
- PageUp/PageDown navigate across questions (when multiple are present).
- Esc interrupts the run in option selection mode.
- When notes are open for an option question, Tab or Esc clears notes and returns
  to option selection.

## Layout priorities

The layout prefers to keep the question and all options visible. Notes and
footer hints collapse as space shrinks, with notes falling back to a single-line
"Notes: ..." input in tight terminals.
