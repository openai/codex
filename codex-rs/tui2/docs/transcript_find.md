# Transcript Find (Inline Viewport)

This document describes the design for “find in transcript” in the **TUI2 inline viewport** (the
main transcript region above the composer), not the full-screen transcript overlay.

The goal is to provide fast, low-friction navigation through the in-memory transcript while keeping
the UI predictable and the implementation easy to review/maintain.

---

## Goals

- **Search the inline viewport content**, derived from the same flattened transcript lines used for
  scrolling/selection, so search results track what the user sees.
- **Ephemeral UI**: no always-on search bar and no scroll bar in this iteration.
- **Fast navigation**:
  - highlight all matches
  - jump to the next match repeatedly without reopening the prompt
- **Stable anchoring**: jumping should land on stable content anchors (cell + line), not raw screen
  rows.
- **Reviewable architecture**: keep `app.rs` changes small by placing feature logic in a dedicated
  module and calling it from the render loop and key handler.

---

## Current Implementation (What We Have Today)

This section documents the current state so it’s easy to compare against the “ideal/perfect” end
state discussed in review.

For implementation details, see the rustdoc comments and unit tests in `tui2/src/transcript_find.rs`.

### UI

- When active, a single prompt row is rendered **above the composer**:
  - `"/ query  current/total"`
- Matches are highlighted in the transcript:
  - all matches: underlined
  - current match: reversed + bold + underlined
- The prompt is **not persistent**: it only appears while editing.

### Keys

- `Ctrl-F`: open the find prompt and start editing the query.
- While editing:
  - type to edit the query (highlights update as you type)
  - `Backspace`: delete one character
  - `Ctrl-U`: clear the query
  - `Enter`: close the prompt and jump to a match (if any)
  - `Esc`: close the prompt without clearing the query (highlights remain)
- `Ctrl-G`: jump to next match.
  - Works while editing (prompt stays open).
  - Works even after the prompt is closed, as long as the query is still active.
- `Esc` (when not editing and a query is active): clears the search/highlights.

### Footer hints

- When the find prompt is visible, the footer shows `Ctrl-G next match`:
  - in the shortcut summary line
  - and in the `?` shortcut overlay

### Implementation layout

- Core logic lives in `tui2/src/transcript_find.rs`:
  - key handling
  - match computation/caching
  - jump selection
  - per-line rendering helper (`render_line`) and prompt rendering helper (`render_prompt_line`)
- `tui2/src/app.rs` is kept mostly additive by delegating:
  - early key handling delegation in `App::handle_key_event`
  - per-frame recompute/jump hook after transcript flattening
  - per-row render hook for match highlighting
  - prompt + cursor positioning while editing
- Footer hint integration is wired via `set_transcript_ui_state(..., find_visible)` through:
  - `tui2/src/chatwidget.rs`
  - `tui2/src/bottom_pane/mod.rs`
  - `tui2/src/bottom_pane/chat_composer.rs`
  - `tui2/src/bottom_pane/footer.rs`

---

## UX and Keybindings

### Entering search

- `Ctrl-F` opens the find prompt on the line immediately above the composer.
- While the prompt is open, typed characters update the query and immediately update highlights.

### Navigating results

- `Ctrl-G` jumps to the next match.
  - Works while the prompt is open.
  - Also works after the prompt is closed as long as a non-empty query is still active (so users can
    “keep stepping” through matches).

### Exiting / clearing

- `Esc` closes the prompt without clearing the active query (and therefore keeps highlights).
- `Esc` again (when not editing and a query is active) clears the search/highlights.

### Footer hints

When the find prompt is visible, we surface the relevant navigation key (`Ctrl-G`) in:

- the shortcut summary line (the default footer mode)
- the “?” shortcut overlay

This keeps the prompt itself visually minimal.

---

## Data Model: Search Over Flattened Lines

Search operates over the same representation as scrolling and selection:

1. Cells are flattened into a list of `Line<'static>` plus parallel `TranscriptLineMeta` entries
   (see `tui2/src/tui/scrolling.rs` and `tui2/docs/tui_viewport_and_history.md`).
2. The find module searches **plain text** extracted from each flattened line (by concatenating its
   spans’ contents).
3. Each match stores:
   - `line_index` (index into flattened lines)
   - `range` (byte range within the flattened line’s plain text)
   - `anchor` derived from `TranscriptLineMeta::CellLine { cell_index, line_in_cell }`

The anchor is used to update `TranscriptScroll` when jumping so the viewport lands on stable content
even if the transcript grows.

---

## Matching Semantics

### Smart-case

The search is “smart-case”:

- If the query contains any ASCII uppercase, the match is case-sensitive.
- Otherwise, both haystack and needle are matched in ASCII-lowercased form.

This avoids expensive Unicode case folding and keeps behavior predictable in terminals.

---

## Rendering

### Highlights

- All matches are highlighted (currently: underlined).
- The “current match” is emphasized more strongly (currently: reversed + bold + underlined).

Highlighting is applied at render time for each visible line by splitting spans into segments and
patching styles for the match ranges.

### Prompt line

While editing, the line directly above the composer shows:

`/ query  current/total`

It is rendered inside the transcript viewport area (not as a persistent UI element), and the cursor
is moved into this line while editing.

---

## Performance / Caching

Recomputing matches happens only when needed. The search module caches based on:

- transcript width (wrapping changes can change the flattened line list)
- number of flattened lines (transcript growth)

This keeps the work proportional to actual content changes rather than every frame.

---

## Code Layout (Additive, Review-Friendly)

The implementation is structured so `app.rs` only delegates:

- `tui2/src/transcript_find.rs` owns:
  - query/edit state
  - match computation and caching
  - key handling for find-related shortcuts
  - rendering helpers for highlighted lines and the prompt line
  - producing a scroll anchor when a jump is requested

`app.rs` integration points are intentionally small:

- **Key handling**: early delegation to `TranscriptFind::handle_key_event`.
- **Render**:
  - call `TranscriptFind::on_render` after building flattened lines to apply pending jumps
  - call `TranscriptFind::render_line` per visible row
  - render `render_prompt_line` when active and set cursor with `cursor_position`
- **Footer**:
  - `set_transcript_ui_state(..., find_visible)` so the footer can show find-related hints only when
    the prompt is visible.

---

## Comparison to the “Ideal” End State

### Ideal UX (what “perfect” looks like)

- **Ephemeral, minimal UI**: no always-on search bar, and no scroll bar for this feature.
- **Fast entry**: `Ctrl-F` opens a single prompt row above the composer.
- **Live feedback**: highlights update as you type, and the prompt shows `current/total`.
- **Repeat navigation without closing**: `Ctrl-G` jumps to the next match while the prompt stays
  open, and continues to work after the prompt closes as long as the query is active.
- **Predictable exit semantics**:
  - `Enter`: accept query, close prompt, and jump (if any matches)
  - `Esc`: close the prompt but keep the query/highlights
  - `Esc` again (with an active query): clear the query/highlights
- **Stable jumping**: navigation targets stable transcript anchors (cell + line-in-cell), so jumping
  behaves well as the transcript grows.
- **Discoverability without clutter**: when the prompt is visible, the footer/shortcuts surface the
  navigation key (`Ctrl-G`) so the prompt itself stays tight.
- **Future marker integration**: if/when a scroll indicator is introduced, match markers integrate
  with it (faint ticks for match lines, stronger marker for the current match).

### Already aligned with the ideal

- Ephemeral prompt (no always-on bar).
- Live highlighting while typing.
- `Ctrl-G` repeat navigation without reopening the prompt (including while editing).
- Stable jump anchoring via `(cell_index, line_in_cell)` metadata.
- Footer hints (`Ctrl-G next match`) shown only while the prompt is visible.
- Minimal, review-friendly integration points in `app.rs` via `tui2/src/transcript_find.rs`.

### Not implemented yet (intentional deferrals)

- Prev match (e.g. `Ctrl-Shift-G`).
- “Contextual landing” when jumping (e.g. padding/centering so the match isn’t pinned to the top).
- Match markers integrated with a future scroll indicator.

### Known limitations / trade-offs in the current version

- Matching is ASCII smart-case (no full Unicode case folding).
- Match ranges are byte ranges in the flattened plain text. This is fine for styling spans by byte
  slicing, but any future “column-precise” behaviors should be careful with multi-byte characters.

---

## Future Work (Not Implemented Here)

- **Prev match**: add `Ctrl-Shift-G` for previous match if desired.
- **Marker integration**: if/when a scroll indicator is added, include match markers derived from
  match line indices (faint ticks) and a stronger marker for the current match.
- **Contextual jump placement**: center the current match (or provide padding above) rather than
  placing it at the exact top row when jumping.
