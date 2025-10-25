## Overview
`seek_sequence.rs` locates context lines within file contents when applying patch chunks. It progressively relaxes matching rules to emulate git’s tolerance for whitespace and some Unicode punctuation differences.

## Detailed Behavior
- `seek_sequence`:
  - Accepts the file’s existing lines, the patch pattern, a starting index, and an EOF hint.
  - Handles special cases (empty pattern returns `start`, pattern longer than input returns `None`) to avoid panics.
  - Tries four passes in order: exact match, trailing-whitespace-insensitive match, fully trimmed match, and a final normalised match that maps smart quotes/dashes/whitespace to ASCII equivalents.
  - Supports EOF-aware matching by first attempting to align the sequence at the end of the file.
- Tests cover exact, whitespace-trimmed, normalization behavior, and pattern-length safety.

## Broader Context
- Used by `compute_replacements` in `lib.rs` to align patch hunks with existing file content before generating replacements, improving resilience to formatting drift.
- Complements the upstream parser and diff logic so `apply_patch` can apply context-aware updates without invoking git.

## Technical Debt
- None; normalization covers known unicode variants and the guard clauses prevent the historic panic from recurring.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./lib.rs.spec.md
