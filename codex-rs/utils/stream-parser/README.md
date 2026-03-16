# codex-utils-stream-parser

Small, dependency-free utilities for parsing streamed text incrementally.

**Disclaimer**: This code is pretty complex and Codex did not manage to write it so before updating the code, make
sure to deeply understand it and don't blindly trust Codex on it. Feel free to update the documentation as you
modify the code

## What it provides

- `StreamTextParser`: trait for incremental parsers that consume string chunks
- `InlineHiddenTagParser<T>`: generic parser that hides inline tags and extracts their contents
- `CitationStreamParser`: convenience wrapper for `<oai-mem-citation>...</oai-mem-citation>`
- `strip_citations(...)`: one-shot helper for non-streamed strings
- `Utf8StreamParser<P>`: adapter for raw `&[u8]` streams that may split UTF-8 code points

## Why this exists

Some model outputs arrive as a stream and may contain hidden markup (for example
`<oai-mem-citation>...</oai-mem-citation>`) split across chunk boundaries. Parsing each chunk
independently is incorrect because tags can be split (`<oai-mem-` + `citation>`).

This crate keeps parser state across chunks, returns visible text safe to render
immediately, and extracts hidden payloads separately.

## Example: citation streaming

```rust
use codex_utils_stream_parser::CitationStreamParser;
use codex_utils_stream_parser::StreamTextParser;

let mut parser = CitationStreamParser::new();

let first = parser.push_str("Hello <oai-mem-");
assert_eq!(first.visible_text, "Hello ");
assert!(first.extracted.is_empty());

let second = parser.push_str("citation>doc A</oai-mem-citation> world");
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

# fn demo() -> Result<(), codex_utils_stream_parser::Utf8StreamParserError> {
let mut parser = Utf8StreamParser::new(CitationStreamParser::new());

// "é" split across chunks: 0xC3 + 0xA9
let first = parser.push_bytes(&[b'H', 0xC3])?;
assert_eq!(first.visible_text, "H");

let second = parser.push_bytes(&[0xA9, b'!'])?;
assert_eq!(second.visible_text, "é!");

let tail = parser.finish()?;
assert!(tail.visible_text.is_empty());
# Ok(())
# }
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

## Known limitations

- Tags are matched literally and case-sensitively
- No nested tag support
- A stream can return empty objects.