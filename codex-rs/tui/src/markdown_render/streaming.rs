//! Streaming markdown render metadata collected during the writer's single parse pass.
//!
//! Top-level block offsets always refer to the exact source passed to this renderer; callers that
//! normalize source before rendering must not apply those offsets to the original source.

use super::DecodedTextMerge;
use super::Event;
use super::FileCitations;
use super::HyperlinkLine;
use super::ListSpacing;
use super::Options;
use super::Parser;
use super::Tag;
use super::Writer;
use super::math::MathMarkdown;
use std::ops::Range;
use std::path::Path;

/// Rendered lines and the block metadata needed to keep only the final block mutable.
pub(crate) struct StreamingMarkdownRender {
    /// Styled output produced by the same parser pass that collected the metadata below.
    pub(crate) lines: Vec<HyperlinkLine>,
    /// Source line containing an unfinished display equation, which must not enter scrollback.
    pub(crate) pending_math_start: Option<usize>,
    /// Byte offset of the final top-level block when at least one earlier block exists.
    pub(crate) last_top_level_block_start: Option<usize>,
    /// Whether a reference definition can retroactively change another block's rendering.
    pub(crate) has_reference_link_definition: bool,
    /// Whether the first block is raw HTML, which joins a retained prefix without a separator.
    pub(crate) first_top_level_block_is_html: bool,
    /// Transformable fences in the final block stay mutable, including within a list or quote.
    pub(crate) mutable_fence_start: Option<usize>,
}

/// Render `input` while tracking the final mutable top-level block.
///
/// Every reported byte offset indexes the exact `input` passed here. Callers that transform source
/// before rendering must map the offset back to their original source before retaining a prefix.
pub(crate) fn render_streaming_markdown_lines_with_width_and_cwd(
    input: &str,
    width: Option<usize>,
    cwd: Option<&Path>,
    is_hidden_link_destination: &dyn Fn(&str) -> bool,
    list_spacing: ListSpacing,
) -> StreamingMarkdownRender {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TABLES);
    let citations = FileCitations::new(input, options);
    let math = MathMarkdown::new(&citations.markdown, options, width);
    let parser = Parser::new_ext(&math.markdown, options);
    let has_reference_link_definition = parser.reference_definitions().iter().next().is_some();
    let parser = TopLevelBlockTracker {
        iter: DecodedTextMerge::new(super::source_tables::preserve(
            input,
            citations.events(math.events(parser.into_offset_iter()), cwd),
        )),
        depth: 0,
        block_count: 0,
        last_start: 0,
        first_is_html: false,
        mutable_fence_start: None,
    };
    let mut writer = Writer::new(input, parser, width, cwd, is_hidden_link_destination);
    writer.list_spacing = list_spacing;
    writer.run();
    StreamingMarkdownRender {
        lines: writer.text,
        pending_math_start: math.pending_start,
        last_top_level_block_start: (writer.iter.block_count > 1)
            .then_some(writer.iter.last_start)
            .filter(|start| math.pending_start.is_none_or(|pending| *start <= pending))
            .filter(|start| {
                !math
                    .display_ranges
                    .iter()
                    .any(|range| range.start < *start && *start < range.end)
            }),
        has_reference_link_definition,
        first_top_level_block_is_html: writer.iter.first_is_html,
        mutable_fence_start: writer.iter.mutable_fence_start,
    }
}

/// Records top-level block boundaries without adding a second parser traversal.
struct TopLevelBlockTracker<I> {
    iter: I,
    depth: usize,
    block_count: usize,
    last_start: usize,
    first_is_html: bool,
    mutable_fence_start: Option<usize>,
}

impl<'a, I> Iterator for TopLevelBlockTracker<I>
where
    I: Iterator<Item = (Event<'a>, Range<usize>)>,
{
    type Item = (Event<'a>, Range<usize>);

    fn next(&mut self) -> Option<Self::Item> {
        let (event, range) = self.iter.next()?;
        if self.depth == 0 && matches!(&event, Event::Start(_) | Event::Rule | Event::Html(_)) {
            self.mutable_fence_start = None;
            self.block_count += 1;
            self.last_start = range.start;
            if self.block_count == 1 {
                self.first_is_html =
                    matches!(&event, Event::Start(Tag::HtmlBlock) | Event::Html(_));
            }
        }
        if let Event::Start(Tag::CodeBlock(pulldown_cmark::CodeBlockKind::Fenced(info))) = &event
            && (super::preferences::current().mermaid
                && info.split([',', ' ', '\t']).next() == Some("mermaid")
                || !super::preferences::current().tables
                    && crate::table_detect::is_markdown_fence_info(info, /*marker_len*/ 0))
        {
            self.mutable_fence_start.get_or_insert(self.last_start);
        }
        match event {
            Event::Start(_) => self.depth += 1,
            Event::End(_) => self.depth = self.depth.saturating_sub(1),
            _ => {}
        }
        Some((event, range))
    }
}
