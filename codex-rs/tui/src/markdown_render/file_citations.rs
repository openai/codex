//! Prepare file citations once, then feed them to the ordinary Markdown link renderer.

use super::local_links::extract_colon_location_suffix;
use super::local_links::is_local_path_like_link;
use crate::assistant_directives::AssistantDirective;
use crate::assistant_directives::QuoteEscaping;
use crate::assistant_directives::parse_assistant_directive_with_budget;
use itertools::Either;
use pulldown_cmark::Event;
use pulldown_cmark::LinkType;
use pulldown_cmark::Options;
use pulldown_cmark::Parser;
use pulldown_cmark::Tag;
use pulldown_cmark::TagEnd;
use std::borrow::Cow;
use std::ops::Range;
use std::path::Path;

/// Offset-preserving Markdown plus the original, fully parsed citation metadata.
pub(super) struct FileCitations<'a> {
    input: &'a str,
    pub(super) markdown: Cow<'a, str>,
    citations: Vec<(Range<usize>, AssistantDirective<'a>)>,
}

impl<'a> FileCitations<'a> {
    pub(super) fn new(input: &'a str, options: Options) -> Self {
        let mut prepared = Self {
            input,
            markdown: Cow::Borrowed(input),
            citations: Vec::new(),
        };
        if !input.contains("codex-file-citation") {
            return prepared;
        }

        let parser = Parser::new_ext(input, options);
        let mut literal_ranges: Vec<_> = parser
            .reference_definitions()
            .iter()
            .map(|(_, definition)| definition.span.clone())
            .collect();
        literal_ranges.extend(parser.into_offset_iter().filter_map(|(event, range)| {
            matches!(
                event,
                Event::Code(_)
                    | Event::Html(_)
                    | Event::InlineHtml(_)
                    | Event::Start(Tag::CodeBlock(_) | Tag::Link { .. } | Tag::Image { .. })
            )
            .then_some(range)
        }));
        // Reference definitions arrive separately; visit all literal ranges in source order.
        literal_ranges.sort_unstable_by_key(|range| range.start);
        let mut literal_ranges = literal_ranges.into_iter().peekable();

        let mut directive_end = 0;
        // Share the allowance across offsets and quote modes: malformed retries must stay linear.
        let mut scan_budget = input.len().saturating_mul(/*rhs*/ 4);
        for (start, _) in input.match_indices(':') {
            while literal_ranges.next_if(|range| range.end <= start).is_some() {}
            if start < directive_end
                || input[..start].ends_with(':')
                || literal_ranges
                    .peek()
                    .is_some_and(|range| range.contains(&start))
            {
                continue;
            }
            let source = &input[start..];
            // Citations prefer literal quoting; other directives prefer escaped quotes.
            let escaping = if source
                .trim_start_matches(':')
                .starts_with("codex-file-citation{")
            {
                [QuoteEscaping::Literal, QuoteEscaping::Backslash]
            } else {
                [QuoteEscaping::Backslash, QuoteEscaping::Literal]
            };
            let Some(directive) = escaping.into_iter().find_map(|escaping| {
                parse_assistant_directive_with_budget(source, escaping, &mut scan_budget)
            }) else {
                continue;
            };
            let end = start + directive.raw.len();
            directive_end = end;
            if input[..start]
                .bytes()
                .rev()
                .take_while(|byte| *byte == b'\\')
                .count()
                % 2
                != 0
                || directive.name != "codex-file-citation"
                || directive
                    .attributes
                    .get("path")
                    .is_none_or(|path| path.is_empty())
            {
                continue;
            }
            // Mask the interior without moving offsets or changing Markdown delimiter flanking.
            let markdown = prepared.markdown.to_mut();
            markdown.replace_range(start + 1..end - 1, &"x".repeat(end - start - 2));
            prepared.citations.push((start..end, directive));
        }
        prepared
    }

    /// Adapt before `DecodedTextMerge`, while plain text still has exact source offsets.
    pub(super) fn events<'s>(
        &'s self,
        events: impl Iterator<Item = (Event<'s>, Range<usize>)>,
        cwd: Option<&'s Path>,
    ) -> impl Iterator<Item = (Event<'s>, Range<usize>)> {
        let mut citations = self.citations.iter().peekable();
        events.flat_map(move |(event, range)| {
            while citations
                .next_if(|(span, _)| span.end <= range.start)
                .is_some()
            {}
            let Event::Text(text) = event else {
                return Either::Left(std::iter::once((event, range)));
            };
            if citations
                .peek()
                .is_none_or(|(span, _)| span.start >= range.end)
            {
                return Either::Left(std::iter::once((Event::Text(text), range)));
            }
            // Never apply source offsets to entity-decoded text or a partial citation.
            if text.as_ref() != &self.markdown[range.clone()]
                || citations
                    .peek()
                    .is_some_and(|(span, _)| span.start < range.start || span.end > range.end)
            {
                let text = self.input.get(range.clone()).map_or(text, Into::into);
                return Either::Left(std::iter::once((Event::Text(text), range)));
            }

            let mut events = Vec::new();
            let mut offset = range.start;
            while let Some((span, directive)) = citations.next_if(|(span, _)| span.end <= range.end)
            {
                if offset < span.start {
                    events.push((
                        Event::Text(self.markdown[offset..span.start].into()),
                        offset..span.start,
                    ));
                }
                let path = directive.attributes["path"].as_ref();
                let destination = if is_local_path_like_link(path) {
                    path.to_string()
                } else {
                    cwd.map_or_else(
                        || format!("./{path}"),
                        |cwd| cwd.join(path).to_string_lossy().into_owned(),
                    )
                };
                // Citation paths are literal; the existing link renderer decodes destinations.
                let mut destination = destination
                    .replace('%', "%25")
                    .replace('#', "%23")
                    .replace('?', "%3F");
                if let Some(suffix) = extract_colon_location_suffix(&destination) {
                    let suffix_start = destination.len() - suffix.len();
                    destination.replace_range(suffix_start.., &suffix.replace(':', "%3A"));
                }
                // Citations have no descriptive label; compare the same encoded path on both sides.
                events.extend([
                    (
                        Event::Start(Tag::Link {
                            link_type: LinkType::Inline,
                            dest_url: destination.clone().into(),
                            title: "".into(),
                            id: "".into(),
                        }),
                        span.clone(),
                    ),
                    (Event::Text(destination.into()), span.clone()),
                    (Event::End(TagEnd::Link), span.clone()),
                ]);
                offset = span.end;
            }
            if offset < range.end {
                events.push((
                    Event::Text(self.markdown[offset..range.end].into()),
                    offset..range.end,
                ));
            }
            Either::Right(events.into_iter())
        })
    }
}

#[cfg(test)]
#[path = "file_citations_tests.rs"]
mod tests;
