use crate::color::blend;
use crate::color::is_light;
use crate::color::perceptual_distance;
use crate::terminal_palette::basic_palette;
use crate::terminal_palette::terminal_palette;
use ratatui::style::Color;
use ratatui::style::Style;

/// Returns the style for a user-authored message using the provided terminal background.
pub fn user_message_style(terminal_bg: Option<(u8, u8, u8)>) -> Style {
    match terminal_bg {
        Some(bg) => Style::default().bg(user_message_bg(bg)),
        None => Style::default(),
    }
}

#[allow(clippy::disallowed_methods)]
pub fn user_message_bg(terminal_bg: (u8, u8, u8)) -> Color {
    // Determine a reference "top" color to blend toward. For dark backgrounds we lighten
    // the mix with white, and for light backgrounds we darken it with black.
    let top = if is_light(terminal_bg) {
        (0, 0, 0)
    } else {
        (255, 255, 255)
    };
    let bottom = terminal_bg;
    let Some(mut color_level) = supports_color::on_cached(supports_color::Stream::Stdout) else {
        return Color::default();
    };

    #[cfg(windows)]
    // Windows Terminal has been the default shell application for Windows since October 2022
    // and has supported truecolor even longer. However it usually does not set COLORTERM to indicate that.
    // so this is a pretty safe heuristic.
    if std::env::var_os("WT_SESSION").is_some() {
        color_level.has_16m = true;
    }

    let target = blend(top, bottom, 0.1);
    if color_level.has_16m {
        // In truecolor terminals we can use the exact RGB value.
        let (r, g, b) = target;
        Color::Rgb(r, g, b)
    } else if color_level.has_256
        && let Some(palette) = terminal_palette()
        && let Some((i, _)) = palette.into_iter().enumerate().min_by(|(_, a), (_, b)| {
            perceptual_distance(*a, target)
                .partial_cmp(&perceptual_distance(*b, target))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    {
        // If we have a captured 256-color palette, pick whichever indexed color is
        // perceptually closest to the blended target.
        Color::Indexed(i as u8)
    } else if color_level.has_basic {
        if let Some(palette) = basic_palette() {
            // On Windows terminals the palette is configurable, so evaluate the actual
            // runtime color table to keep the blended shading aligned with custom themes.
            closest_runtime_basic_color(target, terminal_bg, &palette)
        } else {
            // Finally, degrade to the well-known ANSI 16-color defaults using a perceptual
            // distance match.
            closest_basic_color(target, terminal_bg)
        }
    } else {
        Color::default()
    }
}

fn closest_runtime_basic_color(
    target: (u8, u8, u8),
    terminal_bg: (u8, u8, u8),
    palette: &[(u8, u8, u8); 16],
) -> Color {
    select_basic_palette_color(
        target,
        terminal_bg,
        palette.iter().enumerate().filter_map(|(idx, rgb)| {
            BASIC_TERMINAL_COLORS
                .get(idx)
                .map(|(color, _)| (*rgb, *color))
        }),
    )
}

fn closest_basic_color(target: (u8, u8, u8), terminal_bg: (u8, u8, u8)) -> Color {
    // Iterate through the baked-in ANSI colors and return whichever one is closest to the
    // desired RGB shade while maintaining contrast with the background.
    select_basic_palette_color(
        target,
        terminal_bg,
        BASIC_TERMINAL_COLORS
            .iter()
            .map(|(color, rgb)| (*rgb, *color)),
    )
}

const MIN_PERCEPTUAL_DISTANCE: f32 = 6.0;

fn select_basic_palette_color(
    target: (u8, u8, u8),
    terminal_bg: (u8, u8, u8),
    entries: impl Iterator<Item = ((u8, u8, u8), Color)>,
) -> Color {
    let mut best = None;
    let mut fallback = None;
    for (rgb, color) in entries {
        let dist = perceptual_distance(rgb, target);
        if fallback.is_none_or(|(_, best_dist)| dist < best_dist) {
            fallback = Some((color, dist));
        }
        if perceptual_distance(rgb, terminal_bg) > MIN_PERCEPTUAL_DISTANCE
            && best.is_none_or(|(_, best_dist)| dist < best_dist)
        {
            best = Some((color, dist));
        }
    }
    best.or(fallback)
        .map(|(color, _)| color)
        .unwrap_or(Color::default())
}

// Mapping of ANSI color indices to approximate RGB tuples. These values mirror Windows'
// default console palette so the fallback path stays visually consistent across platforms.
const BASIC_TERMINAL_COLORS: [(Color, (u8, u8, u8)); 16] = [
    (Color::Black, (0, 0, 0)),
    (Color::Blue, (0, 0, 128)),
    (Color::Green, (0, 128, 0)),
    (Color::Cyan, (0, 128, 128)),
    (Color::Red, (128, 0, 0)),
    (Color::Magenta, (128, 0, 128)),
    (Color::Yellow, (128, 128, 0)),
    (Color::Gray, (192, 192, 192)),
    (Color::DarkGray, (128, 128, 128)),
    (Color::LightBlue, (0, 0, 255)),
    (Color::LightGreen, (0, 255, 0)),
    (Color::LightCyan, (0, 255, 255)),
    (Color::LightRed, (255, 0, 0)),
    (Color::LightMagenta, (255, 0, 255)),
    (Color::LightYellow, (255, 255, 0)),
    (Color::White, (255, 255, 255)),
];
