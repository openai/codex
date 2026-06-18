//! Renders account token usage summaries and activity charts for `/usage`.
//!
//! This module owns the chart data bucketing and ratatui line construction. The
//! async card lifecycle stays in the parent `tokens` module so chart rendering
//! remains a pure transformation from a loaded usage response.

mod palette;

use std::collections::BTreeMap;

use chrono::Datelike;
use chrono::Duration;
use chrono::NaiveDate;
use codex_app_server_protocol::GetAccountTokenUsageResponse;
use ratatui::style::Style;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;

use crate::render::highlight::foreground_style_for_scopes;
use crate::status::format_tokens_compact;
use palette::TokenActivityPalette;

const WEEK_COUNT: usize = 52;
const DAY_COUNT: usize = 7;
const CELL_COUNT: usize = WEEK_COUNT * DAY_COUNT;
const CHART_LEFT_WIDTH: usize = 4;
const SUMMARY_INDENT: &str = " ";
const SUMMARY_INDENT_WIDTH: u16 = 1;

/// Selects the aggregation represented by the token activity chart.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TokenActivityView {
    Daily,
    Weekly,
    Cumulative,
}

impl TokenActivityView {
    /// Parses the optional `/usage` argument into a supported chart view.
    ///
    /// An empty argument selects the daily view so `/usage` and `/usage daily`
    /// behave identically. Returning `None` lets the slash-command dispatcher
    /// report unsupported arguments instead of silently choosing a view.
    pub(in crate::chatwidget) fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "" | "day" | "daily" => Some(Self::Daily),
            "week" | "weekly" => Some(Self::Weekly),
            "cumulative" => Some(Self::Cumulative),
            _ => None,
        }
    }

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Daily => "Daily",
            Self::Weekly => "Weekly",
            Self::Cumulative => "Cumulative",
        }
    }
}

pub(super) fn loaded_lines(
    view: TokenActivityView,
    response: &GetAccountTokenUsageResponse,
    today: NaiveDate,
    width: u16,
) -> Vec<Line<'static>> {
    let mut lines = vec![
        vec![
            Span::from(" Token activity").bold(),
            Span::styled("   last 12 months", label_style()),
        ]
        .into(),
    ];
    lines.extend(summary_lines(response, graph_width(width)));
    // Separate the headline numbers from the calendar below.
    lines.push(Line::default());
    let Some(buckets) = response.daily_usage_buckets.as_ref() else {
        lines.push("   Token activity history unavailable".dim().into());
        return lines;
    };

    lines.extend(chart_lines(view, buckets, today, width));
    lines
}

fn chart_lines(
    view: TokenActivityView,
    buckets: &[codex_app_server_protocol::AccountTokenUsageDailyBucket],
    today: NaiveDate,
    width: u16,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let values = daily_values(buckets, today);
    let shown_columns = shown_columns(width);
    if shown_columns == 0 {
        lines.push("   Widen terminal to show activity graph".dim().into());
        return lines;
    }

    let palette = TokenActivityPalette::current();
    let levels = levels_for_view(&values, view);
    let first_column = WEEK_COUNT - shown_columns;
    lines.push(month_labels(today, first_column, shown_columns));
    for row in 0..DAY_COUNT {
        let mut spans = vec![weekday_label(view, row)];
        for column in first_column..WEEK_COUNT {
            if column > first_column {
                spans.push(" ".into());
            }
            let index = column * DAY_COUNT + row;
            if view == TokenActivityView::Daily
                && cell_date(today, index).is_some_and(|date| date > today)
            {
                spans.push(" ".into());
            } else {
                let style = if view == TokenActivityView::Daily {
                    palette.for_level(levels[index])
                } else {
                    palette.for_bar_level(levels[index])
                };
                spans.push(Span::styled(palette.glyph(view, levels[index]), style));
            }
        }
        lines.push(spans.into());
    }
    // Separate the calendar from the legend/footer below.
    lines.push(Line::default());
    match view {
        TokenActivityView::Daily => lines.push(legend_line(&palette)),
        TokenActivityView::Weekly | TokenActivityView::Cumulative => {
            lines.push(bar_caption(view, &values))
        }
    }
    lines.push(view_footer(view));
    lines
}

fn shown_columns(width: u16) -> usize {
    (usize::from(width)
        .saturating_sub(CHART_LEFT_WIDTH)
        .saturating_add(/*rhs*/ 1)
        / 2)
    .min(WEEK_COUNT)
}

fn graph_width(width: u16) -> u16 {
    if width == u16::MAX {
        return width;
    }
    (CHART_LEFT_WIDTH + shown_columns(width) * 2 - 1) as u16
}

fn summary_lines(response: &GetAccountTokenUsageResponse, width: u16) -> Vec<Line<'static>> {
    let summary = &response.summary;
    let fields = [
        ("Lifetime", format_optional_tokens(summary.lifetime_tokens)),
        ("Peak", format_optional_tokens(summary.peak_daily_tokens)),
        (
            "Streak",
            format_streak(summary.current_streak_days, summary.longest_streak_days),
        ),
        (
            "Longest task",
            format_optional_duration(summary.longest_running_turn_sec),
        ),
    ];
    pack_fields(&fields, width)
        .into_iter()
        .map(|group| align_summary_line(summary_line(&fields, &group), width))
        .collect()
}

/// Greedily pack summary fields into as few lines as fit `width`,
/// keeping field order. `u16::MAX` (raw/copy mode) always yields one line.
fn pack_fields(fields: &[(&str, String)], width: u16) -> Vec<Vec<usize>> {
    if width == u16::MAX {
        return vec![(0..fields.len()).collect()];
    }
    let max = usize::from(width.saturating_sub(SUMMARY_INDENT_WIDTH));
    let mut groups: Vec<Vec<usize>> = Vec::new();
    let mut current: Vec<usize> = Vec::new();
    for index in 0..fields.len() {
        let mut candidate = current.clone();
        candidate.push(index);
        if !current.is_empty() && summary_line(fields, &candidate).width() > max {
            groups.push(std::mem::take(&mut current));
            current.push(index);
        } else {
            current = candidate;
        }
    }
    if !current.is_empty() {
        groups.push(current);
    }
    groups
}

fn summary_line(fields: &[(&str, String)], indexes: &[usize]) -> Line<'static> {
    let mut spans = Vec::new();
    for (index, field_index) in indexes.iter().enumerate() {
        if index > 0 {
            spans.push(Span::styled(" · ", label_style()));
        }
        let (label, value) = &fields[*field_index];
        spans.push(Span::styled(format!("{label} "), label_style()));
        spans.push(Span::styled(value.clone(), numeric_style()));
    }
    spans.into()
}

fn align_summary_line(mut line: Line<'static>, width: u16) -> Line<'static> {
    if width == u16::MAX {
        return line;
    }
    line.spans.insert(/*index*/ 0, SUMMARY_INDENT.into());
    line
}

fn format_optional_tokens(value: Option<i64>) -> String {
    value
        .map(format_tokens_compact)
        .unwrap_or_else(|| "-".to_string())
}

/// Combine the current and longest streak into one field: a bare `54d` when
/// they match, otherwise `12d (best 54d)`.
fn format_streak(current: Option<i64>, longest: Option<i64>) -> String {
    match (current, longest) {
        (Some(current), Some(longest)) if current == longest => format!("{current}d"),
        (Some(current), Some(longest)) => format!("{current}d (best {longest}d)"),
        (Some(current), None) => format!("{current}d"),
        (None, Some(longest)) => format!("- (best {longest}d)"),
        (None, None) => "-".to_string(),
    }
}

fn format_optional_duration(value: Option<i64>) -> String {
    value.map_or_else(
        || "-".to_string(),
        |seconds| {
            let seconds = seconds.max(/*other*/ 0);
            let hours = seconds / 3600;
            let minutes = (seconds % 3600) / 60;
            match (hours, minutes) {
                (0, 0) => format!("{seconds}s"),
                (0, minutes) => format!("{minutes}m"),
                (hours, 0) => format!("{hours}h"),
                (hours, minutes) => format!("{hours}h {minutes}m"),
            }
        },
    )
}

fn numeric_style() -> Style {
    foreground_style_for_scopes(&["constant.numeric", "constant"])
        .unwrap_or_else(|| Style::default().green())
}

fn label_style() -> Style {
    foreground_style_for_scopes(&["comment"]).unwrap_or_else(|| Style::default().dim())
}

fn weekday_label(view: TokenActivityView, row: usize) -> Span<'static> {
    if view != TokenActivityView::Daily {
        // Bar views fill from the bottom (row 6) upward, so the gutter doubles
        // as a coarse Y-axis: peak at the top, baseline at the bottom.
        return Span::styled(
            match row {
                0 => "max ",
                6 => "  0 ",
                _ => "    ",
            },
            label_style(),
        );
    }
    Span::styled(
        match row {
            0 => " Su ",
            1 => " Mo ",
            2 => " Tu ",
            3 => " We ",
            4 => " Th ",
            5 => " Fr ",
            6 => " Sa ",
            _ => "    ",
        },
        label_style(),
    )
}

fn legend_line(palette: &TokenActivityPalette) -> Line<'static> {
    let mut spans = vec![Span::styled("   Less ", label_style())];
    for level in 0..=4 {
        if level > 0 {
            spans.push(" ".into());
        }
        spans.push(Span::styled(
            palette.glyph(TokenActivityView::Daily, level),
            palette.for_level(level),
        ));
    }
    spans.push(Span::styled(" More", label_style()));
    spans.into()
}

/// Caption for the bar-chart views, where the 5-step daily legend would be
/// misleading. States what each bar represents and the peak it is scaled to.
fn bar_caption(view: TokenActivityView, values: &[i64]) -> Line<'static> {
    let weeks = weekly_totals(values);
    let (lead, peak) = match view {
        TokenActivityView::Weekly => (
            "Each column = 1 week · tallest ",
            weeks.iter().copied().max().unwrap_or(/*default*/ 0),
        ),
        TokenActivityView::Cumulative => ("Running total · top ", weeks.iter().sum::<i64>()),
        TokenActivityView::Daily => ("", 0),
    };
    if peak <= 0 {
        return Span::styled("   No token activity in the last 12 months", label_style()).into();
    }
    vec![
        Span::styled(format!("   {lead}"), label_style()),
        Span::styled(format_tokens_compact(peak), numeric_style()),
    ]
    .into()
}

/// Dim footer that surfaces the other `/usage` views and emphasizes the
/// active one, so the weekly/cumulative modes are discoverable from the card.
fn view_footer(active: TokenActivityView) -> Line<'static> {
    let mut spans = vec![Span::styled("   ", label_style())];
    let views = [
        (TokenActivityView::Daily, "daily"),
        (TokenActivityView::Weekly, "weekly"),
        (TokenActivityView::Cumulative, "cumulative"),
    ];
    for (index, (view, name)) in views.into_iter().enumerate() {
        if index > 0 {
            spans.push(Span::styled(" · ", label_style()));
        }
        let style = if view == active {
            numeric_style().bold()
        } else {
            label_style()
        };
        spans.push(Span::styled(name, style));
    }
    spans.into()
}

fn month_labels(today: NaiveDate, first_column: usize, shown_columns: usize) -> Line<'static> {
    let mut cells = vec![' '; shown_columns * 2 - 1];
    let start = chart_start(today);
    let mut last_end = 0;
    for column in first_column..WEEK_COUNT {
        let date = start + Duration::days((column * DAY_COUNT) as i64);
        if date.day() > 7 {
            continue;
        }
        let label = date.format("%b").to_string();
        let offset = (column - first_column) * 2;
        if offset < last_end || offset + label.len() > cells.len() {
            continue;
        }
        for (index, ch) in label.chars().enumerate() {
            cells[offset + index] = ch;
        }
        last_end = offset + label.len() + 1;
    }
    vec![
        "    ".into(),
        Span::styled(cells.into_iter().collect::<String>(), label_style()),
    ]
    .into()
}

/// Normalizes backend daily buckets into the fixed 52-week display window.
///
/// The returned vector is ordered by chart cell, starting with the oldest Sunday.
/// Invalid, out-of-window, and future dates are ignored. Duplicate dates are
/// accumulated and negative token values do not reduce activity.
fn daily_values(
    buckets: &[codex_app_server_protocol::AccountTokenUsageDailyBucket],
    today: NaiveDate,
) -> Vec<i64> {
    let start = chart_start(today);
    let end = start + Duration::days(CELL_COUNT as i64);
    let mut by_date = BTreeMap::new();
    for bucket in buckets {
        let Ok(date) = NaiveDate::parse_from_str(&bucket.start_date, "%Y-%m-%d") else {
            continue;
        };
        if date < start || date >= end || date > today {
            continue;
        }
        *by_date.entry(date).or_insert(/*default*/ 0) += bucket.tokens.max(/*other*/ 0);
    }
    (0..CELL_COUNT)
        .map(|offset| {
            by_date
                .get(&(start + Duration::days(offset as i64)))
                .copied()
                .unwrap_or(/*default*/ 0)
        })
        .collect()
}

fn levels_for_view(values: &[i64], view: TokenActivityView) -> Vec<usize> {
    match view {
        TokenActivityView::Daily => graded_levels(values),
        TokenActivityView::Weekly => bar_levels(&weekly_totals(values)),
        TokenActivityView::Cumulative => {
            let cumulative = weekly_totals(values)
                .into_iter()
                .scan(/*initial_state*/ 0, |sum, value| {
                    *sum += value;
                    Some(*sum)
                })
                .collect::<Vec<_>>();
            bar_levels(&cumulative)
        }
    }
}

fn graded_levels(values: &[i64]) -> Vec<usize> {
    let max = values.iter().copied().max().unwrap_or(/*default*/ 0);
    values
        .iter()
        .map(|value| match (*value, max) {
            (0, _) | (_, 0) => 0,
            (value, max) if value * 4 > max * 3 => 4,
            (value, max) if value * 2 > max => 3,
            (value, max) if value * 4 > max => 2,
            _ => 1,
        })
        .collect()
}

fn weekly_totals(values: &[i64]) -> Vec<i64> {
    values
        .chunks(DAY_COUNT)
        .map(|week| week.iter().sum())
        .collect()
}

fn bar_levels(totals: &[i64]) -> Vec<usize> {
    let max = totals.iter().copied().max().unwrap_or(/*default*/ 0);
    totals
        .iter()
        .flat_map(|value| {
            let height = if *value <= 0 || max <= 0 {
                0
            } else {
                ((*value * DAY_COUNT as i64 + max - 1) / max) as usize
            };
            (0..DAY_COUNT).map(move |row| if DAY_COUNT - row <= height { 4 } else { 0 })
        })
        .collect()
}

fn chart_start(today: NaiveDate) -> NaiveDate {
    let week_start = today - Duration::days(i64::from(today.weekday().num_days_from_sunday()));
    week_start - Duration::weeks((WEEK_COUNT - 1) as i64)
}

fn cell_date(today: NaiveDate, index: usize) -> Option<NaiveDate> {
    chart_start(today).checked_add_signed(Duration::days(index as i64))
}

#[cfg(test)]
#[path = "chart_tests.rs"]
mod tests;
