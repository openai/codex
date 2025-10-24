## Overview
`codex-tui::render::renderable` defines a trait-based abstraction for Ratatui renderables. It lets the TUI compose heterogeneous widgets (strings, spans, paragraphs, nested layouts) without repetitive manual rendering logic.

## Detailed Behavior
- `Renderable` trait: components implement `render(area, buf)` and `desired_height(width)`.
  - Blanket implementations exist for primitive types (`&str`, `String`, `Span`, `Line`, `Paragraph`), `Option`, `Arc`, and unit `()`.
- Containers:
  - `ColumnRenderable`: renders children top-to-bottom, stacking each child using its desired height.
  - `RowRenderable`: renders children left-to-right with fixed widths; height is the max of child heights within remaining width.
  - `InsetRenderable`: wraps a child with padding (`Insets`), adjusting render area and height calculations accordingly.
- `RenderableExt::inset` extension method eases applying padding to any renderable.
- The module relies on `WidgetRef::render_ref` to render Ratatui-native types without consuming them.

## Broader Context
- History cells, bottom pane widgets, and diff renderers use these abstractions to assemble complex layouts (e.g., status indicators atop composable columns) while keeping code concise.
- Aligns with other layout utilities in `render/mod.rs` (Insets, RectExt).
- Context can't yet be determined for future dynamic sizing; current design assumes synchronous layout computation.

## Technical Debt
- None; the trait-based approach keeps rendering flexible.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./render/mod.rs.spec.md
  - ./bottom_pane/mod.rs.spec.md
