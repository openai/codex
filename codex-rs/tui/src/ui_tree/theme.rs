//! Literal provider colors use the terminal's palette at paint time.

use crate::terminal_palette;
use ratatui::style::Color as TerminalColor;
use serde::Deserialize;
use serde::Serialize;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum Color {
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
    #[serde(untagged)]
    Rgb(HexRgb),
}

impl Color {
    pub(super) fn resolve(&self) -> TerminalColor {
        match self {
            Self::Rgb(rgb) => terminal_palette::best_color(rgb.0),
            Self::Black => TerminalColor::Black,
            Self::Red => TerminalColor::Red,
            Self::Green => TerminalColor::Green,
            Self::Yellow => TerminalColor::Yellow,
            Self::Blue => TerminalColor::Blue,
            Self::Magenta => TerminalColor::Magenta,
            Self::Cyan => TerminalColor::Cyan,
            // Ratatui maps Gray to ANSI slot 7 and White to slot 15.
            Self::White => TerminalColor::Gray,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub(super) struct HexRgb((u8, u8, u8));

impl TryFrom<String> for HexRgb {
    type Error = &'static str;

    fn try_from(source: String) -> Result<Self, Self::Error> {
        const INVALID: &str = "expected #RRGGBB";
        if source.len() != 7 {
            return Err(INVALID);
        }
        let color = source
            .parse::<syntect::highlighting::Color>()
            .map_err(|_| INVALID)?;
        Ok(Self((color.r, color.g, color.b)))
    }
}

impl From<HexRgb> for String {
    fn from(HexRgb((r, g, b)): HexRgb) -> Self {
        format!("#{r:02x}{g:02x}{b:02x}")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum TextWeight {
    Normal,
    Bold,
}
