# codex-utils-stream-parser

Small, dependency-free utilities for parsing streamed text incrementally.

## What it provides

- `StreamTextParser`: trait for incremental parsers that consume string chunks
- `InlineHiddenTagParser<T>`: generic parser that hides inline tags and extracts their contents
- `CitationStreamParser`: convenience wrapper for `<citation>...</citation>`
- `strip_citations(...)`: one-shot helper for non-streamed strings
- `Utf8StreamParser<P>`: adapter for raw `&[u8]` streams that may split UTF-8 code points

## Why this exists

Some model outputs arrive as a stream and may contain hidden markup (for example
`<citation>...</citation>`) split across chunk boundaries. Parsing each chunk
independently is incorrect because tags can be split (`<cita` + `tion>`).

This crate keeps parser state across chunks, returns visible text safe to render
immediately, and extracts hidden payloads separately.

## Example: citation streaming

```rust
use codex_utils_stream_parser::CitationStreamParser;
use codex_utils_stream_parser::StreamTextParser;

let mut parser = CitationStreamParser::new();

let first = parser.push_str("Hello <cita");
assert_eq!(first.visible_text, "Hello ");
assert!(first.extracted.is_empty());

let second = parser.push_str("tion>doc A</citation> world");
assert_eq!(second.visible_text, " world");
assert_eq!(second.extracted, vec!["doc A".to_string()]);

let tail = parser.finish();
assert!(tail.visible_text.is_empty());
assert!(tail.extracted.is_empty());
```

## Example: raw byte streaming with split UTF-8 code points

```rust
use codex_utils_stream_parser::CitationStreamParser;
use codex_utils_stream_parser::Utf8StreamParser;

let mut parser = Utf8StreamParser::new(CitationStreamParser::new());

// "é" split across chunks: 0xC3 + 0xA9
let first = parser.push_bytes(&[b'H', 0xC3]).unwrap();
assert_eq!(first.visible_text, "H");

let second = parser.push_bytes(&[0xA9, b'!']).unwrap();
assert_eq!(second.visible_text, "é!");

let tail = parser.finish().unwrap();
assert!(tail.visible_text.is_empty());
```

## Example: custom hidden tags

```rust
use codex_utils_stream_parser::InlineHiddenTagParser;
use codex_utils_stream_parser::InlineTagSpec;
use codex_utils_stream_parser::StreamTextParser;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Tag {
    Secret,
}

let mut parser = InlineHiddenTagParser::new(vec![InlineTagSpec {
    tag: Tag::Secret,
    open: "<secret>",
    close: "</secret>",
}]);

let out = parser.push_str("a<secret>x</secret>b");
assert_eq!(out.visible_text, "ab");
assert_eq!(out.extracted.len(), 1);
assert_eq!(out.extracted[0].content, "x");
```

## Notes / limitations

- Tags are matched literally and case-sensitively
- No tag attributes
- No nested tag support
- Unterminated open tags are auto-closed on `finish()` (buffered content is returned as extracted)
- `StreamTextParser::push_str(...)` accepts only valid UTF-8 (`&str`); use `Utf8StreamParser` if your
  upstream source yields raw bytes
