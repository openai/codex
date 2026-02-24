use crate::InlineHiddenTagParser;
use crate::InlineTagSpec;
use crate::StreamTextChunk;
use crate::StreamTextParser;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CitationTag {
    Citation,
}

const CITATION_OPEN: &str = "<oai-mem-citation>";
const CITATION_CLOSE: &str = "</oai-mem-citation>";

/// Stream parser for `<oai-mem-citation>...</oai-mem-citation>` tags.
///
/// This is a thin convenience wrapper around [`InlineHiddenTagParser`]. It returns citation bodies
/// as plain strings and omits the citation tags from visible text.
///
/// Matching is literal and non-nested. If EOF is reached before a closing
/// `</oai-mem-citation>`, the parser auto-closes the tag and returns the buffered body as an
/// extracted citation.
#[derive(Debug)]
pub struct CitationStreamParser {
    inner: InlineHiddenTagParser<CitationTag>,
}

impl CitationStreamParser {
    pub fn new() -> Self {
        Self {
            inner: InlineHiddenTagParser::new(vec![InlineTagSpec {
                tag: CitationTag::Citation,
                open: CITATION_OPEN,
                close: CITATION_CLOSE,
            }]),
        }
    }
}

impl Default for CitationStreamParser {
    fn default() -> Self {
        Self::new()
    }
}

impl StreamTextParser for CitationStreamParser {
    type Extracted = String;

    fn push_str(&mut self, chunk: &str) -> StreamTextChunk<Self::Extracted> {
        let inner = self.inner.push_str(chunk);
        StreamTextChunk {
            visible_text: inner.visible_text,
            extracted: inner.extracted.into_iter().map(|tag| tag.content).collect(),
        }
    }

    fn finish(&mut self) -> StreamTextChunk<Self::Extracted> {
        let inner = self.inner.finish();
        StreamTextChunk {
            visible_text: inner.visible_text,
            extracted: inner.extracted.into_iter().map(|tag| tag.content).collect(),
        }
    }
}

/// Strip citation tags from a complete string and return `(visible_text, citations)`.
///
/// This uses [`CitationStreamParser`] internally, so it inherits the same semantics:
/// literal, non-nested matching and auto-closing unterminated citations at EOF.
pub fn strip_citations(text: &str) -> (String, Vec<String>) {
    let mut parser = CitationStreamParser::new();
    let mut out = parser.push_str(text);
    let tail = parser.finish();
    out.visible_text.push_str(&tail.visible_text);
    out.extracted.extend(tail.extracted);
    (out.visible_text, out.extracted)
}

#[cfg(test)]
mod tests {
    use super::CitationStreamParser;
    use super::strip_citations;
    use crate::StreamTextChunk;
    use crate::StreamTextParser;
    use pretty_assertions::assert_eq;

    fn collect_chunks<P>(parser: &mut P, chunks: &[&str]) -> StreamTextChunk<P::Extracted>
    where
        P: StreamTextParser,
    {
        let mut all = StreamTextChunk::default();
        for chunk in chunks {
            let next = parser.push_str(chunk);
            all.visible_text.push_str(&next.visible_text);
            all.extracted.extend(next.extracted);
        }
        let tail = parser.finish();
        all.visible_text.push_str(&tail.visible_text);
        all.extracted.extend(tail.extracted);
        all
    }

    #[test]
    fn citation_parser_streams_across_chunk_boundaries() {
        let mut parser = CitationStreamParser::new();
        let out = collect_chunks(
            &mut parser,
            &[
                "Hello <oai-mem-",
                "citation>source A</oai-mem-",
                "citation> world",
            ],
        );

        assert_eq!(out.visible_text, "Hello  world");
        assert_eq!(out.extracted, vec!["source A".to_string()]);
    }

    #[test]
    fn citation_parser_buffers_partial_open_tag_prefix() {
        let mut parser = CitationStreamParser::new();

        let first = parser.push_str("abc <oai-mem-");
        assert_eq!(first.visible_text, "abc ");
        assert_eq!(first.extracted, Vec::<String>::new());

        let second = parser.push_str("citation>x</oai-mem-citation>z");
        let tail = parser.finish();

        assert_eq!(second.visible_text, "z");
        assert_eq!(second.extracted, vec!["x".to_string()]);
        assert!(tail.is_empty());
    }

    #[test]
    fn citation_parser_auto_closes_unterminated_tag_on_finish() {
        let mut parser = CitationStreamParser::new();
        let out = collect_chunks(&mut parser, &["x<oai-mem-citation>source"]);

        assert_eq!(out.visible_text, "x");
        assert_eq!(out.extracted, vec!["source".to_string()]);
    }

    #[test]
    fn citation_parser_preserves_partial_open_tag_at_eof_if_not_a_full_tag() {
        let mut parser = CitationStreamParser::new();
        let out = collect_chunks(&mut parser, &["hello <oai-mem-"]);

        assert_eq!(out.visible_text, "hello <oai-mem-");
        assert_eq!(out.extracted, Vec::<String>::new());
    }

    #[test]
    fn strip_citations_collects_all_citations() {
        let (visible, citations) = strip_citations(
            "a<oai-mem-citation>one</oai-mem-citation>b<oai-mem-citation>two</oai-mem-citation>c",
        );

        assert_eq!(visible, "abc");
        assert_eq!(citations, vec!["one".to_string(), "two".to_string()]);
    }

    #[test]
    fn strip_citations_auto_closes_unterminated_citation_at_eof() {
        let (visible, citations) = strip_citations("x<oai-mem-citation>y");

        assert_eq!(visible, "x");
        assert_eq!(citations, vec!["y".to_string()]);
    }

    #[test]
    fn citation_parser_does_not_support_nested_tags() {
        let (visible, citations) = strip_citations(
            "a<oai-mem-citation>x<oai-mem-citation>y</oai-mem-citation>z</oai-mem-citation>b",
        );

        assert_eq!(visible, "az</oai-mem-citation>b");
        assert_eq!(citations, vec!["x<oai-mem-citation>y".to_string()]);
    }
}
