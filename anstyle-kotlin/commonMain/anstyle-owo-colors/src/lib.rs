//!  Convert between [owo-colors](https://lib.rs/owo-colors) and generic styling types

#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(missing_docs)]
#![warn(clippy::print_stderr)]
#![warn(clippy::print_stdout)]

/// Adapt generic styling to [`owo_colors`]
pub fn to_owo_style(style: anstyle::Style) -> owo_colors::Style {
    let fg = style.get_fg_color().map(to_owo_colors);
    let bg = style.get_bg_color().map(to_owo_colors);
    let effects = style.get_effects();

    let mut style = owo_colors::Style::new();
    if let Some(fg) = fg {
        style = style.color(fg);
    }
    if let Some(bg) = bg {
        style = style.on_color(bg);
    }
    if effects.contains(anstyle::Effects::BOLD) {
        style = style.bold();
    }
    if effects.contains(anstyle::Effects::DIMMED) {
        style = style.dimmed();
    }
    if effects.contains(anstyle::Effects::ITALIC) {
        style = style.italic();
    }
    if effects.contains(anstyle::Effects::UNDERLINE) {
        style = style.underline();
    }
    if effects.contains(anstyle::Effects::BLINK) {
        style = style.blink();
    }
    if effects.contains(anstyle::Effects::INVERT) {
        style = style.reversed();
    }
    if effects.contains(anstyle::Effects::HIDDEN) {
        style = style.hidden();
    }
    if effects.contains(anstyle::Effects::STRIKETHROUGH) {
        style = style.strikethrough();
    }
    style
}

/// Adapt generic colors to [`owo_colors`]
pub fn to_owo_colors(color: anstyle::Color) -> owo_colors::DynColors {
    match color {
        anstyle::Color::Ansi(ansi) => owo_colors::DynColors::Ansi(ansi_to_owo_colors_color(ansi)),
        anstyle::Color::Ansi256(xterm) => {
            owo_colors::DynColors::Xterm(xterm_to_owo_colors_color(xterm))
        }
        anstyle::Color::Rgb(rgb) => {
            let (r, g, b) = rgb_to_owo_colors_color(rgb);
            owo_colors::DynColors::Rgb(r, g, b)
        }
    }
}

fn ansi_to_owo_colors_color(color: anstyle::AnsiColor) -> owo_colors::colored::Color {
    match color {
        anstyle::AnsiColor::Black => owo_colors::colored::Color::Black,
        anstyle::AnsiColor::Red => owo_colors::colored::Color::Red,
        anstyle::AnsiColor::Green => owo_colors::colored::Color::Green,
        anstyle::AnsiColor::Yellow => owo_colors::colored::Color::Yellow,
        anstyle::AnsiColor::Blue => owo_colors::colored::Color::Blue,
        anstyle::AnsiColor::Magenta => owo_colors::colored::Color::Magenta,
        anstyle::AnsiColor::Cyan => owo_colors::colored::Color::Cyan,
        anstyle::AnsiColor::White => owo_colors::colored::Color::White,
        anstyle::AnsiColor::BrightBlack => owo_colors::colored::Color::BrightBlack,
        anstyle::AnsiColor::BrightRed => owo_colors::colored::Color::BrightRed,
        anstyle::AnsiColor::BrightGreen => owo_colors::colored::Color::BrightGreen,
        anstyle::AnsiColor::BrightYellow => owo_colors::colored::Color::BrightYellow,
        anstyle::AnsiColor::BrightBlue => owo_colors::colored::Color::BrightBlue,
        anstyle::AnsiColor::BrightMagenta => owo_colors::colored::Color::BrightMagenta,
        anstyle::AnsiColor::BrightCyan => owo_colors::colored::Color::BrightCyan,
        anstyle::AnsiColor::BrightWhite => owo_colors::colored::Color::BrightWhite,
    }
}

fn xterm_to_owo_colors_color(color: anstyle::Ansi256Color) -> owo_colors::XtermColors {
    owo_colors::XtermColors::from(color.0)
}

fn rgb_to_owo_colors_color(color: anstyle::RgbColor) -> (u8, u8, u8) {
    (color.0, color.1, color.2)
}

#[doc = include_str!("../README.md")]
#[cfg(doctest)]
pub struct ReadmeDoctests;
