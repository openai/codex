## Overview
`frames` embeds ASCII animation frames used by the TUI for loading indicators and onboarding sequences. It exposes preloaded variants and the default frame cadence shared by `AsciiAnimation`.

## Detailed Behavior
- `frames_for!` macro expands to a static array of 36 frame strings pulled from `frames/<variant>/frame_*.txt` at compile time.
- Constants define named variants (`FRAMES_DEFAULT`, `FRAMES_CODEX`, etc.), each a ` [&str; 36]`.
- `ALL_VARIANTS` collects references to every variant for randomized selection.
- `FRAME_TICK_DEFAULT` sets the default frame interval (`80ms`), keeping animations consistent across widgets.

## Broader Context
- `AsciiAnimation` consumes these variants to cycle through frames, while popups and the onboarding flow pick specific variants to match branding.

## Technical Debt
- None; adding or removing variants requires only adjusting the compile-time assets and constant list.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./ascii_animation.rs.spec.md
