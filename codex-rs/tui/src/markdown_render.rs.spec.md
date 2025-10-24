## Overview
`codex-tui::markdown_render` converts Markdown into Ratatui `Text<'static>` with Codex-specific styling and wrapping. It leverages `pulldown_cmark` while preserving indentation, list markers, code blocks, and inline styles compatible with terminal output.

## Detailed Behavior
- Entry points:
  - `render_markdown_text(input)` defaults to unlimited width.
  - `render_markdown_text_with_width(input, width)` wraps via `Writer`.
- `Writer` parses events from `pulldown_cmark::Parser`, maintaining:
  - Inline style stack (`Style` instances for bold/italic/strikethrough/links).
  - Indent stack (`IndentContext`) for block quotes, code blocks, and nested lists.
  - List index tracking to render ordered list markers appropriately.
  - Optional wrap width (uses `RtOptions` + `word_wrap_line` to rewrap lines with initial/subsequent indent handling).
  - State flags (paragraph, code block, pending markers) to mirror Markdown semantics.
- Event handling:
  - `start_tag`/`end_tag` manage paragraphs, headings, block quotes, lists/items, emphasis, links, and code blocks (including fenced vs indented distinction).
  - `text`, `code`, `soft_break`, `hard_break`, `html` append spans to the current line or flush lines as needed.
  - `push_line`, `flush_current_line`, `push_blank_line` manage `Text` output.
  - Styles (e.g., block quotes) use color/styling conventions (green for block quotes).
- Helpers convert spans to owned lines (`line_to_static`) and apply wrapping.

## Broader Context
- Used by `markdown::append_markdown` and streaming collector to render agent responses, diff summaries, and status output.
- `wrapping.rs` complements this module for width-aware layouts.
- Context can't yet be determined for advanced Markdown (tables/images); current implementation skips unsupported tags gracefully.

## Technical Debt
- The renderer is complex and stateful; adding more unit tests (especially for nested lists and mixed content) would safeguard behavior.

---
tech_debt:
  severity: high
  highest_priority_items:
    - Increase test coverage for nested Markdown constructs to prevent regressions.
related_specs:
  - ./markdown_stream.rs.spec.md
  - ./wrapping.rs.spec.md
