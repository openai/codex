## Overview
Public widgets exposed by the TUI crate. Currently this module re-exports the `composer_input` wrapper so other crates can embed Codex’s chat composer behavior without pulling in all internal modules.

## Detailed Behavior
- Re-exports `composer_input`, which contains the `ComposerInput` widget and its associated API surface.

## Broader Context
- Used by integration crates such as `codex-cloud-tasks` that need a reusable text input with Codex composer semantics (multi-line entry, paste bursts, keyboard shortcuts).
- Future public widgets should be added here and documented alongside their specs.

## Technical Debt
- The module is a thin facade; any additional public exports should ensure they maintain backward compatibility and follow the shared spec format.

---
tech_debt:
  severity: low
  highest_priority_items:
    - None at this time.
related_specs:
  - composer_input.rs.spec.md
