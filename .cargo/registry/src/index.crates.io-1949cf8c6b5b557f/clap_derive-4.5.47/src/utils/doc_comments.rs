//! The preprocessing we apply to doc comments.
//!
//! #[derive(Parser)] works in terms of "paragraphs". Paragraph is a sequence of
//! non-empty adjacent lines, delimited by sequences of blank (whitespace only) lines.

#[cfg(feature = "unstable-markdown")]
use markdown::parse_markdown;

pub(crate) fn extract_doc_comment(attrs: &[syn::Attribute]) -> Vec<String> {
    // multiline comments (`/** ... */`) may have LFs (`\n`) in them,
    // we need to split so we could handle the lines correctly
    //
    // we also need to remove leading and trailing blank lines
    let mut lines: Vec<_> = attrs
        .iter()
        .filter(|attr| attr.path().is_ident("doc"))
        .filter_map(|attr| {
            // non #[doc = "..."] attributes are not our concern
            // we leave them for rustc to handle
            match &attr.meta {
                syn::Meta::NameValue(syn::MetaNameValue {
                    value:
                        syn::Expr::Lit(syn::ExprLit {
                            lit: syn::Lit::Str(s),
                            ..
                        }),
                    ..
                }) => Some(s.value()),
                _ => None,
            }
        })
        .skip_while(|s| is_blank(s))
        .flat_map(|s| {
            let lines = s
                .split('\n')
                .map(|s| {
                    // remove one leading space no matter what
                    let s = s.strip_prefix(' ').unwrap_or(s);
                    s.to_owned()
                })
                .collect::<Vec<_>>();
            lines
        })
        .collect();

    while let Some(true) = lines.last().map(|s| is_blank(s)) {
        lines.pop();
    }

    lines
}

pub(crate) fn format_doc_comment(
    lines: &[String],
    preprocess: bool,
    force_long: bool,
) -> (Option<String>, Option<String>) {
    if preprocess {
        let (short, long) = parse_markdown(lines);
        let long = long.or_else(|| force_long.then(|| short.clone()));

        (Some(remove_period(short)), long)
    } else if let Some(first_blank) = lines.iter().position(|s| is_blank(s)) {
        let short = lines[..first_blank].join("\n");
        let long = lines.join("\n");

        (Some(short), Some(long))
    } else {
        let short = lines.join("\n");
        let long = force_long.then(|| short.clone());

        (Some(short), long)
    }
}

#[cfg(not(feature = "unstable-markdown"))]
fn split_paragraphs(lines: &[String]) -> Vec<String> {
    use std::iter;

    let mut last_line = 0;
    iter::from_fn(|| {
        let slice = &lines[last_line..];
        let start = slice.iter().position(|s| !is_blank(s)).unwrap_or(0);

        let slice = &slice[start..];
        let len = slice
            .iter()
            .position(|s| is_blank(s))
            .unwrap_or(slice.len());

        last_line += start + len;

        if len != 0 {
            Some(merge_lines(&slice[..len]))
        } else {
            None
        }
    })
    .collect()
}

fn remove_period(mut s: String) -> String {
    if s.ends_with('.') && !s.ends_with("..") {
        s.pop();
    }
    s
}

fn is_blank(s: &str) -> bool {
    s.trim().is_empty()
}

#[cfg(not(feature = "unstable-markdown"))]
fn merge_lines(lines: impl IntoIterator<Item = impl AsRef<str>>) -> String {
    lines
        .into_iter()
        .map(|s| s.as_ref().trim().to_owned())
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(not(feature = "unstable-markdown"))]
fn parse_markdown(lines: &[String]) -> (String, Option<String>) {
    if lines.iter().any(|s| is_blank(s)) {
        let paragraphs = split_paragraphs(lines);
        let short = paragraphs[0].clone();
        let long = paragraphs.join("\n\n");
        (short, Some(long))
    } else {
        let short = merge_lines(lines);
        (short, None)
    }
}

#[cfg(feature = "unstable-markdown")]
mod markdown {
    use anstyle::{Reset, Style};
    use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
    use std::fmt;
    use std::fmt::Write;
    use std::ops::AddAssign;

    #[derive(Default)]
    struct MarkdownWriter {
        output: String,
        /// Prefix inserted for each line.
        prefix: String,
        /// Should an empty line be inserted before the next anything.
        hanging_paragraph: bool,
        /// Are we in an empty line
        dirty_line: bool,
        styles: Vec<Style>,
    }

    impl MarkdownWriter {
        fn newline(&mut self) {
            self.reset();
            self.output.push('\n');
            self.dirty_line = false;
        }
        fn endline(&mut self) {
            if self.dirty_line {
                self.newline();
            }
        }
        fn new_paragraph(&mut self) {
            self.endline();
            self.hanging_paragraph = true;
        }

        fn write_fmt(&mut self, arguments: fmt::Arguments<'_>) {
            if self.hanging_paragraph {
                self.hanging_paragraph = false;
                self.newline();
            }
            if !self.dirty_line {
                self.output.push_str(&self.prefix);
                self.apply_styles();
                self.dirty_line = true;
            }
            self.output.write_fmt(arguments).unwrap();
        }

        fn start_link(&mut self, dest_url: pulldown_cmark::CowStr<'_>) {
            write!(self, "\x1B]8;;{dest_url}\x1B\\");
        }
        fn end_link(&mut self) {
            write!(self, "\x1B]8;;\x1B\\");
        }

        fn start_style(&mut self, style: Style) {
            self.styles.push(style);
            write!(self, "{style}");
        }
        fn end_style(&mut self, style: Style) {
            let last_style = self.styles.pop();
            debug_assert_eq!(last_style.unwrap(), style);

            write!(self, "{Reset}");
            self.apply_styles();
        }

        fn reset(&mut self) {
            write!(self, "{Reset}");
        }

        fn apply_styles(&mut self) {
            // Reapplying all, because anstyle doesn't support merging styles
            // (probably because the ambiguity around colors)
            // TODO If we decide not to support any colors, we can replace this with
            // anstyle::Effects and remove the need for applying them all individually.
            for style in &self.styles {
                write!(self.output, "{style}").unwrap();
            }
        }

        fn remove_prefix(&mut self, quote_prefix: &str) {
            debug_assert!(self.prefix.ends_with(quote_prefix));
            let new_len = self.prefix.len() - quote_prefix.len();
            self.prefix.truncate(new_len);
        }

        fn add_prefix(&mut self, quote_prefix: &str) {
            if self.hanging_paragraph {
                self.hanging_paragraph = false;
                self.newline();
            }
            self.prefix += quote_prefix;
        }
    }

    pub(super) fn parse_markdown(input: &[String]) -> (String, Option<String>) {
        // Markdown Configuration
        let parsing_options = Options::ENABLE_STRIKETHROUGH;
        // Minimal Styling for now, because we cannot configure it
        let style_heading = Style::new().bold().underline();
        let style_emphasis = Style::new().italic();
        let style_strong = Style::new().bold();
        let style_strike_through = Style::new().strikethrough();
        let style_link = Style::new().underline();
        let style_code = Style::new().bold();
        let list_symbol = '-';
        let quote_prefix = "| ";
        let indentation = "  ";

        let input = input.join("\n");
        let input = Parser::new_ext(&input, parsing_options);

        let mut short = None;
        let mut has_details = false;

        let mut writer = MarkdownWriter::default();

        let mut list_indices = Vec::new();

        for event in input {
            if short.is_some() {
                has_details = true;
            }
            match event {
                Event::Start(Tag::Paragraph) => { /* nothing to do */ }
                Event::End(TagEnd::Paragraph) => {
                    if short.is_none() {
                        short = Some(writer.output.trim().to_owned());
                    }
                    writer.new_paragraph();
                }

                Event::Start(Tag::Heading { .. }) => writer.start_style(style_heading),
                Event::End(TagEnd::Heading(..)) => {
                    writer.end_style(style_heading);
                    writer.new_paragraph();
                }

                Event::Start(Tag::Image { .. } | Tag::HtmlBlock) => { /* IGNORED */ }
                Event::End(TagEnd::Image) => { /* IGNORED */ }
                Event::End(TagEnd::HtmlBlock) => writer.new_paragraph(),

                Event::Start(Tag::BlockQuote(_)) => writer.add_prefix(quote_prefix),
                Event::End(TagEnd::BlockQuote(_)) => {
                    writer.remove_prefix(quote_prefix);
                    writer.new_paragraph();
                }

                Event::Start(Tag::CodeBlock(_)) => {
                    writer.add_prefix(indentation);
                    writer.start_style(style_code);
                }
                Event::End(TagEnd::CodeBlock) => {
                    writer.remove_prefix(indentation);
                    writer.end_style(style_code);
                    writer.dirty_line = false;
                    writer.hanging_paragraph = true;
                }

                Event::Start(Tag::List(list_start)) => {
                    list_indices.push(list_start);
                    writer.endline();
                }
                Event::End(TagEnd::List(_)) => {
                    let list = list_indices.pop();
                    debug_assert!(list.is_some());
                    if list_indices.is_empty() {
                        writer.new_paragraph();
                    }
                }
                Event::Start(Tag::Item) => {
                    if let Some(Some(index)) = list_indices.last_mut() {
                        write!(writer, "{index}. ");
                        index.add_assign(1);
                    } else {
                        write!(writer, "{list_symbol} ");
                    }
                    writer.add_prefix(indentation);
                }
                Event::End(TagEnd::Item) => {
                    writer.remove_prefix(indentation);
                    writer.endline();
                }

                Event::Start(Tag::Emphasis) => writer.start_style(style_emphasis),
                Event::End(TagEnd::Emphasis) => writer.end_style(style_emphasis),
                Event::Start(Tag::Strong) => writer.start_style(style_strong),
                Event::End(TagEnd::Strong) => writer.end_style(style_strong),
                Event::Start(Tag::Strikethrough) => writer.start_style(style_strike_through),
                Event::End(TagEnd::Strikethrough) => writer.end_style(style_strike_through),

                Event::Start(Tag::Link { dest_url, .. }) => {
                    writer.start_link(dest_url);
                    writer.start_style(style_link);
                }
                Event::End(TagEnd::Link) => {
                    writer.end_link();
                    writer.end_style(style_link);
                }

                Event::Text(segment) => {
                    // split into lines to support code blocks
                    let mut lines = segment.lines();
                    // `.lines()`  always returns at least one
                    write!(writer, "{}", lines.next().unwrap());
                    for line in lines {
                        writer.endline();
                        write!(writer, "{line}");
                    }
                    if segment.ends_with('\n') {
                        writer.endline();
                    }
                }

                Event::Code(code) => {
                    writer.start_style(style_code);
                    write!(writer, "{code}");
                    writer.end_style(style_code);
                }

                // There is not really anything useful to do with block level html.
                Event::Html(html) => write!(writer, "{html}"),
                // At some point we could support custom tags like `<red>`
                Event::InlineHtml(html) => write!(writer, "{html}"),
                Event::SoftBreak => write!(writer, " "),
                Event::HardBreak => writer.endline(),

                Event::Rule => {
                    writer.new_paragraph();
                    write!(writer, "---");
                    writer.new_paragraph();
                }

                // Markdown features currently not supported
                Event::Start(
                    Tag::FootnoteDefinition(_)
                    | Tag::DefinitionList
                    | Tag::DefinitionListTitle
                    | Tag::DefinitionListDefinition
                    | Tag::Table(_)
                    | Tag::TableHead
                    | Tag::TableRow
                    | Tag::TableCell
                    | Tag::MetadataBlock(_)
                    | Tag::Superscript
                    | Tag::Subscript,
                )
                | Event::End(
                    TagEnd::FootnoteDefinition
                    | TagEnd::DefinitionList
                    | TagEnd::DefinitionListTitle
                    | TagEnd::DefinitionListDefinition
                    | TagEnd::Table
                    | TagEnd::TableHead
                    | TagEnd::TableRow
                    | TagEnd::TableCell
                    | TagEnd::MetadataBlock(_)
                    | TagEnd::Superscript
                    | TagEnd::Subscript,
                )
                | Event::InlineMath(_)
                | Event::DisplayMath(_)
                | Event::FootnoteReference(_)
                | Event::TaskListMarker(_) => {
                    unimplemented!("feature not enabled {event:?}")
                }
            }
        }
        let short = short.unwrap_or_else(|| writer.output.trim_end().to_owned());
        let long = writer.output.trim_end();
        let long = has_details.then(|| long.to_owned());
        (short, long)
    }
}
