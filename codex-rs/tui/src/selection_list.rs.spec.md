## Overview
Helper for building selectable list rows used in menus and dialogs. It produces `Renderable` instances with numbered prefixes, optional highlighting, and wrapped labels so higher-level widgets can compose consistent selection lists.

## Detailed Behavior
- `selection_option_row` accepts an index, label, and selection flag:
  - Prefix is either `› {idx}.` for the selected row (highlight arrow plus number) or two leading spaces for unselected rows.
  - When selected, the entire row (prefix and label) is styled cyan; otherwise default style is used.
  - Uses `RowRenderable` to assemble two segments: a fixed-width prefix (measured via `UnicodeWidthStr`) and a `Paragraph` for the label with wrapping disabled from trimming.
  - Returns a boxed `Renderable` so callers can insert it into columns or overlays without caring about implementation details.

## Broader Context
- Shared by palette dialogs, onboarding flows, and resume picker variants to render consistent numeric option lists.
- Integrates with other renderables under `render/`, enabling mixed content lists alongside textual entries.

## Technical Debt
- Hardcodes cyan as the selection color; exposing a style parameter would let different widgets tailor the highlight.
- Prefix numbering is 1-based and cannot be customized; adapters might need alternative prefixes (e.g., bullet lists).

---
tech_debt:
  severity: low
  highest_priority_items:
    - Allow callers to provide a custom highlight style to match different themes.
related_specs:
  - render/renderable.rs.spec.md
  - pager_overlay.rs.spec.md
