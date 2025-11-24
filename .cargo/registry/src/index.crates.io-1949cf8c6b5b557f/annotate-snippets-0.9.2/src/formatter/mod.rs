use std::{
    cmp,
    fmt::{self, Display, Write},
    iter::once,
};

pub mod style;

use self::style::{Style, StyleClass, Stylesheet};

#[cfg(feature = "color")]
use crate::stylesheets::color::AnsiTermStylesheet;
use crate::{display_list::*, stylesheets::no_color::NoColorStylesheet};

fn format_repeat_char(c: char, n: usize, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    for _ in 0..n {
        f.write_char(c)?;
    }
    Ok(())
}

#[inline]
fn is_annotation_empty(annotation: &Annotation<'_>) -> bool {
    annotation
        .label
        .iter()
        .all(|fragment| fragment.content.is_empty())
}

#[cfg(feature = "color")]
#[inline]
pub fn get_term_style(color: bool) -> Box<dyn Stylesheet> {
    if color {
        Box::new(AnsiTermStylesheet)
    } else {
        Box::new(NoColorStylesheet)
    }
}

#[cfg(not(feature = "color"))]
#[inline]
pub fn get_term_style(_color: bool) -> Box<dyn Stylesheet> {
    Box::new(NoColorStylesheet)
}

impl<'a> fmt::Display for DisplayList<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let lineno_width = self.body.iter().fold(0, |max, line| match line {
            DisplayLine::Source {
                lineno: Some(lineno),
                ..
            } => {
                // The largest line is the largest width.
                cmp::max(*lineno, max)
            }
            _ => max,
        });
        let lineno_width = if lineno_width == 0 {
            lineno_width
        } else if self.anonymized_line_numbers {
            Self::ANONYMIZED_LINE_NUM.len()
        } else {
            ((lineno_width as f64).log10().floor() as usize) + 1
        };
        let inline_marks_width = self.body.iter().fold(0, |max, line| match line {
            DisplayLine::Source { inline_marks, .. } => cmp::max(inline_marks.len(), max),
            _ => max,
        });

        for (i, line) in self.body.iter().enumerate() {
            self.format_line(line, lineno_width, inline_marks_width, f)?;
            if i + 1 < self.body.len() {
                f.write_char('\n')?;
            }
        }
        Ok(())
    }
}

impl<'a> DisplayList<'a> {
    const ANONYMIZED_LINE_NUM: &'static str = "LL";
    const ERROR_TXT: &'static str = "error";
    const HELP_TXT: &'static str = "help";
    const INFO_TXT: &'static str = "info";
    const NOTE_TXT: &'static str = "note";
    const WARNING_TXT: &'static str = "warning";

    #[inline]
    fn format_annotation_type(
        annotation_type: &DisplayAnnotationType,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        match annotation_type {
            DisplayAnnotationType::Error => f.write_str(Self::ERROR_TXT),
            DisplayAnnotationType::Help => f.write_str(Self::HELP_TXT),
            DisplayAnnotationType::Info => f.write_str(Self::INFO_TXT),
            DisplayAnnotationType::Note => f.write_str(Self::NOTE_TXT),
            DisplayAnnotationType::Warning => f.write_str(Self::WARNING_TXT),
            DisplayAnnotationType::None => Ok(()),
        }
    }

    fn annotation_type_len(annotation_type: &DisplayAnnotationType) -> usize {
        match annotation_type {
            DisplayAnnotationType::Error => Self::ERROR_TXT.len(),
            DisplayAnnotationType::Help => Self::HELP_TXT.len(),
            DisplayAnnotationType::Info => Self::INFO_TXT.len(),
            DisplayAnnotationType::Note => Self::NOTE_TXT.len(),
            DisplayAnnotationType::Warning => Self::WARNING_TXT.len(),
            DisplayAnnotationType::None => 0,
        }
    }

    fn get_annotation_style(&self, annotation_type: &DisplayAnnotationType) -> Box<dyn Style> {
        self.stylesheet.get_style(match annotation_type {
            DisplayAnnotationType::Error => StyleClass::Error,
            DisplayAnnotationType::Warning => StyleClass::Warning,
            DisplayAnnotationType::Info => StyleClass::Info,
            DisplayAnnotationType::Note => StyleClass::Note,
            DisplayAnnotationType::Help => StyleClass::Help,
            DisplayAnnotationType::None => StyleClass::None,
        })
    }

    fn format_label(
        &self,
        label: &[DisplayTextFragment<'_>],
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        let emphasis_style = self.stylesheet.get_style(StyleClass::Emphasis);

        for fragment in label {
            match fragment.style {
                DisplayTextStyle::Regular => fragment.content.fmt(f)?,
                DisplayTextStyle::Emphasis => emphasis_style.paint(fragment.content, f)?,
            }
        }
        Ok(())
    }

    fn format_annotation(
        &self,
        annotation: &Annotation<'_>,
        continuation: bool,
        in_source: bool,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        let color = self.get_annotation_style(&annotation.annotation_type);
        let formatted_len = if let Some(id) = &annotation.id {
            2 + id.len() + Self::annotation_type_len(&annotation.annotation_type)
        } else {
            Self::annotation_type_len(&annotation.annotation_type)
        };

        if continuation {
            format_repeat_char(' ', formatted_len + 2, f)?;
            return self.format_label(&annotation.label, f);
        }
        if formatted_len == 0 {
            self.format_label(&annotation.label, f)
        } else {
            color.paint_fn(
                Box::new(|f| {
                    Self::format_annotation_type(&annotation.annotation_type, f)?;
                    if let Some(id) = &annotation.id {
                        f.write_char('[')?;
                        f.write_str(id)?;
                        f.write_char(']')?;
                    }
                    Ok(())
                }),
                f,
            )?;
            if !is_annotation_empty(annotation) {
                if in_source {
                    color.paint_fn(
                        Box::new(|f| {
                            f.write_str(": ")?;
                            self.format_label(&annotation.label, f)
                        }),
                        f,
                    )?;
                } else {
                    f.write_str(": ")?;
                    self.format_label(&annotation.label, f)?;
                }
            }
            Ok(())
        }
    }

    #[inline]
    fn format_source_line(
        &self,
        line: &DisplaySourceLine<'_>,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        match line {
            DisplaySourceLine::Empty => Ok(()),
            DisplaySourceLine::Content { text, .. } => {
                f.write_char(' ')?;
                if let Some(margin) = self.margin {
                    let line_len = text.chars().count();
                    let mut left = margin.left(line_len);
                    let right = margin.right(line_len);

                    if margin.was_cut_left() {
                        // We have stripped some code/whitespace from the beginning, make it clear.
                        "...".fmt(f)?;
                        left += 3;
                    }

                    // On long lines, we strip the source line, accounting for unicode.
                    let mut taken = 0;
                    let cut_right = if margin.was_cut_right(line_len) {
                        taken += 3;
                        true
                    } else {
                        false
                    };
                    // Specifies that it will end on the next character, so it will return
                    // until the next one to the final condition.
                    let mut ended = false;
                    let range = text
                        .char_indices()
                        .skip(left)
                        // Complete char iterator with final character
                        .chain(once((text.len(), '\0')))
                        // Take until the next one to the final condition
                        .take_while(|(_, ch)| {
                            // Fast return to iterate over final byte position
                            if ended {
                                return false;
                            }
                            // Make sure that the trimming on the right will fall within the terminal width.
                            // FIXME: `unicode_width` sometimes disagrees with terminals on how wide a `char` is.
                            // For now, just accept that sometimes the code line will be longer than desired.
                            taken += unicode_width::UnicodeWidthChar::width(*ch).unwrap_or(1);
                            if taken > right - left {
                                ended = true;
                            }
                            true
                        })
                        // Reduce to start and end byte position
                        .fold((None, 0), |acc, (i, _)| {
                            if acc.0.is_some() {
                                (acc.0, i)
                            } else {
                                (Some(i), i)
                            }
                        });

                    // Format text with margins
                    text[range.0.expect("One character at line")..range.1].fmt(f)?;

                    if cut_right {
                        // We have stripped some code after the right-most span end, make it clear we did so.
                        "...".fmt(f)?;
                    }
                    Ok(())
                } else {
                    text.fmt(f)
                }
            }
            DisplaySourceLine::Annotation {
                range,
                annotation,
                annotation_type,
                annotation_part,
            } => {
                let indent_char = match annotation_part {
                    DisplayAnnotationPart::Standalone => ' ',
                    DisplayAnnotationPart::LabelContinuation => ' ',
                    DisplayAnnotationPart::Consequitive => ' ',
                    DisplayAnnotationPart::MultilineStart => '_',
                    DisplayAnnotationPart::MultilineEnd => '_',
                };
                let mark = match annotation_type {
                    DisplayAnnotationType::Error => '^',
                    DisplayAnnotationType::Warning => '-',
                    DisplayAnnotationType::Info => '-',
                    DisplayAnnotationType::Note => '-',
                    DisplayAnnotationType::Help => '-',
                    DisplayAnnotationType::None => ' ',
                };
                let color = self.get_annotation_style(annotation_type);
                let indent_length = match annotation_part {
                    DisplayAnnotationPart::LabelContinuation => range.1,
                    DisplayAnnotationPart::Consequitive => range.1,
                    _ => range.0,
                };

                color.paint_fn(
                    Box::new(|f| {
                        format_repeat_char(indent_char, indent_length + 1, f)?;
                        format_repeat_char(mark, range.1 - indent_length, f)
                    }),
                    f,
                )?;

                if !is_annotation_empty(annotation) {
                    f.write_char(' ')?;
                    color.paint_fn(
                        Box::new(|f| {
                            self.format_annotation(
                                annotation,
                                annotation_part == &DisplayAnnotationPart::LabelContinuation,
                                true,
                                f,
                            )
                        }),
                        f,
                    )?;
                }

                Ok(())
            }
        }
    }

    #[inline]
    fn format_raw_line(
        &self,
        line: &DisplayRawLine<'_>,
        lineno_width: usize,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        match line {
            DisplayRawLine::Origin {
                path,
                pos,
                header_type,
            } => {
                let header_sigil = match header_type {
                    DisplayHeaderType::Initial => "-->",
                    DisplayHeaderType::Continuation => ":::",
                };
                let lineno_color = self.stylesheet.get_style(StyleClass::LineNo);

                if let Some((col, row)) = pos {
                    format_repeat_char(' ', lineno_width, f)?;
                    lineno_color.paint(header_sigil, f)?;
                    f.write_char(' ')?;
                    path.fmt(f)?;
                    f.write_char(':')?;
                    col.fmt(f)?;
                    f.write_char(':')?;
                    row.fmt(f)
                } else {
                    format_repeat_char(' ', lineno_width, f)?;
                    lineno_color.paint(header_sigil, f)?;
                    f.write_char(' ')?;
                    path.fmt(f)
                }
            }
            DisplayRawLine::Annotation {
                annotation,
                source_aligned,
                continuation,
            } => {
                if *source_aligned {
                    if *continuation {
                        format_repeat_char(' ', lineno_width + 3, f)?;
                    } else {
                        let lineno_color = self.stylesheet.get_style(StyleClass::LineNo);
                        format_repeat_char(' ', lineno_width, f)?;
                        f.write_char(' ')?;
                        lineno_color.paint("=", f)?;
                        f.write_char(' ')?;
                    }
                }
                self.format_annotation(annotation, *continuation, false, f)
            }
        }
    }

    #[inline]
    fn format_line(
        &self,
        dl: &DisplayLine<'_>,
        lineno_width: usize,
        inline_marks_width: usize,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        match dl {
            DisplayLine::Source {
                lineno,
                inline_marks,
                line,
            } => {
                let lineno_color = self.stylesheet.get_style(StyleClass::LineNo);
                if self.anonymized_line_numbers && lineno.is_some() {
                    lineno_color.paint_fn(
                        Box::new(|f| {
                            f.write_str(Self::ANONYMIZED_LINE_NUM)?;
                            f.write_str(" |")
                        }),
                        f,
                    )?;
                } else {
                    lineno_color.paint_fn(
                        Box::new(|f| {
                            match lineno {
                                Some(n) => write!(f, "{:>width$}", n, width = lineno_width),
                                None => format_repeat_char(' ', lineno_width, f),
                            }?;
                            f.write_str(" |")
                        }),
                        f,
                    )?;
                }
                if *line != DisplaySourceLine::Empty {
                    if !inline_marks.is_empty() || 0 < inline_marks_width {
                        f.write_char(' ')?;
                        self.format_inline_marks(inline_marks, inline_marks_width, f)?;
                    }
                    self.format_source_line(line, f)?;
                } else if !inline_marks.is_empty() {
                    f.write_char(' ')?;
                    self.format_inline_marks(inline_marks, inline_marks_width, f)?;
                }
                Ok(())
            }
            DisplayLine::Fold { inline_marks } => {
                f.write_str("...")?;
                if !inline_marks.is_empty() || 0 < inline_marks_width {
                    format_repeat_char(' ', lineno_width, f)?;
                    self.format_inline_marks(inline_marks, inline_marks_width, f)?;
                }
                Ok(())
            }
            DisplayLine::Raw(line) => self.format_raw_line(line, lineno_width, f),
        }
    }

    fn format_inline_marks(
        &self,
        inline_marks: &[DisplayMark],
        inline_marks_width: usize,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        format_repeat_char(' ', inline_marks_width - inline_marks.len(), f)?;
        for mark in inline_marks {
            self.get_annotation_style(&mark.annotation_type).paint_fn(
                Box::new(|f| {
                    f.write_char(match mark.mark_type {
                        DisplayMarkType::AnnotationThrough => '|',
                        DisplayMarkType::AnnotationStart => '/',
                    })
                }),
                f,
            )?;
        }
        Ok(())
    }
}
