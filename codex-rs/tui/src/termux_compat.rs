use std::time::Duration;

/// Enhanced Termux compatibility utilities
pub struct TermuxCompat;

impl TermuxCompat {
    /// Detect if running in Termux environment
    pub fn is_termux() -> bool {
        std::env::var("TERMUX_VERSION").is_ok()
            || std::env::var("PREFIX")
                .unwrap_or_default()
                .contains("com.termux")
    }

    /// Get appropriate polling timeout for the current environment
    pub fn get_poll_timeout() -> Duration {
        if Self::is_termux() {
            // Longer timeout for Termux due to slower ANSI processing
            Duration::from_millis(500)
        } else {
            Duration::from_millis(100)
        }
    }

    /// Get stabilization delay after resize events
    pub fn get_resize_delay() -> Option<Duration> {
        if Self::is_termux() {
            // Delay for Termux to handle screen changes
            Some(Duration::from_millis(50))
        } else {
            None
        }
    }

    /// Handle terminal event polling errors gracefully
    pub fn handle_poll_error() {
        if Self::is_termux() {
            // In Termux, sleep briefly to prevent tight error loops
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// Check if bracketed paste should be enabled
    pub fn should_enable_bracketed_paste() -> bool {
        !Self::is_termux()
    }
}
