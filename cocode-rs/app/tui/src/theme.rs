//! Theme system for the TUI.
//!
//! This module provides a theme system with 5 built-in themes:
//! - Default: Balanced colors for general use
//! - Dark: High contrast dark theme
//! - Light: Clean light theme
//! - Dracula: Popular dark purple theme
//! - Nord: Cool blue nordic theme

use ratatui::style::Color;

/// Available theme names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemeName {
    /// Default balanced theme.
    #[default]
    Default,
    /// High contrast dark theme.
    Dark,
    /// Clean light theme.
    Light,
    /// Dracula purple theme.
    Dracula,
    /// Nord blue theme.
    Nord,
}

impl ThemeName {
    /// Get all available theme names.
    pub fn all() -> &'static [ThemeName] {
        &[
            ThemeName::Default,
            ThemeName::Dark,
            ThemeName::Light,
            ThemeName::Dracula,
            ThemeName::Nord,
        ]
    }

    /// Get theme name as a string.
    pub fn as_str(&self) -> &'static str {
        match self {
            ThemeName::Default => "default",
            ThemeName::Dark => "dark",
            ThemeName::Light => "light",
            ThemeName::Dracula => "dracula",
            ThemeName::Nord => "nord",
        }
    }

    /// Parse theme name from string.
    pub fn from_str(s: &str) -> Option<ThemeName> {
        match s.to_lowercase().as_str() {
            "default" => Some(ThemeName::Default),
            "dark" => Some(ThemeName::Dark),
            "light" => Some(ThemeName::Light),
            "dracula" => Some(ThemeName::Dracula),
            "nord" => Some(ThemeName::Nord),
            _ => None,
        }
    }
}

impl std::fmt::Display for ThemeName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Theme configuration for the TUI.
#[derive(Debug, Clone)]
pub struct Theme {
    /// Theme name.
    pub name: ThemeName,

    // ========== Base Colors ==========
    /// Primary accent color.
    pub primary: Color,
    /// Secondary accent color.
    pub secondary: Color,
    /// Tertiary/highlight color.
    pub accent: Color,

    // ========== Text Colors ==========
    /// Normal text color.
    pub text: Color,
    /// Dimmed/muted text color.
    pub text_dim: Color,
    /// Bold/emphasized text color.
    pub text_bold: Color,

    // ========== Background Colors ==========
    /// Main background color.
    pub bg: Color,
    /// Secondary/elevated background.
    pub bg_secondary: Color,
    /// Selected/highlighted background.
    pub bg_selected: Color,

    // ========== Message Colors ==========
    /// User message color.
    pub user_message: Color,
    /// Assistant message color.
    pub assistant_message: Color,
    /// Thinking content color.
    pub thinking: Color,
    /// System message color.
    pub system_message: Color,

    // ========== Status Colors ==========
    /// Tool running indicator.
    pub tool_running: Color,
    /// Tool completed indicator.
    pub tool_completed: Color,
    /// Tool error indicator.
    pub tool_error: Color,
    /// Warning color.
    pub warning: Color,
    /// Success color.
    pub success: Color,
    /// Error color.
    pub error: Color,

    // ========== UI Element Colors ==========
    /// Border color.
    pub border: Color,
    /// Border focused color.
    pub border_focused: Color,
    /// Scrollbar color.
    pub scrollbar: Color,
    /// Plan mode indicator.
    pub plan_mode: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self::default_theme()
    }
}

impl Theme {
    /// Create the default theme.
    pub fn default_theme() -> Self {
        Self {
            name: ThemeName::Default,
            // Base
            primary: Color::Cyan,
            secondary: Color::Blue,
            accent: Color::Magenta,
            // Text
            text: Color::Reset,
            text_dim: Color::DarkGray,
            text_bold: Color::Reset,
            // Background
            bg: Color::Reset,
            bg_secondary: Color::DarkGray,
            bg_selected: Color::DarkGray,
            // Messages
            user_message: Color::Green,
            assistant_message: Color::Cyan,
            thinking: Color::Magenta,
            system_message: Color::Yellow,
            // Status
            tool_running: Color::Yellow,
            tool_completed: Color::Green,
            tool_error: Color::Red,
            warning: Color::Yellow,
            success: Color::Green,
            error: Color::Red,
            // UI
            border: Color::DarkGray,
            border_focused: Color::Cyan,
            scrollbar: Color::DarkGray,
            plan_mode: Color::Blue,
        }
    }

    /// Create the dark theme.
    pub fn dark() -> Self {
        Self {
            name: ThemeName::Dark,
            // Base
            primary: Color::Rgb(0, 255, 255),     // Bright cyan
            secondary: Color::Rgb(100, 149, 237), // Cornflower blue
            accent: Color::Rgb(255, 105, 180),    // Hot pink
            // Text
            text: Color::Rgb(230, 230, 230),
            text_dim: Color::Rgb(128, 128, 128),
            text_bold: Color::Rgb(255, 255, 255),
            // Background
            bg: Color::Reset,
            bg_secondary: Color::Rgb(40, 40, 40),
            bg_selected: Color::Rgb(60, 60, 60),
            // Messages
            user_message: Color::Rgb(144, 238, 144), // Light green
            assistant_message: Color::Rgb(135, 206, 250), // Light sky blue
            thinking: Color::Rgb(218, 112, 214),     // Orchid
            system_message: Color::Rgb(255, 215, 0), // Gold
            // Status
            tool_running: Color::Rgb(255, 193, 7),   // Amber
            tool_completed: Color::Rgb(76, 175, 80), // Green 500
            tool_error: Color::Rgb(244, 67, 54),     // Red 500
            warning: Color::Rgb(255, 152, 0),        // Orange
            success: Color::Rgb(76, 175, 80),
            error: Color::Rgb(244, 67, 54),
            // UI
            border: Color::Rgb(80, 80, 80),
            border_focused: Color::Rgb(0, 255, 255),
            scrollbar: Color::Rgb(100, 100, 100),
            plan_mode: Color::Rgb(33, 150, 243), // Blue 500
        }
    }

    /// Create the light theme.
    pub fn light() -> Self {
        Self {
            name: ThemeName::Light,
            // Base
            primary: Color::Rgb(0, 150, 136),   // Teal
            secondary: Color::Rgb(63, 81, 181), // Indigo
            accent: Color::Rgb(156, 39, 176),   // Purple
            // Text
            text: Color::Rgb(33, 33, 33),
            text_dim: Color::Rgb(117, 117, 117),
            text_bold: Color::Rgb(0, 0, 0),
            // Background
            bg: Color::Reset,
            bg_secondary: Color::Rgb(245, 245, 245),
            bg_selected: Color::Rgb(224, 224, 224),
            // Messages
            user_message: Color::Rgb(27, 94, 32), // Dark green
            assistant_message: Color::Rgb(13, 71, 161), // Dark blue
            thinking: Color::Rgb(74, 20, 140),    // Deep purple
            system_message: Color::Rgb(230, 81, 0), // Deep orange
            // Status
            tool_running: Color::Rgb(245, 127, 23),  // Orange
            tool_completed: Color::Rgb(56, 142, 60), // Green
            tool_error: Color::Rgb(211, 47, 47),     // Red
            warning: Color::Rgb(245, 124, 0),
            success: Color::Rgb(56, 142, 60),
            error: Color::Rgb(211, 47, 47),
            // UI
            border: Color::Rgb(189, 189, 189),
            border_focused: Color::Rgb(0, 150, 136),
            scrollbar: Color::Rgb(158, 158, 158),
            plan_mode: Color::Rgb(63, 81, 181),
        }
    }

    /// Create the Dracula theme.
    pub fn dracula() -> Self {
        Self {
            name: ThemeName::Dracula,
            // Base (Dracula palette)
            primary: Color::Rgb(139, 233, 253),   // Cyan
            secondary: Color::Rgb(189, 147, 249), // Purple
            accent: Color::Rgb(255, 121, 198),    // Pink
            // Text
            text: Color::Rgb(248, 248, 242),    // Foreground
            text_dim: Color::Rgb(98, 114, 164), // Comment
            text_bold: Color::Rgb(255, 255, 255),
            // Background
            bg: Color::Reset,
            bg_secondary: Color::Rgb(68, 71, 90), // Current line
            bg_selected: Color::Rgb(68, 71, 90),
            // Messages
            user_message: Color::Rgb(80, 250, 123), // Green
            assistant_message: Color::Rgb(139, 233, 253), // Cyan
            thinking: Color::Rgb(189, 147, 249),    // Purple
            system_message: Color::Rgb(241, 250, 140), // Yellow
            // Status
            tool_running: Color::Rgb(255, 184, 108), // Orange
            tool_completed: Color::Rgb(80, 250, 123), // Green
            tool_error: Color::Rgb(255, 85, 85),     // Red
            warning: Color::Rgb(255, 184, 108),
            success: Color::Rgb(80, 250, 123),
            error: Color::Rgb(255, 85, 85),
            // UI
            border: Color::Rgb(68, 71, 90),
            border_focused: Color::Rgb(189, 147, 249),
            scrollbar: Color::Rgb(68, 71, 90),
            plan_mode: Color::Rgb(189, 147, 249),
        }
    }

    /// Create the Nord theme.
    pub fn nord() -> Self {
        Self {
            name: ThemeName::Nord,
            // Base (Nord palette)
            primary: Color::Rgb(136, 192, 208),   // Nord8 (frost)
            secondary: Color::Rgb(129, 161, 193), // Nord9
            accent: Color::Rgb(180, 142, 173),    // Nord15 (aurora purple)
            // Text
            text: Color::Rgb(236, 239, 244),   // Nord6
            text_dim: Color::Rgb(76, 86, 106), // Nord3
            text_bold: Color::Rgb(236, 239, 244),
            // Background
            bg: Color::Reset,
            bg_secondary: Color::Rgb(59, 66, 82), // Nord1
            bg_selected: Color::Rgb(67, 76, 94),  // Nord2
            // Messages
            user_message: Color::Rgb(163, 190, 140), // Nord14 (green)
            assistant_message: Color::Rgb(136, 192, 208), // Nord8
            thinking: Color::Rgb(180, 142, 173),     // Nord15 (purple)
            system_message: Color::Rgb(235, 203, 139), // Nord13 (yellow)
            // Status
            tool_running: Color::Rgb(208, 135, 112), // Nord12 (orange)
            tool_completed: Color::Rgb(163, 190, 140), // Nord14 (green)
            tool_error: Color::Rgb(191, 97, 106),    // Nord11 (red)
            warning: Color::Rgb(208, 135, 112),
            success: Color::Rgb(163, 190, 140),
            error: Color::Rgb(191, 97, 106),
            // UI
            border: Color::Rgb(76, 86, 106),           // Nord3
            border_focused: Color::Rgb(136, 192, 208), // Nord8
            scrollbar: Color::Rgb(76, 86, 106),
            plan_mode: Color::Rgb(129, 161, 193), // Nord9
        }
    }

    /// Get a theme by name.
    pub fn by_name(name: ThemeName) -> Self {
        match name {
            ThemeName::Default => Self::default_theme(),
            ThemeName::Dark => Self::dark(),
            ThemeName::Light => Self::light(),
            ThemeName::Dracula => Self::dracula(),
            ThemeName::Nord => Self::nord(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_theme_name_roundtrip() {
        for name in ThemeName::all() {
            let s = name.as_str();
            let parsed = ThemeName::from_str(s);
            assert_eq!(parsed, Some(*name));
        }
    }

    #[test]
    fn test_theme_name_case_insensitive() {
        assert_eq!(ThemeName::from_str("DARK"), Some(ThemeName::Dark));
        assert_eq!(ThemeName::from_str("Dracula"), Some(ThemeName::Dracula));
        assert_eq!(ThemeName::from_str("NORD"), Some(ThemeName::Nord));
    }

    #[test]
    fn test_theme_by_name() {
        for name in ThemeName::all() {
            let theme = Theme::by_name(*name);
            assert_eq!(theme.name, *name);
        }
    }

    #[test]
    fn test_default_theme() {
        let theme = Theme::default();
        assert_eq!(theme.name, ThemeName::Default);
    }
}
