use crate::theme::ThemeMode;
use crossterm::terminal;
use std::io::Write;
use std::io::{self};
use std::time::Duration;

/// Terminal background detection with multiple fallback methods
///
/// Uses a comprehensive fallback chain to detect terminal background:
/// 1. COLORFGBG environment variable (bash/zsh)
/// 2. TERM_PROGRAM_BACKGROUND (iTerm2)
/// 3. Terminal-specific environment variables (VS Code, Windows Terminal)
/// 4. OSC 11 sequence query (when available)
/// 5. Default fallback to dark theme
pub struct TerminalDetector {
    timeout: Duration,
}

impl Default for TerminalDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl TerminalDetector {
    /// Create a new terminal detector with default settings
    pub fn new() -> Self {
        Self {
            timeout: Duration::from_millis(100), // Fast timeout for UI responsiveness
        }
    }

    /// Create a terminal detector with custom timeout
    pub fn with_timeout(timeout: Duration) -> Self {
        Self { timeout }
    }

    /// Detect terminal background using comprehensive fallback chain
    ///
    /// Returns the detected theme mode, defaulting to Dark if detection fails
    pub fn detect_background(&self) -> ThemeMode {
        // 1. Check COLORFGBG environment variable (most reliable)
        if let Some(mode) = self.detect_from_colorfgbg() {
            tracing::debug!("Theme detected via COLORFGBG: {:?}", mode);
            return mode;
        }

        // 2. Check TERM_PROGRAM_BACKGROUND (iTerm2)
        if let Some(mode) = self.detect_from_iterm() {
            tracing::debug!("Theme detected via iTerm2: {:?}", mode);
            return mode;
        }

        // 3. Check other terminal-specific variables
        if let Some(mode) = self.detect_from_terminal_env() {
            tracing::debug!("Theme detected via terminal environment: {:?}", mode);
            return mode;
        }

        // 4. Query terminal using OSC 11 sequence (when implemented)
        if let Some(mode) = self.detect_from_osc11() {
            tracing::debug!("Theme detected via OSC 11: {:?}", mode);
            return mode;
        }

        // 5. Default fallback to dark theme
        tracing::debug!("No theme detection method succeeded, defaulting to dark");
        ThemeMode::Dark
    }

    /// Detect theme from COLORFGBG environment variable
    ///
    /// COLORFGBG format: "foreground;background"
    /// - Background 0-7: Dark theme
    /// - Background 8-15: Light theme
    /// - Example: "15;0" = light text on dark background (dark theme)
    pub fn detect_from_colorfgbg(&self) -> Option<ThemeMode> {
        let colorfgbg = std::env::var("COLORFGBG").ok()?;

        // Parse "foreground;background" format
        let (_fg, bg) = colorfgbg.split_once(';')?;

        match bg.parse::<u8>() {
            Ok(bg_color) if bg_color < 8 => {
                // Colors 0-7 are dark backgrounds
                Some(ThemeMode::Dark)
            }
            Ok(bg_color) if bg_color >= 8 => {
                // Colors 8-15 are light backgrounds
                Some(ThemeMode::Light)
            }
            _ => {
                tracing::warn!("Invalid COLORFGBG background color: {}", bg);
                None
            }
        }
    }

    /// Detect theme from iTerm2-specific environment variables
    pub fn detect_from_iterm(&self) -> Option<ThemeMode> {
        // Only check if we're actually in iTerm2
        if std::env::var("TERM_PROGRAM").as_deref() != Ok("iTerm.app") {
            return None;
        }

        match std::env::var("TERM_PROGRAM_BACKGROUND").as_deref() {
            Ok("light") => Some(ThemeMode::Light),
            Ok("dark") => Some(ThemeMode::Dark),
            Ok(other) => {
                tracing::warn!("Unknown iTerm2 background setting: {}", other);
                None
            }
            Err(_) => None,
        }
    }

    /// Detect theme from other terminal-specific environment variables
    pub fn detect_from_terminal_env(&self) -> Option<ThemeMode> {
        // VS Code integrated terminal
        if std::env::var("VSCODE_INJECTION").is_ok() {
            if let Ok(theme) = std::env::var("VSCODE_THEME_KIND") {
                match theme.as_str() {
                    "vscode-light" | "vscode-high-contrast-light" => {
                        return Some(ThemeMode::Light);
                    }
                    "vscode-dark" | "vscode-high-contrast" => {
                        return Some(ThemeMode::Dark);
                    }
                    _ => {}
                }
            }
        }

        // Windows Terminal - check for WT_SESSION
        if std::env::var("WT_SESSION").is_ok() {
            // Windows Terminal doesn't expose theme info easily via env vars
            // This could be extended in the future with registry queries
            tracing::debug!("Detected Windows Terminal but no theme info available");
        }

        // JetBrains IDEs
        if std::env::var("IDEA_INITIAL_DIRECTORY").is_ok()
            || std::env::var("PYCHARM_HOSTED").is_ok()
        {
            // JetBrains IDEs don't typically expose theme info via env vars
            tracing::debug!("Detected JetBrains IDE terminal but no theme info available");
        }

        None
    }

    /// Detect theme using OSC 11 terminal background query
    ///
    /// Sends OSC 11 sequence to query terminal background color.
    /// This method is synchronous and includes timeout handling.
    ///
    /// OSC 11 format: ESC ] 11 ; ? BEL
    /// Response format: ESC ] 11 ; rgb:RRRR/GGGG/BBBB ESC \
    fn detect_from_osc11(&self) -> Option<ThemeMode> {
        // Only attempt OSC queries if we're in a real terminal
        if !self.is_terminal_suitable_for_osc() {
            return None;
        }

        // Check if we're in raw mode - OSC queries need proper terminal state
        if !terminal::is_raw_mode_enabled().unwrap_or(false) {
            tracing::debug!("Terminal not in raw mode, skipping OSC 11 query");
            return None;
        }

        self.query_background_color_sync()
    }

    /// Check if terminal is suitable for OSC sequence queries
    pub fn is_terminal_suitable_for_osc(&self) -> bool {
        // Don't try OSC queries if stdout is not a terminal
        if !crossterm::tty::IsTty::is_tty(&io::stdout()) {
            return false;
        }

        // Skip OSC queries in some environments where they're problematic
        if let Ok(term) = std::env::var("TERM") {
            match term.as_str() {
                // Skip for basic terminals that don't support OSC
                "dumb" | "unknown" => return false,
                // Skip for terminals known to have issues with OSC 11
                "linux" | "console" => return false,
                _ => {}
            }
        }

        // Skip in CI environments where OSC queries don't work
        if std::env::var("CI").is_ok()
            || std::env::var("GITHUB_ACTIONS").is_ok()
            || std::env::var("GITLAB_CI").is_ok()
        {
            return false;
        }

        true
    }

    /// Synchronously query terminal background color using OSC 11
    fn query_background_color_sync(&self) -> Option<ThemeMode> {
        // OSC 11 query sequence: ESC ] 11 ; ? BEL
        let query = "\x1b]11;?\x07";

        // Try to send the query
        if let Err(e) = io::stdout().write_all(query.as_bytes()) {
            tracing::debug!("Failed to send OSC 11 query: {}", e);
            return None;
        }

        if let Err(e) = io::stdout().flush() {
            tracing::debug!("Failed to flush OSC 11 query: {}", e);
            return None;
        }

        // Read response with timeout
        // Note: This is a simplified implementation
        // In a real implementation, you'd want proper async I/O with timeout
        self.read_osc_response_with_timeout()
    }

    /// Read OSC response with timeout (simplified implementation)
    fn read_osc_response_with_timeout(&self) -> Option<ThemeMode> {
        use std::sync::mpsc;
        use std::thread;

        let (tx, rx) = mpsc::channel();
        let timeout = self.timeout;

        // Spawn a thread to read the response
        thread::spawn(move || {
            let mut buffer = [0u8; 256];
            if let Ok(stdin) = std::fs::File::open("/dev/tty") {
                use std::io::Read;
                if let Ok(n) = (&stdin).read(&mut buffer) {
                    let response = String::from_utf8_lossy(&buffer[..n]);
                    let _ = tx.send(response.to_string());
                }
            }
        });

        // Wait for response with timeout
        match rx.recv_timeout(timeout) {
            Ok(response) => {
                tracing::debug!("Received OSC 11 response: {:?}", response);
                self.parse_osc11_response(&response)
            }
            Err(_) => {
                tracing::debug!("OSC 11 query timed out after {:?}", timeout);
                None
            }
        }
    }

    /// Parse OSC 11 response to determine theme
    ///
    /// Expected format: ESC ] 11 ; rgb:RRRR/GGGG/BBBB ESC \
    /// or: ESC ] 11 ; rgb:RRRR/GGGG/BBBB BEL
    pub fn parse_osc11_response(&self, response: &str) -> Option<ThemeMode> {
        // Handle both ESC \ and BEL terminators
        let rgb_part = if let Some(part) = response.strip_prefix("\x1b]11;rgb:") {
            part.strip_suffix("\x1b\\")
                .or_else(|| part.strip_suffix("\x07"))?
        } else {
            return None;
        };

        // Parse RGB components: RRRR/GGGG/BBBB
        let components: Vec<&str> = rgb_part.split('/').collect();
        if components.len() != 3 {
            tracing::warn!("Invalid OSC 11 RGB format: {}", rgb_part);
            return None;
        }

        // Parse hex values (typically 16-bit)
        let r = u16::from_str_radix(components[0], 16).ok()?;
        let g = u16::from_str_radix(components[1], 16).ok()?;
        let b = u16::from_str_radix(components[2], 16).ok()?;

        // Calculate perceived brightness using standard formula
        // Use 16-bit values (0-65535) and convert to 0-1 range
        let r_norm = r as f64 / 65535.0;
        let g_norm = g as f64 / 65535.0;
        let b_norm = b as f64 / 65535.0;

        // ITU-R BT.709 luma coefficients
        let brightness = 0.2126 * r_norm + 0.7152 * g_norm + 0.0722 * b_norm;

        tracing::debug!(
            "OSC 11 parsed RGB: #{:04x}/{:04x}/{:04x}, brightness: {:.3}",
            r,
            g,
            b,
            brightness
        );

        // Threshold at 0.5 - above is light theme, below is dark theme
        if brightness > 0.5 {
            Some(ThemeMode::Light)
        } else {
            Some(ThemeMode::Dark)
        }
    }

    /// Get the configured timeout for terminal queries
    pub fn timeout(&self) -> Duration {
        self.timeout
    }
}

/// Convenience function for one-shot terminal background detection
pub fn detect_terminal_background() -> ThemeMode {
    TerminalDetector::new().detect_background()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_detector_creation() {
        let detector = TerminalDetector::new();
        assert_eq!(detector.timeout(), Duration::from_millis(100));

        let custom_detector = TerminalDetector::with_timeout(Duration::from_millis(200));
        assert_eq!(custom_detector.timeout(), Duration::from_millis(200));
    }

    #[test]
    fn test_colorfgbg_detection() {
        // Test dark background detection (bg color 0-7)
        unsafe {
            env::set_var("COLORFGBG", "15;0");
        }
        let detector = TerminalDetector::new();
        assert_eq!(detector.detect_from_colorfgbg(), Some(ThemeMode::Dark));

        // Test light background detection (bg color 8-15)
        unsafe {
            env::set_var("COLORFGBG", "0;15");
        }
        assert_eq!(detector.detect_from_colorfgbg(), Some(ThemeMode::Light));

        // Test invalid format
        unsafe {
            env::set_var("COLORFGBG", "invalid");
        }
        assert_eq!(detector.detect_from_colorfgbg(), None);

        // Test invalid background color
        unsafe {
            env::set_var("COLORFGBG", "15;invalid");
        }
        assert_eq!(detector.detect_from_colorfgbg(), None);

        // Clean up
        unsafe {
            env::remove_var("COLORFGBG");
        }
    }

    // iTerm detection tests are in the integration test suite to avoid env var conflicts

    // VS Code detection tests are in the integration test suite to avoid env var conflicts

    #[test]
    fn test_fallback_chain() {
        // Ensure no environment variables are set
        let vars_to_clean = [
            "COLORFGBG",
            "TERM_PROGRAM",
            "TERM_PROGRAM_BACKGROUND",
            "VSCODE_INJECTION",
            "VSCODE_THEME_KIND",
        ];

        for var in &vars_to_clean {
            unsafe {
                env::remove_var(var);
            }
        }

        let detector = TerminalDetector::new();
        // Should default to dark when no detection methods work
        assert_eq!(detector.detect_background(), ThemeMode::Dark);
    }

    #[test]
    fn test_convenience_function() {
        // Test that the convenience function works
        let result = detect_terminal_background();
        // Should return a valid theme mode (either detected or default Dark)
        assert!(matches!(result, ThemeMode::Dark | ThemeMode::Light));
    }

    #[test]
    fn test_osc11_response_parsing() {
        let detector = TerminalDetector::new();

        // Test light background response (white: ffff/ffff/ffff)
        let light_response = "\x1b]11;rgb:ffff/ffff/ffff\x1b\\";
        assert_eq!(
            detector.parse_osc11_response(light_response),
            Some(ThemeMode::Light)
        );

        // Test dark background response (black: 0000/0000/0000)
        let dark_response = "\x1b]11;rgb:0000/0000/0000\x1b\\";
        assert_eq!(
            detector.parse_osc11_response(dark_response),
            Some(ThemeMode::Dark)
        );

        // Test medium gray background (should be dark theme)
        let gray_response = "\x1b]11;rgb:4000/4000/4000\x1b\\";
        assert_eq!(
            detector.parse_osc11_response(gray_response),
            Some(ThemeMode::Dark)
        );

        // Test light gray background (should be light theme)
        let light_gray_response = "\x1b]11;rgb:c000/c000/c000\x1b\\";
        assert_eq!(
            detector.parse_osc11_response(light_gray_response),
            Some(ThemeMode::Light)
        );

        // Test BEL terminator instead of ESC \
        let bel_response = "\x1b]11;rgb:ffff/ffff/ffff\x07";
        assert_eq!(
            detector.parse_osc11_response(bel_response),
            Some(ThemeMode::Light)
        );

        // Test invalid format
        assert_eq!(detector.parse_osc11_response("invalid"), None);
        assert_eq!(detector.parse_osc11_response("\x1b]11;invalid\x1b\\"), None);
        assert_eq!(
            detector.parse_osc11_response("\x1b]11;rgb:invalid\x1b\\"),
            None
        );
    }

    #[test]
    fn test_brightness_calculation() {
        let detector = TerminalDetector::new();

        // Test pure colors
        let red_response = "\x1b]11;rgb:ffff/0000/0000\x1b\\";
        let green_response = "\x1b]11;rgb:0000/ffff/0000\x1b\\";
        let blue_response = "\x1b]11;rgb:0000/0000/ffff\x1b\\";

        // Green should be brightest due to luma coefficients
        assert_eq!(
            detector.parse_osc11_response(green_response),
            Some(ThemeMode::Light)
        );

        // Red and blue should be darker (below 0.5 brightness)
        assert_eq!(
            detector.parse_osc11_response(red_response),
            Some(ThemeMode::Dark)
        );
        assert_eq!(
            detector.parse_osc11_response(blue_response),
            Some(ThemeMode::Dark)
        );
    }

    #[test]
    fn test_terminal_suitability() {
        let detector = TerminalDetector::new();

        // Test CI environment detection
        unsafe {
            env::set_var("CI", "true");
        }
        assert!(!detector.is_terminal_suitable_for_osc());
        unsafe {
            env::remove_var("CI");
        }

        unsafe {
            env::set_var("GITHUB_ACTIONS", "true");
        }
        assert!(!detector.is_terminal_suitable_for_osc());
        unsafe {
            env::remove_var("GITHUB_ACTIONS");
        }

        // Test problematic terminal types
        unsafe {
            env::set_var("TERM", "dumb");
        }
        assert!(!detector.is_terminal_suitable_for_osc());

        unsafe {
            env::set_var("TERM", "linux");
        }
        assert!(!detector.is_terminal_suitable_for_osc());

        unsafe {
            env::remove_var("TERM");
        }
    }
}
