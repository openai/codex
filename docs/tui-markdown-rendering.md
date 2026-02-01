# TUI markdown rendering specification (termimad)

This document defines the markdown subset and rendering rules used by the TUI
when an agent response is displayed. It is normative for output intended to be
parsed by the termimad-based renderer in `codex-rs/tui/src/markdown_render.rs`.
The termimad renderer is gated behind the experimental `markdown_rendering`
feature and is only used when a user enables it; the default renderer remains
the existing pulldown-cmark implementation.

## Scope and goals

- Use termimad (and its minimad parser) for markdown parsing and layout.
- Match the termimad markdown subset; unsupported constructs render as literal
  text.
- Preserve TUI streaming behavior (newline-gated updates) and deterministic
  wrapping for a given width.

## Parser and renderer

- The renderer must use termimad for parsing and layout. termimad is a small,
  terminal-focused markdown renderer (not a full markdown engine), so the TUI
  must not enable other markdown extensions or attempt to match CommonMark.
- The TUI must render with a termimad `MadSkin` configured to the TUI style
  guide (see `codex-rs/tui/styles.md`) and then convert the styled output into
  `ratatui::text::Text`.

## Supported markdown (termimad subset)

termimad supports the following markdown constructs:

- Headings (lines starting with `#`).
- Tables (pipe-based tables with optional alignment markers).
- Italic and bold.
- Inline code (backquoted).
- Fenced or tab-indented code blocks.
- Strikethrough (`~~`).
- Horizontal rules (`---`).
- Unordered lists.
- Blockquotes (`>`).

## Unsupported or verbatim features

The following constructs are not supported by termimad and must render as
literal text (no special styling or link expansion):

- Ordered lists.
- Links and autolinks.
- Images.
- HTML blocks or inline HTML.
- Footnotes, definition lists, task lists, or metadata blocks.

## Rendering rules

- Headings are styled via the skin's header styles. The rendered output should
  follow termimad's semantics (heading markers are not preserved unless the
  renderer explicitly injects them).
- Blockquotes are styled via the skin's quote style and use termimad's quote
  prefixing behavior.
- Lists are only unordered; list markers are produced by termimad and must be
  preserved as-is.
- Code blocks preserve leading whitespace and are not re-wrapped after termimad
  formatting.
- Inline code and strikethrough are styled via the skin and do not alter
  surrounding spacing.
- Tables are laid out by termimad so column widths are aligned and wrapped for
  the available width.
- When the termimad renderer is enabled, fenced code blocks explicitly labeled
  `markdown` or `md` are unwrapped and rendered as markdown content.

## Wrapping and layout

- When a wrap width is provided by the caller, the renderer must ask termimad to
  format for that width (via `MadSkin::area_text` or equivalent). The width must
  be derived from the caller's available columns, not the terminal width.
- When no width is provided, the renderer must use the "no fixed width" API
  (`MadSkin::text` or equivalent) to avoid depending on the terminal size.
- The final renderer output must be converted into `ratatui::text::Text` without
  stripping ANSI styling so `ratatui` can render the same styles termimad chose.

## Streaming behavior

- Streaming behavior remains newline-gated: lines are only committed when a
  newline is received, and the final line is emitted only on finalize.
- The renderer must be deterministic: the same source and width must produce
  the same list of output lines.

## Required code changes

This section is normative for the termimad migration.

1. Dependencies
   - Add `termimad` as a dependency for `codex-rs/tui`.
   - Keep `pulldown-cmark` only if required by other crates; the TUI markdown
     renderer must stop using it.

2. Renderer replacement
   - Replace the custom pulldown-cmark renderer in
     `codex-rs/tui/src/markdown_render.rs` with a termimad-based adapter:
     - Build a `MadSkin` configured to match the TUI style guide (bold headers,
       cyan for code and links if desired, green for quotes, and default for
       ordinary text).
     - If a wrap width is provided, format with `MadSkin::area_text` for that
       width; otherwise format with `MadSkin::text`.
     - Render the formatted output to an ANSI string (via `Display`/`to_string`)
       and convert it to `ratatui::text::Text` using the existing
       `codex_ansi_escape::ansi_escape` helpers.
     - Preserve any termimad-inserted prefixes (quotes, list bullets, table
       separators) and do not add additional wrapping after conversion.

3. Streaming adapter
   - Keep `MarkdownStreamCollector` as-is, but ensure it calls the new renderer.
   - Validate that a partial final line is only emitted on finalize.

4. Tests and snapshots
   - Update `codex-rs/tui/src/markdown_render_tests.rs` expectations to reflect
     the termimad subset (no ordered lists or link expansion).
   - Update `codex-rs/tui/src/markdown_stream.rs` expectations where list
     markers, quote prefixes, or wrapping behavior change.
   - Update any vt100 snapshots in `codex-rs/tui/src/chatwidget/tests.rs` that
     rely on the old renderer.

5. Documentation
   - Keep this document as the normative spec for the termimad renderer.
   - If heading marker preservation is required, document and implement the
     chosen strategy (either accept termimad's header rendering or add a
     preprocessing pass to re-inject markers).
