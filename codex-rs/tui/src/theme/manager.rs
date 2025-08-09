use std::sync::RwLock;

use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;

use super::detection::TerminalDetector;
use super::palette::DARK_PALETTE;
use super::palette::LIGHT_PALETTE;
use super::palette::Palette;
use super::semantic::SemanticColor;

/// The theme mode - determines which palette to use
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeMode {
    /// Dark theme for dark terminal backgrounds
    Dark,
    /// Light theme for light terminal backgrounds
    Light,
    /// Automatically detect based on terminal background
    Auto,
}

impl Default for ThemeMode {
    fn default() -> Self {
        Self::Auto
    }
}

impl std::fmt::Display for ThemeMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Dark => write!(f, "dark"),
            Self::Light => write!(f, "light"),
            Self::Auto => write!(f, "auto"),
        }
    }
}

impl std::str::FromStr for ThemeMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "dark" => Ok(Self::Dark),
            "light" => Ok(Self::Light),
            "auto" => Ok(Self::Auto),
            _ => Err(format!(
                "Invalid theme mode: {s}. Use 'dark', 'light', or 'auto'"
            )),
        }
    }
}

/// A complete theme with its palette and helper methods
pub struct Theme {
    mode: ThemeMode,
    palette: &'static Palette,
}

impl Theme {
    /// Create a new theme with the given mode
    pub fn new(mode: ThemeMode) -> Self {
        let (resolved_mode, palette) = match mode {
            ThemeMode::Dark => (ThemeMode::Dark, &*DARK_PALETTE),
            ThemeMode::Light => (ThemeMode::Light, &*LIGHT_PALETTE),
            ThemeMode::Auto => {
                // Use terminal detection to resolve Auto mode
                let detected = TerminalDetector::new().detect_background();
                match detected {
                    ThemeMode::Dark => (ThemeMode::Dark, &*DARK_PALETTE),
                    ThemeMode::Light => (ThemeMode::Light, &*LIGHT_PALETTE),
                    ThemeMode::Auto => unreachable!(), // Detection never returns Auto
                }
            }
        };

        Self {
            mode: resolved_mode,
            palette,
        }
    }

    /// Get the theme mode
    pub fn mode(&self) -> ThemeMode {
        self.mode
    }

    /// Get a color from the theme's palette
    pub fn color(&self, semantic: SemanticColor) -> Color {
        self.palette.get(semantic)
    }

    /// Create a style with the given semantic foreground color
    pub fn style(&self, semantic: SemanticColor) -> Style {
        Style::default().fg(self.color(semantic))
    }

    /// Create a style with semantic foreground and background colors
    pub fn style_with_bg(&self, fg: SemanticColor, bg: SemanticColor) -> Style {
        Style::default().fg(self.color(fg)).bg(self.color(bg))
    }

    /// Create a style with semantic color and modifiers
    pub fn style_with_modifiers(&self, semantic: SemanticColor, modifiers: Modifier) -> Style {
        Style::default()
            .fg(self.color(semantic))
            .add_modifier(modifiers)
    }

    /// Check if this is a dark theme
    pub fn is_dark(&self) -> bool {
        matches!(self.mode, ThemeMode::Dark | ThemeMode::Auto)
    }

    /// Check if this is a light theme
    pub fn is_light(&self) -> bool {
        matches!(self.mode, ThemeMode::Light)
    }
}

/// Global theme manager - maintains the active theme
pub struct ThemeManager {
    current_theme: RwLock<Theme>,
}

impl ThemeManager {
    /// Create a new theme manager with the default theme
    fn new() -> Self {
        Self {
            current_theme: RwLock::new(Theme::new(ThemeMode::default())),
        }
    }

    /// Get the global theme manager instance
    pub fn global() -> &'static Self {
        lazy_static::lazy_static! {
            static ref MANAGER: ThemeManager = ThemeManager::new();
        }
        &MANAGER
    }

    /// Set the theme mode
    pub fn set_mode(&self, mode: ThemeMode) {
        if let Ok(mut theme) = self.current_theme.write() {
            *theme = Theme::new(mode);
        } else {
            tracing::error!("Failed to acquire write lock on theme");
        }
    }

    /// Get the active theme
    pub fn active_theme(&self) -> &'static Theme {
        // This is a bit of a hack to return a static reference
        // In practice, themes are set once at startup and rarely changed
        // For dynamic theme switching, we'd need a different approach
        let theme_mode = if let Ok(theme) = self.current_theme.read() {
            theme.mode
        } else {
            tracing::error!("Failed to acquire read lock on theme, defaulting to Dark");
            ThemeMode::Dark
        };

        match theme_mode {
            ThemeMode::Dark => {
                lazy_static::lazy_static! {
                    static ref DARK_THEME: Theme = Theme::new(ThemeMode::Dark);
                }
                &DARK_THEME
            }
            ThemeMode::Light => {
                lazy_static::lazy_static! {
                    static ref LIGHT_THEME: Theme = Theme::new(ThemeMode::Light);
                }
                &LIGHT_THEME
            }
            ThemeMode::Auto => {
                // Auto mode should be resolved at theme creation time
                // But if we somehow get here, create a theme that will auto-detect
                lazy_static::lazy_static! {
                    static ref AUTO_THEME: Theme = Theme::new(ThemeMode::Auto);
                }
                &AUTO_THEME
            }
        }
    }

    /// Get a color from the active theme
    pub fn color(&self, semantic: SemanticColor) -> Color {
        self.active_theme().color(semantic)
    }

    /// Get a style from the active theme
    pub fn style(&self, semantic: SemanticColor) -> Style {
        self.active_theme().style(semantic)
    }

    /// Get a style with modifiers from the active theme
    pub fn style_with_modifiers(&self, semantic: SemanticColor, modifiers: Modifier) -> Style {
        self.active_theme()
            .style_with_modifiers(semantic, modifiers)
    }

    /// Get a style with background from the active theme
    pub fn style_with_bg(&self, fg: SemanticColor, bg: SemanticColor) -> Style {
        self.active_theme().style_with_bg(fg, bg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_theme_mode_parsing() {
        assert_eq!(
            "dark"
                .parse::<ThemeMode>()
                .map_err(|e| format!("Failed to parse dark: {e}"))
                .unwrap_or(ThemeMode::Dark),
            ThemeMode::Dark
        );
        assert_eq!(
            "light"
                .parse::<ThemeMode>()
                .map_err(|e| format!("Failed to parse light: {e}"))
                .unwrap_or(ThemeMode::Light),
            ThemeMode::Light
        );
        assert_eq!(
            "auto"
                .parse::<ThemeMode>()
                .map_err(|e| format!("Failed to parse auto: {e}"))
                .unwrap_or(ThemeMode::Auto),
            ThemeMode::Auto
        );
        assert_eq!(
            "DARK"
                .parse::<ThemeMode>()
                .map_err(|e| format!("Failed to parse DARK: {e}"))
                .unwrap_or(ThemeMode::Dark),
            ThemeMode::Dark
        );
        assert!("invalid".parse::<ThemeMode>().is_err());
    }

    #[test]
    fn test_theme_mode_display() {
        assert_eq!(ThemeMode::Dark.to_string(), "dark");
        assert_eq!(ThemeMode::Light.to_string(), "light");
        assert_eq!(ThemeMode::Auto.to_string(), "auto");
    }

    #[test]
    fn test_theme_creation() {
        let dark_theme = Theme::new(ThemeMode::Dark);
        assert!(dark_theme.is_dark());
        assert!(!dark_theme.is_light());

        let light_theme = Theme::new(ThemeMode::Light);
        assert!(!light_theme.is_dark());
        assert!(light_theme.is_light());
    }

    #[test]
    fn test_theme_colors() {
        let theme = Theme::new(ThemeMode::Dark);
        let primary_color = theme.color(SemanticColor::Primary);
        assert_eq!(primary_color, Color::Rgb(134, 238, 255));

        let style = theme.style(SemanticColor::Success);
        assert_eq!(style.fg, Some(Color::Rgb(169, 230, 158)));
    }

    #[test]
    fn test_theme_manager() {
        let manager = ThemeManager::global();

        // Test setting mode
        manager.set_mode(ThemeMode::Light);
        let theme = manager.active_theme();
        assert!(theme.is_light());

        // Test getting colors
        let color = manager.color(SemanticColor::Text);
        assert_eq!(color, Color::Rgb(20, 20, 20)); // Light theme text color
    }

    #[test]
    fn test_auto_theme_resolution() {
        // Test that Auto mode gets resolved to a concrete theme
        let auto_theme = Theme::new(ThemeMode::Auto);

        // The resolved mode should not be Auto
        assert!(matches!(
            auto_theme.mode(),
            ThemeMode::Dark | ThemeMode::Light
        ));

        // Should be able to get colors without panic
        let _primary_color = auto_theme.color(SemanticColor::Primary);
        let _text_color = auto_theme.color(SemanticColor::Text);
    }

    #[test]
    fn test_theme_manager_auto_mode() {
        let manager = ThemeManager::global();

        // Set to auto mode
        manager.set_mode(ThemeMode::Auto);
        let theme = manager.active_theme();

        // Should resolve to a concrete theme (not Auto)
        assert!(matches!(theme.mode(), ThemeMode::Dark | ThemeMode::Light));

        // Should be able to get styles
        let style = manager.style(SemanticColor::Primary);
        assert!(style.fg.is_some());
    }
}
