use codex_core::protocol::RateLimitSnapshotEvent;
use ratatui::prelude::*;
use ratatui::style::Stylize;

/// Aggregated output used by the `/limits` command.
/// It contains the rendered summary lines, optional legend,
/// and the precomputed gauge state when one can be shown.
#[derive(Debug)]
pub(crate) struct LimitsView {
    pub(crate) summary_lines: Vec<Line<'static>>,
    pub(crate) legend_lines: Vec<Line<'static>>,
    grid_state: Option<GridState>,
    grid: GridConfig,
}

impl LimitsView {
    /// Render the gauge for the provided width if the data supports it.
    pub(crate) fn gauge_lines(&self, width: u16) -> Vec<Line<'static>> {
        match self.grid_state {
            Some(state) => render_limit_grid(state, self.grid, width),
            None => Vec::new(),
        }
    }
}

/// Configuration for the simple grid gauge rendered by `/limits`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct GridConfig {
    pub(crate) weekly_slots: usize,
    pub(crate) logo: &'static str,
}

/// Default gauge configuration used by the TUI.
pub(crate) const DEFAULT_GRID_CONFIG: GridConfig = GridConfig {
    weekly_slots: 100,
    logo: "(>_)",
};

/// Build the lines and optional gauge used by the `/limits` view.
pub(crate) fn build_limits_view(
    snapshot: &RateLimitSnapshotEvent,
    grid_config: GridConfig,
) -> LimitsView {
    let metrics = RateLimitMetrics::from_snapshot(snapshot);
    let grid_state = extract_capacity_fraction(snapshot)
        .and_then(|fraction| compute_grid_state(&metrics, fraction))
        .map(|state| scale_grid_state(state, grid_config));

    LimitsView {
        summary_lines: build_summary_lines(&metrics),
        legend_lines: build_legend_lines(grid_state.is_some()),
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
    hourly_window_label: String,
    weekly_window_label: String,
    hourly_reset_hint: String,
    weekly_reset_hint: String,
}

impl RateLimitMetrics {
    fn from_snapshot(snapshot: &RateLimitSnapshotEvent) -> Self {
        let hourly_used = snapshot.primary_used_percent.clamp(0.0, 100.0);
        let weekly_used = snapshot.weekly_used_percent.clamp(0.0, 100.0);
        Self {
            hourly_used,
            weekly_used,
            hourly_remaining: (100.0 - hourly_used).max(0.0),
            weekly_remaining: (100.0 - weekly_used).max(0.0),
            hourly_window_label: format_window_label(Some(snapshot.primary_window_minutes)),
            weekly_window_label: format_window_label(Some(snapshot.weekly_window_minutes)),
            hourly_reset_hint: format_reset_hint(Some(snapshot.primary_window_minutes)),
            weekly_reset_hint: format_reset_hint(Some(snapshot.weekly_window_minutes)),
        }
    }

    fn hourly_exhausted(&self) -> bool {
        self.hourly_remaining <= 0.0
    }

    fn weekly_exhausted(&self) -> bool {
        self.weekly_remaining <= 0.0
    }
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

#[derive(Clone, Copy, Debug)]
struct GridState {
    weekly_used_ratio: f64,
    hourly_remaining_ratio: f64,
}

fn build_summary_lines(metrics: &RateLimitMetrics) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = vec![
        "/limits".magenta().into(),
        "".into(),
        vec!["Rate limit usage snapshot".bold()].into(),
        vec!["  Tip: run `/limits` right after Codex replies for freshest numbers.".dim()].into(),
        build_usage_line(
            "  • Hourly limit",
            &metrics.hourly_window_label,
            metrics.hourly_used,
        ),
        build_usage_line(
            "  • Weekly limit",
            &metrics.weekly_window_label,
            metrics.weekly_used,
        ),
    ];
    lines.push(build_status_line(metrics));
    lines
}

fn build_usage_line(label: &str, window_label: &str, used_percent: f64) -> Line<'static> {
    Line::from(vec![
        label.to_string().into(),
        format!(" ({window_label})").dim(),
        ": ".into(),
        format!("{used_percent:.1}% used").dark_gray().bold(),
    ])
}

fn build_status_line(metrics: &RateLimitMetrics) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    if metrics.weekly_exhausted() || metrics.hourly_exhausted() {
        spans.push("  Rate limited: ".into());
        let reason = match (metrics.hourly_exhausted(), metrics.weekly_exhausted()) {
            (true, true) => "weekly and hourly windows exhausted",
            (true, false) => "hourly window exhausted",
            (false, true) => "weekly window exhausted",
            (false, false) => unreachable!(),
        };
        spans.push(reason.red());
        if metrics.hourly_exhausted() {
            spans.push(" — hourly resets in ".into());
            spans.push(metrics.hourly_reset_hint.clone().dim());
        }
        if metrics.weekly_exhausted() {
            spans.push(" — weekly resets in ".into());
            spans.push(metrics.weekly_reset_hint.clone().dim());
        }
    } else {
        spans.push("  Within current limits".green());
    }
    Line::from(spans)
}

fn build_legend_lines(show_gauge: bool) -> Vec<Line<'static>> {
    if !show_gauge {
        return Vec::new();
    }
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
}

fn extract_capacity_fraction(snapshot: &RateLimitSnapshotEvent) -> Option<f64> {
    let ratio = snapshot.primary_to_weekly_ratio_percent;
    if ratio.is_finite() {
        Some((ratio / 100.0).clamp(0.0, 1.0))
    } else {
        None
    }
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
fn render_limit_grid(state: GridState, grid_config: GridConfig, width: u16) -> Vec<Line<'static>> {
    GridLayout::new(grid_config, width)
        .map(|layout| layout.render(state))
        .unwrap_or_default()
}

/// Precomputed layout information for the usage grid.
struct GridLayout {
    size: usize,
    inner_width: usize,
    config: GridConfig,
}

impl GridLayout {
    const MIN_SIDE: usize = 4;
    const MAX_SIDE: usize = 12;
    const PREFIX: &'static str = "  ";

    fn new(config: GridConfig, width: u16) -> Option<Self> {
        if config.weekly_slots == 0 || config.logo.is_empty() {
            return None;
        }
        let cell_width = config.logo.chars().count();
        if cell_width == 0 {
            return None;
        }

        let available_inner = width.saturating_sub((Self::PREFIX.len() + 2) as u16) as usize;
        if available_inner == 0 {
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
        let side = side.clamp(1, Self::MAX_SIDE);
        if side == 0 {
            return None;
        }

        let inner_width = side * cell_width + side.saturating_sub(1);
        Some(Self {
            size: side,
            inner_width,
            config,
        })
    }

    /// Render the grid into styled lines for the history cell.
    fn render(&self, state: GridState) -> Vec<Line<'static>> {
        let counts = self.cell_counts(state);
        let mut lines = Vec::new();
        lines.push("".into());
        lines.push(self.render_border('╭', '╮'));

        let mut cell_index = 0isize;
        for _ in 0..self.size {
            let mut spans: Vec<Span<'static>> = Vec::new();
            spans.push(Self::PREFIX.into());
            spans.push("│".dim());

            for col in 0..self.size {
                if col > 0 {
                    spans.push(" ".into());
                }
                let span = if cell_index < counts.dark_cells {
                    self.config.logo.dark_gray()
                } else if cell_index < counts.dark_cells + counts.green_cells {
                    self.config.logo.green()
                } else {
                    self.config.logo.into()
                };
                spans.push(span);
                cell_index += 1;
            }

            spans.push("│".dim());
            lines.push(Line::from(spans));
        }

        lines.push(self.render_border('╰', '╯'));
        lines.push("".into());

        if counts.white_cells == 0 {
            lines.push(vec!["  (No unused weekly capacity remaining)".dim()].into());
            lines.push("".into());
        }

        lines
    }

    fn render_border(&self, left: char, right: char) -> Line<'static> {
        let mut text = String::from(Self::PREFIX);
        text.push(left);
        text.push_str(&"─".repeat(self.inner_width));
        text.push(right);
        vec![Span::from(text).dim()].into()
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
        let white_cells = (total_cells as isize - dark_cells - green_cells).max(0);

        GridCellCounts {
            dark_cells,
            green_cells,
            white_cells,
        }
    }
}

/// Number of weekly (dark), hourly (green) and unused (default) cells.
struct GridCellCounts {
    dark_cells: isize,
    green_cells: isize,
    white_cells: isize,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot() -> RateLimitSnapshotEvent {
        RateLimitSnapshotEvent {
            primary_used_percent: 30.0,
            weekly_used_percent: 60.0,
            primary_to_weekly_ratio_percent: 40.0,
            primary_window_minutes: 300,
            weekly_window_minutes: 10_080,
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
        let display = build_limits_view(&snapshot(), DEFAULT_GRID_CONFIG);
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
        let display = build_limits_view(&snapshot(), DEFAULT_GRID_CONFIG);
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
        s.primary_to_weekly_ratio_percent = f64::NAN;
        let display = build_limits_view(&s, DEFAULT_GRID_CONFIG);
        assert!(display.gauge_lines(80).is_empty());
        assert!(display.legend_lines.is_empty());
    }
}
