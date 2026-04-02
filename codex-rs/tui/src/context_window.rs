use codex_app_server_protocol::ThreadContextWindowBreakdown;
use codex_app_server_protocol::ThreadContextWindowDetail;
use codex_app_server_protocol::ThreadContextWindowSection;
use ratatui::prelude::Line;
use ratatui::prelude::Span;
use ratatui::style::Style;
use ratatui::style::Stylize;
use std::cmp::Reverse;

use crate::exec_cell::spinner;
use crate::history_cell::HistoryCell;
use crate::render::line_utils::line_to_static;
use crate::wrapping::RtOptions;
use crate::wrapping::adaptive_wrap_line;

const CONTEXT_WINDOW_LABEL: &str = "/context";
const CONTEXT_BAR_LABEL_WIDTH: usize = 6;
const MIN_BAR_WIDTH: usize = 18;
const MAX_BAR_WIDTH: usize = 44;
const SECTION_BAR_WIDTH: usize = 10;

pub(crate) fn new_context_window_output(
    context: &ThreadContextWindowBreakdown,
) -> ContextWindowOutputCell {
    ContextWindowOutputCell {
        context: context.clone(),
    }
}

#[derive(Debug)]
pub(crate) struct ContextWindowOutputCell {
    context: ThreadContextWindowBreakdown,
}

impl HistoryCell for ContextWindowOutputCell {
    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        render_context_window_output(&self.context, width)
    }
}

#[derive(Debug)]
pub(crate) struct ContextWindowLoadingCell {
    created_at: std::time::Instant,
    animations_enabled: bool,
}

impl ContextWindowLoadingCell {
    fn new(animations_enabled: bool) -> Self {
        Self {
            created_at: std::time::Instant::now(),
            animations_enabled,
        }
    }
}

impl HistoryCell for ContextWindowLoadingCell {
    fn display_lines(&self, _width: u16) -> Vec<Line<'static>> {
        vec![
            CONTEXT_WINDOW_LABEL.magenta().into(),
            "".into(),
            vec![
                if self.animations_enabled {
                    spinner(Some(self.created_at), self.animations_enabled)
                } else {
                    "•".into()
                },
                " Loading context breakdown...".dim(),
            ]
            .into(),
        ]
    }

    fn transcript_animation_tick(&self) -> Option<u64> {
        self.animations_enabled
            .then(|| self.created_at.elapsed().as_millis() as u64 / 80)
    }
}

pub(crate) fn new_context_window_loading(animations_enabled: bool) -> ContextWindowLoadingCell {
    ContextWindowLoadingCell::new(animations_enabled)
}

fn format_context_summary(context: &ThreadContextWindowBreakdown) -> String {
    match context.model_context_window {
        Some(model_context_window) if model_context_window > 0 => {
            let remaining = model_context_window
                .saturating_sub(context.total_tokens)
                .max(0);
            let used_percent = percentage(context.total_tokens, model_context_window);
            format!(
                "{} used of {} ({used_percent:.1}%), {} remaining",
                format_tokens(context.total_tokens),
                format_tokens(model_context_window),
                format_tokens(remaining),
            )
        }
        _ => format!("{} used", format_tokens(context.total_tokens)),
    }
}

fn render_context_window_output(
    context: &ThreadContextWindowBreakdown,
    width: u16,
) -> Vec<Line<'static>> {
    let bar_width = context_bar_width(width);
    let mut sections = context.sections.iter().collect::<Vec<_>>();
    sections.sort_by_key(|section| Reverse(section.tokens.max(0)));

    let mut lines = vec![
        CONTEXT_WINDOW_LABEL.magenta().into(),
        "".into(),
        "Context map".bold().into(),
        format!("  {}.", format_context_summary(context)).into(),
        render_context_window_bar(
            "window",
            &sections,
            context
                .model_context_window
                .filter(|model_context_window| *model_context_window > 0)
                .unwrap_or(context.total_tokens.max(0)),
            bar_width,
        ),
    ];

    if !sections.is_empty() {
        lines.push(render_context_window_bar(
            "used",
            &sections,
            context.total_tokens.max(0),
            bar_width,
        ));
        lines.push("".into());
        lines.extend(render_context_window_legend(&sections, context, width));
    }

    if context.sections.is_empty() {
        lines.push("".into());
        lines.push("  • No context rows found.".italic().into());
        return lines;
    }

    let label_width = sections
        .iter()
        .map(|section| section.label.chars().count())
        .max()
        .unwrap_or(0)
        .clamp(8, 18);
    for section in sections {
        lines.push("".into());
        lines.push(render_section_header(section, context, label_width));
        if section.details.is_empty() {
            lines.push("    • <none>".dim().into());
            continue;
        }
        for detail in &section.details {
            lines.extend(render_detail_lines(
                detail,
                section.tokens,
                &section.label,
                width,
            ));
        }
    }

    lines
}

fn render_context_window_bar(
    label: &str,
    sections: &[&ThreadContextWindowSection],
    scale_total: i64,
    bar_width: usize,
) -> Line<'static> {
    let section_tokens = sections
        .iter()
        .map(|section| section.tokens.max(0))
        .collect::<Vec<_>>();
    let section_widths = allocate_token_widths(&section_tokens, scale_total, bar_width);

    let mut spans = vec![
        format!("  {label:<CONTEXT_BAR_LABEL_WIDTH$}").dim(),
        "▕".dim(),
    ];
    for (section, width) in sections.iter().zip(section_widths) {
        if width > 0 {
            spans.push(Span::styled(
                "█".repeat(width),
                section_style(&section.label),
            ));
        }
    }

    let used_width = spans
        .iter()
        .skip(1)
        .map(|span| span.content.chars().count())
        .sum::<usize>();
    if used_width < bar_width {
        spans.push("░".repeat(bar_width - used_width).dim());
    }
    spans.push("▏".dim());
    spans.into()
}

fn render_context_window_legend(
    sections: &[&ThreadContextWindowSection],
    context: &ThreadContextWindowBreakdown,
    width: u16,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    for section in sections {
        let mut spans = vec!["  ".into(), section_marker(&section.label), " ".into()];
        spans.push(Span::styled(
            section.label.clone(),
            section_style(&section.label),
        ));
        if context.total_tokens > 0 {
            spans.push(format!(" {:.1}%", percentage(section.tokens, context.total_tokens)).dim());
        }
        lines.extend(
            adaptive_wrap_line(
                &Line::from(spans),
                RtOptions::new(width as usize)
                    .initial_indent("".into())
                    .subsequent_indent("    ".into()),
            )
            .into_iter()
            .map(|line| line_to_static(&line)),
        );
    }
    lines
}

fn render_section_header(
    section: &ThreadContextWindowSection,
    context: &ThreadContextWindowBreakdown,
    label_width: usize,
) -> Line<'static> {
    let mut spans = vec![
        "  ".into(),
        section_marker(&section.label),
        " ".into(),
        Span::styled(
            format!("{:<label_width$}", section.label),
            section_style(&section.label),
        ),
        "  ".into(),
        format!("{:>14}", format_tokens(section.tokens)).dim(),
    ];
    if context.total_tokens > 0 {
        spans.push(
            format!(
                "  {:>5.1}% of used",
                percentage(section.tokens, context.total_tokens)
            )
            .dim(),
        );
    }
    spans.into()
}

fn render_detail_lines(
    detail: &ThreadContextWindowDetail,
    section_tokens: i64,
    section_label: &str,
    width: u16,
) -> Vec<Line<'static>> {
    let active_width =
        allocate_token_widths(&[detail.tokens], section_tokens.max(0), SECTION_BAR_WIDTH)[0];
    let line = Line::from(vec![
        "    ".into(),
        Span::styled("█".repeat(active_width), section_style(section_label)),
        "░".repeat(SECTION_BAR_WIDTH - active_width).dim(),
        " ".into(),
        detail.label.clone().into(),
        " ".into(),
        format!("({})", format_tokens(detail.tokens)).dim(),
    ]);
    adaptive_wrap_line(
        &line,
        RtOptions::new(width as usize)
            .initial_indent("".into())
            .subsequent_indent("      ".into()),
    )
    .into_iter()
    .map(|line| line_to_static(&line))
    .collect()
}

fn context_bar_width(width: u16) -> usize {
    usize::from(width.saturating_sub(10)).clamp(MIN_BAR_WIDTH, MAX_BAR_WIDTH)
}

fn section_marker(label: &str) -> Span<'static> {
    Span::styled("■", section_style(label))
}

fn section_style(label: &str) -> Style {
    match label {
        "Conversation" => Style::new().cyan().bold(),
        "Skills" => Style::new().green().bold(),
        "AGENTS.md" => Style::new().magenta().bold(),
        "Runtime context" => Style::new().red().bold(),
        "Built-in" => Style::new().fg(ratatui::style::Color::Yellow).bold(),
        _ => Style::new(),
    }
}

fn allocate_token_widths(tokens: &[i64], total_tokens: i64, width: usize) -> Vec<usize> {
    if tokens.is_empty() {
        return Vec::new();
    }
    if total_tokens <= 0 || width == 0 {
        return vec![0; tokens.len()];
    }

    let total_tokens = total_tokens as u128;
    let width = width as u128;
    let represented_tokens = tokens
        .iter()
        .map(|tokens| tokens.max(&0))
        .map(|tokens| *tokens as u128)
        .sum::<u128>()
        .min(total_tokens);
    let mut represented_width = (represented_tokens * width / total_tokens) as usize;
    if represented_tokens > 0 && represented_width == 0 {
        represented_width = 1;
    }

    let mut allocations = Vec::with_capacity(tokens.len());
    let mut assigned_width = 0usize;
    let mut remainders = Vec::new();

    for (index, tokens) in tokens.iter().enumerate() {
        let tokens = tokens.max(&0);
        let scaled_width = (*tokens as u128) * width;
        let cell_width = (scaled_width / total_tokens) as usize;
        assigned_width = assigned_width.saturating_add(cell_width);
        allocations.push(cell_width);
        remainders.push((index, scaled_width % total_tokens, *tokens));
    }

    let mut remaining_width = represented_width.saturating_sub(assigned_width);
    remainders
        .sort_by_key(|(index, remainder, tokens)| (Reverse(*remainder), Reverse(*tokens), *index));
    for (index, _, tokens) in remainders {
        if remaining_width == 0 || tokens <= 0 {
            break;
        }
        allocations[index] = allocations[index].saturating_add(1);
        remaining_width -= 1;
    }

    allocations
}

fn format_tokens(tokens: i64) -> String {
    format!("~{} tokens", format_number(tokens.max(0)))
}

fn percentage(numerator: i64, denominator: i64) -> f64 {
    if denominator <= 0 {
        return 0.0;
    }
    100.0 * numerator.max(0) as f64 / denominator as f64
}

fn format_number(value: i64) -> String {
    let mut digits = value.abs().to_string();
    let mut formatted = String::new();
    while digits.len() > 3 {
        let tail = digits.split_off(digits.len() - 3);
        if formatted.is_empty() {
            formatted = tail;
        } else {
            formatted = format!("{tail},{formatted}");
        }
    }
    if formatted.is_empty() {
        formatted = digits;
    } else if !digits.is_empty() {
        formatted = format!("{digits},{formatted}");
    }
    if value < 0 {
        format!("-{formatted}")
    } else {
        formatted
    }
}

#[cfg(test)]
mod tests {
    use insta::assert_snapshot;
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn renders_default_context_window_breakdown() {
        let cell = new_context_window_output(&ThreadContextWindowBreakdown {
            model_context_window: Some(200_000),
            total_tokens: 20_000,
            sections: vec![
                ThreadContextWindowSection {
                    label: "Skills".to_string(),
                    tokens: 12_000,
                    details: vec![ThreadContextWindowDetail {
                        label: "Implicit skills catalog (42 skills)".to_string(),
                        tokens: 12_000,
                    }],
                },
                ThreadContextWindowSection {
                    label: "Conversation".to_string(),
                    tokens: 8_000,
                    details: vec![
                        ThreadContextWindowDetail {
                            label: "Tool output".to_string(),
                            tokens: 5_000,
                        },
                        ThreadContextWindowDetail {
                            label: "User message".to_string(),
                            tokens: 3_000,
                        },
                    ],
                },
            ],
        });

        assert_snapshot!(render_lines(&cell.display_lines(/*width*/ 80)));
    }

    #[test]
    fn allocates_bar_width_by_largest_remainder() {
        assert_eq!(allocate_token_widths(&[5, 3, 2], 10, 7), vec![4, 2, 1]);
    }

    fn render_lines(lines: &[Line<'static>]) -> String {
        let mut rendered = Vec::new();
        for line in lines {
            let mut text = String::new();
            for span in &line.spans {
                text.push_str(&span.content);
            }
            rendered.push(text);
        }
        rendered.join("\n")
    }
}
