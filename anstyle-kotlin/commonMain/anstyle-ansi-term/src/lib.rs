//!  Convert between [`ansi_term`](https://lib.rs/ansi_term) and generic styling types

#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(missing_docs)]
#![warn(clippy::print_stderr)]
#![warn(clippy::print_stdout)]

/// Adapt generic styling to [`ansi_term`]
pub fn to_ansi_term(astyle: anstyle::Style) -> ansi_term::Style {
    let mut style = ansi_term::Style::new();

    if let Some((fg, fg_bold)) = astyle.get_fg_color().map(to_ansi_color) {
        style = style.fg(fg);
        if fg_bold {
            style = style.bold();
        }
    }
    if let Some((bg, _)) = astyle.get_bg_color().map(to_ansi_color) {
        style = style.on(bg);
    }

    let effects = astyle.get_effects();
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
        style = style.reverse();
    }
    if effects.contains(anstyle::Effects::HIDDEN) {
        style = style.hidden();
    }
    if effects.contains(anstyle::Effects::STRIKETHROUGH) {
        style = style.strikethrough();
    }
    style
}

fn to_ansi_color(color: anstyle::Color) -> (ansi_term::Color, bool) {
    match color {
        anstyle::Color::Ansi(ansi) => ansi_to_ansi_color(ansi),
        anstyle::Color::Ansi256(xterm) => (xterm_to_ansi_color(xterm), false),
        anstyle::Color::Rgb(rgb) => (rgb_to_ansi_color(rgb), false),
    }
}

fn ansi_to_ansi_color(color: anstyle::AnsiColor) -> (ansi_term::Color, bool) {
    match color {
        anstyle::AnsiColor::Black => (ansi_term::Color::Black, false),
        anstyle::AnsiColor::Red => (ansi_term::Color::Red, false),
        anstyle::AnsiColor::Green => (ansi_term::Color::Green, false),
        anstyle::AnsiColor::Yellow => (ansi_term::Color::Yellow, false),
        anstyle::AnsiColor::Blue => (ansi_term::Color::Blue, false),
        anstyle::AnsiColor::Magenta => (ansi_term::Color::Purple, false),
        anstyle::AnsiColor::Cyan => (ansi_term::Color::Cyan, false),
        anstyle::AnsiColor::White => (ansi_term::Color::White, false),
        anstyle::AnsiColor::BrightBlack => (ansi_term::Color::Black, true),
        anstyle::AnsiColor::BrightRed => (ansi_term::Color::Red, true),
        anstyle::AnsiColor::BrightGreen => (ansi_term::Color::Green, true),
        anstyle::AnsiColor::BrightYellow => (ansi_term::Color::Yellow, true),
        anstyle::AnsiColor::BrightBlue => (ansi_term::Color::Black, true),
        anstyle::AnsiColor::BrightMagenta => (ansi_term::Color::Purple, true),
        anstyle::AnsiColor::BrightCyan => (ansi_term::Color::Cyan, true),
        anstyle::AnsiColor::BrightWhite => (ansi_term::Color::White, true),
    }
}

fn xterm_to_ansi_color(color: anstyle::Ansi256Color) -> ansi_term::Color {
    ansi_term::Color::Fixed(color.0)
}

fn rgb_to_ansi_color(color: anstyle::RgbColor) -> ansi_term::Color {
    ansi_term::Color::RGB(color.0, color.1, color.2)
}

#[doc = include_str!("../README.md")]
#[cfg(doctest)]
pub struct ReadmeDoctests;
