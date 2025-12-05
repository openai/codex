//! Convert between [`crossterm`](https://lib.rs/crossterm) and [generic styling types][anstyle]

#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(missing_docs)]
#![warn(clippy::print_stderr)]
#![warn(clippy::print_stdout)]

/// Adapt generic styling to [`crossterm`]
pub fn to_crossterm(astyle: anstyle::Style) -> crossterm::style::ContentStyle {
    let foreground_color = astyle.get_fg_color().map(to_ansi_color);
    let background_color = astyle.get_bg_color().map(to_ansi_color);
    let underline_color = astyle.get_underline_color().map(to_ansi_color);

    let mut attributes = crossterm::style::Attributes::default();

    let effects = astyle.get_effects();
    if effects.contains(anstyle::Effects::BOLD) {
        attributes.set(crossterm::style::Attribute::Bold);
    }
    if effects.contains(anstyle::Effects::DIMMED) {
        attributes.set(crossterm::style::Attribute::Dim);
    }
    if effects.contains(anstyle::Effects::ITALIC) {
        attributes.set(crossterm::style::Attribute::Italic);
    }
    if effects.contains(anstyle::Effects::UNDERLINE) {
        attributes.set(crossterm::style::Attribute::Underlined);
    }
    if effects.contains(anstyle::Effects::BLINK) {
        attributes.set(crossterm::style::Attribute::SlowBlink);
    }
    if effects.contains(anstyle::Effects::INVERT) {
        attributes.set(crossterm::style::Attribute::Reverse);
    }
    if effects.contains(anstyle::Effects::HIDDEN) {
        attributes.set(crossterm::style::Attribute::Hidden);
    }
    if effects.contains(anstyle::Effects::STRIKETHROUGH) {
        attributes.set(crossterm::style::Attribute::OverLined);
    }

    crossterm::style::ContentStyle {
        foreground_color,
        background_color,
        underline_color,
        attributes,
    }
}

fn to_ansi_color(color: anstyle::Color) -> crossterm::style::Color {
    match color {
        anstyle::Color::Ansi(ansi) => ansi_to_ansi_color(ansi),
        anstyle::Color::Ansi256(xterm) => xterm_to_ansi_color(xterm),
        anstyle::Color::Rgb(rgb) => rgb_to_ansi_color(rgb),
    }
}

fn ansi_to_ansi_color(color: anstyle::AnsiColor) -> crossterm::style::Color {
    match color {
        anstyle::AnsiColor::Black => crossterm::style::Color::Black,
        anstyle::AnsiColor::Red => crossterm::style::Color::DarkRed,
        anstyle::AnsiColor::Green => crossterm::style::Color::DarkGreen,
        anstyle::AnsiColor::Yellow => crossterm::style::Color::DarkYellow,
        anstyle::AnsiColor::Blue => crossterm::style::Color::DarkBlue,
        anstyle::AnsiColor::Magenta => crossterm::style::Color::DarkMagenta,
        anstyle::AnsiColor::Cyan => crossterm::style::Color::DarkCyan,
        anstyle::AnsiColor::White => crossterm::style::Color::Grey,
        anstyle::AnsiColor::BrightBlack => crossterm::style::Color::DarkGrey,
        anstyle::AnsiColor::BrightRed => crossterm::style::Color::Red,
        anstyle::AnsiColor::BrightGreen => crossterm::style::Color::Green,
        anstyle::AnsiColor::BrightYellow => crossterm::style::Color::Yellow,
        anstyle::AnsiColor::BrightBlue => crossterm::style::Color::Blue,
        anstyle::AnsiColor::BrightMagenta => crossterm::style::Color::Magenta,
        anstyle::AnsiColor::BrightCyan => crossterm::style::Color::Cyan,
        anstyle::AnsiColor::BrightWhite => crossterm::style::Color::White,
    }
}

fn xterm_to_ansi_color(color: anstyle::Ansi256Color) -> crossterm::style::Color {
    crossterm::style::Color::AnsiValue(color.0)
}

fn rgb_to_ansi_color(color: anstyle::RgbColor) -> crossterm::style::Color {
    crossterm::style::Color::Rgb {
        r: color.0,
        g: color.1,
        b: color.2,
    }
}

#[doc = include_str!("../README.md")]
#[cfg(doctest)]
pub struct ReadmeDoctests;
