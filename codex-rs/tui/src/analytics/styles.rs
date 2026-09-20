//! Charts and legends share the terminal's ANSI palette.
//! Secondary text blends probed defaults only when the terminal supports that color depth.

use crate::color::blend;
use crate::style::accent_style;
use crate::terminal_palette::StdoutColorLevel;
use crate::terminal_palette::best_color;
use crate::terminal_palette::default_bg;
use crate::terminal_palette::default_fg;
use crate::terminal_palette::effective_stdout_color_level;
use ratatui::style::Color;
use ratatui::style::Style;
use ratatui::text::Span;

pub(super) fn series_colors() -> [Color; 4] {
    [Color::Cyan, Color::Magenta, Color::Green, Color::Reset]
}

pub(super) fn number(text: impl Into<String>) -> Span<'static> {
    Span::styled(text.into(), accent_style())
}

/// Labels and instructions need more contrast than decorative rules or inactive values.
pub(super) fn secondary_style() -> Style {
    if !matches!(
        effective_stdout_color_level(),
        StdoutColorLevel::TrueColor | StdoutColorLevel::Ansi256
    ) {
        return Style::default().dim();
    }
    match (default_fg(), default_bg()) {
        (Some(fg), Some(bg)) => {
            Style::default().fg(best_color(blend(fg, bg, /*alpha*/ 0.70)))
        }
        _ => Style::default().dim(),
    }
}

/// Paint one selectable row, including its padding, using the shared picker accent.
pub(super) fn select_row(line: &mut ratatui::text::Line<'static>, width: usize) {
    let style = crate::bottom_pane::selection_style();
    line.spans
        .push(" ".repeat(width.saturating_sub(line.width())).into());
    line.style = style;
    for span in &mut line.spans {
        span.style = style;
    }
}
