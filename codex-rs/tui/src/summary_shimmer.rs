//! Paint a glimmer across the visible terminal cells of a status heading.
//! On a handoff, fade the outgoing text before revealing the new heading
//! during its first sweep. Later sweeps only change its brightness.

use std::time::Duration;

use ratatui::buffer::CellWidth;
use ratatui::style::Style;
use ratatui::text::Span;
use unicode_segmentation::UnicodeSegmentation;

use super::HeaderSample;
use crate::color::blend;
use crate::line_truncation::span_width;
use crate::motion::MotionMode;
use crate::terminal_palette::StdoutColorLevel;
use crate::terminal_palette::default_colors;
use crate::terminal_palette::effective_stdout_color_level;
use crate::terminal_palette::rgb_color;
use crate::text_formatting::contains_rtl_or_arabic_number;

const START_DELAY: Duration = Duration::from_millis(/*millis*/ 600);
const SWEEP_SECONDS: f64 = 1.0;
const INTERVAL_SECONDS: f64 = 4.0;
const FADE_SECONDS: f64 = 0.1;
const REVEAL_SECONDS: f64 = 0.74;
const SOFT_EDGE_COLUMNS: f64 = 2.0;

pub(super) fn summary_shimmer(
    text: &str,
    elapsed: Duration,
    motion: MotionMode,
    columns: usize,
    sample: Option<&HeaderSample>,
) -> Vec<Span<'static>> {
    if motion == MotionMode::Reduced {
        return vec![text.to_owned().into()];
    }
    let text = sample.map_or(text, |s| &s.header.text);
    let Some((fg, bg)) = palette(text) else {
        return vec![Span::styled(text.to_owned(), Style::default().dim())];
    };
    let (sweep_time, reveal) = if let Some(outgoing) = sample.and_then(|s| s.outgoing.as_ref()) {
        let seconds = elapsed.as_secs_f64();
        if seconds < FADE_SECONDS {
            let alpha = 0.5 * (1.0 - seconds / FADE_SECONDS);
            let style = Style::default().fg(rgb_color(blend(fg, bg, alpha as f32)));
            return vec![Span::styled(outgoing.clone(), style)];
        }
        (elapsed, (1.08 * seconds / REVEAL_SECONDS - 0.08).min(1.0))
    } else {
        (elapsed.saturating_sub(START_DELAY), 1.0)
    };
    let width = span_width(&Span::raw(text)).min(columns) as f64;
    // Keep a broad band on short labels and scale it to the visible cells on long ones.
    let half_width = (width * 0.25).max(/*other*/ 3.0);
    let edge = reveal * (width + SOFT_EDGE_COLUMNS);
    let sweep = (sweep_time.as_secs_f64() % INTERVAL_SECONDS).min(SWEEP_SECONDS);
    // Desktop moves the center from -25% to 150%. Let the minimum-width
    // band travel far enough to clear both ends even on a short label.
    let travel = (1.75 * width).max(width + 2.0 * half_width);
    let position = sweep / SWEEP_SECONDS * travel - half_width;
    let mut column = 0.0;
    text.graphemes(/*is_extended*/ true)
        .map(|grapheme| {
            let glyph_width = grapheme.cell_width();
            let center = column + f64::from(glyph_width) / 2.0;
            column += f64::from(glyph_width);
            let distance = ((center - position).abs() / half_width).min(/*other*/ 1.0);
            let intensity = 0.5 * (1.0 + (std::f64::consts::PI * distance).cos());
            let opacity = ((edge - center) / SOFT_EDGE_COLUMNS).clamp(0.0, 1.0);
            let content = if opacity == 0.0 {
                // Color emoji and terminal contrast correction ignore matching fg and bg.
                " ".repeat(usize::from(glyph_width))
            } else {
                grapheme.to_owned()
            };
            let alpha = ((0.5 + 0.5 * intensity) * opacity) as f32;
            let style = Style::default().fg(rgb_color(blend(fg, bg, alpha)));
            Span::styled(content, style)
        })
        .collect()
}

type Rgb = (u8, u8, u8);

pub(super) fn palette(text: &str) -> Option<(Rgb, Rgb)> {
    // Per-cell color updates break native bidirectional rendering.
    if contains_rtl_or_arabic_number(text)
        || effective_stdout_color_level() != StdoutColorLevel::TrueColor
    {
        return None;
    }
    let colors = default_colors()?;
    Some((colors.fg?, colors.bg))
}

#[cfg(test)]
#[path = "summary_shimmer_tests.rs"]
mod tests;
