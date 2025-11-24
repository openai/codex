//! Trait for converting `Snippet` to `DisplayList`.
use super::*;
use crate::{formatter::get_term_style, snippet};

struct CursorLines<'a>(&'a str);

impl<'a> CursorLines<'a> {
    fn new(src: &str) -> CursorLines<'_> {
        CursorLines(src)
    }
}

enum EndLine {
    EOF = 0,
    CRLF = 1,
    LF = 2,
}

impl<'a> Iterator for CursorLines<'a> {
    type Item = (&'a str, EndLine);

    fn next(&mut self) -> Option<Self::Item> {
        if self.0.is_empty() {
            None
        } else {
            self.0
                .find('\n')
                .map(|x| {
                    let ret = if 0 < x {
                        if self.0.as_bytes()[x - 1] == b'\r' {
                            (&self.0[..x - 1], EndLine::LF)
                        } else {
                            (&self.0[..x], EndLine::CRLF)
                        }
                    } else {
                        ("", EndLine::CRLF)
                    };
                    self.0 = &self.0[x + 1..];
                    ret
                })
                .or_else(|| {
                    let ret = Some((self.0, EndLine::EOF));
                    self.0 = "";
                    ret
                })
        }
    }
}

fn format_label(
    label: Option<&str>,
    style: Option<DisplayTextStyle>,
) -> Vec<DisplayTextFragment<'_>> {
    let mut result = vec![];
    if let Some(label) = label {
        let element_style = style.unwrap_or(DisplayTextStyle::Regular);
        result.push(DisplayTextFragment {
            content: label,
            style: element_style,
        });
    }
    result
}

fn format_title(annotation: snippet::Annotation<'_>) -> DisplayLine<'_> {
    let label = annotation.label.unwrap_or_default();
    DisplayLine::Raw(DisplayRawLine::Annotation {
        annotation: Annotation {
            annotation_type: DisplayAnnotationType::from(annotation.annotation_type),
            id: annotation.id,
            label: format_label(Some(label), Some(DisplayTextStyle::Emphasis)),
        },
        source_aligned: false,
        continuation: false,
    })
}

fn format_annotation(annotation: snippet::Annotation<'_>) -> Vec<DisplayLine<'_>> {
    let mut result = vec![];
    let label = annotation.label.unwrap_or_default();
    for (i, line) in label.lines().enumerate() {
        result.push(DisplayLine::Raw(DisplayRawLine::Annotation {
            annotation: Annotation {
                annotation_type: DisplayAnnotationType::from(annotation.annotation_type),
                id: None,
                label: format_label(Some(line), None),
            },
            source_aligned: true,
            continuation: i != 0,
        }));
    }
    result
}

fn format_slice(
    slice: snippet::Slice<'_>,
    is_first: bool,
    has_footer: bool,
    margin: Option<Margin>,
) -> Vec<DisplayLine<'_>> {
    let main_range = slice.annotations.get(0).map(|x| x.range.0);
    let origin = slice.origin;
    let need_empty_header = origin.is_some() || is_first;
    let mut body = format_body(slice, need_empty_header, has_footer, margin);
    let header = format_header(origin, main_range, &body, is_first);
    let mut result = vec![];

    if let Some(header) = header {
        result.push(header);
    }
    result.append(&mut body);
    result
}

#[inline]
// TODO: option_zip
fn zip_opt<A, B>(a: Option<A>, b: Option<B>) -> Option<(A, B)> {
    a.and_then(|a| b.map(|b| (a, b)))
}

fn format_header<'a>(
    origin: Option<&'a str>,
    main_range: Option<usize>,
    body: &[DisplayLine<'_>],
    is_first: bool,
) -> Option<DisplayLine<'a>> {
    let display_header = if is_first {
        DisplayHeaderType::Initial
    } else {
        DisplayHeaderType::Continuation
    };

    if let Some((main_range, path)) = zip_opt(main_range, origin) {
        let mut col = 1;
        let mut line_offset = 1;

        for item in body {
            if let DisplayLine::Source {
                line: DisplaySourceLine::Content { range, .. },
                lineno,
                ..
            } = item
            {
                if main_range >= range.0 && main_range <= range.1 {
                    col = main_range - range.0 + 1;
                    line_offset = lineno.unwrap_or(1);
                    break;
                }
            }
        }

        return Some(DisplayLine::Raw(DisplayRawLine::Origin {
            path,
            pos: Some((line_offset, col)),
            header_type: display_header,
        }));
    }

    if let Some(path) = origin {
        return Some(DisplayLine::Raw(DisplayRawLine::Origin {
            path,
            pos: None,
            header_type: display_header,
        }));
    }

    None
}

fn fold_body(mut body: Vec<DisplayLine<'_>>) -> Vec<DisplayLine<'_>> {
    enum Line {
        Fold(usize),
        Source(usize),
    }

    let mut lines = vec![];
    let mut no_annotation_lines_counter = 0;

    for (idx, line) in body.iter().enumerate() {
        match line {
            DisplayLine::Source {
                line: DisplaySourceLine::Annotation { .. },
                ..
            } => {
                let fold_start = idx - no_annotation_lines_counter;
                if no_annotation_lines_counter > 2 {
                    let fold_end = idx;
                    let pre_len = if no_annotation_lines_counter > 8 {
                        4
                    } else {
                        0
                    };
                    let post_len = if no_annotation_lines_counter > 8 {
                        2
                    } else {
                        1
                    };
                    for (i, _) in body
                        .iter()
                        .enumerate()
                        .take(fold_start + pre_len)
                        .skip(fold_start)
                    {
                        lines.push(Line::Source(i));
                    }
                    lines.push(Line::Fold(idx));
                    for (i, _) in body
                        .iter()
                        .enumerate()
                        .take(fold_end)
                        .skip(fold_end - post_len)
                    {
                        lines.push(Line::Source(i));
                    }
                } else {
                    for (i, _) in body.iter().enumerate().take(idx).skip(fold_start) {
                        lines.push(Line::Source(i));
                    }
                }
                no_annotation_lines_counter = 0;
            }
            DisplayLine::Source { .. } => {
                no_annotation_lines_counter += 1;
                continue;
            }
            _ => {
                no_annotation_lines_counter += 1;
            }
        }
        lines.push(Line::Source(idx));
    }

    let mut new_body = vec![];
    let mut removed = 0;
    for line in lines {
        match line {
            Line::Source(i) => {
                new_body.push(body.remove(i - removed));
                removed += 1;
            }
            Line::Fold(i) => {
                if let DisplayLine::Source {
                    line: DisplaySourceLine::Annotation { .. },
                    ref inline_marks,
                    ..
                } = body.get(i - removed).unwrap()
                {
                    new_body.push(DisplayLine::Fold {
                        inline_marks: inline_marks.clone(),
                    })
                } else {
                    unreachable!()
                }
            }
        }
    }

    new_body
}

fn format_body(
    slice: snippet::Slice<'_>,
    need_empty_header: bool,
    has_footer: bool,
    margin: Option<Margin>,
) -> Vec<DisplayLine<'_>> {
    let source_len = slice.source.chars().count();
    if let Some(bigger) = slice.annotations.iter().find_map(|x| {
        if source_len < x.range.1 {
            Some(x.range)
        } else {
            None
        }
    }) {
        panic!(
            "SourceAnnotation range `{:?}` is bigger than source length `{}`",
            bigger, source_len
        )
    }

    let mut body = vec![];
    let mut current_line = slice.line_start;
    let mut current_index = 0;
    let mut line_info = vec![];

    struct LineInfo {
        line_start_index: usize,
        line_end_index: usize,
        // How many spaces each character in the line take up when displayed
        char_widths: Vec<usize>,
    }

    for (line, end_line) in CursorLines::new(slice.source) {
        let line_length = line.chars().count();
        let line_range = (current_index, current_index + line_length);
        let char_widths = line
            .chars()
            .map(|c| unicode_width::UnicodeWidthChar::width(c).unwrap_or(0))
            .chain(std::iter::once(1)) // treat the end of line as single-width
            .collect::<Vec<_>>();
        body.push(DisplayLine::Source {
            lineno: Some(current_line),
            inline_marks: vec![],
            line: DisplaySourceLine::Content {
                text: line,
                range: line_range,
            },
        });
        line_info.push(LineInfo {
            line_start_index: line_range.0,
            line_end_index: line_range.1,
            char_widths,
        });
        current_line += 1;
        current_index += line_length + end_line as usize;
    }

    let mut annotation_line_count = 0;
    let mut annotations = slice.annotations;
    for (
        idx,
        LineInfo {
            line_start_index,
            line_end_index,
            char_widths,
        },
    ) in line_info.into_iter().enumerate()
    {
        let margin_left = margin
            .map(|m| m.left(line_end_index - line_start_index))
            .unwrap_or_default();
        // It would be nice to use filter_drain here once it's stable.
        annotations = annotations
            .into_iter()
            .filter(|annotation| {
                let body_idx = idx + annotation_line_count;
                let annotation_type = match annotation.annotation_type {
                    snippet::AnnotationType::Error => DisplayAnnotationType::None,
                    snippet::AnnotationType::Warning => DisplayAnnotationType::None,
                    _ => DisplayAnnotationType::from(annotation.annotation_type),
                };
                match annotation.range {
                    (start, _) if start > line_end_index => true,
                    (start, end)
                        if start >= line_start_index && end <= line_end_index
                            || start == line_end_index && end - start <= 1 =>
                    {
                        let annotation_start_col = char_widths
                            .iter()
                            .take(start - line_start_index)
                            .sum::<usize>()
                            - margin_left;
                        let annotation_end_col = char_widths
                            .iter()
                            .take(end - line_start_index)
                            .sum::<usize>()
                            - margin_left;
                        let range = (annotation_start_col, annotation_end_col);
                        body.insert(
                            body_idx + 1,
                            DisplayLine::Source {
                                lineno: None,
                                inline_marks: vec![],
                                line: DisplaySourceLine::Annotation {
                                    annotation: Annotation {
                                        annotation_type,
                                        id: None,
                                        label: format_label(Some(annotation.label), None),
                                    },
                                    range,
                                    annotation_type: DisplayAnnotationType::from(
                                        annotation.annotation_type,
                                    ),
                                    annotation_part: DisplayAnnotationPart::Standalone,
                                },
                            },
                        );
                        annotation_line_count += 1;
                        false
                    }
                    (start, end)
                        if start >= line_start_index
                            && start <= line_end_index
                            && end > line_end_index =>
                    {
                        if start - line_start_index == 0 {
                            if let DisplayLine::Source {
                                ref mut inline_marks,
                                ..
                            } = body[body_idx]
                            {
                                inline_marks.push(DisplayMark {
                                    mark_type: DisplayMarkType::AnnotationStart,
                                    annotation_type: DisplayAnnotationType::from(
                                        annotation.annotation_type,
                                    ),
                                });
                            }
                        } else {
                            let annotation_start_col = char_widths
                                .iter()
                                .take(start - line_start_index)
                                .sum::<usize>();
                            let range = (annotation_start_col, annotation_start_col + 1);
                            body.insert(
                                body_idx + 1,
                                DisplayLine::Source {
                                    lineno: None,
                                    inline_marks: vec![],
                                    line: DisplaySourceLine::Annotation {
                                        annotation: Annotation {
                                            annotation_type: DisplayAnnotationType::None,
                                            id: None,
                                            label: vec![],
                                        },
                                        range,
                                        annotation_type: DisplayAnnotationType::from(
                                            annotation.annotation_type,
                                        ),
                                        annotation_part: DisplayAnnotationPart::MultilineStart,
                                    },
                                },
                            );
                            annotation_line_count += 1;
                        }
                        true
                    }
                    (start, end) if start < line_start_index && end > line_end_index => {
                        if let DisplayLine::Source {
                            ref mut inline_marks,
                            ..
                        } = body[body_idx]
                        {
                            inline_marks.push(DisplayMark {
                                mark_type: DisplayMarkType::AnnotationThrough,
                                annotation_type: DisplayAnnotationType::from(
                                    annotation.annotation_type,
                                ),
                            });
                        }
                        true
                    }
                    (start, end)
                        if start < line_start_index
                            && end >= line_start_index
                            && end <= line_end_index =>
                    {
                        if let DisplayLine::Source {
                            ref mut inline_marks,
                            ..
                        } = body[body_idx]
                        {
                            inline_marks.push(DisplayMark {
                                mark_type: DisplayMarkType::AnnotationThrough,
                                annotation_type: DisplayAnnotationType::from(
                                    annotation.annotation_type,
                                ),
                            });
                        }

                        let end_mark = char_widths
                            .iter()
                            .take(end - line_start_index)
                            .sum::<usize>()
                            .saturating_sub(1);
                        let range = (end_mark - margin_left, (end_mark + 1) - margin_left);
                        body.insert(
                            body_idx + 1,
                            DisplayLine::Source {
                                lineno: None,
                                inline_marks: vec![DisplayMark {
                                    mark_type: DisplayMarkType::AnnotationThrough,
                                    annotation_type: DisplayAnnotationType::from(
                                        annotation.annotation_type,
                                    ),
                                }],
                                line: DisplaySourceLine::Annotation {
                                    annotation: Annotation {
                                        annotation_type,
                                        id: None,
                                        label: format_label(Some(annotation.label), None),
                                    },
                                    range,
                                    annotation_type: DisplayAnnotationType::from(
                                        annotation.annotation_type,
                                    ),
                                    annotation_part: DisplayAnnotationPart::MultilineEnd,
                                },
                            },
                        );
                        annotation_line_count += 1;
                        false
                    }
                    _ => true,
                }
            })
            .collect();
    }

    if slice.fold {
        body = fold_body(body);
    }

    if need_empty_header {
        body.insert(
            0,
            DisplayLine::Source {
                lineno: None,
                inline_marks: vec![],
                line: DisplaySourceLine::Empty,
            },
        );
    }

    if has_footer {
        body.push(DisplayLine::Source {
            lineno: None,
            inline_marks: vec![],
            line: DisplaySourceLine::Empty,
        });
    } else if let Some(DisplayLine::Source { .. }) = body.last() {
        body.push(DisplayLine::Source {
            lineno: None,
            inline_marks: vec![],
            line: DisplaySourceLine::Empty,
        });
    }
    body
}

impl<'a> From<snippet::Snippet<'a>> for DisplayList<'a> {
    fn from(
        snippet::Snippet {
            title,
            footer,
            slices,
            opt,
        }: snippet::Snippet<'a>,
    ) -> DisplayList<'a> {
        let mut body = vec![];
        if let Some(annotation) = title {
            body.push(format_title(annotation));
        }

        for (idx, slice) in slices.into_iter().enumerate() {
            body.append(&mut format_slice(
                slice,
                idx == 0,
                !footer.is_empty(),
                opt.margin,
            ));
        }

        for annotation in footer {
            body.append(&mut format_annotation(annotation));
        }

        let FormatOptions {
            color,
            anonymized_line_numbers,
            margin,
        } = opt;

        Self {
            body,
            stylesheet: get_term_style(color),
            anonymized_line_numbers,
            margin,
        }
    }
}

impl From<snippet::AnnotationType> for DisplayAnnotationType {
    fn from(at: snippet::AnnotationType) -> Self {
        match at {
            snippet::AnnotationType::Error => DisplayAnnotationType::Error,
            snippet::AnnotationType::Warning => DisplayAnnotationType::Warning,
            snippet::AnnotationType::Info => DisplayAnnotationType::Info,
            snippet::AnnotationType::Note => DisplayAnnotationType::Note,
            snippet::AnnotationType::Help => DisplayAnnotationType::Help,
        }
    }
}
