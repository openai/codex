//! Convert from ansi stylings to ROFF Control Lines
//! Currently uses [roff](https://docs.rs/roff/0.2.1/roff/) as the engine for generating
//! roff output.

#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(missing_docs)]
#![warn(clippy::print_stderr)]
#![warn(clippy::print_stdout)]

mod styled_str;
use anstyle::{Ansi256Color, AnsiColor, Color, RgbColor, Style};
use anstyle_lossy::palette::Palette;
use roff::{bold, italic, Roff};
use styled_str::StyledStr;

/// Static Strings defining ROFF Control Requests
mod control_requests {
    /// Control to Create a Color definition
    pub(crate) const CREATE_COLOR: &str = "defcolor";
    /// Roff control request to set background color (fill color)
    pub(crate) const BACKGROUND: &str = "fcolor";
    /// Roff control request to set foreground color (glyph color)
    pub(crate) const FOREGROUND: &str = "gcolor";
}

/// Generate a [`Roff`] from ANSI escape codes
///
/// ```rust
/// let text = "\u{1b}[44;31mtest\u{1b}[0m";
///
/// let roff_doc = anstyle_roff::to_roff(text);
/// let expected = r#".gcolor red
/// .fcolor blue
/// test
/// "#;
///
/// assert_eq!(roff_doc.to_roff(), expected);
/// ```
pub fn to_roff(styled_text: &str) -> Roff {
    let mut doc = Roff::new();
    let mut previous_fg_color = None;
    let mut previous_bg_color = None;
    for styled in styled_str::styled_stream(styled_text) {
        if previous_fg_color != styled.style.get_fg_color() {
            add_color_to_roff(
                &mut doc,
                control_requests::FOREGROUND,
                &styled.style.get_fg_color(),
            );
            previous_fg_color = styled.style.get_fg_color();
        }
        if previous_bg_color != styled.style.get_bg_color() {
            add_color_to_roff(
                &mut doc,
                control_requests::BACKGROUND,
                &styled.style.get_bg_color(),
            );
            previous_bg_color = styled.style.get_bg_color();
        }
        set_effects_and_text(&styled, &mut doc);
    }
    doc
}

fn set_effects_and_text(styled: &StyledStr<'_>, doc: &mut Roff) {
    // Roff (the crate) only supports these inline commands
    //  - Bold
    //  - Italic
    //  - Roman (plain text)
    // If we want more support, or even support combined formats, we will need
    // to push improvements to roff upstream or implement a more thorough roff crate
    // perhaps by spinning off some of this code
    let effects = styled.style.get_effects();
    if effects.contains(anstyle::Effects::BOLD) | has_bright_fg(&styled.style) {
        doc.text([bold(styled.text)]);
    } else if effects.contains(anstyle::Effects::ITALIC) {
        doc.text([italic(styled.text)]);
    } else {
        doc.text([roff::roman(styled.text)]);
    }
}

fn has_bright_fg(style: &Style) -> bool {
    style
        .get_fg_color()
        .as_ref()
        .map(is_bright)
        .unwrap_or(false)
}

/// Check if [`Color`] is an [`AnsiColor::Bright*`][AnsiColor] variant
fn is_bright(fg_color: &Color) -> bool {
    if let Color::Ansi(color) = fg_color {
        matches!(
            color,
            AnsiColor::BrightRed
                | AnsiColor::BrightBlue
                | AnsiColor::BrightBlack
                | AnsiColor::BrightCyan
                | AnsiColor::BrightGreen
                | AnsiColor::BrightWhite
                | AnsiColor::BrightYellow
                | AnsiColor::BrightMagenta
        )
    } else {
        false
    }
}

fn add_color_to_roff(doc: &mut Roff, control_request: &str, color: &Option<Color>) {
    match color {
        Some(Color::Rgb(c)) => {
            // Adding Support for RGB colors, however cansi does not support
            // RGB Colors, so this is not executed. If we switch to a provider
            // That has RGB support we will also get it for Roff
            let name = rgb_name(c);
            doc.control(
                control_requests::CREATE_COLOR,
                [name.as_str(), "rgb", to_hex(c).as_str()],
            )
            .control(control_request, [name.as_str()]);
        }

        Some(Color::Ansi(c)) => {
            doc.control(control_request, [ansi_color_to_roff(c)]);
        }
        Some(Color::Ansi256(c)) => {
            // Adding Support for Ansi256 colors, however cansi does not support
            // Ansi256 Colors, so this is not executed. If we switch to a provider
            // That has Xterm support we will also get it for Roff
            add_color_to_roff(doc, control_request, &Some(xterm_to_ansi_or_rgb(*c)));
        }
        None => {
            // TODO: get rid of "default" hardcoded str?
            doc.control(control_request, ["default"]);
        }
    }
}

/// Non Lossy Conversion of Xterm color to one that Roff can handle
fn xterm_to_ansi_or_rgb(color: Ansi256Color) -> Color {
    match color.into_ansi() {
        Some(ansi_color) => Color::Ansi(ansi_color),
        None => Color::Rgb(anstyle_lossy::xterm_to_rgb(color, Palette::default())),
    }
}

fn rgb_name(c: &RgbColor) -> String {
    format!("hex_{}", to_hex(c).as_str())
}

fn to_hex(rgb: &RgbColor) -> String {
    let val: usize = ((rgb.0 as usize) << 16) + ((rgb.1 as usize) << 8) + (rgb.2 as usize);
    format!("#{val:06x}")
}

/// Map Color and Bright Variants to Roff Color styles
fn ansi_color_to_roff(color: &AnsiColor) -> &'static str {
    match color {
        AnsiColor::Black | AnsiColor::BrightBlack => "black",
        AnsiColor::Red | AnsiColor::BrightRed => "red",
        AnsiColor::Green | AnsiColor::BrightGreen => "green",
        AnsiColor::Yellow | AnsiColor::BrightYellow => "yellow",
        AnsiColor::Blue | AnsiColor::BrightBlue => "blue",
        AnsiColor::Magenta | AnsiColor::BrightMagenta => "magenta",
        AnsiColor::Cyan | AnsiColor::BrightCyan => "cyan",
        AnsiColor::White | AnsiColor::BrightWhite => "white",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anstyle::RgbColor;

    #[test]
    fn test_to_hex() {
        assert_eq!(to_hex(&RgbColor(0, 0, 0)).as_str(), "#000000");
        assert_eq!(to_hex(&RgbColor(255, 0, 0)).as_str(), "#ff0000");
        assert_eq!(to_hex(&RgbColor(0, 255, 0)).as_str(), "#00ff00");
        assert_eq!(to_hex(&RgbColor(0, 0, 255)).as_str(), "#0000ff");
    }
}

#[doc = include_str!("../README.md")]
#[cfg(doctest)]
pub struct ReadmeDoctests;
