//! Convert between [termcolor](https://lib.rs/termcolor) and [generic styling types][anstyle]

#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(missing_docs)]
#![warn(clippy::print_stderr)]
#![warn(clippy::print_stdout)]

/// Adapt generic styling to [`termcolor`]
pub fn to_termcolor_spec(style: anstyle::Style) -> termcolor::ColorSpec {
    let fg = style.get_fg_color().map(to_termcolor_color);
    let bg = style.get_bg_color().map(to_termcolor_color);
    let effects = style.get_effects();

    let mut style = termcolor::ColorSpec::new();
    style.set_fg(fg);
    style.set_bg(bg);
    style.set_bold(effects.contains(anstyle::Effects::BOLD));
    style.set_dimmed(effects.contains(anstyle::Effects::DIMMED));
    style.set_italic(effects.contains(anstyle::Effects::ITALIC));
    style.set_underline(effects.contains(anstyle::Effects::UNDERLINE));
    style
}

/// Adapt generic colors to [`termcolor`]
pub fn to_termcolor_color(color: anstyle::Color) -> termcolor::Color {
    match color {
        anstyle::Color::Ansi(ansi) => ansi_to_termcolor_color(ansi),
        anstyle::Color::Ansi256(xterm) => xterm_to_termcolor_color(xterm),
        anstyle::Color::Rgb(rgb) => rgb_to_termcolor_color(rgb),
    }
}

fn ansi_to_termcolor_color(color: anstyle::AnsiColor) -> termcolor::Color {
    match color {
        anstyle::AnsiColor::Black => termcolor::Color::Black,
        anstyle::AnsiColor::Red => termcolor::Color::Red,
        anstyle::AnsiColor::Green => termcolor::Color::Green,
        anstyle::AnsiColor::Yellow => termcolor::Color::Yellow,
        anstyle::AnsiColor::Blue => termcolor::Color::Blue,
        anstyle::AnsiColor::Magenta => termcolor::Color::Magenta,
        anstyle::AnsiColor::Cyan => termcolor::Color::Cyan,
        anstyle::AnsiColor::White => termcolor::Color::White,
        anstyle::AnsiColor::BrightBlack => termcolor::Color::Black,
        anstyle::AnsiColor::BrightRed => termcolor::Color::Red,
        anstyle::AnsiColor::BrightGreen => termcolor::Color::Green,
        anstyle::AnsiColor::BrightYellow => termcolor::Color::Yellow,
        anstyle::AnsiColor::BrightBlue => termcolor::Color::Black,
        anstyle::AnsiColor::BrightMagenta => termcolor::Color::Magenta,
        anstyle::AnsiColor::BrightCyan => termcolor::Color::Cyan,
        anstyle::AnsiColor::BrightWhite => termcolor::Color::White,
    }
}

fn xterm_to_termcolor_color(color: anstyle::Ansi256Color) -> termcolor::Color {
    termcolor::Color::Ansi256(color.0)
}

fn rgb_to_termcolor_color(color: anstyle::RgbColor) -> termcolor::Color {
    termcolor::Color::Rgb(color.0, color.1, color.2)
}

#[doc = include_str!("../README.md")]
#[cfg(doctest)]
pub struct ReadmeDoctests;
