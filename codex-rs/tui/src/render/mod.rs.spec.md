## Overview
`codex-tui::render` contains layout utilities and generic renderable abstractions that the TUI uses to compose Ratatui widgets. It provides padding helpers (`Insets`, `RectExt`) and traits for building composite renderables (columns, rows, inset wrappers).

## Detailed Behavior
- `Insets` describes top/left/bottom/right padding. Constructors `tlbr` and `vh` create asymmetric or uniform insets.
- `RectExt::inset` (trait) shrinks a `Rect` by provided insets, saturating to prevent underflow.
- `renderable` submodule defines:
  - `Renderable` trait (render + desired_height).
  - Implementations for primitive types (`&str`, `String`, `Span`, `Line`, `Paragraph`, `Option`, `Arc`, unit).
  - Container renderables:
    - `ColumnRenderable` renders children vertically, summing heights.
    - `RowRenderable` renders fixed-width children horizontally.
    - `InsetRenderable` wraps a child with padding.
  - `RenderableExt::inset` helper for ergonomic padding.
- `highlight` submodule provides Bash syntax highlighting (documented separately).
- `line_utils` (not yet covered) handles line wrapping utilities for multi-line rendering.

## Broader Context
- `ChatWidget`, `BottomPane`, and history cells rely on these abstractions to simplify custom UI layouts without duplicating Ratatui boilerplate.
- Combined with `line_utils`, they enable consistent spacing/styling across the transcript and ancillary widgets.
- Context can't yet be determined for future themes; the abstractions are general enough to support theme-aware rendering when introduced.

## Technical Debt
- None significant; the module centralizes layout helpers effectively.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./render/renderable.rs.spec.md
  - ./chatwidget.rs.spec.md
