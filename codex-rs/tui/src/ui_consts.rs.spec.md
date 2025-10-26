## Overview
Defines shared column offsets used across the TUI to keep left gutters and footers aligned.

## Detailed Behavior
- `LIVE_PREFIX_COLS` (2 columns) reserves space for the left gutter used by live cells, composer borders, and status indicators.
- `FOOTER_INDENT_COLS` mirrors the same width for footer alignment.

## Broader Context
- Referenced by the chat composer, status indicator, and history rendering code to maintain consistent padding.

## Technical Debt
- Hard-coded width works for the current design; future theming or multi-character gutters may require revisiting these constants.

---
tech_debt:
  severity: low
  highest_priority_items:
    - Revisit these constants if the gutter design changes (e.g., multi-column icons).
related_specs:
  - status_indicator_widget.rs.spec.md
  - resume_picker.rs.spec.md
