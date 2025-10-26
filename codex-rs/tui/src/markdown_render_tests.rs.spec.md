## Overview
Test module asserting that the Markdown renderer produces the expected Ratatui `Text` structures. It exercises headings, blockquotes, lists, code fences, HTML passthrough, inline styles, and complex mixed documents so UI rendering stays faithful when upstream parsing changes.

## Detailed Behavior
- Uses `render_markdown_text` as the system under test and primarily compares the resulting `Text` to explicit constructions leveraging `Stylize`.
- Blockquote coverage ensures lazy continuations, nested quotes, list markers, headings, code fences, and color propagation all behave correctly.
- List tests span ordered/unordered nesting, loose vs tight spacing, continuation paragraphs, and integration with blockquotes or HTML content. Marker styling (e.g., light-blue numerals) is explicitly verified.
- Inline formatting verifies bold, italic, strikethrough, inline code, and links (including URL display format). Code-block scenarios cover fenced, indented, nested fences, and list-contained blocks.
- Large composite snapshot (`markdown_render_complex_snapshot`) feeds a broad sample document into `insta::assert_snapshot` to catch regressions across multiple features simultaneously.
- HTML passthrough tests confirm inline and block HTML remains verbatim while respecting indentation within list continuations.

## Broader Context
- Guards `markdown_render.rs` behavior, which in turn feeds history insertion and diff rendering. Keeping these tests up to date prevents regressions in rendered conversations within the TUI.
- Snapshot outputs help detect formatting drift when updating parser crates or stylizing logic.

## Technical Debt
- Tests rely on string concatenation of spans, ignoring style attributes beyond targeted cases; failures might miss subtle styling regressions without additional assertions.
- Snapshot coverage depends on Insta fixtures; ensuring they are reviewed and accepted when legitimate changes occur is critical to prevent stale expectations.

---
tech_debt:
  severity: low
  highest_priority_items:
    - Extend tests to assert style attributes (foreground colors, modifiers) more broadly, not just content concatenation.
related_specs:
  - markdown_render.rs.spec.md
  - insert_history.rs.spec.md
