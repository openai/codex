//! Builds terminal-aware styles and glyph choices for token activity charts.
//!
//! The palette adapts theme colors to the active terminal color level while
//! keeping chart-specific glyph policy local to the token activity renderer.

use ratatui::style::Color;
use ratatui::style::Style;
use ratatui::style::Stylize;

use super::TokenActivityView;
use crate::color::blend;
use crate::render::highlight::foreground_style_for_scopes;
use crate::style::accent_style;
use crate::terminal_palette::StdoutColorLevel;
use crate::terminal_palette::best_color_for_level;
use crate::terminal_palette::default_bg;
use crate::terminal_palette::default_fg;
use crate::terminal_palette::stdout_color_level;

// In low-color terminals we distinguish empty vs active cells by glyph (a
// width-matched filled/hollow pair). In truecolor terminals the grid uses a
// single glyph and lets color carry the intensity (GitHub-style), which keeps
// the grid perfectly aligned and free of texture noise.
const EMPTY_CELL_GLYPH: &str = "□";
const ACTIVE_CELL_GLYPH: &str = "■";
const BAR_CELL_GLYPH: &str = "█";

/// Stores the terminal-specific styles and glyph strategy for token activity cells.
pub(super) struct TokenActivityPalette {
    styles: [Style; 5],
    bar_style: Style,
    /// True when the terminal supports a truecolor gradient, so the grid can
    /// encode intensity purely by color and render every cell with a single
    /// glyph. False on low-color terminals, where we fall back to a
    /// filled/hollow glyph pair so empty and active cells stay distinguishable.
    uses_color: bool,
}

impl TokenActivityPalette {
    pub(super) fn current() -> Self {
        Self::from_parts(
            default_fg(),
            default_bg(),
            stdout_color_level(),
            theme_activity_style(),
        )
    }

    fn from_parts(
        default_fg: Option<(u8, u8, u8)>,
        default_bg: Option<(u8, u8, u8)>,
        color_level: StdoutColorLevel,
        active_style: Style,
    ) -> Self {
        let fallback_palette = || Self::fallback(active_style);
        let (Some(fg), Some(bg), Some(anchor)) =
            (default_fg, default_bg, activity_anchor_rgb(active_style))
        else {
            return fallback_palette();
        };
        if matches!(
            color_level,
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
            Style::default().fg(best_color_for_level(color, color_level))
        });
        let bar_style = Style::default().fg(best_color_for_level(
            blend(anchor, bg, /*alpha*/ 0.78),
            color_level,
        ));
        Self {
            styles,
            bar_style,
            uses_color: true,
        }
    }

    fn fallback(active_style: Style) -> Self {
        let empty_style = Style::default().dim();
        Self {
            styles: [
                empty_style,
                active_style,
                active_style,
                active_style,
                active_style,
            ],
            bar_style: active_style,
            uses_color: false,
        }
    }

    pub(super) fn for_level(&self, level: usize) -> Style {
        self.styles[level.min(/*other*/ 4)]
    }

    pub(super) fn for_bar_level(&self, level: usize) -> Style {
        if level == 0 {
            self.for_level(/*level*/ 0)
        } else {
            self.bar_style
        }
    }

    /// The glyph for a cell at `level`. Daily truecolor renders every visible
    /// cell with the same square glyph and lets color carry the intensity; in
    /// low-color we use the hollow glyph for empty cells so they remain visible
    /// without a color gradient. Bar views use full blocks for filled height and
    /// spaces for empty height so the silhouette reads as a column chart.
    pub(super) fn glyph(&self, view: TokenActivityView, level: usize) -> &'static str {
        if view != TokenActivityView::Daily {
            return if level == 0 { " " } else { BAR_CELL_GLYPH };
        }
        if self.uses_color || level > 0 {
            ACTIVE_CELL_GLYPH
        } else {
            EMPTY_CELL_GLYPH
        }
    }
}

fn theme_activity_style() -> Style {
    foreground_style_for_scopes(&["entity.name.type", "support.type", "variable"])
        .unwrap_or_else(accent_style)
        .bold()
}

fn activity_anchor_rgb(style: Style) -> Option<(u8, u8, u8)> {
    match style.fg? {
        Color::Rgb(r, g, b) => Some((r, g, b)),
        _ => None,
    }
}

#[cfg(test)]
#[path = "palette_tests.rs"]
mod tests;
