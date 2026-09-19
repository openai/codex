//! Completed request-user-input transcript rendering.

use super::*;
use crate::style::accent_color;

/// Renders a completed (or interrupted) request_user_input exchange in history.
#[derive(Debug)]
pub(crate) struct RequestUserInputResultCell {
    pub(crate) questions: Vec<ToolRequestUserInputQuestion>,
    pub(crate) answers: HashMap<String, ToolRequestUserInputAnswer>,
    pub(crate) interrupted: bool,
}

impl HistoryCell for RequestUserInputResultCell {
    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        visible_lines(self.display_hyperlink_lines(width))
    }

    fn display_hyperlink_lines(&self, width: u16) -> Vec<HyperlinkLine> {
        let width = width.max(1) as usize;
        let total = self.questions.len();
        let answered = self
            .questions
            .iter()
            .filter(|question| {
                self.answers
                    .get(&question.id)
                    .is_some_and(|answer| !answer.answers.is_empty())
            })
            .count();
        let unanswered = total.saturating_sub(answered);

        let mut header = vec!["•".dim(), " ".into(), "Questions".bold()];
        header.push(format!(" {answered}/{total} answered").dim());
        if self.interrupted {
            header.push(" (interrupted)".fg(accent_color()));
        }

        let mut lines = vec![HyperlinkLine::new(header.into())];

        for question in &self.questions {
            let answer = self.answers.get(&question.id);
            let answer_missing = match answer {
                Some(answer) => answer.answers.is_empty(),
                None => true,
            };
            let mut question_lines = wrap_with_prefix(
                &question.question,
                width,
                "  • ".into(),
                "    ".into(),
                Style::default(),
            );
            if answer_missing && let Some(last) = question_lines.last_mut() {
                let suffix = " (unanswered)";
                last.line.spans.push(suffix.dim());
                if let Some(source) = &mut last.source {
                    // The suffix is deliberately appended after wrapping, matching the existing UI.
                    let mut logical = source.styled_range(0..source.range.end);
                    logical.spans.push(suffix.dim());
                    let logical =
                        crate::terminal_hyperlinks::LogicalLineSource::from_line(&logical);
                    source.range.end = logical.text.len();
                    for line in &mut question_lines {
                        if let Some(source) = &mut line.source {
                            source.text = std::sync::Arc::clone(&logical.text);
                            source.styles = std::sync::Arc::clone(&logical.styles);
                            source.line_style = logical.line_style;
                            source.span_style = Style::default();
                        }
                    }
                }
            }
            lines.extend(question_lines);

            let Some(answer) = answer.filter(|answer| !answer.answers.is_empty()) else {
                continue;
            };
            if question.is_secret {
                lines.extend(wrap_with_prefix(
                    "••••••",
                    width,
                    "    answer: ".dim(),
                    "            ".dim(),
                    Style::default().fg(accent_color()),
                ));
                continue;
            }

            let (options, note) = split_request_user_input_answer(answer);

            for option in options {
                lines.extend(wrap_with_prefix(
                    &option,
                    width,
                    "    answer: ".dim(),
                    "            ".dim(),
                    Style::default().fg(accent_color()),
                ));
            }
            if let Some(note) = note {
                let (label, continuation, style) = if question.options.is_some() {
                    (
                        "    note: ".dim(),
                        "          ".dim(),
                        Style::default().fg(accent_color()),
                    )
                } else {
                    (
                        "    answer: ".dim(),
                        "            ".dim(),
                        Style::default().fg(accent_color()),
                    )
                };
                lines.extend(wrap_with_prefix(&note, width, label, continuation, style));
            }
        }

        if self.interrupted && unanswered > 0 {
            let summary = format!("interrupted with {unanswered} unanswered");
            lines.extend(wrap_with_prefix(
                &summary,
                width,
                "  ↳ ".fg(accent_color()),
                "    ".dim(),
                Style::default().fg(accent_color()),
            ));
        }

        lines
    }

    fn transcript_hyperlink_lines(&self, width: u16) -> Vec<HyperlinkLine> {
        self.display_hyperlink_lines(width)
    }

    fn raw_lines(&self) -> Vec<Line<'static>> {
        let total = self.questions.len();
        let answered = self
            .questions
            .iter()
            .filter(|question| {
                self.answers
                    .get(&question.id)
                    .is_some_and(|answer| !answer.answers.is_empty())
            })
            .count();
        let mut lines = vec![Line::from(format!("Questions {answered}/{total} answered"))];
        if self.interrupted {
            lines.push(Line::from("(interrupted)"));
        }
        for question in &self.questions {
            lines.push(Line::from(question.question.clone()));
            if let Some(answer) = self
                .answers
                .get(&question.id)
                .filter(|answer| !answer.answers.is_empty())
            {
                if question.is_secret {
                    lines.push(Line::from("answer: ******"));
                } else {
                    let (options, note) = split_request_user_input_answer(answer);
                    lines.extend(
                        options
                            .into_iter()
                            .map(|option| Line::from(format!("answer: {option}"))),
                    );
                    if let Some(note) = note {
                        lines.push(Line::from(format!("note: {note}")));
                    }
                }
            } else {
                lines.push(Line::from("(unanswered)"));
            }
        }
        lines
    }
}

/// Retain the first-line label as text, keeping continuation alignment display-only.
fn wrap_with_prefix(
    text: &str,
    width: usize,
    initial_prefix: Span<'static>,
    subsequent_prefix: Span<'static>,
    style: Style,
) -> Vec<HyperlinkLine> {
    let label = initial_prefix.content.trim_start_matches(' ').to_owned();
    let line = HyperlinkLine::new(Line::from(Span::from(text.to_string()).set_style(style)));
    let opts = RtOptions::new(width.max(1))
        .initial_indent(Line::from(vec![initial_prefix]))
        .subsequent_indent(Line::from(vec![subsequent_prefix]));
    let mut wrapped = crate::terminal_hyperlinks::adaptive_wrap_hyperlink_lines(&[line], opts);
    crate::terminal_hyperlinks::retain_initial_prefix(&mut wrapped, &label);
    wrapped
}

/// Split a request_user_input answer into option labels and an optional freeform note.
/// Notes are encoded as "user_note: <text>" entries in the answers list.
fn split_request_user_input_answer(
    answer: &ToolRequestUserInputAnswer,
) -> (Vec<String>, Option<String>) {
    let mut options = Vec::new();
    let mut note = None;
    for entry in &answer.answers {
        if let Some(note_text) = entry.strip_prefix("user_note: ") {
            note = Some(note_text.to_string());
        } else {
            options.push(entry.clone());
        }
    }
    (options, note)
}

#[cfg(test)]
#[path = "request_user_input_tests.rs"]
mod tests;
