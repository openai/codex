use codex_core::protocol::RateLimitSnapshotEvent;
use ratatui::prelude::*;
use ratatui::style::Stylize;

#[derive(Clone, Copy, Debug)]
pub(crate) struct LimitGaugeConfig {
    pub(crate) weekly_slots: usize,
    pub(crate) logo: &'static str,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct LimitGaugeState {
    pub(crate) weekly_used_ratio: f64,
    pub(crate) hourly_remaining_ratio: f64,
}

#[derive(Debug)]
pub(crate) struct RateLimitDisplay {
    pub(crate) summary_lines: Vec<Line<'static>>,
    pub(crate) legend_lines: Vec<Line<'static>>,
    gauge_state: Option<LimitGaugeState>,
    gauge: LimitGaugeConfig,
}

impl RateLimitDisplay {
    pub(crate) fn gauge_lines(&self, width: u16) -> Vec<Line<'static>> {
        match self.gauge_state {
            Some(state) => render_limit_gauge(state, self.gauge, width),
            None => Vec::new(),
        }
    }
}

pub(crate) const DEFAULT_LIMIT_GAUGE_CONFIG: LimitGaugeConfig = LimitGaugeConfig {
    weekly_slots: 100,
    logo: "(>_)",
};

pub(crate) fn build_rate_limit_display(
    snapshot: &RateLimitSnapshotEvent,
    gauge: LimitGaugeConfig,
) -> RateLimitDisplay {
    let hourly_used = snapshot.primary_used_percent.clamp(0.0, 100.0);
    let weekly_used = snapshot.protection_used_percent.clamp(0.0, 100.0);
    let hourly_remaining = (100.0 - hourly_used).max(0.0);
    let weekly_remaining = (100.0 - weekly_used).max(0.0);

    let hourly_window_label = format_window_label(Some(snapshot.primary_window_minutes));
    let weekly_window_label = format_window_label(Some(snapshot.protection_window_minutes));

    let mut summary_lines: Vec<Line<'static>> = vec![
        "/limits".magenta().into(),
        "".into(),
        vec!["Rate limit usage snapshot".bold()].into(),
        vec!["  Tip: run `/limits` right after Codex replies for freshest numbers.".dim()].into(),
        vec![
            "  • Hourly limit".into(),
            format!(" ({hourly_window_label})").dim(),
            ": ".into(),
            format!("{hourly_used:.1}% used").dark_gray().bold(),
        ]
        .into(),
        vec![
            "  • Weekly limit".into(),
            format!(" ({weekly_window_label})").dim(),
            ": ".into(),
            format!("{weekly_used:.1}% used").dark_gray().bold(),
        ]
        .into(),
    ];

    let hourly_exhausted = hourly_remaining <= 0.0;
    let weekly_exhausted = weekly_remaining <= 0.0;
    let mut status_line: Vec<Span<'static>> = Vec::new();
    if weekly_exhausted || hourly_exhausted {
        status_line.push("  Rate limited: ".into());
        let reason = match (weekly_exhausted, hourly_exhausted) {
            (true, true) => "weekly and hourly windows exhausted",
            (true, false) => "weekly window exhausted",
            (false, true) => "hourly window exhausted",
            (false, false) => unreachable!(),
        };
        status_line.push(reason.red());
        if hourly_exhausted {
            status_line.push(" — hourly resets in ".into());
            status_line.push(format_reset_hint(Some(snapshot.primary_window_minutes)).dim());
        }
        if weekly_exhausted {
            status_line.push(" — weekly resets in ".into());
            status_line.push(format_reset_hint(Some(snapshot.protection_window_minutes)).dim());
        }
    } else {
        status_line.push("  Within current limits".green());
    }
    summary_lines.push(status_line.into());

    let gauge_state = compute_gauge_state(snapshot).map(|state| scale_gauge_state(state, gauge));

    let legend_lines = if gauge_state.is_some() {
        vec![
            vec!["Legend".bold()].into(),
            vec![
                "  • ".into(),
                "Dark gray".dark_gray().bold(),
                " = weekly usage so far".into(),
            ]
            .into(),
            vec![
                "  • ".into(),
                "Green".green().bold(),
                " = hourly capacity still available".into(),
            ]
            .into(),
            vec![
                "  • ".into(),
                "Default".bold(),
                " = weekly capacity beyond the hourly window".into(),
            ]
            .into(),
        ]
    } else {
        Vec::new()
    };

    RateLimitDisplay {
        summary_lines,
        legend_lines,
        gauge_state,
        gauge,
    }
}

fn compute_gauge_state(snapshot: &RateLimitSnapshotEvent) -> Option<LimitGaugeState> {
    let weekly_used_ratio = (snapshot.protection_used_percent / 100.0).clamp(0.0, 1.0);
    let weekly_remaining_ratio = (1.0 - weekly_used_ratio).max(0.0);

    let ratio_fraction = {
        let ratio = snapshot.primary_to_protection_ratio_percent;
        if ratio.is_finite() {
            Some((ratio / 100.0).clamp(0.0, 1.0))
        } else {
            None
        }
    };

    let capacity_fraction = ratio_fraction?;
    if capacity_fraction <= 0.0 {
        return None;
    }

    let hourly_used_ratio = (snapshot.primary_used_percent / 100.0).clamp(0.0, 1.0);
    let hourly_used_within_capacity =
        (hourly_used_ratio * capacity_fraction).min(capacity_fraction);
    let hourly_remaining_within_capacity =
        (capacity_fraction - hourly_used_within_capacity).max(0.0);

    let hourly_remaining_ratio = hourly_remaining_within_capacity.min(weekly_remaining_ratio);

    Some(LimitGaugeState {
        weekly_used_ratio,
        hourly_remaining_ratio,
    })
}

fn scale_gauge_state(state: LimitGaugeState, gauge: LimitGaugeConfig) -> LimitGaugeState {
    if gauge.weekly_slots == 0 {
        return LimitGaugeState {
            weekly_used_ratio: 0.0,
            hourly_remaining_ratio: 0.0,
        };
    }
    state
}

fn render_limit_gauge(
    state: LimitGaugeState,
    gauge: LimitGaugeConfig,
    width: u16,
) -> Vec<Line<'static>> {
    const MIN_SQUARE_SIDE: usize = 4;
    const MAX_SQUARE_SIDE: usize = 12;
    const PREFIX: &str = "  ";

    if gauge.weekly_slots == 0 || gauge.logo.is_empty() {
        return Vec::new();
    }

    let cell_width = gauge.logo.chars().count();
    if cell_width == 0 {
        return Vec::new();
    }

    let available_inner_width = width.saturating_sub((PREFIX.len() + 2) as u16) as usize;
    if available_inner_width == 0 {
        return Vec::new();
    }

    let base_side = (gauge.weekly_slots as f64)
        .sqrt()
        .round()
        .clamp(1.0, MAX_SQUARE_SIDE as f64) as usize;
    let width_limited_side =
        ((available_inner_width + 1) / (cell_width + 1)).clamp(1, MAX_SQUARE_SIDE);

    let mut side = base_side.min(width_limited_side.max(1));
    if width_limited_side >= MIN_SQUARE_SIDE {
        side = side.max(MIN_SQUARE_SIDE.min(width_limited_side));
    }
    side = side.clamp(1, MAX_SQUARE_SIDE);

    if side == 0 {
        return Vec::new();
    }

    let inner_width = side * cell_width + side.saturating_sub(1);
    let total_cells = side * side;

    let mut dark_cells = (state.weekly_used_ratio * total_cells as f64).round() as isize;
    dark_cells = dark_cells.clamp(0, total_cells as isize);
    let mut green_cells = (state.hourly_remaining_ratio * total_cells as f64).round() as isize;
    if dark_cells + green_cells > total_cells as isize {
        green_cells = (total_cells as isize - dark_cells).max(0);
    }
    let white_cells = (total_cells as isize - dark_cells - green_cells).max(0);

    let mut lines: Vec<Line<'static>> = Vec::new();
    lines.push("".into());

    let mut top = String::from(PREFIX);
    top.push('╭');
    top.push_str(&"─".repeat(inner_width));
    top.push('╮');
    lines.push(vec![Span::from(top).dim()].into());

    let mut cell_index = 0isize;
    for _row in 0..side {
        let mut spans: Vec<Span<'static>> = Vec::new();
        spans.push(PREFIX.into());
        spans.push("│".dim());

        for col in 0..side {
            if col > 0 {
                spans.push(" ".into());
            }
            let span = if cell_index < dark_cells {
                gauge.logo.dark_gray()
            } else if cell_index < dark_cells + green_cells {
                gauge.logo.green()
            } else {
                gauge.logo.into()
            };
            spans.push(span);
            cell_index += 1;
        }

        spans.push("│".dim());
        lines.push(Line::from(spans));
    }

    let mut bottom = String::from(PREFIX);
    bottom.push('╰');
    bottom.push_str(&"─".repeat(inner_width));
    bottom.push('╯');
    lines.push(vec![Span::from(bottom).dim()].into());
    lines.push("".into());

    if white_cells == 0 {
        lines.push(vec!["  (No unused weekly capacity remaining)".dim()].into());
        lines.push("".into());
    }

    lines
}

fn format_window_label(minutes: Option<u64>) -> String {
    approximate_duration(minutes)
        .map(|(value, unit)| format!("≈{value} {} window", pluralize_unit(unit, value)))
        .unwrap_or_else(|| "window unknown".to_string())
}

fn format_reset_hint(minutes: Option<u64>) -> String {
    approximate_duration(minutes)
        .map(|(value, unit)| format!("≈{value} {}", pluralize_unit(unit, value)))
        .unwrap_or_else(|| "unknown".to_string())
}

fn approximate_duration(minutes: Option<u64>) -> Option<(u64, DurationUnit)> {
    let minutes = minutes?;
    if minutes == 0 {
        return Some((1, DurationUnit::Minute));
    }
    if minutes < 60 {
        return Some((minutes, DurationUnit::Minute));
    }
    if minutes < 1_440 {
        let hours = ((minutes as f64) / 60.0).round().max(1.0) as u64;
        return Some((hours, DurationUnit::Hour));
    }
    let days = ((minutes as f64) / 1_440.0).round().max(1.0) as u64;
    if days >= 7 {
        let weeks = ((days as f64) / 7.0).round().max(1.0) as u64;
        Some((weeks, DurationUnit::Week))
    } else {
        Some((days, DurationUnit::Day))
    }
}

fn pluralize_unit(unit: DurationUnit, value: u64) -> String {
    match unit {
        DurationUnit::Minute => {
            if value == 1 {
                "minute".to_string()
            } else {
                "minutes".to_string()
            }
        }
        DurationUnit::Hour => {
            if value == 1 {
                "hour".to_string()
            } else {
                "hours".to_string()
            }
        }
        DurationUnit::Day => {
            if value == 1 {
                "day".to_string()
            } else {
                "days".to_string()
            }
        }
        DurationUnit::Week => {
            if value == 1 {
                "week".to_string()
            } else {
                "weeks".to_string()
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DurationUnit {
    Minute,
    Hour,
    Day,
    Week,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot() -> RateLimitSnapshotEvent {
        RateLimitSnapshotEvent {
            primary_used_percent: 30.0,
            protection_used_percent: 60.0,
            primary_to_protection_ratio_percent: 40.0,
            primary_window_minutes: 300,
            protection_window_minutes: 10_080,
        }
    }

    #[test]
    fn approximate_duration_handles_hours_and_weeks() {
        assert_eq!(
            approximate_duration(Some(299)),
            Some((5, DurationUnit::Hour))
        );
        assert_eq!(
            approximate_duration(Some(10_080)),
            Some((1, DurationUnit::Week))
        );
        assert_eq!(
            approximate_duration(Some(90)),
            Some((2, DurationUnit::Hour))
        );
    }

    #[test]
    fn build_display_constructs_summary_and_gauge() {
        let display = build_rate_limit_display(&snapshot(), DEFAULT_LIMIT_GAUGE_CONFIG);
        assert!(display.summary_lines.iter().any(|line| {
            line.spans
                .iter()
                .any(|span| span.content.contains("Weekly limit"))
        }));
        assert!(display.summary_lines.iter().any(|line| {
            line.spans
                .iter()
                .any(|span| span.content.contains("Hourly limit"))
        }));
        assert!(!display.gauge_lines(80).is_empty());
    }

    #[test]
    fn hourly_and_weekly_percentages_are_not_swapped() {
        let display = build_rate_limit_display(&snapshot(), DEFAULT_LIMIT_GAUGE_CONFIG);
        let summary = display
            .summary_lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");

        assert!(summary.contains("Hourly limit (≈5 hours window): 30.0% used"));
        assert!(summary.contains("Weekly limit (≈1 week window): 60.0% used"));
    }

    #[test]
    fn build_display_without_ratio_skips_gauge() {
        let mut s = snapshot();
        s.primary_to_protection_ratio_percent = f64::NAN;
        let display = build_rate_limit_display(&s, DEFAULT_LIMIT_GAUGE_CONFIG);
        assert!(display.gauge_lines(80).is_empty());
        assert!(display.legend_lines.is_empty());
    }
}
