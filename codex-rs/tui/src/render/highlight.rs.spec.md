## Overview
`codex-tui::render::highlight` applies syntax highlighting to embedded Bash snippets. It leverages tree-sitter’s Bash grammar to convert script text into styled Ratatui `Line`s for display in the transcript (e.g., command previews, plan updates).

## Detailed Behavior
- `BashHighlight` enum maps tree-sitter capture names (comment, constant, embedded, function, keyword, number, operator, property, string) to styles. Comments/operators/strings are dimmed; others use default style.
- Highlight configuration:
  - `highlight_config` builds a static `HighlightConfiguration` using tree-sitter-bash queries (initialized once via `OnceLock`).
  - `highlight_names` caches capture names for the configuration.
- `highlight_bash_to_lines(script)`:
  - Runs `tree_sitter_highlight::Highlighter` on the input.
  - Streams `HighlightEvent`s to accumulate styled spans, splitting on newlines via `push_segment`.
  - Falls back to plain text if highlighting fails (errors or unsupported input).
- Tests verify DIM styling for operators and strings while preserving content.

## Broader Context
- Utilized by `diff_render` and `chatwidget` when rendering commands and plan snippets.
- Keeps styling consistent with other Shell/Bash renderers in Codex, helping users scan command sequences.
- Context can't yet be determined for additional languages; the module is Bash-specific, but the structure permits extending to other grammars.

## Technical Debt
- Highlight styles are minimal; consider adding more nuanced styling (colors) once Ratatui themes support it.

---
tech_debt:
  severity: low
  highest_priority_items:
    - Evaluate multi-language support if future features require highlighting other shell dialects or structured outputs.
related_specs:
  - ./render/mod.rs.spec.md (to add)
  - ./chatwidget.rs.spec.md
