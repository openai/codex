//! Renders account token activity and coordinates asynchronous `/tokens` cards.
//!
//! The slash command builds a composite history cell immediately, but the widget
//! keeps that cell transient while the account request runs. The transient card is
//! rendered above the composer through [`ChatWidget::pending_token_activity_output`]
//! so loading never requires clearing or rewriting transcript history. When the
//! matching response arrives, [`TokenActivityHandle`] updates the shared card state
//! and [`ChatWidget::finish_token_activity_refresh`] moves the cell into a completed
//! slot. Event dispatch commits that completed cell into history only after active
//! output and stream consolidation no longer block insertion.
//!
//! Chart rendering is intentionally terminal-native. Daily mode displays a
//! GitHub-style 52-week calendar, while weekly and cumulative modes reuse the same
//! grid as bottom-aligned bars. Truecolor terminals encode activity intensity with
//! color. Lower-color terminals fall back to distinct hollow and filled glyphs so
//! empty days remain legible.
//!
//! Backend buckets are normalized before rendering: malformed dates, future dates,
//! and dates outside the visible window are ignored; duplicate dates are summed;
//! and negative token counts are clamped to zero.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::RwLock;

use chrono::Datelike;
use chrono::Duration;
use chrono::NaiveDate;
use chrono::Utc;
use codex_app_server_protocol::GetAccountTokenUsageResponse;
use ratatui::style::Color;
use ratatui::style::Style;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;

use super::ChatWidget;
use crate::app_event::AppEvent;
use crate::color::blend;
use crate::history_cell::CompositeHistoryCell;
use crate::history_cell::HistoryCell;
use crate::history_cell::PlainHistoryCell;
use crate::history_cell::plain_lines;
use crate::render::highlight::foreground_style_for_scopes;
use crate::status::format_tokens_compact;
use crate::terminal_palette::StdoutColorLevel;
use crate::terminal_palette::best_color;
use crate::terminal_palette::default_bg;
use crate::terminal_palette::default_fg;
use crate::terminal_palette::stdout_color_level;

// In low-color terminals we distinguish empty vs active cells by glyph (a
// width-matched filled/hollow pair). In truecolor terminals the grid uses a
// single glyph and lets color carry the intensity (GitHub-style), which keeps
// the grid perfectly aligned and free of texture noise.
const EMPTY_CELL_GLYPH: &str = "□";
const ACTIVE_CELL_GLYPH: &str = "■";
const WEEK_COUNT: usize = 52;
const DAY_COUNT: usize = 7;
const CELL_COUNT: usize = WEEK_COUNT * DAY_COUNT;
const CHART_LEFT_WIDTH: usize = 4;
const SUMMARY_INDENT: &str = " ";
const SUMMARY_INDENT_WIDTH: u16 = 1;

/// Selects the aggregation represented by the token activity chart.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TokenActivityView {
    Daily,
    Weekly,
    Cumulative,
}

impl TokenActivityView {
    /// Parses the optional `/tokens` argument into a supported chart view.
    ///
    /// An empty argument selects the daily view so `/tokens` and `/tokens daily`
    /// behave identically. Returning `None` lets the slash-command dispatcher
    /// report unsupported arguments instead of silently choosing a view.
    pub(super) fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "" | "day" | "daily" => Some(Self::Daily),
            "week" | "weekly" => Some(Self::Weekly),
            "cumulative" => Some(Self::Cumulative),
            _ => None,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Daily => "Daily",
            Self::Weekly => "Weekly",
            Self::Cumulative => "Cumulative",
        }
    }
}

/// Tracks the renderable lifecycle of one token activity history cell.
#[derive(Debug)]
enum TokenActivityState {
    Loading,
    Loaded(GetAccountTokenUsageResponse),
    Error,
}

/// Completes an asynchronously rendered token activity history cell.
///
/// Clones share the same card state, allowing the background request path to
/// update a cell still owned by the widget's transient-output state. The widget
/// remains responsible for request-ID matching, redraws, and history insertion.
#[derive(Clone, Debug)]
pub(super) struct TokenActivityHandle {
    state: Arc<RwLock<TokenActivityState>>,
}

/// Holds the one transient token activity card waiting on its background response.
///
/// The request ID prevents late results from mutating a newer `/tokens` card. The
/// cell stays out of transcript history until the matching response completes and
/// the widget confirms that active output no longer blocks insertion.
pub(super) struct PendingTokenActivityOutput {
    request_id: u64,
    cell: CompositeHistoryCell,
    handle: TokenActivityHandle,
}

impl TokenActivityHandle {
    /// Replaces the loading state with either fetched activity or an unavailable state.
    ///
    /// This method intentionally discards the error string because the TUI exposes
    /// one stable unavailable message. Calling it more than once replaces the prior
    /// terminal state, so request-ID matching should happen before completion.
    pub(super) fn finish(&self, result: Result<GetAccountTokenUsageResponse, String>) {
        let state = match result {
            Ok(response) => TokenActivityState::Loaded(response),
            Err(_) => TokenActivityState::Error,
        };
        #[expect(clippy::expect_used)]
        let mut current = self.state.write().expect("token activity state poisoned");
        *current = state;
    }
}

/// Renders one `/tokens` card from shared asynchronous state.
#[derive(Debug)]
struct TokenActivityHistoryCell {
    view: TokenActivityView,
    state: Arc<RwLock<TokenActivityState>>,
}

/// Creates the card contents and completion handle for one `/tokens` invocation.
///
/// The composite cell includes the echoed slash command and a loading card from
/// the start. Callers must retain the returned handle and complete it when the
/// matching background response arrives; otherwise the transient card stays loading.
pub(super) fn new_token_activity_output(
    view: TokenActivityView,
) -> (CompositeHistoryCell, TokenActivityHandle) {
    let command = PlainHistoryCell::new(vec![
        format!("/tokens {}", view.label().to_lowercase())
            .magenta()
            .into(),
    ]);
    let state = Arc::new(RwLock::new(TokenActivityState::Loading));
    let handle = TokenActivityHandle {
        state: Arc::clone(&state),
    };
    let card = TokenActivityHistoryCell { view, state };
    (
        CompositeHistoryCell::new(vec![Box::new(command), Box::new(card)]),
        handle,
    )
}

impl HistoryCell for TokenActivityHistoryCell {
    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        #[expect(clippy::expect_used)]
        let state = self.state.read().expect("token activity state poisoned");
        match &*state {
            TokenActivityState::Loading => {
                vec![
                    " Token activity".bold().into(),
                    "   Loading...".dim().into(),
                ]
            }
            TokenActivityState::Error => vec![
                " Token activity".bold().into(),
                "   Token activity unavailable".dim().into(),
            ],
            TokenActivityState::Loaded(response) => self.loaded_lines(response, width),
        }
    }

    fn raw_lines(&self) -> Vec<Line<'static>> {
        plain_lines(self.display_lines(u16::MAX))
    }
}

impl TokenActivityHistoryCell {
    fn loaded_lines(
        &self,
        response: &GetAccountTokenUsageResponse,
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

        lines.extend(self.chart_lines(buckets, Utc::now().date_naive(), width));
        lines
    }

    fn chart_lines(
        &self,
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
        let levels = levels_for_view(&values, self.view);
        let first_column = WEEK_COUNT - shown_columns;
        lines.push(month_labels(today, first_column, shown_columns));
        for row in 0..DAY_COUNT {
            let mut spans = vec![weekday_label(self.view, row)];
            for column in first_column..WEEK_COUNT {
                if column > first_column {
                    spans.push(" ".into());
                }
                let index = column * DAY_COUNT + row;
                if self.view == TokenActivityView::Daily
                    && cell_date(today, index).is_some_and(|date| date > today)
                {
                    spans.push(" ".into());
                } else {
                    let style = if self.view == TokenActivityView::Daily {
                        palette.for_level(levels[index])
                    } else {
                        palette.for_bar_level(levels[index])
                    };
                    spans.push(Span::styled(palette.glyph(levels[index]), style));
                }
            }
            lines.push(spans.into());
        }
        // Separate the calendar from the legend/footer below.
        lines.push(Line::default());
        match self.view {
            TokenActivityView::Daily => lines.push(legend_line(&palette)),
            TokenActivityView::Weekly | TokenActivityView::Cumulative => {
                lines.push(bar_caption(self.view, &values))
            }
        }
        lines.push(view_footer(self.view));
        lines
    }
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
        spans.push(Span::styled(palette.glyph(level), palette.for_level(level)));
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

/// Dim footer that surfaces the other `/tokens` views and emphasizes the
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

/// Stores the terminal-specific styles and glyph strategy for token activity cells.
struct TokenActivityPalette {
    styles: [Style; 5],
    bar_style: Style,
    /// True when the terminal supports a truecolor gradient, so the grid can
    /// encode intensity purely by color and render every cell with a single
    /// glyph. False on low-color terminals, where we fall back to a
    /// filled/hollow glyph pair so empty and active cells stay distinguishable.
    uses_color: bool,
}

impl TokenActivityPalette {
    fn current() -> Self {
        let fallback = [
            Style::default().dim(),
            Style::default().green().dim(),
            Style::default().green(),
            Style::default().light_green(),
            Style::default().light_green().bold(),
        ];
        let fallback_bar_style = Style::default().light_green();
        let fallback_palette = || Self {
            styles: fallback,
            bar_style: fallback_bar_style,
            uses_color: false,
        };
        let (Some(fg), Some(bg), Some(anchor)) = (default_fg(), default_bg(), theme_anchor_rgb())
        else {
            return fallback_palette();
        };
        if matches!(
            stdout_color_level(),
            StdoutColorLevel::Ansi16 | StdoutColorLevel::Unknown
        ) {
            return fallback_palette();
        }
        let empty_alpha = if crate::color::is_light(bg) {
            0.18
        } else {
            0.14
        };
        let alphas = [empty_alpha, 0.22, 0.42, 0.68, 1.00];
        let styles = std::array::from_fn(|index| {
            let color = if index == 0 {
                blend(fg, bg, alphas[index])
            } else {
                blend(anchor, bg, alphas[index])
            };
            Style::default().fg(best_color(color))
        });
        let bar_style = Style::default().fg(best_color(blend(anchor, bg, /*alpha*/ 0.78)));
        Self {
            styles,
            bar_style,
            uses_color: true,
        }
    }

    fn for_level(&self, level: usize) -> Style {
        self.styles[level.min(/*other*/ 4)]
    }

    fn for_bar_level(&self, level: usize) -> Style {
        if level == 0 {
            self.for_level(/*level*/ 0)
        } else {
            self.bar_style
        }
    }

    /// The glyph for a cell at `level`. In truecolor we always use the filled
    /// glyph and let color carry the intensity; in low-color we use the hollow
    /// glyph for empty cells so they remain visible without a color gradient.
    fn glyph(&self, level: usize) -> &'static str {
        if self.uses_color || level > 0 {
            ACTIVE_CELL_GLYPH
        } else {
            EMPTY_CELL_GLYPH
        }
    }
}

fn theme_anchor_rgb() -> Option<(u8, u8, u8)> {
    match numeric_style().fg? {
        Color::Rgb(r, g, b) => Some((r, g, b)),
        _ => None,
    }
}

impl ChatWidget {
    /// Starts a token activity refresh and replaces the current transient card.
    ///
    /// Each invocation receives a request ID so background responses update only
    /// their own card. The card remains outside transcript history until completion,
    /// which keeps loading visible without disturbing existing transcript content.
    pub(super) fn add_token_activity_output(&mut self, view: TokenActivityView) {
        let request_id = self.next_token_activity_request_id;
        self.next_token_activity_request_id =
            self.next_token_activity_request_id.wrapping_add(/*rhs*/ 1);
        let (cell, handle) = new_token_activity_output(view);
        self.completed_token_activity_output = None;
        self.refreshing_token_activity_output = Some(PendingTokenActivityOutput {
            request_id,
            cell,
            handle,
        });
        self.bump_active_cell_revision();
        self.request_redraw();
        self.app_event_tx
            .send(AppEvent::RefreshTokenActivity { request_id });
    }

    /// Returns the transient token activity card that should render above the composer.
    ///
    /// A loading card takes precedence over a completed card waiting for history
    /// insertion. Callers should render the returned cell but leave ownership with
    /// the widget so completion and insertion can update it safely.
    pub(super) fn pending_token_activity_output(&self) -> Option<&dyn HistoryCell> {
        self.refreshing_token_activity_output
            .as_ref()
            .map(|output| &output.cell as &dyn HistoryCell)
            .or_else(|| {
                self.completed_token_activity_output
                    .as_ref()
                    .map(|cell| cell as &dyn HistoryCell)
            })
    }

    /// Applies a background token activity result to its matching transient card.
    ///
    /// Returns `true` when the pending request matched and moved into the completed
    /// slot. Late responses return `false`, including responses for cards replaced
    /// by a newer `/tokens` invocation or cleared during transcript changes.
    pub(crate) fn finish_token_activity_refresh(
        &mut self,
        request_id: u64,
        result: Result<GetAccountTokenUsageResponse, String>,
    ) -> bool {
        let Some(output) = self.refreshing_token_activity_output.take() else {
            return false;
        };
        if output.request_id != request_id {
            self.refreshing_token_activity_output = Some(output);
            return false;
        }
        output.handle.finish(result);
        self.completed_token_activity_output = Some(output.cell);
        self.bump_active_cell_revision();
        self.request_redraw();
        true
    }

    /// Reports whether a completed token activity card must wait before insertion.
    ///
    /// Inserting while a stream, queued consolidation, or active transcript cell is
    /// present can reorder the card relative to visible output, so callers retry once
    /// these barriers clear.
    pub(crate) fn token_activity_history_insertion_blocked(&self) -> bool {
        self.stream_controller.is_some()
            || self.plan_stream_controller.is_some()
            || self.pending_stream_consolidations > 0
            || self.transcript.active_cell.is_some()
    }

    /// Records a stream consolidation barrier that delays token card insertion.
    ///
    /// Each queued consolidation should eventually call
    /// [`ChatWidget::note_stream_consolidation_completed`].
    pub(crate) fn note_stream_consolidation_queued(&mut self) {
        self.pending_stream_consolidations =
            self.pending_stream_consolidations.saturating_add(/*rhs*/ 1);
    }

    /// Releases one queued stream consolidation barrier.
    ///
    /// The counter saturates at zero so an unmatched completion does not underflow,
    /// but paired queue/completion calls are still the intended contract.
    pub(crate) fn note_stream_consolidation_completed(&mut self) {
        self.pending_stream_consolidations =
            self.pending_stream_consolidations.saturating_sub(/*rhs*/ 1);
    }

    /// Transfers the completed token activity card into the history insertion path.
    ///
    /// Callers should use this only after
    /// [`ChatWidget::token_activity_history_insertion_blocked`] returns `false`;
    /// taking the card removes it from the transient render area.
    pub(crate) fn take_completed_token_activity_output(&mut self) -> Option<CompositeHistoryCell> {
        let output = self.completed_token_activity_output.take()?;
        self.bump_active_cell_revision();
        Some(output)
    }

    /// Requests another insertion attempt when a completed card is waiting.
    ///
    /// This is used after stream or history lifecycle events that may have cleared
    /// the insertion barriers without directly owning the completed card.
    pub(crate) fn request_completed_token_activity_output_insertion(&self) {
        if self.completed_token_activity_output.is_some() {
            self.app_event_tx
                .send(AppEvent::CommitCompletedTokenActivityOutput);
        }
    }

    /// Drops transient and completed token cards that must no longer update.
    ///
    /// Late background responses cannot mutate cards after a transcript reset,
    /// backtrack, or replacement flow clears this widget-owned state.
    pub(crate) fn clear_pending_token_activity_refreshes(&mut self) {
        let cleared_refresh = self.refreshing_token_activity_output.take().is_some();
        let cleared_completed = self.completed_token_activity_output.take().is_some();
        if cleared_refresh || cleared_completed {
            self.bump_active_cell_revision();
            self.request_redraw();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_app_server_protocol::AccountTokenUsageDailyBucket;
    use codex_app_server_protocol::AccountTokenUsageSummary;
    use insta::assert_snapshot;
    use pretty_assertions::assert_eq;

    #[test]
    fn duplicate_dates_sum_and_negative_values_clamp() {
        let today =
            NaiveDate::from_ymd_opt(/*year*/ 2026, /*month*/ 5, /*day*/ 29).expect("valid date");
        let buckets = vec![
            AccountTokenUsageDailyBucket {
                start_date: "2026-05-29".to_string(),
                tokens: 10,
            },
            AccountTokenUsageDailyBucket {
                start_date: "2026-05-29".to_string(),
                tokens: 5,
            },
            AccountTokenUsageDailyBucket {
                start_date: "2026-05-28".to_string(),
                tokens: -4,
            },
        ];

        let values = daily_values(&buckets, today);

        assert_eq!(values.iter().sum::<i64>(), 15);
    }

    #[test]
    fn bar_levels_fill_from_bottom() {
        let levels = bar_levels(&[0, 10]);

        assert_eq!(&levels[..DAY_COUNT], &[0; DAY_COUNT]);
        assert_eq!(&levels[DAY_COUNT..], &[4; DAY_COUNT]);
    }

    #[test]
    fn token_activity_view_aliases_parse() {
        assert_eq!(TokenActivityView::parse(""), Some(TokenActivityView::Daily));
        assert_eq!(
            TokenActivityView::parse("day"),
            Some(TokenActivityView::Daily)
        );
        assert_eq!(
            TokenActivityView::parse("week"),
            Some(TokenActivityView::Weekly)
        );
        assert_eq!(
            TokenActivityView::parse("cumulative"),
            Some(TokenActivityView::Cumulative)
        );
        assert_eq!(TokenActivityView::parse("year"), None);
    }

    #[test]
    fn daily_graph_snapshot_uses_distinct_empty_and_active_cells() {
        let today =
            NaiveDate::from_ymd_opt(/*year*/ 2026, /*month*/ 5, /*day*/ 29).expect("valid date");
        let buckets = vec![
            AccountTokenUsageDailyBucket {
                start_date: "2026-05-25".to_string(),
                tokens: 1,
            },
            AccountTokenUsageDailyBucket {
                start_date: "2026-05-29".to_string(),
                tokens: 4,
            },
        ];
        let cell = TokenActivityHistoryCell {
            view: TokenActivityView::Daily,
            state: Arc::new(RwLock::new(TokenActivityState::Loading)),
        };

        let rendered = cell
            .chart_lines(&buckets, today, /*width*/ 22)
            .into_iter()
            .map(|line| line.to_string().trim_end().to_string())
            .collect::<Vec<_>>()
            .join("\n");

        assert_snapshot!(rendered, @r"
             Apr     May
        Su □ □ □ □ □ □ □ □ □
        Mo □ □ □ □ □ □ □ □ ■
        Tu □ □ □ □ □ □ □ □ □
        We □ □ □ □ □ □ □ □ □
        Th □ □ □ □ □ □ □ □ □
        Fr □ □ □ □ □ □ □ □ ■
        Sa □ □ □ □ □ □ □ □

          Less □ ■ ■ ■ ■ More
          daily · weekly · cumulative
        ");
    }

    #[test]
    fn daily_graph_snapshot_stays_left_aligned_in_wide_terminal() {
        assert_eq!(graph_width(/*width*/ 160), 107);
        assert_eq!(graph_width(/*width*/ u16::MAX), u16::MAX);

        let today =
            NaiveDate::from_ymd_opt(/*year*/ 2026, /*month*/ 5, /*day*/ 29).expect("valid date");
        let cell = TokenActivityHistoryCell {
            view: TokenActivityView::Daily,
            state: Arc::new(RwLock::new(TokenActivityState::Loading)),
        };
        let lines = cell.chart_lines(&[], today, /*width*/ 160);
        let rendered = [&lines[0], &lines[1], lines.last().expect("legend line")]
            .into_iter()
            .map(|line| line.to_string().trim_end().to_string())
            .collect::<Vec<_>>()
            .join("\n");

        assert_snapshot!(rendered, @"
            Jun       Jul     Aug       Sep     Oct     Nov       Dec     Jan     Feb     Mar       Apr     May
         Su □ □ □ □ □ □ □ □ □ □ □ □ □ □ □ □ □ □ □ □ □ □ □ □ □ □ □ □ □ □ □ □ □ □ □ □ □ □ □ □ □ □ □ □ □ □ □ □ □ □ □ □
           daily · weekly · cumulative
        ");
    }

    #[test]
    fn summary_snapshot_left_aligns_and_splits_when_needed() {
        let response = GetAccountTokenUsageResponse {
            summary: AccountTokenUsageSummary {
                lifetime_tokens: Some(21_400_000_000),
                peak_daily_tokens: Some(835_000_000),
                longest_running_turn_sec: Some(13_920),
                current_streak_days: Some(54),
                longest_streak_days: Some(54),
            },
            daily_usage_buckets: None,
        };
        let rendered = |width| {
            summary_lines(&response, graph_width(width))
                .into_iter()
                .map(|line| line.to_string().trim_end().to_string())
                .collect::<Vec<_>>()
                .join("\n")
        };

        assert_snapshot!(
            format!(
                "wide:\n{}\n\nnarrow:\n{}\n\ntight:\n{}",
                rendered(/*width*/ 120),
                rendered(/*width*/ 80),
                rendered(/*width*/ 62)
            ),
            @"
        wide:
         Lifetime 21.4B · Peak 835M · Streak 54d · Longest task 3h 52m

        narrow:
         Lifetime 21.4B · Peak 835M · Streak 54d · Longest task 3h 52m

        tight:
         Lifetime 21.4B · Peak 835M · Streak 54d
         Longest task 3h 52m
        "
        );
    }
}
