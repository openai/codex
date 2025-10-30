#[cfg(feature = "code-fork")]
use crate::foundation::palette as colors;
#[cfg(not(feature = "code-fork"))]
use crate::colors;
use chrono::{DateTime, Datelike, Local, Utc};
use code_common::elapsed::format_duration;
use code_core::protocol::RateLimitSnapshotEvent;
use code_protocol::num_format::format_with_separators;
use ratatui::prelude::*;
use ratatui::style::Stylize;
use std::time::Duration;

const WEEKLY_CELL: &str = "▇▇";
const HOURLY_CELL: &str = "▓▓";
const UNUSED_CELL: &str = "░░";
const BAR_SLOTS: usize = 20;
const BAR_FILLED: &str = "▰";
const BAR_EMPTY: &str = "▱";
const CHART_LINE_PREFIX: &str = "    ";
struct IndentSpec {
    header: &'static str,
    label_extra: &'static str,
    label_target_width: usize,
    label_gap: usize,
}

const INDENTS: IndentSpec = IndentSpec {
    header: "",
    label_extra: "   ",
    label_target_width: 7,
    label_gap: 2,
};

fn header_indent() -> &'static str {
    INDENTS.header
}

fn label_indent() -> String {
    format!("{}{}", INDENTS.header, INDENTS.label_extra)
}

fn chart_indent() -> String {
    CHART_LINE_PREFIX.to_string()
}

fn chart_indent_width() -> usize {
    CHART_LINE_PREFIX.len()
}

fn content_column_width() -> usize {
    CHART_LINE_PREFIX.len()
}

fn label_text(text: &str) -> String {
    let mut result = label_indent();
    result.push_str(text);
    result
}

/// Aggregated output used by the `/limits` command.
/// It contains the rendered summary lines, optional legend,
/// and the precomputed gauge state when one can be shown.
#[derive(Clone, Debug)]
pub(crate) struct LimitsView {
    pub(crate) summary_lines: Vec<Line<'static>>,
    pub(crate) legend_lines: Vec<Line<'static>>,
    pub(crate) footer_lines: Vec<Line<'static>>,
    grid_state: Option<GridState>,
    grid: GridConfig,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct RateLimitDisplayConfig {
    pub(crate) show_usage_sections: bool,
    pub(crate) show_chart: bool,
}

impl Default for RateLimitDisplayConfig {
    fn default() -> Self {
        Self {
            show_usage_sections: true,
            show_chart: true,
        }
    }
}

pub(crate) const DEFAULT_DISPLAY_CONFIG: RateLimitDisplayConfig = RateLimitDisplayConfig {
    show_usage_sections: true,
    show_chart: true,
};

impl LimitsView {
    pub(crate) fn lines_for_width(&self, width: u16) -> Vec<Line<'static>> {
        let mut lines = self.summary_lines.clone();
        lines.extend(self.gauge_lines(width));
        lines.extend(self.legend_lines.clone());
        lines.extend(self.footer_lines.clone());
        lines
    }

    pub(crate) fn gauge_lines(&self, width: u16) -> Vec<Line<'static>> {
        self.grid_state
            .filter(|state| state.weekly_used_ratio.is_finite())
            .map(|state| render_limit_grid(state, self.grid, width))
            .unwrap_or_default()
    }
}

/// Configuration for the simple grid gauge rendered by `/limits`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct GridConfig {
    pub(crate) weekly_slots: usize,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct RateLimitResetInfo {
    pub(crate) primary_next_reset: Option<DateTime<Utc>>,
    pub(crate) secondary_next_reset: Option<DateTime<Utc>>,
    pub(crate) auto_compact_tokens_used: Option<u64>,
    pub(crate) auto_compact_limit: Option<u64>,
    pub(crate) overflow_auto_compact: bool,
    pub(crate) context_window: Option<u64>,
    pub(crate) context_tokens_used: Option<u64>,
}

/// Default gauge configuration used by the TUI.
pub(crate) const DEFAULT_GRID_CONFIG: GridConfig = GridConfig {
    weekly_slots: 100,
};

/// Build the lines and optional gauge used by the `/limits` view.
pub(crate) fn build_limits_view(
    snapshot: &RateLimitSnapshotEvent,
    reset_info: RateLimitResetInfo,
    grid_config: GridConfig,
    display_config: RateLimitDisplayConfig,
) -> LimitsView {
    let metrics = RateLimitMetrics::from_snapshot(snapshot);
    let grid_state = if display_config.show_chart && gauge_inputs_available(snapshot) {
        extract_capacity_fraction(snapshot)
            .and_then(|fraction| compute_grid_state(&metrics, fraction))
            .map(|state| scale_grid_state(state, grid_config))
    } else {
        None
    };

    LimitsView {
        summary_lines: build_summary_lines(&metrics, snapshot, &reset_info, display_config),
        legend_lines: build_legend_lines(grid_state.is_some()),
        footer_lines: build_footer_lines(&metrics),
        grid_state,
        grid: grid_config,
    }
}

#[derive(Debug)]
struct RateLimitMetrics {
    hourly_used: f64,
    weekly_used: f64,
    hourly_remaining: f64,
    weekly_remaining: f64,
    primary_window_minutes: u64,
    weekly_window_minutes: u64,
}

impl RateLimitMetrics {
    fn from_snapshot(snapshot: &RateLimitSnapshotEvent) -> Self {
        let hourly_used = snapshot.primary_used_percent.clamp(0.0, 100.0);
        let weekly_used = snapshot.secondary_used_percent.clamp(0.0, 100.0);
        Self {
            hourly_used,
            weekly_used,
            hourly_remaining: (100.0 - hourly_used).max(0.0),
            weekly_remaining: (100.0 - weekly_used).max(0.0),
            primary_window_minutes: snapshot.primary_window_minutes,
            weekly_window_minutes: snapshot.secondary_window_minutes,
        }
    }

    fn hourly_exhausted(&self) -> bool {
        self.hourly_remaining <= 0.0
    }

    fn weekly_exhausted(&self) -> bool {
        self.weekly_remaining <= 0.0
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct GridState {
    weekly_used_ratio: f64,
    hourly_remaining_ratio: f64,
}

fn build_summary_lines(
    metrics: &RateLimitMetrics,
    snapshot: &RateLimitSnapshotEvent,
    reset_info: &RateLimitResetInfo,
    display: RateLimitDisplayConfig,
) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();
    lines.push("/limits".magenta().into());
    lines.push("".into());

    if display.show_usage_sections {
        lines.push(section_header("Hourly Limit"));
        lines.push(build_bar_line(
            "Used",
            metrics.hourly_used,
            "",
            Style::default().fg(colors::text()),
        ));
        lines.push(build_hourly_window_line(
            metrics,
            reset_info.primary_next_reset,
        ));
        lines.push(build_hourly_reset_line(
            snapshot.primary_window_minutes,
            reset_info.primary_next_reset,
        ));

        lines.push("".into());

        lines.push(section_header("Weekly Limit"));
        lines.push(build_bar_line(
            "Usage",
            metrics.weekly_used,
            "",
            Style::default().fg(colors::text()),
        ));
        lines.push(build_weekly_window_line(
            metrics.weekly_window_minutes,
            reset_info.secondary_next_reset,
        ));
        lines.push(build_weekly_reset_line(
            snapshot.secondary_window_minutes,
            reset_info.secondary_next_reset,
        ));

        lines.push("".into());
    }

    lines.extend(build_compact_lines(reset_info));

    if display.show_chart {
        lines.push("".into());
        lines.push(section_header("Chart"));
    }

    lines
}

fn build_footer_lines(metrics: &RateLimitMetrics) -> Vec<Line<'static>> {
    vec!["".into(), build_status_line(metrics)]
}

fn section_header(title: &str) -> Line<'static> {
    let mut text = String::with_capacity(header_indent().len() + title.len());
    text.push_str(header_indent());
    text.push_str(title);
    Line::from(vec![Span::styled(
        text,
        Style::default().add_modifier(Modifier::BOLD),
    )])
}

fn build_bar_line(label: &str, percent: f64, suffix: &str, style: Style) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    spans.push(Span::raw(field_prefix(label)));
    spans.extend(render_percent_bar(percent));
    let mut text = format_percent(percent);
    if !suffix.is_empty() {
        text.push_str(suffix);
    }
    spans.push(Span::styled(format!(" {text}"), style));
    Line::from(spans)
}

fn build_hourly_window_line(
    metrics: &RateLimitMetrics,
    next_reset: Option<DateTime<Utc>>,
) -> Line<'static> {
    let prefix = field_prefix("Window");
    if metrics.primary_window_minutes == 0 {
        return Line::from(vec![
            Span::raw(prefix),
            Span::styled(
                "window length unavailable".to_string(),
                Style::default().fg(colors::dim()),
            ),
        ]);
    }

    if let Some(next) = next_reset {
        if let Some(timing) = compute_window_timing(metrics.primary_window_minutes, next) {
            let window_secs = timing.window.as_secs_f64();
            if window_secs > 0.0 {
                let elapsed = timing.elapsed();
                let percent = ((elapsed.as_secs_f64() / window_secs) * 100.0).clamp(0.0, 100.0);
                let mut spans: Vec<Span<'static>> = Vec::new();
                spans.push(Span::raw(prefix.clone()));
                spans.extend(render_percent_bar(percent));
                spans.push(Span::styled(
                    format!(" {}", format_percent(percent)),
                    Style::default().fg(colors::text()),
                ));
                let elapsed_display = format_duration(elapsed);
                let total_display =
                    format_minutes_round_units(metrics.primary_window_minutes);
                spans.push(Span::styled(
                    format!(" ({elapsed_display} / {total_display})"),
                    Style::default().fg(colors::dim()),
                ));
                return Line::from(spans);
            }
        }
    }

    let mut spans: Vec<Span<'static>> = Vec::new();
    spans.push(Span::raw(prefix));
    spans.push(Span::styled(
        format!(
            "(unknown / {})",
            format_minutes_round_units(metrics.primary_window_minutes)
        ),
        Style::default().fg(colors::dim()),
    ));
    Line::from(spans)
}

fn build_hourly_reset_line(
    window_minutes: u64,
    next_reset: Option<DateTime<Utc>>,
) -> Line<'static> {
    if let (Some(next), true) = (next_reset, window_minutes > 0) {
        let prefix = field_prefix("Resets");
        if let Some(timing) = compute_window_timing(window_minutes, next) {
            let remaining = format_duration(timing.remaining);
            let mut spans: Vec<Span<'static>> = Vec::new();
            spans.push(Span::raw(prefix.clone()));
            let time_display = format_reset_timestamp(timing.next_reset_local, false);
            spans.push(Span::raw("at "));
            spans.push(Span::raw(time_display));
            spans.push(Span::styled(
                format!(" (in {remaining})"),
                Style::default().fg(colors::dim()),
            ));
            return Line::from(spans);
        }
        return Line::from(vec![
            Span::raw(prefix),
            Span::styled(
                "timing updating…".to_string(),
                Style::default().fg(colors::dim()),
            ),
        ]);
    }
    let mut spans: Vec<Span<'static>> = Vec::new();
    spans.push(Span::raw(field_prefix("Resets")));
    spans.push(Span::styled(
        "awaiting reset timing…".to_string(),
        Style::default().fg(colors::dim()),
    ));
    Line::from(spans)
}

fn build_weekly_window_line(
    weekly_minutes: u64,
    next_reset: Option<DateTime<Utc>>,
) -> Line<'static> {
    let prefix = field_prefix("Window");
    if weekly_minutes == 0 {
        return Line::from(vec![
            Span::raw(prefix),
            Span::styled(
                "window length unavailable".to_string(),
                Style::default().fg(colors::dim()),
            ),
        ]);
    }

    if let Some(next) = next_reset {
        if let Some(timing) = compute_window_timing(weekly_minutes, next) {
            let window_secs = timing.window.as_secs_f64();
            if window_secs > 0.0 {
                let elapsed = timing.elapsed();
                let percent = ((elapsed.as_secs_f64() / window_secs) * 100.0).clamp(0.0, 100.0);
                let mut spans: Vec<Span<'static>> = Vec::new();
                spans.push(Span::raw(prefix.clone()));
                spans.extend(render_percent_bar(percent));
                spans.push(Span::styled(
                    format!(" {}", format_percent(percent)),
                    Style::default().fg(colors::text()),
                ));
                let elapsed_display = format_duration(elapsed);
                let total_display = format_minutes_round_units(weekly_minutes);
                spans.push(Span::styled(
                    format!(" ({elapsed_display} / {total_display})"),
                    Style::default().fg(colors::dim()),
                ));
                return Line::from(spans);
            }
        }
    }

    Line::from(vec![
        Span::raw(prefix),
        Span::styled(
            format!(
                "(unknown / {})",
                format_minutes_round_units(weekly_minutes)
            ),
            Style::default().fg(colors::dim()),
        ),
    ])
}

fn build_weekly_reset_line(
    window_minutes: u64,
    next_reset: Option<DateTime<Utc>>,
) -> Line<'static> {
    if let (Some(next), true) = (next_reset, window_minutes > 0) {
        let prefix = field_prefix("Resets");
        if let Some(timing) = compute_window_timing(window_minutes, next) {
            let remaining = format_duration(timing.remaining);
            let mut spans: Vec<Span<'static>> = Vec::new();
            spans.push(Span::raw(prefix.clone()));
            let detailed_display = format_reset_timestamp(timing.next_reset_local, true);
            spans.push(Span::raw(detailed_display));
            spans.push(Span::styled(
                format!(" (in {remaining})"),
                Style::default().fg(colors::dim()),
            ));
            return Line::from(spans);
        }
        return Line::from(vec![
            Span::raw(prefix),
            Span::styled(
                "timing updating…".to_string(),
                Style::default().fg(colors::dim()),
            ),
        ]);
    }
    let mut spans: Vec<Span<'static>> = Vec::new();
    spans.push(Span::raw(field_prefix("Resets")));
    spans.push(Span::styled(
        "awaiting reset timing…".to_string(),
        Style::default().fg(colors::dim()),
    ));
    Line::from(spans)
}

fn build_compact_lines(reset_info: &RateLimitResetInfo) -> Vec<Line<'static>> {
    let mut detail_lines: Vec<Line<'static>> = Vec::new();

    match reset_info.auto_compact_limit {
        Some(limit) => {
            if let Some(used) = reset_info.auto_compact_tokens_used {
                detail_lines.push(build_compact_tokens_line(used, limit));
                detail_lines.push(build_compact_status_line(used, limit));
            } else {
                detail_lines.push(Line::from(vec![Span::styled(
                    label_text("Session usage updating…"),
                    Style::default().fg(colors::dim()),
                )]));
            }
        }
        None => {
            if let (Some(window), Some(used)) =
                (reset_info.context_window, reset_info.context_tokens_used)
            {
                detail_lines.push(build_context_tokens_line(used, window));
                detail_lines.push(build_context_status_line(
                    used,
                    window,
                    reset_info.overflow_auto_compact,
                ));
            } else if reset_info.overflow_auto_compact {
                detail_lines.push(Line::from(vec![Span::styled(
                    label_text("Auto-compaction runs after overflow errors"),
                    Style::default().fg(colors::dim()),
                )]));
            } else {
                // No compact data available for this snapshot (likely a historic account view).
                // Hide the section entirely instead of rendering the "unavailable" placeholder.
                return Vec::new();
            }
        }
    }

    let mut lines: Vec<Line<'static>> = Vec::new();
    if !detail_lines.is_empty() {
        lines.push(section_header("Compact Limit"));
        lines.extend(detail_lines);
    }
    lines
}

fn build_compact_tokens_line(used: u64, limit: u64) -> Line<'static> {
    let percent = if limit == 0 {
        0.0
    } else {
        (used as f64 / limit as f64) * 100.0
    };
    let percent_display = if percent > 100.0 {
        format!("{percent:.0}%")
    } else {
        format_percent(percent)
    };

    let mut spans: Vec<Span<'static>> = Vec::new();
    spans.push(Span::raw(field_prefix("Tokens")));
    spans.extend(render_percent_bar(percent));
    spans.push(Span::styled(
        format!(" {percent_display}"),
        Style::default().fg(colors::text()),
    ));

    let used_fmt = format_with_separators(used);
    let limit_fmt = format_with_separators(limit);
    spans.push(Span::styled(
        format!(" ({used_fmt} / {limit_fmt})"),
        Style::default().fg(colors::dim()),
    ));

    Line::from(spans)
}

fn build_compact_status_line(used: u64, limit: u64) -> Line<'static> {
    if used < limit {
        let remaining = limit - used;
        return Line::from(vec![
            Span::raw(field_prefix("Status")),
            Span::styled(
                format!(
                    "{} tokens before compact",
                    format_with_separators(remaining)
                ),
                Style::default().fg(colors::dim()),
            ),
        ]);
    }

    if used == limit {
        return Line::from(vec![
            Span::raw(field_prefix("Status")),
            Span::styled(
                "Auto-compact will trigger on the next turn".to_string(),
                Style::default().fg(colors::warning()),
            ),
        ]);
    }

    let overage = used.saturating_sub(limit);
    Line::from(vec![
        Span::raw(field_prefix("Status")),
        Span::styled(
            format!("Exceeded by {} tokens", format_with_separators(overage)),
            Style::default().fg(colors::error()),
        ),
    ])
}

fn build_context_tokens_line(used: u64, window: u64) -> Line<'static> {
    let percent = if window == 0 {
        0.0
    } else {
        (used as f64 / window as f64) * 100.0
    };
    let mut spans: Vec<Span<'static>> = Vec::new();
    spans.push(Span::raw(field_prefix("Context")));
    spans.extend(render_percent_bar(percent));
    let percent_display = if percent > 100.0 {
        format!("{percent:.0}%")
    } else {
        format_percent(percent)
    };
    spans.push(Span::styled(
        format!(" {percent_display}"),
        Style::default().fg(colors::text()),
    ));
    spans.push(Span::styled(
        format!(
            " ({} / {})",
            format_with_separators(used),
            format_with_separators(window)
        ),
        Style::default().fg(colors::dim()),
    ));
    Line::from(spans)
}

fn build_context_status_line(
    used: u64,
    window: u64,
    overflow_auto_compact: bool,
) -> Line<'static> {
    if used < window {
        let remaining = window - used;
        return Line::from(vec![
            Span::raw(field_prefix("Status")),
            Span::styled(
                format!(
                    "{} tokens before compact",
                    format_with_separators(remaining)
                ),
                Style::default().fg(colors::dim()),
            ),
        ]);
    }

    if used == window {
        return Line::from(vec![
            Span::raw(field_prefix("Status")),
            Span::styled(
                "Auto-compact will trigger on the next turn".to_string(),
                Style::default().fg(colors::warning()),
            ),
        ]);
    }

    let overage = used.saturating_sub(window);
    let mut message = format!(
        "Exceeded by {} tokens",
        format_with_separators(overage)
    );
    if overflow_auto_compact {
        message.push_str("; auto-compaction runs after overflow");
    }
    Line::from(vec![
        Span::raw(field_prefix("Status")),
        Span::styled(message, Style::default().fg(colors::error())),
    ])
}

#[derive(Debug)]
struct WindowTiming {
    remaining: Duration,
    window: Duration,
    next_reset_local: chrono::DateTime<Local>,
}

impl WindowTiming {
    fn elapsed(&self) -> Duration {
        self.window
            .checked_sub(self.remaining)
            .unwrap_or(Duration::ZERO)
    }
}

fn compute_window_timing(
    window_minutes: u64,
    next_reset: DateTime<Utc>,
) -> Option<WindowTiming> {
    let window_seconds = (window_minutes as i64).checked_mul(60)?;
    if window_seconds <= 0 {
        return None;
    }

    let now = Utc::now();
    let mut remaining_secs = next_reset.signed_duration_since(now).num_seconds();
    if remaining_secs < 0 {
        remaining_secs = 0;
    }
    if remaining_secs > window_seconds {
        remaining_secs = window_seconds;
    }

    let window = Duration::from_secs(window_seconds as u64);
    let remaining = Duration::from_secs(remaining_secs as u64);
    Some(WindowTiming {
        remaining,
        window,
        next_reset_local: next_reset.with_timezone(&Local),
    })
}

fn format_reset_timestamp(ts: chrono::DateTime<Local>, include_calendar: bool) -> String {
    let time_part = ts.format("%I:%M%P").to_string();
    if !include_calendar {
        return time_part;
    }

    let dow = ts.format("%a").to_string();
    let day = format_day_ordinal(ts.day());
    let month = month_abbrev(ts.month());
    format!("{dow} {day} {month} at {time_part}")
}

fn format_day_ordinal(day: u32) -> String {
    let suffix = match day % 100 {
        11 | 12 | 13 => "th",
        _ => match day % 10 {
            1 => "st",
            2 => "nd",
            3 => "rd",
            _ => "th",
        },
    };
    format!("{day}{suffix}")
}

fn month_abbrev(month: u32) -> &'static str {
    match month {
        1 => "Jan",
        2 => "Feb",
        3 => "Mar",
        4 => "Apr",
        5 => "May",
        6 => "Jun",
        7 => "Jul",
        8 => "Aug",
        9 => "Sept",
        10 => "Oct",
        11 => "Nov",
        12 => "Dec",
        _ => "",
    }
}

fn build_status_line(metrics: &RateLimitMetrics) -> Line<'static> {
    if metrics.weekly_exhausted() || metrics.hourly_exhausted() {
        let reason = match (metrics.hourly_exhausted(), metrics.weekly_exhausted()) {
            (true, true) => "weekly and hourly windows exhausted",
            (true, false) => "hourly window exhausted",
            (false, true) => "weekly window exhausted",
            (false, false) => unreachable!(),
        };
        Line::from(vec![
            Span::styled(
                format!("✕ Rate limited: {reason}"),
                Style::default().fg(colors::error()),
            ),
        ])
    } else {
        Line::from(vec![
            Span::styled(
                "✓ Within current limits".to_string(),
                Style::default().fg(colors::success()),
            ),
        ])
    }
}

fn build_legend_lines(show_gauge: bool) -> Vec<Line<'static>> {
    if !show_gauge {
        return Vec::new();
    }

    let mut lines = Vec::new();
    let indent = chart_indent();
    lines.push(Line::from(vec![
        Span::styled(
            indent.clone(),
            Style::default().fg(colors::dim()),
        ),
        Span::styled(
            WEEKLY_CELL.to_string(),
            Style::default()
                .fg(colors::text())
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            " weekly usage".to_string(),
            Style::default().fg(colors::dim()),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled(
            indent.clone(),
            Style::default().fg(colors::dim()),
        ),
        Span::styled(
            HOURLY_CELL.to_string(),
            Style::default()
                .fg(colors::primary())
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            " hourly headroom".to_string(),
            Style::default().fg(colors::dim()),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled(
            indent,
            Style::default().fg(colors::dim()),
        ),
        Span::styled(
            UNUSED_CELL.to_string(),
            Style::default()
                .fg(colors::info())
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            " unused weekly".to_string(),
            Style::default().fg(colors::dim()),
        ),
    ]));
    lines
}

fn field_prefix(label: &str) -> String {
    let padding = INDENTS
        .label_gap
        .saturating_add(INDENTS.label_target_width.saturating_sub(label.len()));
    let spaces = " ".repeat(padding);
    let indent = label_indent();
    let mut text = String::with_capacity(indent.len() + label.len() + 1 + spaces.len());
    text.push_str(&indent);
    text.push_str(label);
    text.push(':');
    text.push_str(&spaces);
    text
}

fn render_percent_bar(percent: f64) -> Vec<Span<'static>> {
    let clamped = percent.clamp(0.0, 100.0);
    let mut filled = ((clamped / 100.0) * BAR_SLOTS as f64).round() as usize;
    if clamped > 0.0 && filled == 0 {
        filled = 1;
    }
    let filled = filled.min(BAR_SLOTS);
    let empty = BAR_SLOTS.saturating_sub(filled);
    let mut spans: Vec<Span<'static>> = Vec::new();
    if filled > 0 {
        spans.push(Span::styled(
            BAR_FILLED.repeat(filled),
            Style::default()
                .fg(colors::primary())
                .add_modifier(Modifier::BOLD),
        ));
    }
    if empty > 0 {
        spans.push(Span::styled(
            BAR_EMPTY.repeat(empty),
            Style::default()
                .fg(colors::primary())
                .add_modifier(Modifier::BOLD),
        ));
    }
    if spans.is_empty() {
        spans.push(Span::styled(
            BAR_EMPTY.repeat(BAR_SLOTS),
            Style::default()
                .fg(colors::primary())
                .add_modifier(Modifier::BOLD),
        ));
    }
    spans
}

fn format_percent(percent: f64) -> String {
    let clamped = percent.clamp(0.0, 100.0);
    if clamped == 0.0 || clamped >= 1.0 {
        format!("{clamped:.0}%")
    } else {
        format!("{clamped:.1}%")
    }
}

fn format_minutes_round_units(minutes: u64) -> String {
    if minutes == 0 {
        return "0 minutes".to_string();
    }

    if minutes >= 1_440 {
        let mut days = ((minutes as f64) / 1_440.0).round() as u64;
        if days == 0 {
            days = 1;
        }
        let unit = if days == 1 { "day" } else { "days" };
        return format!("{days} {unit}");
    }

    if minutes >= 60 {
        let mut hours = ((minutes as f64) / 60.0).round() as u64;
        if hours == 0 {
            hours = 1;
        }
        let unit = if hours == 1 { "hour" } else { "hours" };
        return format!("{hours} {unit}");
    }

    let unit = if minutes == 1 { "minute" } else { "minutes" };
    format!("{minutes} {unit}")
}

fn extract_capacity_fraction(snapshot: &RateLimitSnapshotEvent) -> Option<f64> {
    let ratio = snapshot.primary_to_secondary_ratio_percent;
    if ratio.is_finite() && ratio > 0.0 {
        return Some((ratio / 100.0).clamp(0.0, 1.0));
    }

    // Fallback: derive a plausible capacity fraction from the rolling window
    // lengths when the upstream ratio header is unavailable. This keeps the
    // chart visible even on older accounts that do not yet expose the ratio.
    let primary = snapshot.primary_window_minutes;
    let secondary = snapshot.secondary_window_minutes;
    if primary == 0 || secondary == 0 {
        return None;
    }
    if primary >= secondary {
        return Some(1.0);
    }

    let derived = (primary as f64 / secondary as f64).clamp(0.0, 1.0);
    if derived > 0.0 && derived.is_finite() {
        Some(derived)
    } else {
        None
    }
}

fn gauge_inputs_available(snapshot: &RateLimitSnapshotEvent) -> bool {
    if extract_capacity_fraction(snapshot).is_none() {
        return false;
    }

    snapshot.primary_used_percent.is_finite()
        && snapshot.secondary_used_percent.is_finite()
        && snapshot.primary_window_minutes > 0
        && snapshot.secondary_window_minutes > 0
}

fn compute_grid_state(metrics: &RateLimitMetrics, capacity_fraction: f64) -> Option<GridState> {
    if capacity_fraction <= 0.0 {
        return None;
    }

    let weekly_used_ratio = (metrics.weekly_used / 100.0).clamp(0.0, 1.0);
    let weekly_remaining_ratio = (1.0 - weekly_used_ratio).max(0.0);

    let hourly_used_ratio = (metrics.hourly_used / 100.0).clamp(0.0, 1.0);
    let hourly_used_within_capacity =
        (hourly_used_ratio * capacity_fraction).min(capacity_fraction);
    let hourly_remaining_within_capacity =
        (capacity_fraction - hourly_used_within_capacity).max(0.0);

    let hourly_remaining_ratio = hourly_remaining_within_capacity.min(weekly_remaining_ratio);

    Some(GridState {
        weekly_used_ratio,
        hourly_remaining_ratio,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain_text(line: &Line<'static>) -> String {
        let mut text = String::new();
        for span in &line.spans {
            text.push_str(span.content.as_ref());
        }
        text
    }

    fn base_snapshot() -> RateLimitSnapshotEvent {
        RateLimitSnapshotEvent {
            primary_used_percent: 10.0,
            secondary_used_percent: 15.0,
            primary_to_secondary_ratio_percent: 0.0,
            primary_window_minutes: 15,
            secondary_window_minutes: 60,
            primary_reset_after_seconds: Some(900),
            secondary_reset_after_seconds: Some(3600),
        }
    }

    #[test]
    fn extract_capacity_fraction_falls_back_to_window_ratio() {
        let snapshot = base_snapshot();
        let fraction = extract_capacity_fraction(&snapshot).expect("fraction");
        assert!((fraction - 0.25).abs() < f64::EPSILON);
    }

    #[test]
    fn compact_limit_uses_context_tokens() {
        let info = RateLimitResetInfo {
            auto_compact_limit: Some(350_000),
            auto_compact_tokens_used: Some(42_972),
            ..RateLimitResetInfo::default()
        };
        let lines = build_compact_lines(&info);
        assert!(lines.len() >= 3, "expected section header + two rows");
        let tokens_line = plain_text(&lines[1]);
        assert!(
            tokens_line.contains("42,972 / 350,000"),
            "compact tokens line should show context usage: {tokens_line}"
        );
    }

    #[test]
    fn hides_usage_sections_when_disabled() {
        let snapshot = base_snapshot();
        let view = build_limits_view(
            &snapshot,
            RateLimitResetInfo::default(),
            DEFAULT_GRID_CONFIG,
            RateLimitDisplayConfig {
                show_usage_sections: false,
                show_chart: false,
            },
        );
        let rendered: Vec<String> = view
            .summary_lines
            .iter()
            .map(plain_text)
            .collect();
        assert!(rendered.iter().all(|line| !line.contains("Hourly Limit")));
        assert!(rendered.iter().all(|line| !line.contains("Weekly Limit")));
        assert!(rendered.iter().all(|line| !line.contains("Chart")));
        assert!(view.gauge_lines(80).is_empty());
    }
}

fn scale_grid_state(state: GridState, grid: GridConfig) -> GridState {
    if grid.weekly_slots == 0 {
        return GridState {
            weekly_used_ratio: 0.0,
            hourly_remaining_ratio: 0.0,
        };
    }
    state
}

/// Convert the grid state to rendered lines for the TUI.
pub(crate) fn render_limit_grid(
    state: GridState,
    grid_config: GridConfig,
    width: u16,
) -> Vec<Line<'static>> {
    GridLayout::new(grid_config, width)
        .map(|layout| layout.render(state))
        .unwrap_or_default()
}

/// Precomputed layout information for the usage grid.
struct GridLayout {
    size: usize,
}

impl GridLayout {
    const MIN_SIDE: usize = 4;
    const MAX_SIDE: usize = 12;

    fn new(config: GridConfig, width: u16) -> Option<Self> {
        if config.weekly_slots == 0 {
            return None;
        }
        let cell_width = WEEKLY_CELL.chars().count();

        let indent_width = chart_indent_width() as u16;
        let available_inner = width.saturating_sub(indent_width) as usize;
        if available_inner < cell_width {
            return None;
        }

        let base_side = (config.weekly_slots as f64)
            .sqrt()
            .round()
            .clamp(1.0, Self::MAX_SIDE as f64) as usize;
        let width_limited_side =
            ((available_inner + 1) / (cell_width + 1)).clamp(1, Self::MAX_SIDE);

        let mut side = base_side.min(width_limited_side);
        if width_limited_side >= Self::MIN_SIDE {
            side = side.max(Self::MIN_SIDE.min(width_limited_side));
        }
        side = side.clamp(1, width_limited_side);
        while side > 1 && (side * cell_width + side.saturating_sub(1)) > available_inner {
            side -= 1;
        }
        if side == 0 {
            return None;
        }

        Some(Self { size: side })
    }

    /// Render the grid into styled lines for the history cell.
    fn render(&self, state: GridState) -> Vec<Line<'static>> {
        let counts = self.cell_counts(state);
        let mut lines = Vec::new();
        let total_cells = (self.size * self.size) as isize;
        let indent = chart_indent();
        let desired_width = content_column_width();
        for row in 0..self.size {
            let mut spans: Vec<Span<'static>> = Vec::new();
            spans.push(Span::raw(indent.clone()));
            let padding = desired_width.saturating_sub(indent.len());
            spans.push(Span::raw(" ".repeat(padding)));

            for col in 0..self.size {
                if col > 0 {
                    spans.push(" ".into());
                }
                let linear_index = (self.size * row) + col;
                let slot = total_cells - 1 - linear_index as isize;
                let span = if slot < counts.dark_cells {
                    Span::styled(
                        WEEKLY_CELL.to_string(),
                        Style::default().fg(colors::text()),
                    )
                } else if slot < counts.dark_cells + counts.green_cells {
                    Span::styled(
                        HOURLY_CELL.to_string(),
                        Style::default().fg(colors::primary()),
                    )
                } else {
                    Span::styled(
                        UNUSED_CELL.to_string(),
                        Style::default().fg(colors::info()),
                    )
                };
                spans.push(span);
            }
            lines.push(Line::from(spans));
        }
        lines.push("".into());
        lines
    }

    /// Translate usage ratios into the number of coloured cells.
    fn cell_counts(&self, state: GridState) -> GridCellCounts {
        let total_cells = self.size * self.size;
        let mut dark_cells = (state.weekly_used_ratio * total_cells as f64).round() as isize;
        dark_cells = dark_cells.clamp(0, total_cells as isize);
        let mut green_cells = (state.hourly_remaining_ratio * total_cells as f64).round() as isize;
        if dark_cells + green_cells > total_cells as isize {
            green_cells = (total_cells as isize - dark_cells).max(0);
        }
        GridCellCounts {
            dark_cells,
            green_cells,
        }
    }
}

/// Number of weekly (dark) and hourly (green) cells; remaining slots imply unused weekly capacity.
struct GridCellCounts {
    dark_cells: isize,
    green_cells: isize,
}
