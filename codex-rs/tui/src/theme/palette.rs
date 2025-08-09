use ratatui::style::Color;
use std::collections::HashMap;

use super::semantic::SemanticColor;

/// A complete color palette for a theme
#[derive(Debug, Clone)]
pub struct Palette {
    colors: HashMap<SemanticColor, Color>,
}

impl Palette {
    /// Create a new palette with the given color mappings
    pub fn new(colors: HashMap<SemanticColor, Color>) -> Self {
        Self { colors }
    }

    /// Get the color for a semantic token
    pub fn get(&self, semantic: SemanticColor) -> Color {
        self.colors.get(&semantic).copied().unwrap_or(Color::Reset)
    }

    /// Check if the palette has a color for the given semantic token
    pub fn has(&self, semantic: SemanticColor) -> bool {
        self.colors.contains_key(&semantic)
    }

    /// Validate that the palette has all required semantic colors
    pub fn validate(&self) -> Result<(), Vec<SemanticColor>> {
        let missing: Vec<SemanticColor> = SemanticColor::all()
            .iter()
            .filter(|&&color| !self.has(color))
            .copied()
            .collect();

        if missing.is_empty() {
            Ok(())
        } else {
            Err(missing)
        }
    }
}

/// Dark theme palette - optimized for dark backgrounds
pub fn dark_palette() -> Palette {
    let mut colors = HashMap::new();

    // Primary colors
    colors.insert(SemanticColor::Primary, Color::Rgb(134, 238, 255)); // Light blue (from existing LIGHT_BLUE)
    colors.insert(SemanticColor::Secondary, Color::Cyan);
    colors.insert(SemanticColor::Success, Color::Rgb(169, 230, 158)); // Success green (from existing SUCCESS_GREEN)
    colors.insert(SemanticColor::Error, Color::Rgb(255, 100, 100)); // Soft red
    colors.insert(SemanticColor::Warning, Color::Rgb(255, 200, 100)); // Soft yellow/orange
    colors.insert(SemanticColor::Info, Color::Rgb(100, 180, 255)); // Info blue

    // Text colors
    colors.insert(SemanticColor::Text, Color::Rgb(230, 230, 230)); // Almost white
    colors.insert(SemanticColor::TextMuted, Color::Rgb(160, 160, 160)); // Gray
    colors.insert(SemanticColor::TextDisabled, Color::Rgb(100, 100, 100)); // Dark gray

    // Background colors
    colors.insert(SemanticColor::Background, Color::Rgb(15, 15, 15)); // Very dark
    colors.insert(SemanticColor::Surface, Color::Rgb(30, 30, 30)); // Slightly lighter
    colors.insert(SemanticColor::SurfaceElevated, Color::Rgb(45, 45, 45)); // Even lighter

    // Border colors
    colors.insert(SemanticColor::Border, Color::Rgb(60, 60, 60)); // Dark gray
    colors.insert(SemanticColor::BorderFocused, Color::Cyan);

    // Special colors
    colors.insert(SemanticColor::Accent, Color::Magenta);
    colors.insert(SemanticColor::Selection, Color::Rgb(50, 50, 80)); // Dark blue

    // Code colors
    colors.insert(SemanticColor::CodeText, Color::Rgb(200, 200, 200));
    colors.insert(SemanticColor::CodeBackground, Color::Rgb(25, 25, 25));

    // Diff colors
    colors.insert(SemanticColor::DiffAdd, Color::Rgb(50, 200, 50)); // Green
    colors.insert(SemanticColor::DiffRemove, Color::Rgb(200, 50, 50)); // Red
    colors.insert(SemanticColor::DiffModify, Color::Rgb(200, 200, 50)); // Yellow

    // Other colors
    colors.insert(SemanticColor::Link, Color::Rgb(100, 150, 255)); // Blue
    colors.insert(SemanticColor::ShimmerHigh, Color::White);
    colors.insert(SemanticColor::ShimmerMid, Color::Rgb(180, 180, 180));
    colors.insert(SemanticColor::ShimmerLow, Color::Rgb(100, 100, 100));
    colors.insert(SemanticColor::Tool, Color::Blue);
    colors.insert(SemanticColor::Header, Color::Magenta);

    Palette::new(colors)
}

/// Light theme palette - optimized for light backgrounds
pub fn light_palette() -> Palette {
    let mut colors = HashMap::new();

    // Primary colors
    colors.insert(SemanticColor::Primary, Color::Rgb(0, 100, 200)); // Darker blue for light bg
    colors.insert(SemanticColor::Secondary, Color::Rgb(0, 150, 150)); // Darker cyan
    colors.insert(SemanticColor::Success, Color::Rgb(0, 150, 0)); // Darker green
    colors.insert(SemanticColor::Error, Color::Rgb(200, 0, 0)); // Darker red
    colors.insert(SemanticColor::Warning, Color::Rgb(200, 100, 0)); // Darker orange
    colors.insert(SemanticColor::Info, Color::Rgb(0, 100, 200)); // Info blue

    // Text colors
    colors.insert(SemanticColor::Text, Color::Rgb(20, 20, 20)); // Almost black
    colors.insert(SemanticColor::TextMuted, Color::Rgb(100, 100, 100)); // Medium gray
    colors.insert(SemanticColor::TextDisabled, Color::Rgb(160, 160, 160)); // Light gray

    // Background colors
    colors.insert(SemanticColor::Background, Color::Rgb(255, 255, 255)); // White
    colors.insert(SemanticColor::Surface, Color::Rgb(245, 245, 245)); // Very light gray
    colors.insert(SemanticColor::SurfaceElevated, Color::Rgb(255, 255, 255)); // White with shadow

    // Border colors
    colors.insert(SemanticColor::Border, Color::Rgb(200, 200, 200)); // Light gray
    colors.insert(SemanticColor::BorderFocused, Color::Rgb(0, 120, 200)); // Blue

    // Special colors
    colors.insert(SemanticColor::Accent, Color::Rgb(150, 0, 150)); // Purple
    colors.insert(SemanticColor::Selection, Color::Rgb(200, 220, 255)); // Light blue

    // Code colors
    colors.insert(SemanticColor::CodeText, Color::Rgb(40, 40, 40));
    colors.insert(SemanticColor::CodeBackground, Color::Rgb(240, 240, 240));

    // Diff colors
    colors.insert(SemanticColor::DiffAdd, Color::Rgb(0, 150, 0)); // Green
    colors.insert(SemanticColor::DiffRemove, Color::Rgb(200, 0, 0)); // Red
    colors.insert(SemanticColor::DiffModify, Color::Rgb(180, 120, 0)); // Orange

    // Other colors
    colors.insert(SemanticColor::Link, Color::Rgb(0, 80, 200)); // Blue
    colors.insert(SemanticColor::ShimmerHigh, Color::Rgb(100, 100, 100));
    colors.insert(SemanticColor::ShimmerMid, Color::Rgb(150, 150, 150));
    colors.insert(SemanticColor::ShimmerLow, Color::Rgb(200, 200, 200));
    colors.insert(SemanticColor::Tool, Color::Rgb(0, 100, 200));
    colors.insert(SemanticColor::Header, Color::Rgb(150, 0, 150));

    Palette::new(colors)
}

lazy_static::lazy_static! {
    /// Static dark palette instance
    pub static ref DARK_PALETTE: Palette = dark_palette();
    /// Static light palette instance
    pub static ref LIGHT_PALETTE: Palette = light_palette();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dark_palette_complete() {
        let palette = dark_palette();
        assert!(palette.validate().is_ok(), "Dark palette is missing colors");
    }

    #[test]
    fn test_light_palette_complete() {
        let palette = light_palette();
        assert!(
            palette.validate().is_ok(),
            "Light palette is missing colors"
        );
    }

    #[test]
    fn test_palette_get() {
        let palette = dark_palette();
        let color = palette.get(SemanticColor::Primary);
        assert_eq!(color, Color::Rgb(134, 238, 255));
    }

    #[test]
    fn test_contrast_ratios() {
        // This test ensures basic contrast requirements
        // In a full implementation, we'd calculate actual WCAG contrast ratios
        let dark = dark_palette();
        let light = light_palette();

        // Dark theme: light text on dark background
        let dark_text = dark.get(SemanticColor::Text);
        let dark_bg = dark.get(SemanticColor::Background);
        assert_ne!(dark_text, dark_bg);

        // Light theme: dark text on light background
        let light_text = light.get(SemanticColor::Text);
        let light_bg = light.get(SemanticColor::Background);
        assert_ne!(light_text, light_bg);
    }
}
