// Theme module for Codex CLI TUI
// Provides centralized color management and theme support for light/dark modes

pub mod manager;
pub mod palette;
pub mod semantic;

pub use manager::{Theme, ThemeManager, ThemeMode};
pub use palette::{Palette, DARK_PALETTE, LIGHT_PALETTE};
pub use semantic::SemanticColor;

/// Re-export commonly used theme functionality
pub fn get_active_theme() -> &'static Theme {
    ThemeManager::global().active_theme()
}

/// Initialize the theme system with a specific mode
pub fn init_theme(mode: ThemeMode) {
    ThemeManager::global().set_mode(mode);
}