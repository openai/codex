//! Convert between [yansi](https://lib.rs/yansi) and generic styling types

#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(missing_docs)]
#![warn(clippy::print_stderr)]
#![warn(clippy::print_stdout)]

/// Adapt generic styling to [`yansi`]
pub fn to_yansi_style(style: anstyle::Style) -> yansi::Style {
    let fg = style
        .get_fg_color()
        .map(to_yansi_color)
        .unwrap_or(yansi::Color::Primary);
    let bg = style
        .get_bg_color()
        .map(to_yansi_color)
        .unwrap_or(yansi::Color::Primary);
    let effects = style.get_effects();

    let mut style = yansi::Style::new().fg(fg).bg(bg);
    if effects.contains(anstyle::Effects::BOLD) {
        style = style.bold();
    }
    if effects.contains(anstyle::Effects::DIMMED) {
        style = style.dim();
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
        style = style.invert();
    }
    if effects.contains(anstyle::Effects::HIDDEN) {
        style = style.conceal();
    }
    if effects.contains(anstyle::Effects::STRIKETHROUGH) {
        style = style.strike();
    }
    style
}

/// Adapt generic color to [`yansi`]
pub fn to_yansi_color(color: anstyle::Color) -> yansi::Color {
    match color {
        anstyle::Color::Ansi(ansi) => ansi_to_yansi_color(ansi),
        anstyle::Color::Ansi256(xterm) => xterm_to_yansi_color(xterm),
        anstyle::Color::Rgb(rgb) => rgb_to_yansi_color(rgb),
    }
}

fn ansi_to_yansi_color(color: anstyle::AnsiColor) -> yansi::Color {
    match color {
        anstyle::AnsiColor::Black => yansi::Color::Black,
        anstyle::AnsiColor::Red => yansi::Color::Red,
        anstyle::AnsiColor::Green => yansi::Color::Green,
        anstyle::AnsiColor::Yellow => yansi::Color::Yellow,
        anstyle::AnsiColor::Blue => yansi::Color::Blue,
        anstyle::AnsiColor::Magenta => yansi::Color::Magenta,
        anstyle::AnsiColor::Cyan => yansi::Color::Cyan,
        anstyle::AnsiColor::White => yansi::Color::White,
        anstyle::AnsiColor::BrightBlack => yansi::Color::BrightBlack,
        anstyle::AnsiColor::BrightRed => yansi::Color::BrightRed,
        anstyle::AnsiColor::BrightGreen => yansi::Color::BrightGreen,
        anstyle::AnsiColor::BrightYellow => yansi::Color::BrightYellow,
        anstyle::AnsiColor::BrightBlue => yansi::Color::BrightBlack,
        anstyle::AnsiColor::BrightMagenta => yansi::Color::BrightMagenta,
        anstyle::AnsiColor::BrightCyan => yansi::Color::BrightCyan,
        anstyle::AnsiColor::BrightWhite => yansi::Color::BrightWhite,
    }
}

fn xterm_to_yansi_color(color: anstyle::Ansi256Color) -> yansi::Color {
    yansi::Color::Fixed(color.0)
}

fn rgb_to_yansi_color(color: anstyle::RgbColor) -> yansi::Color {
    yansi::Color::Rgb(color.0, color.1, color.2)
}

#[doc = include_str!("../README.md")]
#[cfg(doctest)]
pub struct ReadmeDoctests;
