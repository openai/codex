## Overview
Defines a lightweight `KeyBinding` type plus helpers for rendering keyboard shortcuts as dimmed spans in help bars, palette hints, and overlays. It normalizes modifiers and arrow key names so the UI exposes consistent labels regardless of terminal casing.

## Detailed Behavior
- `KeyBinding` stores a `KeyCode` with `KeyModifiers` and exposes:
  - `new` constructor and convenience constructors (`plain`, `alt`, `shift`, `ctrl`).
  - `is_press`, which returns true for matching key events with `Press` or `Repeat` kinds, filtering out release events.
- `modifiers_to_string` appends `ctrl + `, `shift + `, and `alt + ` in that order for any modifiers present.
- `From<KeyBinding>` and `From<&KeyBinding>` create a dim `Span` containing the label. Special cases convert Enter to `enter`, arrow keys to arrows, PageUp/PageDown to `pgup`/`pgdn`; all other keys use `format!("{key}")` lowercased.
- `key_hint_style` centralizes the dim style applied to every span.

## Broader Context
- Status bars and onboarding overlays use these spans to display keyboard hints without re-implementing formatting.
- Palette and slash-command UIs reuse the same labels so modifier order stays consistent across the TUI.

## Technical Debt
- Modifier ordering is hard-coded and assumes no platform-specific modifiers (e.g., Meta); adding new modifiers would require extending the helper.
- Lowercasing the formatted key string may misrepresent keys where case matters (e.g., shifted characters). Explicit mappings could improve clarity.

---
tech_debt:
  severity: low
  highest_priority_items:
    - Extend modifier rendering to support Meta/Super when the TUI is run on macOS terminals.
related_specs:
  - mod.spec.md
  - status/helpers.rs.spec.md
