//! Restore disabled tables as literal blocks before the writer consumes inline Markdown events.
//!
//! Keep parser offsets and block boundaries intact, including Markdown fences around tables.

use itertools::Either;
use pulldown_cmark::CodeBlockKind;
use pulldown_cmark::Event;
use pulldown_cmark::Options;
use pulldown_cmark::Parser;
use pulldown_cmark::Tag;
use pulldown_cmark::TagEnd;
use std::ops::Range;

pub(super) fn preserve<'a>(
    input: &'a str,
    events: impl Iterator<Item = (Event<'a>, Range<usize>)>,
) -> impl Iterator<Item = (Event<'a>, Range<usize>)> {
    let enabled = super::preferences::current().tables;
    let mut skipped_end = None;
    events.flat_map(move |(event, range)| {
        if let Some(end) = skipped_end {
            if matches!(&event, Event::End(tag) if *tag == end) {
                skipped_end = None;
            }
            return Either::Right(None.into_iter());
        }
        let end = match &event {
            Event::Start(Tag::Table(_)) if !enabled => Some(TagEnd::Table),
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(info)))
                if !enabled
                    && crate::table_detect::is_markdown_fence_info(info, /*marker_len*/ 0) =>
            {
                Some(TagEnd::CodeBlock)
            }
            _ => None,
        };
        if let Some(end) = end {
            // The first line's container prefix is outside the parser range. Strip matching
            // continuation prefixes so the writer's list/quote indentation is applied once.
            let prefix = input[..range.start].rsplit('\n').next().unwrap_or_default();
            let indent = " ".repeat(prefix.len());
            let source = input[range.clone()]
                .split_inclusive('\n')
                .enumerate()
                .map(|(index, line)| {
                    if index == 0 {
                        line
                    } else if let Some(line) = line.strip_prefix(prefix) {
                        line
                    } else if let Some((_, suffix)) = prefix.rsplit_once('>') {
                        // Only enclosing quotes are structural; further markers and newlines
                        // belong to the literal fence body.
                        let mut rest = line;
                        for _ in prefix.matches('>') {
                            let Some(content) =
                                rest.trim_start_matches([' ', '\t']).strip_prefix('>')
                            else {
                                break;
                            };
                            rest = content.strip_prefix(' ').unwrap_or(content);
                        }
                        // Remove only the enclosing list/fence indentation after the final quote.
                        for _ in 0..suffix.strip_prefix(' ').unwrap_or(suffix).len() {
                            rest = rest.strip_prefix(' ').unwrap_or(rest);
                        }
                        rest
                    } else {
                        line.strip_prefix(&indent).unwrap_or(line)
                    }
                })
                .collect::<String>();
            if end == TagEnd::CodeBlock
                && !source.split_once('\n').is_some_and(|(_, body)| {
                    Parser::new_ext(body, Options::ENABLE_TABLES)
                        .any(|event| matches!(event, Event::Start(Tag::Table(_))))
                })
            {
                return Either::Right(Some((event, range)).into_iter());
            }
            skipped_end = Some(end);
            let kind = match event {
                Event::Start(Tag::CodeBlock(kind)) => kind,
                _ => CodeBlockKind::Fenced("".into()),
            };
            Either::Left(
                [
                    (Event::Start(Tag::CodeBlock(kind)), range.clone()),
                    (Event::Text(source.into()), range.clone()),
                    (Event::End(TagEnd::CodeBlock), range),
                ]
                .into_iter(),
            )
        } else {
            Either::Right(Some((event, range)).into_iter())
        }
    })
}
