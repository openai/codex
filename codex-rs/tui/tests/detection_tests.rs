use codex_tui::theme::TerminalDetector;
use codex_tui::theme::ThemeManager;
use codex_tui::theme::ThemeMode;
use codex_tui::theme::detect_terminal_background;
use codex_tui::theme::init_theme_auto;
use std::env;
use std::time::Duration;

// Helper functions for safe environment variable manipulation in tests
fn set_env_var(key: &str, value: &str) {
    unsafe { env::set_var(key, value) };
}

fn remove_env_var(key: &str) {
    unsafe { env::remove_var(key) };
}

#[test]
fn test_terminal_detector_creation() {
    let detector = TerminalDetector::new();
    assert_eq!(detector.timeout(), Duration::from_millis(100));

    let custom_detector = TerminalDetector::with_timeout(Duration::from_millis(500));
    assert_eq!(custom_detector.timeout(), Duration::from_millis(500));

    let default_detector = TerminalDetector::default();
    assert_eq!(default_detector.timeout(), Duration::from_millis(100));
}

#[test]
fn test_colorfgbg_environment_variable() {
    let detector = TerminalDetector::new();

    // Save original value
    let original = env::var("COLORFGBG").ok();

    // Test dark background (0-7)
    for bg_color in 0..8 {
        set_env_var("COLORFGBG", &format!("15;{bg_color}"));
        assert_eq!(detector.detect_from_colorfgbg(), Some(ThemeMode::Dark));
    }

    // Test light background (8-15)
    for bg_color in 8..16 {
        set_env_var("COLORFGBG", &format!("0;{bg_color}"));
        assert_eq!(detector.detect_from_colorfgbg(), Some(ThemeMode::Light));
    }

    // Test invalid formats
    set_env_var("COLORFGBG", "invalid");
    assert_eq!(detector.detect_background(), ThemeMode::Dark); // Should fallback

    set_env_var("COLORFGBG", "15;invalid");
    assert_eq!(detector.detect_background(), ThemeMode::Dark); // Should fallback

    // Restore original value
    match original {
        Some(val) => set_env_var("COLORFGBG", &val),
        None => remove_env_var("COLORFGBG"),
    }
}

#[test]
fn test_iterm2_environment_variables() {
    let detector = TerminalDetector::new();

    // Save original values
    let original_program = env::var("TERM_PROGRAM").ok();
    let original_bg = env::var("TERM_PROGRAM_BACKGROUND").ok();

    // Test iTerm2 light theme
    set_env_var("TERM_PROGRAM", "iTerm.app");
    set_env_var("TERM_PROGRAM_BACKGROUND", "light");
    assert_eq!(detector.detect_from_iterm(), Some(ThemeMode::Light));

    // Test iTerm2 dark theme
    set_env_var("TERM_PROGRAM_BACKGROUND", "dark");
    assert_eq!(detector.detect_from_iterm(), Some(ThemeMode::Dark));

    // Test non-iTerm2 terminal (should fallback)
    set_env_var("TERM_PROGRAM", "Terminal.app");
    set_env_var("TERM_PROGRAM_BACKGROUND", "light");
    assert_eq!(detector.detect_from_iterm(), None); // Should return None for non-iTerm2

    // Test missing background setting
    set_env_var("TERM_PROGRAM", "iTerm.app");
    remove_env_var("TERM_PROGRAM_BACKGROUND");
    assert_eq!(detector.detect_from_iterm(), None); // Should return None without background setting

    // Restore original values
    match original_program {
        Some(val) => set_env_var("TERM_PROGRAM", &val),
        None => remove_env_var("TERM_PROGRAM"),
    }
    match original_bg {
        Some(val) => set_env_var("TERM_PROGRAM_BACKGROUND", &val),
        None => remove_env_var("TERM_PROGRAM_BACKGROUND"),
    }
}

#[test]
fn test_vscode_environment_variables() {
    let detector = TerminalDetector::new();

    // Save original values
    let original_injection = env::var("VSCODE_INJECTION").ok();
    let original_theme = env::var("VSCODE_THEME_KIND").ok();

    // Test VS Code light themes
    set_env_var("VSCODE_INJECTION", "1");
    for theme in ["vscode-light", "vscode-high-contrast-light"] {
        set_env_var("VSCODE_THEME_KIND", theme);
        assert_eq!(detector.detect_from_terminal_env(), Some(ThemeMode::Light));
    }

    // Test VS Code dark themes
    for theme in ["vscode-dark", "vscode-high-contrast"] {
        set_env_var("VSCODE_THEME_KIND", theme);
        assert_eq!(detector.detect_from_terminal_env(), Some(ThemeMode::Dark));
    }

    // Test without VS Code injection
    remove_env_var("VSCODE_INJECTION");
    set_env_var("VSCODE_THEME_KIND", "vscode-light");
    assert_eq!(detector.detect_from_terminal_env(), None); // Should return None without injection

    // Restore original values
    match original_injection {
        Some(val) => set_env_var("VSCODE_INJECTION", &val),
        None => remove_env_var("VSCODE_INJECTION"),
    }
    match original_theme {
        Some(val) => set_env_var("VSCODE_THEME_KIND", &val),
        None => remove_env_var("VSCODE_THEME_KIND"),
    }
}

#[test]
fn test_fallback_chain_priority() {
    let detector = TerminalDetector::new();

    // Clean environment first
    let vars_to_clean = [
        "COLORFGBG",
        "TERM_PROGRAM",
        "TERM_PROGRAM_BACKGROUND",
        "VSCODE_INJECTION",
        "VSCODE_THEME_KIND",
    ];
    let originals: Vec<_> = vars_to_clean
        .iter()
        .map(|var| (*var, env::var(var).ok()))
        .collect();

    for var in &vars_to_clean {
        remove_env_var(var);
    }

    // Test that COLORFGBG takes priority over other methods
    set_env_var("COLORFGBG", "0;15"); // Light theme
    set_env_var("TERM_PROGRAM", "iTerm.app");
    set_env_var("TERM_PROGRAM_BACKGROUND", "dark"); // Dark theme
    set_env_var("VSCODE_INJECTION", "1");
    set_env_var("VSCODE_THEME_KIND", "vscode-dark"); // Dark theme

    // Should detect light theme from COLORFGBG
    assert_eq!(detector.detect_background(), ThemeMode::Light);

    // Remove COLORFGBG, should fall back to iTerm2
    remove_env_var("COLORFGBG");
    assert_eq!(detector.detect_background(), ThemeMode::Dark);

    // Remove iTerm2, should fall back to VS Code
    remove_env_var("TERM_PROGRAM");
    assert_eq!(detector.detect_background(), ThemeMode::Dark);

    // Remove all, should default to dark
    remove_env_var("VSCODE_INJECTION");
    assert_eq!(detector.detect_background(), ThemeMode::Dark);

    // Restore original values
    for (var, original) in originals {
        match original {
            Some(val) => set_env_var(var, &val),
            None => remove_env_var(var),
        }
    }
}

#[test]
fn test_osc11_response_parsing() {
    let detector = TerminalDetector::new();

    // Test various RGB responses
    let test_cases = vec![
        // (response, expected_theme)
        ("\x1b]11;rgb:0000/0000/0000\x1b\\", ThemeMode::Dark), // Black
        ("\x1b]11;rgb:ffff/ffff/ffff\x1b\\", ThemeMode::Light), // White
        ("\x1b]11;rgb:8000/8000/8000\x1b\\", ThemeMode::Light), // Mid-gray (light)
        ("\x1b]11;rgb:4000/4000/4000\x1b\\", ThemeMode::Dark), // Dark gray
        ("\x1b]11;rgb:0000/ffff/0000\x1b\\", ThemeMode::Light), // Pure green (bright)
        ("\x1b]11;rgb:ffff/0000/0000\x1b\\", ThemeMode::Dark), // Pure red (dim)
        ("\x1b]11;rgb:0000/0000/ffff\x1b\\", ThemeMode::Dark), // Pure blue (dim)
        // Test BEL terminator
        ("\x1b]11;rgb:ffff/ffff/ffff\x07", ThemeMode::Light),
        ("\x1b]11;rgb:0000/0000/0000\x07", ThemeMode::Dark),
    ];

    for (response, expected) in test_cases {
        assert_eq!(
            detector.parse_osc11_response(response),
            Some(expected),
            "Failed for response: {response:?}"
        );
    }

    // Test invalid responses
    let invalid_responses = vec![
        "invalid",
        "\x1b]11;invalid\x1b\\",
        "\x1b]11;rgb:invalid\x1b\\",
        "\x1b]11;rgb:ffff\x1b\\",           // Missing components
        "\x1b]11;rgb:ffff/ffff\x1b\\",      // Missing blue
        "\x1b]11;rgb:gggg/ffff/ffff\x1b\\", // Invalid hex
    ];

    for response in invalid_responses {
        assert_eq!(
            detector.parse_osc11_response(response),
            None,
            "Should be invalid: {response:?}"
        );
    }
}

#[test]
fn test_ci_environment_detection() {
    let detector = TerminalDetector::new();

    // Save original values
    let ci_vars = ["CI", "GITHUB_ACTIONS", "GITLAB_CI"];
    let originals: Vec<_> = ci_vars
        .iter()
        .map(|var| (*var, env::var(var).ok()))
        .collect();

    // Test CI environment detection
    for ci_var in &ci_vars {
        set_env_var(ci_var, "true");
        assert!(!detector.is_terminal_suitable_for_osc());
        remove_env_var(ci_var);
    }

    // Restore original values
    for (var, original) in originals {
        match original {
            Some(val) => set_env_var(var, &val),
            None => remove_env_var(var),
        }
    }
}

#[test]
fn test_problematic_terminal_types() {
    let detector = TerminalDetector::new();

    // Save original TERM value
    let original_term = env::var("TERM").ok();

    let problematic_terms = ["dumb", "unknown", "linux", "console"];

    for term in &problematic_terms {
        set_env_var("TERM", term);
        assert!(!detector.is_terminal_suitable_for_osc());
    }

    // Test normal terminal types should be suitable (if not in CI)
    if env::var("CI").is_err() {
        for term in &["xterm", "xterm-256color", "screen"] {
            set_env_var("TERM", term);
            // Note: This may still be false due to TTY detection, but TERM won't be the reason
            let suitable = detector.is_terminal_suitable_for_osc();
            // We can't assert true here because we might not be in a real TTY
            // Just ensure it doesn't panic
            let _ = suitable;
        }
    }

    // Restore original value
    match original_term {
        Some(val) => set_env_var("TERM", &val),
        None => remove_env_var("TERM"),
    }
}

#[test]
fn test_convenience_functions() {
    // Test standalone detection function
    let result = detect_terminal_background();
    assert!(matches!(result, ThemeMode::Dark | ThemeMode::Light));

    // Test auto-initialization function
    let manager = ThemeManager::global();
    init_theme_auto();
    let theme = manager.active_theme();
    assert!(matches!(theme.mode(), ThemeMode::Dark | ThemeMode::Light));
}

#[test]
fn test_detection_consistency() {
    // Multiple detections should be consistent
    let detector = TerminalDetector::new();

    let first = detector.detect_background();
    let second = detector.detect_background();
    let third = detector.detect_background();

    assert_eq!(first, second);
    assert_eq!(second, third);
}

#[test]
fn test_custom_timeout() {
    let short_timeout = TerminalDetector::with_timeout(Duration::from_millis(10));
    let long_timeout = TerminalDetector::with_timeout(Duration::from_millis(1000));

    assert_eq!(short_timeout.timeout(), Duration::from_millis(10));
    assert_eq!(long_timeout.timeout(), Duration::from_millis(1000));

    // Both should still detect something
    let short_result = short_timeout.detect_background();
    let long_result = long_timeout.detect_background();

    assert!(matches!(short_result, ThemeMode::Dark | ThemeMode::Light));
    assert!(matches!(long_result, ThemeMode::Dark | ThemeMode::Light));
}
