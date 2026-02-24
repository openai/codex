use crate::StreamTextChunk;
use crate::StreamTextParser;

/// One hidden inline tag extracted by [`InlineHiddenTagParser`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedInlineTag<T> {
    pub tag: T,
    pub content: String,
}

/// Literal tag specification used by [`InlineHiddenTagParser`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InlineTagSpec<T> {
    pub tag: T,
    pub open: &'static str,
    pub close: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ActiveTag<T> {
    tag: T,
    close: &'static str,
    content: String,
}

/// Generic streaming parser that hides configured inline tags and extracts their contents.
///
/// Example:
/// - input: `hello <oai-mem-citation>doc A</oai-mem-citation> world`
/// - visible output: `hello  world`
/// - extracted: `["doc A"]`
///
/// Matching is literal and non-nested. If EOF is reached while a tag is still open, the parser
/// auto-closes it and returns the buffered content as extracted data.
#[derive(Debug)]
pub struct InlineHiddenTagParser<T>
where
    T: Clone + Eq,
{
    specs: Vec<InlineTagSpec<T>>,
    pending: String,
    active: Option<ActiveTag<T>>,
}

impl<T> InlineHiddenTagParser<T>
where
    T: Clone + Eq,
{
    /// Create a parser for one or more hidden inline tags.
    pub fn new(specs: Vec<InlineTagSpec<T>>) -> Self {
        assert!(
            !specs.is_empty(),
            "InlineHiddenTagParser requires at least one tag spec"
        );
        for spec in &specs {
            assert!(
                !spec.open.is_empty(),
                "InlineHiddenTagParser requires non-empty open delimiters"
            );
            assert!(
                !spec.close.is_empty(),
                "InlineHiddenTagParser requires non-empty close delimiters"
            );
        }
        Self {
            specs,
            pending: String::new(),
            active: None,
        }
    }

    fn find_next_open(&self) -> Option<(usize, usize)> {
        self.specs
            .iter()
            .enumerate()
            .filter_map(|(idx, spec)| {
                self.pending
                    .find(spec.open)
                    .map(|pos| (pos, spec.open.len(), idx))
            })
            .min_by(|(pos_a, len_a, idx_a), (pos_b, len_b, idx_b)| {
                pos_a
                    .cmp(pos_b)
                    .then_with(|| len_b.cmp(len_a))
                    .then_with(|| idx_a.cmp(idx_b))
            })
            .map(|(pos, _len, idx)| (pos, idx))
    }

    fn max_open_prefix_suffix_len(&self) -> usize {
        self.specs
            .iter()
            .map(|spec| longest_suffix_prefix_len(&self.pending, spec.open))
            .max()
            .map_or(0, std::convert::identity)
    }

    fn push_visible_prefix(out: &mut StreamTextChunk<ExtractedInlineTag<T>>, pending: &str) {
        if !pending.is_empty() {
            out.visible_text.push_str(pending);
        }
    }

    fn drain_visible_to_suffix_match(
        &mut self,
        out: &mut StreamTextChunk<ExtractedInlineTag<T>>,
        keep_suffix_len: usize,
    ) {
        let take = self.pending.len().saturating_sub(keep_suffix_len);
        if take == 0 {
            return;
        }
        Self::push_visible_prefix(out, &self.pending[..take]);
        self.pending.drain(..take);
    }
}

impl<T> StreamTextParser for InlineHiddenTagParser<T>
where
    T: Clone + Eq,
{
    type Extracted = ExtractedInlineTag<T>;

    fn push_str(&mut self, chunk: &str) -> StreamTextChunk<Self::Extracted> {
        self.pending.push_str(chunk);
        let mut out = StreamTextChunk::default();

        loop {
            if let Some(close) = self.active.as_ref().map(|active| active.close) {
                if let Some(close_idx) = self.pending.find(close) {
                    let Some(mut active) = self.active.take() else {
                        continue;
                    };
                    active.content.push_str(&self.pending[..close_idx]);
                    out.extracted.push(ExtractedInlineTag {
                        tag: active.tag,
                        content: active.content,
                    });
                    let close_len = close.len();
                    self.pending.drain(..close_idx + close_len);
                    continue;
                }

                let keep = longest_suffix_prefix_len(&self.pending, close);
                let take = self.pending.len().saturating_sub(keep);
                if take > 0 {
                    if let Some(active) = self.active.as_mut() {
                        active.content.push_str(&self.pending[..take]);
                    }
                    self.pending.drain(..take);
                }
                break;
            }

            if let Some((open_idx, spec_idx)) = self.find_next_open() {
                Self::push_visible_prefix(&mut out, &self.pending[..open_idx]);
                let spec = &self.specs[spec_idx];
                let open_len = spec.open.len();
                self.pending.drain(..open_idx + open_len);
                self.active = Some(ActiveTag {
                    tag: spec.tag.clone(),
                    close: spec.close,
                    content: String::new(),
                });
                continue;
            }

            let keep = self.max_open_prefix_suffix_len();
            self.drain_visible_to_suffix_match(&mut out, keep);
            break;
        }

        out
    }

    fn finish(&mut self) -> StreamTextChunk<Self::Extracted> {
        let mut out = StreamTextChunk::default();

        if let Some(mut active) = self.active.take() {
            if !self.pending.is_empty() {
                active.content.push_str(&self.pending);
                self.pending.clear();
            }
            out.extracted.push(ExtractedInlineTag {
                tag: active.tag,
                content: active.content,
            });
            return out;
        }

        if !self.pending.is_empty() {
            out.visible_text.push_str(&self.pending);
            self.pending.clear();
        }

        out
    }
}

fn longest_suffix_prefix_len(s: &str, needle: &str) -> usize {
    let max = s.len().min(needle.len().saturating_sub(1));
    for k in (1..=max).rev() {
        if needle.is_char_boundary(k) && s.ends_with(&needle[..k]) {
            return k;
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::InlineHiddenTagParser;
    use super::InlineTagSpec;
    use crate::StreamTextChunk;
    use crate::StreamTextParser;
    use pretty_assertions::assert_eq;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Tag {
        A,
        B,
    }

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
    fn generic_inline_parser_supports_multiple_tag_types() {
        let mut parser = InlineHiddenTagParser::new(vec![
            InlineTagSpec {
                tag: Tag::A,
                open: "<a>",
                close: "</a>",
            },
            InlineTagSpec {
                tag: Tag::B,
                open: "<b>",
                close: "</b>",
            },
        ]);

        let out = collect_chunks(&mut parser, &["1<a>x</a>2<b>y</b>3"]);

        assert_eq!(out.visible_text, "123");
        assert_eq!(out.extracted.len(), 2);
        assert_eq!(out.extracted[0].tag, Tag::A);
        assert_eq!(out.extracted[0].content, "x");
        assert_eq!(out.extracted[1].tag, Tag::B);
        assert_eq!(out.extracted[1].content, "y");
    }

    #[test]
    fn generic_inline_parser_supports_non_ascii_tag_delimiters() {
        let mut parser = InlineHiddenTagParser::new(vec![InlineTagSpec {
            tag: Tag::A,
            open: "<é>",
            close: "</é>",
        }]);

        let out = collect_chunks(&mut parser, &["a<", "é>中</", "é>b"]);

        assert_eq!(out.visible_text, "ab");
        assert_eq!(out.extracted.len(), 1);
        assert_eq!(out.extracted[0].tag, Tag::A);
        assert_eq!(out.extracted[0].content, "中");
    }

    #[test]
    fn generic_inline_parser_prefers_longest_opener_at_same_offset() {
        let mut parser = InlineHiddenTagParser::new(vec![
            InlineTagSpec {
                tag: Tag::A,
                open: "<a>",
                close: "</a>",
            },
            InlineTagSpec {
                tag: Tag::B,
                open: "<ab>",
                close: "</ab>",
            },
        ]);

        let out = collect_chunks(&mut parser, &["x<ab>y</ab>z"]);

        assert_eq!(out.visible_text, "xz");
        assert_eq!(out.extracted.len(), 1);
        assert_eq!(out.extracted[0].tag, Tag::B);
        assert_eq!(out.extracted[0].content, "y");
    }

    #[test]
    #[should_panic(expected = "non-empty open delimiters")]
    fn generic_inline_parser_rejects_empty_open_delimiter() {
        let _ = InlineHiddenTagParser::new(vec![InlineTagSpec {
            tag: Tag::A,
            open: "",
            close: "</a>",
        }]);
    }

    #[test]
    #[should_panic(expected = "non-empty close delimiters")]
    fn generic_inline_parser_rejects_empty_close_delimiter() {
        let _ = InlineHiddenTagParser::new(vec![InlineTagSpec {
            tag: Tag::A,
            open: "<a>",
            close: "",
        }]);
    }
}
