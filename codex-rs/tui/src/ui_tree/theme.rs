//! Resolve typed wire colors against the active theme and terminal palette.

use ratatui::style::Color as TerminalColor;
use serde::Deserialize;
use serde::Serialize;

use crate::bottom_pane::StatusLineAccent;
use crate::render::highlight;
use crate::terminal_palette;
use crate::terminal_palette::StdoutColorLevel;
use std::cell::OnceCell;
use std::collections::HashMap;
use syntect::highlighting::Highlighter;
use syntect::highlighting::Theme;
use syntect::parsing::Scope;

pub(super) struct RenderTheme<'a> {
    theme: &'a Theme,
    highlighter: OnceCell<Highlighter<'a>>,
    resolved: HashMap<Color, TerminalColor>,
}

impl<'a> RenderTheme<'a> {
    pub(super) fn new(theme: &'a Theme) -> Self {
        Self {
            theme,
            highlighter: OnceCell::new(),
            resolved: HashMap::new(),
        }
    }

    // A color can occur on many chunks. Match its theme scopes once per paint.
    pub(super) fn resolve(&mut self, color: Color) -> TerminalColor {
        if let Some(&resolved) = self.resolved.get(&color) {
            return resolved;
        }
        let resolved = color.resolve(self);
        self.resolved.insert(color, resolved);
        resolved
    }

    // Plain text and literal colors need no TextMate scope matching.
    fn highlighter(&self) -> &Highlighter<'a> {
        self.highlighter
            .get_or_init(|| Highlighter::new(self.theme))
    }
}

fn terminal_color(color: Option<TerminalColor>, fallback: TerminalColor) -> TerminalColor {
    let Some(color) = color else { return fallback };
    let level = terminal_palette::effective_stdout_color_level();
    match (level, color) {
        (
            StdoutColorLevel::Ansi16 | StdoutColorLevel::Unknown,
            TerminalColor::Rgb(..) | TerminalColor::Indexed(16..),
        ) => fallback,
        (_, TerminalColor::Rgb(r, g, b)) => {
            terminal_palette::best_color_for_level((r, g, b), level)
        }
        _ => color,
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum Color {
    Primary,
    Surface,
    Accent,
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
    #[serde(untagged)]
    Status(StatusLineAccent),
    #[serde(untagged)]
    Rgb(HexRgb),
}

impl Color {
    fn resolve(self, palette: &RenderTheme<'_>) -> TerminalColor {
        let setting = |color: Option<syntect::highlighting::Color>| {
            terminal_color(
                color.and_then(highlight::convert_syntect_color),
                TerminalColor::Reset,
            )
        };
        match self {
            Self::Primary => setting(palette.theme.settings.foreground),
            Self::Surface => setting(palette.theme.settings.background),
            Self::Accent => setting(
                Scope::new("codex.accent")
                    .ok()
                    .and_then(|scope| {
                        palette
                            .highlighter()
                            .style_mod_for_stack(&[scope])
                            .foreground
                    })
                    .or(palette.theme.settings.accent),
            ),
            Self::Status(role) => terminal_color(
                highlight::foreground_color_for_scopes(palette.highlighter(), role.scopes()),
                role.fallback_color(),
            ),
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

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
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
