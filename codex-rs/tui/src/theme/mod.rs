// Theme module for Codex CLI TUI
// Provides centralized color management and theme support for light/dark modes

pub mod detection;
pub mod manager;
pub mod palette;
pub mod semantic;

pub use detection::TerminalDetector;
pub use detection::detect_terminal_background;
pub use manager::Theme;
pub use manager::ThemeManager;
pub use manager::ThemeMode;
pub use palette::DARK_PALETTE;
pub use palette::LIGHT_PALETTE;
pub use palette::Palette;
pub use semantic::SemanticColor;

/// Re-export commonly used theme functionality
pub fn get_active_theme() -> &'static Theme {
    ThemeManager::global().active_theme()
}

/// Initialize the theme system with a specific mode
pub fn init_theme(mode: ThemeMode) {
    ThemeManager::global().set_mode(mode);
}

/// Initialize theme system with automatic detection
pub fn init_theme_auto() {
    let detected_mode = detect_terminal_background();
    ThemeManager::global().set_mode(detected_mode);
}
