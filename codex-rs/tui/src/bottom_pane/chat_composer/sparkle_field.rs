//! Paint the deterministic Astra starfield without owning its eligibility, timer, or redraws.
//!
//! Only blank, unstyled true-color cells outside protected content and the terminal cursor can be
//! decorated. The caller supplies elapsed time and visibility to preserve the original appearance.

use std::time::Duration;

use ratatui::buffer::Buffer;
use ratatui::buffer::CellDiffOption;
use ratatui::layout::Position;
use ratatui::layout::Rect;
use ratatui::style::Color;
use unicode_width::UnicodeWidthStr;

use crate::color::blend;
use crate::terminal_palette::rgb_color;

pub(super) const DOTS: [&str; 8] = ["⠁", "⠂", "⠄", "⠈", "⠐", "⠠", "⡀", "⢀"];

pub(super) fn render_stars(
    area: Rect,
    cursor: Option<(u16, u16)>,
    protected_area: Option<Rect>,
    elapsed: Duration,
    foreground: (u8, u8, u8),
    visibility: f32,
    buf: &mut Buffer,
) {
    let time = elapsed.as_secs_f32();
    for y in area.y..area.bottom() {
        let mut occupied_until = area.x;
        for x in area.x..area.right() {
            let cell = &buf[(x, y)];
            if x < occupied_until {
                continue;
            }
            if cell.symbol() != " " {
                occupied_until = x.saturating_add(cell.symbol().width() as u16);
                continue;
            }
            if cursor == Some((x, y))
                || protected_area.is_some_and(|protected| protected.contains(Position::new(x, y)))
                || !cell.modifier.is_empty()
                || cell.diff_option != CellDiffOption::None
            {
                continue;
            }
            let Color::Rgb(r, g, b) = cell.bg else {
                continue;
            };
            let mut hash = u64::from(y - area.y) * 65537 + u64::from(x - area.x);
            hash = (hash ^ (hash >> 16)).wrapping_mul(/*rhs*/ 0x45d9f3b);
            hash = (hash ^ (hash >> 16)).wrapping_mul(/*rhs*/ 0x45d9f3b);
            hash ^= hash >> 16;
            if hash % 5 != 0 {
                continue;
            }
            let phase =
                (time / (4.0 + (hash % 31) as f32 / 10.0) + (hash % 997) as f32 / 997.0).fract();
            let brightness =
                (phase * std::f32::consts::PI).sin().powi(/*n*/ 12) * 0.55 * visibility;
            if brightness < 0.04 {
                continue;
            }
            buf[(x, y)]
                .set_symbol(DOTS[(hash / 161 % 8) as usize])
                .set_fg(rgb_color(blend(foreground, (r, g, b), brightness)));
        }
    }
}
