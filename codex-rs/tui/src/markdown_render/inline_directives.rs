//! Render file citations as links and follow-up directives as labels.
//!
//! Preserve source offsets and literal Markdown boundaries for streaming and completed messages.

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

/// Preserve source Markdown when copying, replacing only recognized follow-up directives.
pub(crate) fn followup_labels(input: &str) -> Cow<'_, str> {
    if !input.contains(":codex-followup[") {
        return Cow::Borrowed(input);
    }
    let prepared = InlineDirectives::new(
        input,
        Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TABLES | Options::ENABLE_TASKLISTS,
    );
    let mut copied = String::new();
    let mut offset = 0;
    for (range, directive) in prepared.directives {
        if directive.name == "codex-followup"
            && let Some(label) = directive.label
        {
            copied.push_str(&input[offset..range.start]);
            copied.push_str(label);
            offset = range.end;
        }
    }
    if offset == 0 {
        return Cow::Borrowed(input);
    }
    copied.push_str(&input[offset..]);
    Cow::Owned(copied)
}

/// Offset-preserving Markdown plus the original, fully parsed directive metadata.
pub(super) struct InlineDirectives<'a> {
    input: &'a str,
    pub(super) markdown: Cow<'a, str>,
    directives: Vec<(Range<usize>, AssistantDirective<'a>)>,
}

impl<'a> InlineDirectives<'a> {
    pub(super) fn new(input: &'a str, options: Options) -> Self {
        let mut prepared = Self {
            input,
            markdown: Cow::Borrowed(input),
            directives: Vec::new(),
        };
        if !input.contains("codex-file-citation") && !input.contains(":codex-followup[") {
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
        // Share separate preferred/fallback allowances across offsets, so fallback retries
        // cannot starve later preferred parses. Both allowances stay proportional to input size.
        let mut scan_budget = [input.len().saturating_mul(/*rhs*/ 4); 2];
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
            let Some(directive) =
                escaping
                    .into_iter()
                    .zip(&mut scan_budget)
                    .find_map(|(escaping, remaining)| {
                        parse_assistant_directive_with_budget(source, escaping, remaining)
                    })
            else {
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
                || match directive.name {
                    "codex-file-citation" => directive
                        .attributes
                        .get("path")
                        .is_none_or(|path| path.is_empty()),
                    "codex-followup" => {
                        !directive.raw.starts_with(":codex-followup[")
                            || directive.label.is_none_or(|label| label.trim().is_empty())
                    }
                    _ => true,
                }
            {
                continue;
            }
            // Mask the interior without moving offsets or changing Markdown delimiter flanking.
            let markdown = prepared.markdown.to_mut();
            markdown.replace_range(start + 1..end - 1, &"x".repeat(end - start - 2));
            prepared.directives.push((start..end, directive));
        }
        prepared
    }

    /// Adapt before `DecodedTextMerge`, while plain text still has exact source offsets.
    pub(super) fn events<'s>(
        &'s self,
        events: impl Iterator<Item = (Event<'s>, Range<usize>)>,
        cwd: Option<&'s Path>,
    ) -> impl Iterator<Item = (Event<'s>, Range<usize>)> {
        let mut directives = self.directives.iter().peekable();
        events.flat_map(move |(event, range)| {
            while directives
                .next_if(|(span, _)| span.end <= range.start)
                .is_some()
            {}
            let Event::Text(text) = event else {
                return Either::Left(std::iter::once((event, range)));
            };
            if directives
                .peek()
                .is_none_or(|(span, _)| span.start >= range.end)
            {
                return Either::Left(std::iter::once((Event::Text(text), range)));
            }
            // Never apply source offsets to entity-decoded text or a partial directive.
            if text.as_ref() != &self.markdown[range.clone()]
                || directives
                    .peek()
                    .is_some_and(|(span, _)| span.start < range.start || span.end > range.end)
            {
                let text = self.input.get(range.clone()).map_or(text, Into::into);
                return Either::Left(std::iter::once((Event::Text(text), range)));
            }

            let mut events = Vec::new();
            let mut offset = range.start;
            while let Some((span, directive)) =
                directives.next_if(|(span, _)| span.end <= range.end)
            {
                if offset < span.start {
                    events.push((
                        Event::Text(self.markdown[offset..span.start].into()),
                        offset..span.start,
                    ));
                }
                offset = span.end;
                if let Some(label) = directive
                    .label
                    .filter(|_| directive.name == "codex-followup")
                {
                    // Keep the unmatched `[` as a sentinel so labels cannot introduce blocks.
                    let inline = &directive.raw
                        [":codex-followup".len()..":codex-followup[".len() + label.len()];
                    events.extend(
                        Parser::new_ext(inline, Options::ENABLE_STRIKETHROUGH)
                            .into_offset_iter()
                            .filter_map(|(event, range)| {
                                let event = match event {
                                    Event::Start(Tag::Paragraph)
                                    | Event::End(TagEnd::Paragraph) => return None,
                                    Event::Text(text) if range.start == 0 => {
                                        Event::Text(text[1..].to_owned().into())
                                    }
                                    event => event,
                                };
                                Some((event, span.clone()))
                            }),
                    );
                    continue;
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

#[cfg(test)]
#[path = "followups_tests.rs"]
mod followups_tests;
