//! Daily stacked bars with fixed calendar ticks, a selection cursor, and an anchored value.
//! Relative usage retains the server's scale; signed credits keep both bands.

mod annotations;
mod layout;
mod painting;

use self::layout::Layout;
use self::painting::Band;
use super::data;
use super::data::compact_amount as tick;
use super::render::parts;
use crate::analytics::models::AccountAnalyticsDay;
use crate::analytics::models::AccountAnalyticsUnit;
use crate::analytics::models::AccountAnalyticsValue;
use chrono::NaiveDate;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::text::Span;
use unicode_width::UnicodeWidthStr;

/// Share the chart's scale with the selected-day summary, including signed values.
pub(super) fn peak(
    days: &[(NaiveDate, Option<&AccountAnalyticsDay>)],
    categories: &[AccountAnalyticsValue],
) -> f64 {
    days.iter()
        .filter_map(|(_, day)| *day)
        .map(|day| {
            parts(&day.values, categories)
                .iter()
                .map(|value| value.abs())
                .sum::<f64>()
                .max(day.total.abs())
        })
        .fold(/*init*/ 0.0, f64::max)
}

/// Two readable intervals cover the range; counts never use fractional ticks.
pub(super) fn axis_max(peak: f64, unit: AccountAnalyticsUnit) -> f64 {
    if peak == 0.0 {
        return if matches!(
            unit,
            AccountAnalyticsUnit::Count | AccountAnalyticsUnit::Tokens
        ) {
            2.0
        } else {
            1.0
        };
    }
    let step = if matches!(
        unit,
        AccountAnalyticsUnit::Count | AccountAnalyticsUnit::Tokens
    ) {
        (peak / 2.0).max(/*other*/ 1.0)
    } else {
        peak / 2.0
    };
    let magnitude = 10.0_f64.powf(step.log10().floor());
    let normalized = step / magnitude;
    let multiple = [1.0, 1.5, 2.0, 2.5, 3.0, 4.0, 5.0, 7.5, 10.0]
        .into_iter()
        .find(|multiple| {
            *multiple >= normalized
                && (!matches!(
                    unit,
                    AccountAnalyticsUnit::Count | AccountAnalyticsUnit::Tokens
                ) || (multiple * magnitude).fract() == 0.0)
        })
        .unwrap_or(/*default*/ 10.0);
    let ceiling = multiple * magnitude * 2.0;
    if ceiling.is_finite() && ceiling >= peak {
        ceiling
    } else {
        peak
    }
}

/// Rendered chart geometry exposes its positive and optional negative value bands.
#[derive(Default)]
pub(super) struct Chart {
    pub(super) lines: Vec<Line<'static>>,
    pub(super) bands: usize,
}

/// Render a calendar selection; the cursor must index days and dimensions fit the terminal.
pub(super) fn chart(
    days: &[(NaiveDate, Option<&AccountAnalyticsDay>)],
    categories: &[AccountAnalyticsValue],
    cursor: usize,
    width: usize,
    height: usize,
    unit: AccountAnalyticsUnit,
) -> Chart {
    if days.is_empty() || width == 0 {
        return Chart::default();
    }
    let values = days
        .iter()
        .map(|(_, day)| {
            day.map_or(
                /*default*/ [0.0; 4],
                |day| parts(&day.values, categories),
            )
        })
        .collect::<Vec<_>>();
    let bands = if values.iter().flatten().any(|value| *value < 0.0) {
        &[Band::Positive, Band::Negative][..]
    } else {
        &[Band::Positive][..]
    };
    let peak = axis_max(peak(days, categories), unit);
    let label_width = tick(peak).width().max(tick(peak / 2.0).width()) + bands.len() - 1 + 2;
    let layout = Layout::new(days.len(), cursor, width, height, label_width, bands);
    let renderer = Renderer {
        days,
        values,
        cursor,
        peak,
        unit,
        bands,
        layout,
    };
    let mut buf = Buffer::empty(Rect::new(
        /*x*/ 0,
        /*y*/ 0,
        width as u16,
        (layout.marker_row + 2) as u16,
    ));
    renderer.paint_bars(&mut buf);
    renderer.annotate_missing(&mut buf);
    renderer.paint_axis(&mut buf);
    renderer.annotate_selection(&mut buf);
    renderer.annotate_calendar(&mut buf);
    let lines = buf
        .content
        .chunks(width)
        .map(|row| {
            Line::from(
                row.iter()
                    .map(|cell| Span::styled(cell.symbol().to_owned(), cell.style()))
                    .collect::<Vec<_>>(),
            )
        })
        .collect();
    Chart {
        lines,
        bands: bands.len(),
    }
}

/// Shared inputs for painting and annotations; all phases use the same calendar geometry.
struct Renderer<'a> {
    days: &'a [(NaiveDate, Option<&'a AccountAnalyticsDay>)],
    values: Vec<[f64; 4]>,
    cursor: usize,
    peak: f64,
    unit: AccountAnalyticsUnit,
    bands: &'static [Band],
    layout: Layout,
}

fn amount(value: f64, unit: AccountAnalyticsUnit) -> String {
    match unit {
        AccountAnalyticsUnit::RelativeUsage => data::amount(value),
        AccountAnalyticsUnit::Count | AccountAnalyticsUnit::Tokens => {
            data::compact_amount(value.round())
        }
        AccountAnalyticsUnit::Credits => data::credit_amount(value),
    }
}

#[cfg(test)]
#[path = "plot_tests.rs"]
mod tests;
