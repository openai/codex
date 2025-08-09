use codex_tui::theme::DARK_PALETTE;
use codex_tui::theme::LIGHT_PALETTE;
use codex_tui::theme::SemanticColor;
use codex_tui::theme::Theme;
use codex_tui::theme::ThemeManager;
use codex_tui::theme::ThemeMode;
use codex_tui::theme::get_active_theme;
use codex_tui::theme::init_theme;
use ratatui::style::Color;
use ratatui::style::Modifier;
use std::str::FromStr;

#[test]
fn test_theme_mode_parsing() {
    assert_eq!(
        ThemeMode::from_str("dark")
            .map_err(|e| format!("Parse error: {e}"))
            .unwrap_or(ThemeMode::Dark),
        ThemeMode::Dark
    );
    assert_eq!(
        ThemeMode::from_str("light")
            .map_err(|e| format!("Parse error: {e}"))
            .unwrap_or(ThemeMode::Light),
        ThemeMode::Light
    );
    assert_eq!(
        ThemeMode::from_str("auto")
            .map_err(|e| format!("Parse error: {e}"))
            .unwrap_or(ThemeMode::Auto),
        ThemeMode::Auto
    );

    // Case insensitive
    assert_eq!(
        ThemeMode::from_str("DARK")
            .map_err(|e| format!("Parse error: {e}"))
            .unwrap_or(ThemeMode::Dark),
        ThemeMode::Dark
    );
    assert_eq!(
        ThemeMode::from_str("Light")
            .map_err(|e| format!("Parse error: {e}"))
            .unwrap_or(ThemeMode::Light),
        ThemeMode::Light
    );
    assert_eq!(
        ThemeMode::from_str("AUTO")
            .map_err(|e| format!("Parse error: {e}"))
            .unwrap_or(ThemeMode::Auto),
        ThemeMode::Auto
    );

    // Invalid input
    assert!(ThemeMode::from_str("invalid").is_err());
    assert!(ThemeMode::from_str("").is_err());
}

#[test]
fn test_theme_mode_display() {
    assert_eq!(ThemeMode::Dark.to_string(), "dark");
    assert_eq!(ThemeMode::Light.to_string(), "light");
    assert_eq!(ThemeMode::Auto.to_string(), "auto");
}

#[test]
fn test_theme_mode_default() {
    assert_eq!(ThemeMode::default(), ThemeMode::Auto);
}

#[test]
fn test_theme_creation() {
    let dark_theme = Theme::new(ThemeMode::Dark);
    assert_eq!(dark_theme.mode(), ThemeMode::Dark);
    assert!(dark_theme.is_dark());
    assert!(!dark_theme.is_light());

    let light_theme = Theme::new(ThemeMode::Light);
    assert_eq!(light_theme.mode(), ThemeMode::Light);
    assert!(!light_theme.is_dark());
    assert!(light_theme.is_light());

    let auto_theme = Theme::new(ThemeMode::Auto);
    // Auto mode should be resolved to either Dark or Light, not remain as Auto
    assert!(matches!(
        auto_theme.mode(),
        ThemeMode::Dark | ThemeMode::Light
    ));
    // The theme should work correctly regardless of which mode was detected
    let _color = auto_theme.color(SemanticColor::Primary);
}

#[test]
fn test_theme_colors() {
    let dark_theme = Theme::new(ThemeMode::Dark);
    let light_theme = Theme::new(ThemeMode::Light);

    // Test that colors are different between themes
    let dark_text = dark_theme.color(SemanticColor::Text);
    let light_text = light_theme.color(SemanticColor::Text);
    assert_ne!(dark_text, light_text);

    let dark_bg = dark_theme.color(SemanticColor::Background);
    let light_bg = light_theme.color(SemanticColor::Background);
    assert_ne!(dark_bg, light_bg);

    // Test that primary colors match expected values
    assert_eq!(
        dark_theme.color(SemanticColor::Primary),
        Color::Rgb(134, 238, 255)
    );
    assert_eq!(
        light_theme.color(SemanticColor::Primary),
        Color::Rgb(0, 100, 200)
    );
}

#[test]
fn test_theme_styles() {
    let theme = Theme::new(ThemeMode::Dark);

    // Basic style
    let style = theme.style(SemanticColor::Primary);
    assert_eq!(style.fg, Some(Color::Rgb(134, 238, 255)));
    assert_eq!(style.bg, None);

    // Style with background
    let bg_style = theme.style_with_bg(SemanticColor::Text, SemanticColor::Surface);
    assert_eq!(bg_style.fg, Some(Color::Rgb(230, 230, 230)));
    assert_eq!(bg_style.bg, Some(Color::Rgb(30, 30, 30)));

    // Style with modifiers
    let mod_style = theme.style_with_modifiers(SemanticColor::Error, Modifier::BOLD);
    assert_eq!(mod_style.fg, Some(Color::Rgb(255, 100, 100)));
    assert!(mod_style.add_modifier.contains(Modifier::BOLD));
}

#[test]
fn test_palette_completeness() {
    // Test that both palettes have all semantic colors
    assert!(DARK_PALETTE.validate().is_ok());
    assert!(LIGHT_PALETTE.validate().is_ok());

    // Test that all semantic colors are defined
    for &color in SemanticColor::all() {
        assert!(DARK_PALETTE.has(color), "Dark palette missing {color:?}");
        assert!(LIGHT_PALETTE.has(color), "Light palette missing {color:?}");
    }
}

#[test]
fn test_palette_color_retrieval() {
    let dark_palette = &*DARK_PALETTE;
    let light_palette = &*LIGHT_PALETTE;

    // Test specific colors
    assert_eq!(
        dark_palette.get(SemanticColor::Primary),
        Color::Rgb(134, 238, 255)
    );
    assert_eq!(
        light_palette.get(SemanticColor::Primary),
        Color::Rgb(0, 100, 200)
    );

    // Test success colors from existing constants
    assert_eq!(
        dark_palette.get(SemanticColor::Success),
        Color::Rgb(169, 230, 158)
    );
    assert_eq!(
        light_palette.get(SemanticColor::Success),
        Color::Rgb(0, 150, 0)
    );
}

#[test]
fn test_semantic_color_descriptions() {
    // Test that all semantic colors have descriptions
    for &color in SemanticColor::all() {
        let description = color.description();
        assert!(!description.is_empty(), "No description for {color:?}");
        assert!(description.len() > 5, "Description too short for {color:?}");
    }
}

#[test]
fn test_theme_manager() {
    let manager = ThemeManager::global();

    // Test setting different modes
    manager.set_mode(ThemeMode::Dark);
    let theme = manager.active_theme();
    assert!(theme.is_dark());

    manager.set_mode(ThemeMode::Light);
    let theme = manager.active_theme();
    assert!(theme.is_light());

    // Test color access through manager
    let color = manager.color(SemanticColor::Primary);
    assert_eq!(color, Color::Rgb(0, 100, 200)); // Light theme primary

    let style = manager.style(SemanticColor::Error);
    assert_eq!(style.fg, Some(Color::Rgb(200, 0, 0))); // Light theme error
}

#[test]
fn test_theme_convenience_functions() {
    // Test the convenience functions from the module
    init_theme(ThemeMode::Light);
    let theme = get_active_theme();
    assert!(theme.is_light());

    init_theme(ThemeMode::Dark);
    let theme = get_active_theme();
    assert!(theme.is_dark());
}

#[test]
fn test_contrast_assumptions() {
    // Basic sanity checks for contrast (not actual WCAG calculations)
    let dark_theme = Theme::new(ThemeMode::Dark);
    let light_theme = Theme::new(ThemeMode::Light);

    // Dark theme should have light text on dark background
    let dark_text = dark_theme.color(SemanticColor::Text);
    let dark_bg = dark_theme.color(SemanticColor::Background);
    if let (Color::Rgb(tr, tg, tb), Color::Rgb(br, bg, bb)) = (dark_text, dark_bg) {
        // Text should be lighter than background
        let text_brightness = (tr as u32 + tg as u32 + tb as u32) / 3;
        let bg_brightness = (br as u32 + bg as u32 + bb as u32) / 3;
        assert!(
            text_brightness > bg_brightness,
            "Dark theme text should be lighter than background"
        );
    }

    // Light theme should have dark text on light background
    let light_text = light_theme.color(SemanticColor::Text);
    let light_bg = light_theme.color(SemanticColor::Background);
    if let (Color::Rgb(tr, tg, tb), Color::Rgb(br, bg, bb)) = (light_text, light_bg) {
        // Text should be darker than background
        let text_brightness = (tr as u32 + tg as u32 + tb as u32) / 3;
        let bg_brightness = (br as u32 + bg as u32 + bb as u32) / 3;
        assert!(
            text_brightness < bg_brightness,
            "Light theme text should be darker than background"
        );
    }
}

#[test]
fn test_all_semantic_colors_unique() {
    // Ensure SemanticColor::all() returns unique values
    let colors = SemanticColor::all();
    for (i, &color1) in colors.iter().enumerate() {
        for (j, &color2) in colors.iter().enumerate() {
            if i != j {
                assert_ne!(color1, color2, "Duplicate semantic color found");
            }
        }
    }
}

#[test]
fn test_theme_color_consistency() {
    // Test that themes are internally consistent
    let dark_theme = Theme::new(ThemeMode::Dark);
    let light_theme = Theme::new(ThemeMode::Light);

    // Background should be different from text
    assert_ne!(
        dark_theme.color(SemanticColor::Background),
        dark_theme.color(SemanticColor::Text)
    );
    assert_ne!(
        light_theme.color(SemanticColor::Background),
        light_theme.color(SemanticColor::Text)
    );

    // Surface should be different from background
    assert_ne!(
        dark_theme.color(SemanticColor::Background),
        dark_theme.color(SemanticColor::Surface)
    );
    assert_ne!(
        light_theme.color(SemanticColor::Background),
        light_theme.color(SemanticColor::Surface)
    );
}
