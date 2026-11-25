//! Resolve typed wire colors against the active theme and terminal palette.

use ratatui::style::Color as TerminalColor;
use ratatui::style::Style;
use serde::Deserialize;
use serde::Serialize;

use super::BadgeColor;
use crate::bottom_pane::StatusLineAccent;
use crate::render::highlight;
use crate::style;
use crate::style::StatusTone;
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
    inline_code: OnceCell<(TerminalColor, TerminalColor)>,
    resolved: HashMap<Color, TerminalColor>,
    badges: HashMap<(BadgeColor, Option<TerminalColor>), Style>,
}

impl<'a> RenderTheme<'a> {
    pub(super) fn new(theme: &'a Theme) -> Self {
        Self {
            theme,
            highlighter: OnceCell::new(),
            inline_code: OnceCell::new(),
            resolved: HashMap::new(),
            badges: HashMap::new(),
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

    pub(super) fn inline_code_style(&self, style: Style) -> Style {
        let &(fg, bg) = self.inline_code.get_or_init(|| {
            let fg = terminal_color(
                highlight::foreground_color_for_scopes(
                    self.highlighter(),
                    highlight::INLINE_CODE_SCOPES,
                ),
                crate::style::accent_color(),
            );
            let bg = terminal_color(
                self.theme
                    .settings
                    .line_highlight
                    // Use the subdued fallback for translucent overlays, retaining
                    // bat’s alpha-zero ANSI palette encoding.
                    .filter(|c| matches!(c.a, 0 | 255))
                    .and_then(highlight::convert_syntect_color),
                crate::style::user_message_style()
                    .bg
                    .unwrap_or(TerminalColor::Reset),
            );
            (fg, bg)
        });
        let bg = style.bg.unwrap_or(bg);
        if style.fg.is_some() {
            return style.bg(bg);
        }
        let fg = crate::style::readable_color_on(fg, Some(bg));
        // The terminal owns the ANSI palette. Use its default pair for an exact collision.
        if fg == bg {
            return style.fg(TerminalColor::Reset).bg(TerminalColor::Reset);
        }
        style.fg(fg).bg(bg)
    }

    fn semantic_foreground(&mut self, role: BadgeColor) -> TerminalColor {
        // TextMate also matches parent scopes, e.g. markup.inserted for markup.inserted.diff.
        let scopes: &[&str] = match role {
            BadgeColor::Secondary => &["codex.secondary"],
            BadgeColor::Info => &["codex.info", "token.info-token"],
            BadgeColor::Success => &["codex.success", "markup.inserted.diff"],
            BadgeColor::Warning => &["codex.warning", "token.warn-token", "markup.changed.diff"],
            BadgeColor::Danger => &["codex.danger", "token.error-token", "markup.deleted.diff"],
        };
        let fg = terminal_color(
            highlight::foreground_color_for_scopes(self.highlighter(), scopes),
            TerminalColor::Reset,
        );
        if fg != TerminalColor::Reset {
            return fg;
        }
        match role {
            BadgeColor::Secondary => match self.resolve(Color::Primary) {
                TerminalColor::Reset => style::secondary_text_style().fg.unwrap_or_default(),
                fg => fg,
            },
            BadgeColor::Info => match self.resolve(Color::Accent) {
                TerminalColor::Reset => style::accent_color(),
                fg => fg,
            },
            BadgeColor::Success => style::status_style(StatusTone::Success)
                .fg
                .unwrap_or_default(),
            BadgeColor::Warning => style::status_style(StatusTone::Attention)
                .fg
                .unwrap_or_default(),
            BadgeColor::Danger => style::status_style(StatusTone::Failure)
                .fg
                .unwrap_or_default(),
        }
    }

    pub(super) fn badge_style(&mut self, role: BadgeColor, parent: Option<TerminalColor>) -> Style {
        let key = (role, parent);
        if let Some(&style) = self.badges.get(&key) {
            return style;
        }
        let style = self.resolve_badge_style(role, parent);
        self.badges.insert(key, style);
        style
    }

    fn resolve_badge_style(&mut self, role: BadgeColor, parent: Option<TerminalColor>) -> Style {
        let base = Style::default()
            .fg(TerminalColor::Reset)
            .bg(TerminalColor::Reset)
            .remove_modifier(ratatui::style::Modifier::BOLD | ratatui::style::Modifier::UNDERLINED);
        let level = terminal_palette::effective_stdout_color_level();
        if matches!(level, StdoutColorLevel::Unknown) {
            return base;
        }

        let fg = self.semantic_foreground(role);
        // Pair ANSI foregrounds with the terminal background they were chosen for.
        let Some(tint) = style::explicit_rgb(fg) else {
            return base.fg(fg);
        };
        // A named ANSI parent has no known RGB. Compositing onto a known surface
        // lets the contrast check use the exact fill the terminal will paint.
        let Some(surface) = style::background_rgb(parent)
            .or_else(terminal_palette::default_bg)
            .or_else(|| style::explicit_rgb(self.resolve(Color::Surface)))
        else {
            return base;
        };
        const BADGE_TINT_ALPHA: f32 = 0.15;
        let bg = terminal_palette::best_color_for_level(
            crate::color::blend(tint, surface, BADGE_TINT_ALPHA),
            level,
        );
        let fg = style::readable_color_on(fg, Some(bg));
        base.fg(fg).bg(bg)
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
#[cfg_attr(test, derive(ts_rs::TS), ts(export_to = "statusline.d.ts"))]
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

/// A six-digit sRGB color in `#RRGGBB` form.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
#[cfg_attr(
    test,
    derive(ts_rs::TS),
    ts(export_to = "statusline.d.ts", type = "`#${string}`")
)]
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
#[cfg_attr(test, derive(ts_rs::TS), ts(export_to = "statusline.d.ts"))]
pub(super) enum TextWeight {
    Normal,
    Bold,
}
