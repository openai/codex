#[allow(unused_imports)]
use crate::{
    AnsiColors, BgDynColorDisplay, CssColors, DynColor, FgDynColorDisplay, Rgb, XtermColors,
};
use core::fmt;

/// An enum describing runtime-configurable colors
///
/// This can be displayed using [`FgDynColorDisplay`](FgDynColorDisplay) or [`BgDynColorDisplay`](BgDynColorDisplay),
/// allowing for multiple types of colors to be used at runtime.
#[allow(missing_docs)]
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum DynColors {
    Ansi(AnsiColors),
    Css(CssColors),
    Xterm(XtermColors),
    Rgb(u8, u8, u8),
}

impl crate::private::Sealed for DynColors {}

impl DynColor for DynColors {
    fn fmt_ansi_fg(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DynColors::Ansi(ansi) => ansi.fmt_ansi_fg(f),
            DynColors::Css(css) => css.fmt_ansi_fg(f),
            DynColors::Xterm(xterm) => xterm.fmt_ansi_fg(f),
            &DynColors::Rgb(r, g, b) => Rgb(r, g, b).fmt_ansi_fg(f),
        }
    }

    fn fmt_ansi_bg(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DynColors::Ansi(ansi) => ansi.fmt_ansi_bg(f),
            DynColors::Css(css) => css.fmt_ansi_bg(f),
            DynColors::Xterm(xterm) => xterm.fmt_ansi_bg(f),
            &DynColors::Rgb(r, g, b) => Rgb(r, g, b).fmt_ansi_bg(f),
        }
    }

    fn fmt_raw_ansi_fg(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DynColors::Ansi(ansi) => ansi.fmt_raw_ansi_fg(f),
            DynColors::Css(css) => css.fmt_raw_ansi_fg(f),
            DynColors::Xterm(xterm) => xterm.fmt_raw_ansi_fg(f),
            &DynColors::Rgb(r, g, b) => Rgb(r, g, b).fmt_raw_ansi_fg(f),
        }
    }

    fn fmt_raw_ansi_bg(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DynColors::Ansi(ansi) => ansi.fmt_raw_ansi_bg(f),
            DynColors::Css(css) => css.fmt_raw_ansi_bg(f),
            DynColors::Xterm(xterm) => xterm.fmt_raw_ansi_bg(f),
            &DynColors::Rgb(r, g, b) => Rgb(r, g, b).fmt_raw_ansi_bg(f),
        }
    }

    #[doc(hidden)]
    fn get_dyncolors_fg(&self) -> crate::DynColors {
        *self
    }

    #[doc(hidden)]
    fn get_dyncolors_bg(&self) -> crate::DynColors {
        *self
    }
}

/// An error for when the color can not be parsed from a string at runtime
#[derive(Debug)]
pub struct ParseColorError;

impl core::str::FromStr for DynColors {
    type Err = ParseColorError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.chars().next().ok_or(ParseColorError)? == '#' {
            match s.len() {
                4 => {
                    // TODO
                    Err(ParseColorError)
                }
                7 => Ok(Self::Rgb(
                    u8::from_str_radix(&s[1..3], 16).or(Err(ParseColorError))?,
                    u8::from_str_radix(&s[3..5], 16).or(Err(ParseColorError))?,
                    u8::from_str_radix(&s[5..7], 16).or(Err(ParseColorError))?,
                )),
                _ => Err(ParseColorError),
            }
        } else {
            let ansi = match s {
                "black" => AnsiColors::Black,
                "red" => AnsiColors::Red,
                "green" => AnsiColors::Green,
                "yellow" => AnsiColors::Yellow,
                "blue" => AnsiColors::Blue,
                "magenta" | "purple" => AnsiColors::Magenta,
                "cyan" => AnsiColors::Cyan,
                "white" => AnsiColors::White,
                "bright black" => AnsiColors::BrightBlack,
                "bright red" => AnsiColors::BrightRed,
                "bright green" => AnsiColors::BrightGreen,
                "bright yellow" => AnsiColors::BrightYellow,
                "bright blue" => AnsiColors::BrightBlue,
                "bright magenta" => AnsiColors::BrightMagenta,
                "bright cyan" => AnsiColors::BrightCyan,
                "bright white" => AnsiColors::BrightWhite,
                _ => return Err(ParseColorError),
            };

            Ok(Self::Ansi(ansi))
        }
    }
}
