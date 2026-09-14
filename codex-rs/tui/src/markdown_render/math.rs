//! Recognize math before Markdown consumes TeX escapes, retaining exact source offsets.
//!
//! Only the rendering copy is masked. Unsupported expressions are restored verbatim; code,
//! links, HTML, and display equations stay under the ordinary Markdown renderer.

use itertools::Either;
use pulldown_cmark::Event;
use pulldown_cmark::Options;
use pulldown_cmark::Parser;
use pulldown_cmark::Tag;
use std::borrow::Cow;
use std::ops::Range;

mod render;

const MAX_MATH_BYTES: usize = 4096;

pub(super) struct MathMarkdown<'a> {
    pub(super) markdown: Cow<'a, str>,
    pub(super) display_ranges: Vec<Range<usize>>,
    replacements: Vec<(Range<usize>, String)>,
}

impl<'a> MathMarkdown<'a> {
    pub(super) fn new(input: &'a str, options: Options) -> Self {
        let mut result = Self {
            markdown: Cow::Borrowed(input),
            display_ranges: Vec::new(),
            replacements: Vec::new(),
        };
        if !input.contains('$') && !input.contains("\\(") && !input.contains("\\[") {
            return result;
        }
        let parser = Parser::new_ext(input, options);
        let mut protected: Vec<_> = parser
            .reference_definitions()
            .iter()
            .map(|(_, def)| def.span.clone())
            .collect();
        protected.extend(parser.into_offset_iter().filter_map(|(event, range)| {
            matches!(
                event,
                Event::Code(_)
                    | Event::Html(_)
                    | Event::InlineHtml(_)
                    | Event::Start(Tag::CodeBlock(_) | Tag::Link { .. } | Tag::Image { .. })
            )
            .then_some(range)
        }));
        protected.sort_unstable_by_key(|range| range.start);
        let mut protected = protected.into_iter().peekable();
        let mut offset = 0;
        let mut scanned = 0;
        let mut line_start = 0;
        while offset < input.len() {
            if let Some(index) = input[scanned..offset].rfind('\n') {
                line_start = scanned + index + 1;
            }
            scanned = offset;
            while protected.next_if(|range| range.end <= offset).is_some() {}
            if let Some(range) = protected.peek()
                && range.contains(&offset)
            {
                offset = range.end;
                continue;
            }
            let rest = &input[offset..];
            let (open, close, display) = if rest.starts_with("$$") {
                ("$$", "$$", true)
            } else if rest.starts_with("\\[") {
                ("\\[", "\\]", true)
            } else if rest.starts_with("\\(") {
                ("\\(", "\\)", false)
            } else if rest.starts_with('$') {
                ("$", "$", false)
            } else {
                let Some(ch) = rest.chars().next() else {
                    break;
                };
                offset += ch.len_utf8();
                continue;
            };
            let start = offset;
            offset += open.len();
            if escaped(input, start) {
                continue;
            }
            let body = &input[offset..];
            if open == "$"
                && (body.starts_with(char::is_whitespace) || body.starts_with(['(', '{']))
            {
                continue;
            }
            let limit = body
                .char_indices()
                .map(|(i, _)| i)
                .find(|i| *i >= MAX_MATH_BYTES)
                .unwrap_or(body.len());
            // Display boundaries outlive the conversion budget; a distant closer still owns its opener.
            let search = if display { body } else { &body[..limit] };
            let end = search.match_indices(close).find_map(|(index, _)| {
                let end = offset + index;
                (!escaped(input, end)).then_some(end)
            });
            if display {
                if open == "$$" && end.is_none() && !input[line_start..start].trim().is_empty() {
                    continue;
                }
                // Exclude display equations from inline recognition, including unfinished ones.
                offset = end.map_or(input.len(), |end| end + close.len());
                result.display_ranges.push(start..offset);
                continue;
            }
            let Some(end) = end else {
                continue;
            };
            let span = start..end + close.len();
            if protected.peek().is_some_and(|range| range.start < span.end) {
                continue;
            }
            let formula = &input[offset..end];
            if formula.contains('\n') {
                continue;
            }
            if open == "$" {
                let next = input[span.end..].chars().next();
                if formula.ends_with(char::is_whitespace) || next.is_some_and(char::is_alphanumeric)
                {
                    continue;
                }
                if formula.starts_with(|ch: char| ch.is_ascii_digit())
                    && !formula.contains(['\\', '^', '_', '=', '+', '-', '*', '/', '<', '>'])
                    || formula.len() > 1 && formula.chars().all(|ch| ch.is_ascii_uppercase())
                {
                    offset = span.end;
                    continue;
                }
            }
            let rendered =
                render::render(formula).unwrap_or_else(|| input[span.clone()].to_owned());
            // Dollars are ordinary text in the Markdown parser and cannot form an HTML tag.
            result
                .markdown
                .to_mut()
                .replace_range(span.clone(), &"$".repeat(span.len()));
            offset = span.end;
            result.replacements.push((span, rendered));
        }
        result
    }

    pub(super) fn events<'s>(
        &'s self,
        events: impl Iterator<Item = (Event<'s>, Range<usize>)>,
    ) -> impl Iterator<Item = (Event<'s>, Range<usize>)> {
        let mut replacements = self.replacements.iter().peekable();
        events.flat_map(move |(event, range)| {
            while replacements
                .next_if(|(span, _)| span.end <= range.start)
                .is_some()
            {}
            let Event::Text(text) = event else {
                return Either::Left(std::iter::once((event, range)));
            };
            if replacements
                .peek()
                .is_none_or(|(span, _)| span.start >= range.end)
            {
                return Either::Left(std::iter::once((Event::Text(text), range)));
            }
            let mut output = Vec::new();
            let mut offset = range.start;
            while let Some((span, text)) = replacements.next_if(|(span, _)| span.end <= range.end) {
                if offset < span.start {
                    output.push((
                        Event::Text(self.markdown[offset..span.start].into()),
                        offset..span.start,
                    ));
                }
                output.push((Event::Text(text.as_str().into()), span.clone()));
                offset = span.end;
            }
            if offset < range.end {
                output.push((
                    Event::Text(self.markdown[offset..range.end].into()),
                    offset..range.end,
                ));
            }
            Either::Right(output.into_iter())
        })
    }
}

fn escaped(input: &str, offset: usize) -> bool {
    input[..offset]
        .bytes()
        .rev()
        .take_while(|byte| *byte == b'\\')
        .count()
        % 2
        == 1
}

#[cfg(test)]
#[path = "math_tests.rs"]
mod tests;
