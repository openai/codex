use super::ChatWidget;
use crate::app_event::AppEvent;
use crate::color::blend;
use crate::color::is_light;
use crate::history_cell::CompositeHistoryCell;
use crate::history_cell::HistoryCell;
use crate::history_cell::PlainHistoryCell;
use crate::history_cell::plain_lines;
use crate::history_cell::with_border_with_inner_width;
use crate::style::accent_style;
use crate::terminal_palette::best_color;
use crate::terminal_palette::default_bg;
use crate::wrapping::RtOptions;
use crate::wrapping::word_wrap_lines;
use chrono::DateTime;
use chrono::Utc;
use codex_app_server_protocol::UsageContributorKind;
use codex_app_server_protocol::UsageEntry;
use codex_app_server_protocol::UsageRange;
use codex_app_server_protocol::UsageReadResponse;
use codex_app_server_protocol::UsageReport;
use ratatui::prelude::*;
use ratatui::style::Stylize;
use unicode_width::UnicodeWidthChar;
use unicode_width::UnicodeWidthStr;

const USAGE_CARD_MAX_INNER_WIDTH: usize = 72;
const USAGE_BAR_WIDTH: usize = 20;
const USAGE_BAR_MIN_LABEL_WIDTH: usize = 8;
const USAGE_BAR_GLYPH: &str = "▄";
const USAGE_TITLE: &str = "Usage by token share";
const USAGE_SUBTITLE: &str = "Percent of consumed tokens in this selected range";

impl ChatWidget {
    pub(crate) fn add_usage_output(&mut self) {
        self.add_usage_output_for_range(UsageRange::Day);
    }

    pub(crate) fn add_usage_output_for_range(&mut self, range: UsageRange) {
        self.request_usage(range);
    }

    fn request_usage(&mut self, range: UsageRange) {
        let request_id = self.next_usage_request_id;
        self.next_usage_request_id = self.next_usage_request_id.saturating_add(/*rhs*/ 1);
        self.active_usage_request_id = Some(request_id);
        self.app_event_tx
            .send(AppEvent::FetchUsage { request_id, range });
    }

    pub(crate) fn on_usage_loaded(
        &mut self,
        request_id: u64,
        result: Result<UsageReadResponse, String>,
    ) {
        if self.active_usage_request_id != Some(request_id) {
            return;
        }
        self.active_usage_request_id = None;
        let cell = match result {
            Ok(response) => new_usage_output(response.report),
            Err(err) => new_usage_error_output(err),
        };
        self.add_to_history(cell);
    }
}

#[derive(Debug)]
struct UsageHistoryCell {
    report: UsageReport,
}

#[derive(Debug)]
struct UsageErrorHistoryCell {
    error: String,
}

fn new_usage_output(report: UsageReport) -> CompositeHistoryCell {
    let command = PlainHistoryCell::new(vec![usage_command_label(report.range).magenta().into()]);
    CompositeHistoryCell::new(vec![
        Box::new(command),
        Box::new(UsageHistoryCell { report }),
    ])
}

fn new_usage_error_output(error: String) -> CompositeHistoryCell {
    let command = PlainHistoryCell::new(vec!["/usage".magenta().into()]);
    CompositeHistoryCell::new(vec![
        Box::new(command),
        Box::new(UsageErrorHistoryCell { error }),
    ])
}

impl HistoryCell for UsageHistoryCell {
    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        usage_report_lines(&self.report, width)
    }

    fn raw_lines(&self) -> Vec<Line<'static>> {
        plain_lines(self.display_lines(u16::MAX))
    }
}

impl HistoryCell for UsageErrorHistoryCell {
    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        let Some(available_width) = usage_card_available_width(width) else {
            return Vec::new();
        };
        let lines = vec![
            Line::from(USAGE_TITLE.bold()),
            Line::from(USAGE_SUBTITLE.dim()),
            Line::default(),
            Line::from(format!(" Failed to load usage: {}", self.error)),
        ];
        let inner_width = lines
            .iter()
            .map(line_display_width)
            .max()
            .unwrap_or(0)
            .min(available_width);
        with_border_with_inner_width(lines, inner_width)
    }

    fn raw_lines(&self) -> Vec<Line<'static>> {
        plain_lines(self.display_lines(u16::MAX))
    }
}

fn usage_report_lines(report: &UsageReport, width: u16) -> Vec<Line<'static>> {
    let Some(available_width) = usage_card_available_width(width) else {
        return Vec::new();
    };
    let sections = [
        ("Skills", report.skills.as_slice()),
        ("Subagents", report.subagents.as_slice()),
        ("Agent tasks", report.agent_tasks.as_slice()),
        ("Apps", report.apps.as_slice()),
        ("MCP servers", report.mcp_servers.as_slice()),
        ("Plugins", report.plugins.as_slice()),
    ];
    let label_column_width = usage_label_column_width(&sections);
    let inner_width = usage_content_width(report, &sections, label_column_width, available_width);
    let mut lines = usage_header_lines(report, inner_width);
    if let Some(headline) = report.headline.as_ref() {
        lines.push(Line::default());
        push_wrapped_line(
            &mut lines,
            vec![
                Span::from(format!(
                    "{}% of consumed tokens came from {} \"{}\"",
                    headline.entry.percent_of_usage,
                    contributor_kind_label(headline.entry.kind),
                    headline.entry.label
                ))
                .italic()
                .dim(),
            ],
            inner_width,
        );
        if let Some(note) = headline.note.as_ref() {
            push_wrapped_text(&mut lines, format!(" {note}"), inner_width);
        }
    }

    if report.total_tokens == 0 {
        lines.push(Line::default());
        push_wrapped_text(
            &mut lines,
            " No tracked usage in this range yet.",
            inner_width,
        );
        return with_border_with_inner_width(lines, inner_width);
    }

    if sections.iter().all(|(_, entries)| entries.is_empty()) {
        lines.push(Line::default());
        push_wrapped_text(
            &mut lines,
            " No attributed skills, subagents, agent tasks, apps, MCP servers, or plugins in this range.",
            inner_width,
        );
        return with_border_with_inner_width(lines, inner_width);
    }

    for (label, entries) in sections {
        push_section(&mut lines, label, entries, label_column_width, inner_width);
    }
    with_border_with_inner_width(lines, inner_width)
}

fn usage_card_available_width(width: u16) -> Option<usize> {
    (width >= 4).then(|| {
        usize::from(width.saturating_sub(/*rhs*/ 4)).min(USAGE_CARD_MAX_INNER_WIDTH)
    })
}

fn usage_label_column_width(sections: &[(&'static str, &[UsageEntry])]) -> usize {
    sections
        .iter()
        .flat_map(|(_, entries)| entries.iter())
        .map(|entry| UnicodeWidthStr::width(entry.label.as_str()))
        .max()
        .unwrap_or(0)
        .saturating_add(/*rhs*/ 3)
}

fn usage_content_width(
    report: &UsageReport,
    sections: &[(&'static str, &[UsageEntry])],
    label_column_width: usize,
    available_width: usize,
) -> usize {
    let mut width = 0usize;
    for line in usage_header_lines(report, available_width) {
        width = width.max(line_display_width(&line));
    }

    if let Some(headline) = report.headline.as_ref() {
        width = width.max(
            text_width(
                format!(
                    "{}% of consumed tokens came from {} \"{}\"",
                    headline.entry.percent_of_usage,
                    contributor_kind_label(headline.entry.kind),
                    headline.entry.label
                )
                .as_str(),
            )
            .min(available_width),
        );
    }

    if report.total_tokens == 0 {
        return width.min(available_width);
    }

    if sections.iter().all(|(_, entries)| entries.is_empty()) {
        return width.min(available_width);
    }

    for (section_label, entries) in sections {
        if entries.is_empty() {
            continue;
        }
        width = width.max(text_width(format!(" {section_label}").as_str()));
    }

    let prefix_width = text_width("  ├─ ");
    let percent_width = text_width("100%");
    let row_width_with_bar =
        prefix_width + label_column_width + USAGE_BAR_WIDTH + 2 + percent_width;
    let can_show_bar =
        available_width >= row_width_with_bar && label_column_width >= USAGE_BAR_MIN_LABEL_WIDTH;
    let row_width = if can_show_bar {
        row_width_with_bar
    } else {
        sections
            .iter()
            .flat_map(|(_, entries)| entries.iter())
            .map(|entry| prefix_width + text_width(entry.label.as_str()) + 1 + percent_width)
            .max()
            .unwrap_or(0)
    };
    width.max(row_width).min(available_width)
}

fn usage_header_lines(report: &UsageReport, inner_width: usize) -> Vec<Line<'static>> {
    match report.range {
        UsageRange::Day => vec![
            Line::from(usage_title(report.range).bold()),
            Line::from(USAGE_SUBTITLE.dim()),
            vec![
                "(".dim(),
                Span::styled("/usage week", accent_style()),
                " for weekly)".dim(),
            ]
            .into(),
        ],
        UsageRange::Week => {
            let Some(period) = usage_period_label(report) else {
                return vec![
                    Line::from(usage_title(report.range).bold()),
                    Line::from(USAGE_SUBTITLE.dim()),
                ];
            };
            let combined_title = format!("{}, {period}", usage_title(report.range));
            if text_width(&combined_title) <= inner_width {
                vec![
                    Line::from(combined_title.bold()),
                    Line::from(USAGE_SUBTITLE.dim()),
                ]
            } else {
                vec![
                    Line::from(usage_title(report.range).bold()),
                    Line::from(period),
                    Line::from(USAGE_SUBTITLE.dim()),
                ]
            }
        }
    }
}

fn usage_title(range: UsageRange) -> &'static str {
    match range {
        UsageRange::Day => "Daily usage by token share",
        UsageRange::Week => "Weekly usage by token share",
    }
}

fn usage_command_label(range: UsageRange) -> &'static str {
    match range {
        UsageRange::Day => "/usage",
        UsageRange::Week => "/usage week",
    }
}

fn usage_period_label(report: &UsageReport) -> Option<String> {
    let range_start = report
        .generated_at
        .saturating_sub(usage_range_seconds(report.range));
    let start = format_usage_date(range_start)?;
    let end = format_usage_date(report.generated_at)?;
    let label = format!("{start} to {end}");
    Some(label)
}

fn usage_range_seconds(range: UsageRange) -> i64 {
    match range {
        UsageRange::Day => 24 * 60 * 60,
        UsageRange::Week => 7 * 24 * 60 * 60,
    }
}

fn format_usage_date(seconds: i64) -> Option<String> {
    DateTime::<Utc>::from_timestamp(seconds, /*nsecs*/ 0)
        .map(|timestamp| timestamp.format("%b %-d").to_string())
}

fn push_wrapped_text(lines: &mut Vec<Line<'static>>, text: impl Into<String>, inner_width: usize) {
    push_wrapped_line(lines, Line::from(text.into()), inner_width);
}

fn push_wrapped_line(
    lines: &mut Vec<Line<'static>>,
    line: impl Into<Line<'static>>,
    inner_width: usize,
) {
    lines.extend(word_wrap_lines(
        [line.into()],
        RtOptions::new(inner_width.max(/*other*/ 1)).subsequent_indent(" ".into()),
    ));
}

fn push_section(
    lines: &mut Vec<Line<'static>>,
    label: &'static str,
    entries: &[UsageEntry],
    label_column_width: usize,
    inner_width: usize,
) {
    if entries.is_empty() {
        return;
    }
    lines.push(Line::default());
    lines.push(Line::from(Span::styled(
        format!(" {label}"),
        accent_style(),
    )));
    for (index, entry) in entries.iter().enumerate() {
        let is_last = index + 1 == entries.len();
        lines.push(usage_entry_line(
            entry,
            is_last,
            label_column_width,
            inner_width,
        ));
    }
}

fn usage_entry_line(
    entry: &UsageEntry,
    is_last: bool,
    label_column_width: usize,
    inner_width: usize,
) -> Line<'static> {
    let prefix = if is_last { "  └─ " } else { "  ├─ " };
    let percent = format!("{:>3}%", entry.percent_of_usage);
    let prefix_width = UnicodeWidthStr::width(prefix);
    let percent_width = UnicodeWidthStr::width(percent.as_str());
    let trailing_bar_width = USAGE_BAR_WIDTH + 2 + percent_width;
    let include_bar = inner_width >= prefix_width + label_column_width + trailing_bar_width
        && label_column_width >= USAGE_BAR_MIN_LABEL_WIDTH;
    let label_width = if include_bar {
        label_column_width
    } else {
        inner_width.saturating_sub(prefix_width + percent_width + 1)
    };
    let label = truncate_to_width(&entry.label, label_width);
    let used_width = if include_bar {
        prefix_width + label_column_width + trailing_bar_width
    } else {
        prefix_width + UnicodeWidthStr::width(label.as_str()) + percent_width
    };
    let spacer_width = if include_bar {
        0
    } else {
        inner_width.saturating_sub(used_width)
    };

    let mut spans = vec![
        Span::from(prefix).dim(),
        Span::from(label),
        Span::from(" ".repeat(spacer_width)).dim(),
    ];
    if include_bar {
        let label_padding = label_column_width
            .saturating_sub(UnicodeWidthStr::width(entry.label.as_str()).min(label_width));
        spans.push(Span::from(" ".repeat(label_padding)).dim());
        let (filled, empty) = usage_bar_segments(entry.percent_of_usage);
        spans.push(usage_bar_filled_span(USAGE_BAR_GLYPH.repeat(filled)));
        spans.push(usage_bar_empty_span(USAGE_BAR_GLYPH.repeat(empty)));
        spans.push(Span::from("  ").dim());
    }
    spans.push(Span::from(percent));
    Line::from(spans)
}

fn usage_bar_segments(percent: u8) -> (usize, usize) {
    let filled = if percent == 0 {
        0
    } else {
        ((usize::from(percent) * USAGE_BAR_WIDTH).saturating_add(99)) / 100
    }
    .min(USAGE_BAR_WIDTH);
    (filled, USAGE_BAR_WIDTH.saturating_sub(filled))
}

fn usage_bar_filled_span(content: String) -> Span<'static> {
    Span::styled(content, usage_bar_filled_style())
}

fn usage_bar_empty_span(content: String) -> Span<'static> {
    Span::styled(content, usage_bar_empty_style())
}

fn usage_bar_filled_style() -> Style {
    usage_bar_filled_style_for(default_bg())
}

fn usage_bar_empty_style() -> Style {
    usage_bar_empty_style_for(default_bg())
}

fn usage_bar_filled_style_for(terminal_bg: Option<(u8, u8, u8)>) -> Style {
    let Some(bg) = terminal_bg else {
        return Style::default().fg(Color::Cyan).bold();
    };
    if is_light(bg) {
        Style::default()
            .fg(usage_best_color(/*target*/ (0, 110, 125), Color::Cyan))
            .bold()
    } else {
        Style::default()
            .fg(usage_best_color(
                /*target*/ (170, 210, 218),
                Color::Cyan,
            ))
            .bold()
    }
}

fn usage_bar_empty_style_for(terminal_bg: Option<(u8, u8, u8)>) -> Style {
    let Some(bg) = terminal_bg else {
        return Style::default().fg(Color::DarkGray);
    };
    if is_light(bg) {
        Style::default().fg(usage_best_color(
            /*target*/ blend(/*fg*/ (0, 0, 0), bg, /*alpha*/ 0.18),
            Color::Gray,
        ))
    } else {
        Style::default().fg(usage_best_color(
            /*target*/ blend(/*fg*/ (255, 255, 255), bg, /*alpha*/ 0.3),
            Color::DarkGray,
        ))
    }
}

fn usage_best_color(target: (u8, u8, u8), fallback: Color) -> Color {
    let color = best_color(target);
    if color == Color::default() {
        fallback
    } else {
        color
    }
}

fn truncate_to_width(value: &str, width: usize) -> String {
    let mut out = String::new();
    let mut used = 0usize;
    for ch in value.chars() {
        let ch_width = UnicodeWidthChar::width(ch).unwrap_or(0);
        if used + ch_width > width {
            break;
        }
        out.push(ch);
        used += ch_width;
    }
    out
}

fn line_display_width(line: &Line<'static>) -> usize {
    line.iter()
        .map(|span| UnicodeWidthStr::width(span.content.as_ref()))
        .sum()
}

fn text_width(value: &str) -> usize {
    UnicodeWidthStr::width(value)
}

fn contributor_kind_label(kind: UsageContributorKind) -> &'static str {
    match kind {
        UsageContributorKind::Skill => "skill",
        UsageContributorKind::Subagent => "subagent",
        UsageContributorKind::AgentTask => "agent task",
        UsageContributorKind::App => "app",
        UsageContributorKind::McpServer => "MCP server",
        UsageContributorKind::Plugin => "plugin",
    }
}
