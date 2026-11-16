//! Smooth, whole-grapheme status shimmer with the desktop's cadenced sweep.
//!
//! Only brightness changes: the moving band uses the terminal foreground, while
//! the remaining text blends halfway into the background. The wave spans at
//! least six terminal columns so short labels do not flash one letter at a time.
//! New headers wait 600 ms, then sweep for one second every four seconds.
//! Unknown palettes use static dim text instead of a stepped animation.

use std::time::Duration;

use ratatui::style::Style;
use ratatui::text::Span;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::color::blend;
use crate::motion::MotionMode;
use crate::terminal_palette::StdoutColorLevel;
use crate::terminal_palette::default_bg;
use crate::terminal_palette::default_fg;
use crate::terminal_palette::effective_stdout_color_level;
use crate::terminal_palette::rgb_color;
use crate::text_formatting::contains_rtl_or_arabic_number;

// Match the desktop app's thinking and reasoning header timings.
const START_DELAY: Duration = Duration::from_millis(/*millis*/ 600);
const SWEEP_SECONDS: f64 = 1.0;
const INTERVAL_SECONDS: f64 = 4.0;

pub(crate) fn summary_shimmer(
    text: &str,
    elapsed: Duration,
    motion: MotionMode,
) -> Vec<Span<'static>> {
    if motion == MotionMode::Reduced {
        return vec![text.to_owned().into()];
    }
    // Per-grapheme color updates redraw individual terminal cells and break native bidi
    // rendering. Keep bidi-sensitive headers as one stable span in logical order.
    if contains_rtl_or_arabic_number(text) {
        return vec![Span::styled(text.to_owned(), Style::default().dim())];
    }
    let (StdoutColorLevel::TrueColor, Some(fg), Some(bg)) =
        (effective_stdout_color_level(), default_fg(), default_bg())
    else {
        return vec![Span::styled(text.to_owned(), Style::default().dim())];
    };
    let width = text.width() as f64;
    let half_width = (width * 0.1).max(/*other*/ 3.0);
    let sweep =
        (elapsed.saturating_sub(START_DELAY).as_secs_f64() % INTERVAL_SECONDS).min(SWEEP_SECONDS);
    let position = sweep / SWEEP_SECONDS * (width + 2.0 * half_width) - half_width;
    let mut column = 0.0;
    text.graphemes(/*is_extended*/ true)
        .map(|grapheme| {
            let glyph_width = grapheme.width() as f64;
            let center = column + glyph_width / 2.0;
            column += glyph_width;
            let distance = ((center - position).abs() / half_width).min(/*other*/ 1.0);
            let intensity = 0.5 * (1.0 + (std::f64::consts::PI * distance).cos());
            let alpha = (0.5 + 0.5 * intensity) as f32;
            let style = Style::default().fg(rgb_color(blend(fg, bg, alpha)));
            Span::styled(grapheme.to_owned(), style)
        })
        .collect()
}

#[cfg(test)]
#[path = "summary_shimmer_tests.rs"]
mod tests;
