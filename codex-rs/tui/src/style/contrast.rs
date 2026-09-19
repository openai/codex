//! Resolve informative foregrounds against their actual background before palette reduction.
//! Missing background samples preserve the requested palette color; unknown color capabilities
//! and user-defined ANSI palettes retain the terminal's default foreground.

use crate::color::blend;
use crate::color::is_light;
use crate::color::perceptual_distance;
use crate::terminal_palette::StdoutColorLevel;
use crate::terminal_palette::XTERM_COLORS;
use crate::terminal_palette::best_color_for_level;
use crate::terminal_palette::indexed_color;
use crate::terminal_palette::rgb_color;
use ratatui::style::Color;
use ratatui::style::Style;
use std::cell::RefCell;
use std::collections::HashMap;

const MIN_TEXT_CONTRAST: f64 = 4.5;
const MAX_CACHED_FOREGROUNDS: usize = 512;

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
struct ForegroundKey {
    preferred: (u8, u8, u8),
    background: Option<(u8, u8, u8)>,
    level: StdoutColorLevel,
}

thread_local! {
    // Diff spans reuse a small palette. Bound storage even when themes contain many colors.
    // Resolved backgrounds and capabilities are part of the key, so probe/theme changes cannot
    // reuse a result for the previous surface.
    static FOREGROUNDS: RefCell<HashMap<ForegroundKey, Color>> = RefCell::default();
}

pub(super) fn selection_style(background: Option<(u8, u8, u8)>, level: StdoutColorLevel) -> Style {
    let fallback = Style::default()
        .fg(Color::Reset)
        .bg(Color::Reset)
        .bold()
        .not_dim()
        .reversed();
    let Some(background) = background else {
        return fallback;
    };
    let (preferred, alternate) = if is_light(background) {
        (super::CHATGPT_BLUE_100, super::CHATGPT_BLUE_200)
    } else {
        (super::CHATGPT_BLUE_200, super::CHATGPT_BLUE_100)
    };
    let mut fill = best_color_for_level(preferred, level);
    let resolve = |color| match color {
        Color::Rgb(r, g, b) => Some((r, g, b)),
        Color::Indexed(index) if index >= 16 => Some(XTERM_COLORS[usize::from(index)]),
        _ => None,
    };
    let Some(mut fill_rgb) = resolve(fill) else {
        return fallback;
    };
    // A subtle fill is intentional, but it must not disappear into a similarly blue canvas.
    if ratio(fill_rgb, background) < 1.25 {
        let alternate_fill = best_color_for_level(alternate, level);
        let alternate_rgb = resolve(alternate_fill).unwrap_or(alternate);
        if ratio(alternate_rgb, background) > ratio(fill_rgb, background) {
            fill = alternate_fill;
            fill_rgb = alternate_rgb;
        }
    }
    Style::default()
        .fg(foreground(
            /*preferred*/ (0, 0, 46),
            Some(fill_rgb),
            level,
        ))
        .bg(fill)
        .bold()
        .not_dim()
        .not_reversed()
}

pub(super) fn foreground(
    preferred: (u8, u8, u8),
    background: Option<(u8, u8, u8)>,
    level: StdoutColorLevel,
) -> Color {
    FOREGROUNDS.with(|cache| {
        let key = ForegroundKey {
            preferred,
            background,
            level,
        };
        let mut cache = cache.borrow_mut();
        if let Some(color) = cache.get(&key) {
            return *color;
        }
        let color = match (background, level) {
            (None, _) => best_color_for_level(preferred, level),
            (Some(background), StdoutColorLevel::TrueColor) => {
                if ratio(preferred, background) >= MIN_TEXT_CONTRAST {
                    rgb_color(preferred)
                } else {
                    let black = (0, 0, 0);
                    let white = (255, 255, 255);
                    let endpoint = if ratio(black, background) >= ratio(white, background) {
                        black
                    } else {
                        white
                    };
                    // Bounded search preserves as much of the requested hue as the surface allows.
                    (1..=255)
                        .map(|step| blend(endpoint, preferred, step as f32 / 255.0))
                        .find(|candidate| ratio(*candidate, background) >= MIN_TEXT_CONTRAST)
                        .map_or_else(|| rgb_color(endpoint), rgb_color)
                }
            }
            (Some(background), StdoutColorLevel::Ansi256) => XTERM_COLORS
                .iter()
                .enumerate()
                .skip(/*n*/ 16)
                .filter(|(_, color)| ratio(**color, background) >= MIN_TEXT_CONTRAST)
                .min_by(|(_, a), (_, b)| {
                    perceptual_distance(**a, preferred)
                        .total_cmp(&perceptual_distance(**b, preferred))
                })
                .map_or(Color::Reset, |(index, _)| indexed_color(index as u8)),
            (Some(_), StdoutColorLevel::Ansi16 | StdoutColorLevel::Unknown) => Color::Reset,
        };
        if cache.len() >= MAX_CACHED_FOREGROUNDS {
            cache.clear();
        }
        cache.insert(key, color);
        color
    })
}

fn ratio(a: (u8, u8, u8), b: (u8, u8, u8)) -> f64 {
    let a = luminance(a);
    let b = luminance(b);
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

fn luminance(rgb: (u8, u8, u8)) -> f64 {
    let [r, g, b] = [rgb.0, rgb.1, rgb.2].map(|channel| {
        let value = f64::from(channel) / 255.0;
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(/*n*/ 2.4)
        }
    });
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

#[cfg(test)]
#[path = "contrast_tests.rs"]
mod tests;
