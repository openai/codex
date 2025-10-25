use crate::color::blend;
use crate::color::is_light;
use crate::terminal_palette::best_color;
use crate::terminal_palette::default_bg;
use codex_core::config_types::CustomThemeColors;
use codex_core::config_types::ThemeMode;
use ratatui::style::Color;

#[derive(Debug, Clone, Copy)]
pub struct Theme {
    user_message_bg: Color,
    assistant_message_bg: Option<Color>,
    pub accent: Color,
    pub secondary: Color,
    pub dim: Color,
}

impl Theme {
    pub fn from_mode(mode: &ThemeMode) -> Self {
        match mode {
            ThemeMode::Auto => Self::auto(),
            ThemeMode::Light => Self::light(),
            ThemeMode::Dark => Self::dark(),
            ThemeMode::CatppuccinMocha => Self::catppuccin_mocha(),
            ThemeMode::GruvboxDark => Self::gruvbox_dark(),
            ThemeMode::Nord => Self::nord(),
            ThemeMode::SolarizedDark => Self::solarized_dark(),
            ThemeMode::SolarizedLight => Self::solarized_light(),
            ThemeMode::Dracula => Self::dracula(),
            ThemeMode::Custom(colors) => Self::custom(colors),
        }
    }

    pub fn user_message_bg(&self) -> Color {
        self.user_message_bg
    }

    pub fn assistant_message_bg(&self) -> Option<Color> {
        self.assistant_message_bg
    }

    fn auto() -> Self {
        if let Some(bg) = default_bg() {
            let overlay = if is_light(bg) {
                (0, 0, 0)
            } else {
                (255, 255, 255)
            };
            let user_bg = blend(overlay, bg, 0.12);
            let assistant_overlay = if is_light(bg) {
                (255, 255, 255)
            } else {
                (0, 0, 0)
            };
            let assistant_bg = blend(assistant_overlay, bg, 0.05);
            Self::with_backgrounds(user_bg, Some(assistant_bg))
        } else {
            Self::dark()
        }
    }

    fn light() -> Self {
        Self::with_backgrounds((242, 244, 248), Some((255, 255, 255)))
    }

    fn dark() -> Self {
        Self::with_backgrounds((33, 37, 43), Some((24, 26, 32)))
    }

    fn catppuccin_mocha() -> Self {
        Self::with_backgrounds((30, 32, 48), Some((24, 24, 37)))
    }

    fn gruvbox_dark() -> Self {
        Self::with_backgrounds((50, 46, 43), Some((40, 36, 33)))
    }

    fn nord() -> Self {
        Self::with_backgrounds((46, 52, 64), Some((67, 76, 94)))
    }

    fn solarized_dark() -> Self {
        Self::with_backgrounds((7, 54, 66), Some((0, 43, 54)))
    }

    fn solarized_light() -> Self {
        Self::with_backgrounds((253, 246, 227), Some((238, 232, 213)))
    }

    fn dracula() -> Self {
        Self::with_backgrounds((40, 42, 54), Some((68, 71, 90)))
    }

    fn custom(colors: &CustomThemeColors) -> Self {
        let mut theme = Self::dark();
        if let Some(rgb) = colors.user_message_bg {
            theme.user_message_bg = map_rgb(rgb);
        }
        theme.assistant_message_bg = colors.assistant_message_bg.map(map_rgb);
        if let Some(rgb) = colors.accent {
            theme.accent = map_rgb(rgb);
        }
        if let Some(rgb) = colors.secondary {
            theme.secondary = map_rgb(rgb);
        }
        if let Some(rgb) = colors.dim {
            theme.dim = map_rgb(rgb);
        }
        theme
    }

    fn base() -> Self {
        Self {
            user_message_bg: Color::Reset,
            assistant_message_bg: None,
            accent: Color::Cyan,
            secondary: Color::Magenta,
            dim: Color::DarkGray,
        }
    }

    fn with_backgrounds(user_bg: (u8, u8, u8), assistant_bg: Option<(u8, u8, u8)>) -> Self {
        let mut theme = Self::base();
        theme.user_message_bg = map_rgb(user_bg);
        theme.assistant_message_bg = assistant_bg.map(map_rgb);
        theme
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::auto()
    }
}

fn map_rgb(rgb: (u8, u8, u8)) -> Color {
    best_color(rgb)
}
