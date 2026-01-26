use clap::ValueEnum;
use ratatui::style::Color;
use std::sync::atomic::AtomicU8;
use std::sync::atomic::Ordering;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, ValueEnum)]
#[value(rename_all = "kebab-case")]
pub enum ThemeName {
    #[default]
    Default,
    #[value(alias = "ember", alias = "ivory")]
    IvoryEmber,
    #[value(alias = "thames", alias = "fog")]
    ThamesFog,
}

const THEMES: [ThemeName; 3] = [
    ThemeName::Default,
    ThemeName::IvoryEmber,
    ThemeName::ThamesFog,
];

static THEME: AtomicU8 = AtomicU8::new(ThemeName::Default as u8);

pub fn all_themes() -> &'static [ThemeName] {
    &THEMES
}

impl ThemeName {
    pub fn id(self) -> &'static str {
        match self {
            ThemeName::Default => "default",
            ThemeName::IvoryEmber => "ivory-ember",
            ThemeName::ThamesFog => "thames-fog",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ThemeName::Default => "Default",
            ThemeName::IvoryEmber => "Ivory Ember",
            ThemeName::ThamesFog => "Thames Fog",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            ThemeName::Default => "Keep the classic Codex palette.",
            ThemeName::IvoryEmber => "Warm amber accents with ivory foreground.",
            ThemeName::ThamesFog => "Cool blue-gray accents with misty contrast.",
        }
    }
}

pub fn parse_theme_name(input: &str) -> Option<ThemeName> {
    let normalized = input.trim().to_lowercase();
    match normalized.as_str() {
        "default" => Some(ThemeName::Default),
        "ivory-ember" | "ivory" | "ember" => Some(ThemeName::IvoryEmber),
        "thames-fog" | "thames" | "fog" | "thame" => Some(ThemeName::ThamesFog),
        _ => None,
    }
}

pub fn set_theme(theme: ThemeName) {
    THEME.store(theme as u8, Ordering::Relaxed);
}

pub fn current_theme() -> ThemeName {
    match THEME.load(Ordering::Relaxed) {
        value if value == ThemeName::IvoryEmber as u8 => ThemeName::IvoryEmber,
        value if value == ThemeName::ThamesFog as u8 => ThemeName::ThamesFog,
        _ => ThemeName::Default,
    }
}

pub(crate) fn remap_color(color: Color) -> Color {
    match current_theme() {
        ThemeName::Default => color,
        ThemeName::IvoryEmber => match color {
            Color::Cyan => Color::Yellow,
            Color::LightCyan => Color::LightYellow,
            Color::Magenta => Color::Yellow,
            Color::LightMagenta => Color::LightYellow,
            _ => color,
        },
        ThemeName::ThamesFog => match color {
            Color::Cyan => Color::LightBlue,
            Color::LightCyan => Color::LightBlue,
            Color::Magenta => Color::LightCyan,
            Color::LightMagenta => Color::LightCyan,
            _ => color,
        },
    }
}
