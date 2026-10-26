//! Render structured replies on the user-message surface. Only the repeated question is shortened.

use super::messages::sanitize_user_text;
use crate::async_question_reply::AsyncQuestionReply;
use crate::line_truncation::truncate_line_to_width;
use crate::markdown_render::hide_web_link_destination;
use crate::markdown_render::render_markdown_lines_with_width_cwd_and_hidden_link_destinations;
use crate::style::accent_color_on;
use crate::style::history_prompt_style;
use crate::terminal_hyperlinks::HyperlinkLine;
use crate::terminal_hyperlinks::LogicalLineSource;
use crate::terminal_hyperlinks::prefix_hyperlink_lines;
use crate::terminal_hyperlinks::remap_source_wrapped_line;
use crate::terminal_hyperlinks::retain_initial_prefix;
use crate::wrapping::RtOptions;
use crate::wrapping::adaptive_wrap_line_to_width;
use ratatui::style::Stylize;
use std::path::Path;

pub(super) fn render(
    replies: &[AsyncQuestionReply],
    cwd: &Path,
    width: usize,
) -> Vec<HyperlinkLine> {
    let surface = history_prompt_style();
    let accent = accent_color_on(surface.bg);
    let markdown = |text: &str, width| {
        render_markdown_lines_with_width_cwd_and_hidden_link_destinations(
            text,
            width,
            Some(cwd),
            &hide_web_link_destination,
        )
    };
    let mut lines = Vec::new();
    for reply in replies {
        if !lines.is_empty() {
            lines.push(HyperlinkLine::default().style(surface));
        }
        let mut question = markdown(&sanitize_user_text(reply.question.as_str().into()), None);
        if question.is_empty() {
            question.push(HyperlinkLine::default());
        }
        let mut wrapped = question
            .into_iter()
            .enumerate()
            .flat_map(|(index, row)| wrap(row, width, if index == 0 { "Re: " } else { "" }));
        let mut caption: Vec<_> = wrapped.by_ref().take(2).collect();
        if wrapped.next().is_some()
            && let Some(last) = caption.last_mut()
        {
            last.line =
                truncate_line_to_width(std::mem::take(&mut last.line), width.saturating_sub(1));
            last.clip_hyperlinks_to_width(last.width());
            last.line.push_span("…");
            // Copy the visible excerpt as prose alongside the Markdown answer.
            caption.iter_mut().for_each(mark_as_prose);
        } else {
            retain_initial_prefix(&mut caption, "Re: ");
        }
        let caption = prefix_hyperlink_lines(caption, "↪ ".fg(accent).not_dim(), "  ".into());
        lines.extend(caption.into_iter().map(|row| {
            let style = surface.fg(accent).dim().patch(row.line.style);
            row.style(style).resolve_span_styles()
        }));
        let text = sanitize_user_text(reply.answer.as_str().into());
        if text.trim().is_empty() {
            continue;
        }
        let mut answer = markdown(&text, Some(width));
        if !answer
            .iter()
            .flat_map(|row| &row.line.spans)
            .any(|span| !span.content.trim().is_empty())
        {
            answer = text
                .lines()
                .map(str::to_owned)
                .map(HyperlinkLine::from)
                .collect();
            answer.iter_mut().for_each(mark_as_prose);
        }
        lines.push(HyperlinkLine::default().style(surface));
        let answer = answer
            .into_iter()
            .flat_map(|row| wrap(row, width, ""))
            .map(|row| {
                let style = row.line.style.patch(surface);
                row.style(style)
            })
            .collect();
        lines.extend(prefix_hyperlink_lines(answer, "  ".into(), "  ".into()));
    }
    lines
}

fn mark_as_prose(row: &mut HyperlinkLine) {
    row.source = Some(LogicalLineSource {
        copy_as_prose: true,
        ..LogicalLineSource::from_line(&row.line)
    });
}

// Markdown leaves some code and URLs overwide. Keep all of the row and its links in the gutter.
fn wrap(row: HyperlinkLine, width: usize, initial: &'static str) -> Vec<HyperlinkLine> {
    if initial.is_empty() && row.width() <= width {
        return vec![row];
    }
    let indent = row
        .source
        .as_ref()
        .map(|s| s.continuation_indent.clone())
        .unwrap_or_default();
    let options = RtOptions::new(width)
        .initial_indent(initial.into())
        .subsequent_indent(indent);
    remap_source_wrapped_line(&row, adaptive_wrap_line_to_width(&row.line, options))
}
