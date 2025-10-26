## Overview
Utility helpers for Ratatui `Line` values. They convert borrowed lines into owned `'static` variants, detect blank lines, and add prefix spans so higher-level renderers can indent blocks without mutating the original data. These helpers keep shared formatting logic in one place for diff summaries and other widgets that assemble text programmatically.

## Detailed Behavior
- `line_to_static` clones a borrowed `Line<'_>` into a `Line<'static>` by copying each span’s style and allocating owned `String` content. Alignment and style flags are preserved.
- `push_owned_lines` appends static copies of borrowed lines into an output vector by reusing `line_to_static` for each element.
- `is_blank_line_spaces_only` treats a line as blank when it has no spans or every span’s content is empty/space-only. Tabs and other whitespace are not ignored, keeping the check conservative for diffs and history views.
- `prefix_lines` prepends either an initial or subsequent prefix `Span` to each line in order, cloning the prefix spans and preserving the original line styles. The result is a new owned `Vec<Line<'static>>`, enabling callers to indent wrapped blocks with minimal allocations.

## Broader Context
- Diff rendering (`../diff_render.rs.spec.md`) prefixes wrapped diff lines with gutter spans and uses the blank-line helper while building summaries.
- Other renderers under `render/` share these helpers when normalizing borrowed lines to `'static` before handing them to Ratatui widgets.

## Technical Debt
- Functions assume prefixes contain ready-to-render styling and do not guard against zero-width or multi-span prefixes; callers must ensure appropriate content.
- `is_blank_line_spaces_only` considers only space characters, so lines containing tabs or other whitespace will be treated as non-blank. Extending the check to full Unicode whitespace may improve future formatting.

---
tech_debt:
  severity: low
  highest_priority_items:
    - Evaluate whether `is_blank_line_spaces_only` should treat tabs or other Unicode whitespace as blank to match Ratatui rendering semantics.
related_specs:
  - ../mod.spec.md
  - diff_render.rs.spec.md
