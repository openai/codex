use std::sync::OnceLock;

static TERMINAL: OnceLock<String> = OnceLock::new();

/// Environment variable access used by terminal detection.
///
/// This trait exists to allow faking the environment in tests.
trait Environment {
    /// Returns an environment variable when set.
    fn var(&self, name: &str) -> Option<String>;
}

/// Reads environment variables from the running process.
struct ProcessEnvironment;

impl Environment for ProcessEnvironment {
    fn var(&self, name: &str) -> Option<String> {
        match std::env::var(name) {
            Ok(value) => Some(value),
            Err(std::env::VarError::NotPresent) => None,
            Err(std::env::VarError::NotUnicode(_)) => {
                tracing::warn!("failed to read env var {name}: value not valid UTF-8");
                None
            }
        }
    }
}

/// Returns a sanitized terminal identifier for User-Agent strings.
pub fn user_agent() -> String {
    TERMINAL.get_or_init(detect_terminal).to_string()
}

/// Sanitize a header value to be used in a User-Agent string.
///
/// This function replaces any characters that are not allowed in a User-Agent string with an underscore.
fn is_valid_header_value_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' || c == '/'
}

/// Sanitizes a terminal token for use in User-Agent headers.
///
/// Invalid header characters are replaced with underscores.
fn sanitize_header_value(value: String) -> String {
    value.replace(|c| !is_valid_header_value_char(c), "_")
}

/// Detects the current terminal from the process environment.
fn detect_terminal() -> String {
    detect_terminal_from_env(&ProcessEnvironment)
}

/// Detects a terminal identifier from an injectable environment.
///
/// The logic mirrors the existing detection order, preferring explicit
/// identifiers like `TERM_PROGRAM` before falling back to capability strings.
fn detect_terminal_from_env(env: &dyn Environment) -> String {
    sanitize_header_value(
        if let Some(tp) = env.var("TERM_PROGRAM")
            && !tp.trim().is_empty()
        {
            let ver = env.var("TERM_PROGRAM_VERSION");
            match ver {
                Some(v) if !v.trim().is_empty() => format!("{tp}/{v}"),
                _ => tp,
            }
        } else if let Some(v) = env.var("WEZTERM_VERSION") {
            if !v.trim().is_empty() {
                format!("WezTerm/{v}")
            } else {
                "WezTerm".to_string()
            }
        } else if env.var("KITTY_WINDOW_ID").is_some()
            || env
                .var("TERM")
                .map(|t| t.contains("kitty"))
                .unwrap_or(false)
        {
            "kitty".to_string()
        } else if env.var("ALACRITTY_SOCKET").is_some()
            || env.var("TERM").map(|t| t == "alacritty").unwrap_or(false)
        {
            "Alacritty".to_string()
        } else if let Some(v) = env.var("KONSOLE_VERSION") {
            if !v.trim().is_empty() {
                format!("Konsole/{v}")
            } else {
                "Konsole".to_string()
            }
        } else if env.var("GNOME_TERMINAL_SCREEN").is_some() {
            return "gnome-terminal".to_string();
        } else if let Some(v) = env.var("VTE_VERSION") {
            if !v.trim().is_empty() {
                format!("VTE/{v}")
            } else {
                "VTE".to_string()
            }
        } else if env.var("WT_SESSION").is_some() {
            return "WindowsTerminal".to_string();
        } else {
            env.var("TERM").unwrap_or_else(|| "unknown".to_string())
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use std::collections::HashMap;

    struct FakeEnvironment {
        vars: HashMap<String, String>,
    }

    impl FakeEnvironment {
        fn new() -> Self {
            Self {
                vars: HashMap::new(),
            }
        }

        fn with_var(mut self, key: &str, value: &str) -> Self {
            self.vars.insert(key.to_string(), value.to_string());
            self
        }
    }

    impl Environment for FakeEnvironment {
        fn var(&self, name: &str) -> Option<String> {
            self.vars.get(name).cloned()
        }
    }

    #[test]
    fn detects_term_program() {
        let env = FakeEnvironment::new()
            .with_var("TERM_PROGRAM", "iTerm.app")
            .with_var("TERM_PROGRAM_VERSION", "3.5.0")
            .with_var("WEZTERM_VERSION", "2024.2");
        assert_eq!(
            detect_terminal_from_env(&env),
            "iTerm.app/3.5.0",
            "term_program_with_version"
        );

        let env = FakeEnvironment::new()
            .with_var("TERM_PROGRAM", "iTerm.app")
            .with_var("TERM_PROGRAM_VERSION", "");
        assert_eq!(
            detect_terminal_from_env(&env),
            "iTerm.app",
            "term_program_without_version"
        );

        let env = FakeEnvironment::new()
            .with_var("TERM_PROGRAM", "iTerm.app")
            .with_var("WEZTERM_VERSION", "2024.2");
        assert_eq!(
            detect_terminal_from_env(&env),
            "iTerm.app",
            "term_program_overrides_wezterm"
        );
    }

    #[test]
    fn detects_wezterm() {
        let env = FakeEnvironment::new().with_var("WEZTERM_VERSION", "2024.2");
        assert_eq!(
            detect_terminal_from_env(&env),
            "WezTerm/2024.2",
            "wezterm_version"
        );

        let env = FakeEnvironment::new().with_var("WEZTERM_VERSION", "");
        assert_eq!(detect_terminal_from_env(&env), "WezTerm", "wezterm_empty");
    }

    #[test]
    fn detects_kitty() {
        let env = FakeEnvironment::new().with_var("KITTY_WINDOW_ID", "1");
        assert_eq!(detect_terminal_from_env(&env), "kitty", "kitty_window_id");

        let env = FakeEnvironment::new()
            .with_var("TERM", "xterm-kitty")
            .with_var("ALACRITTY_SOCKET", "/tmp/alacritty");
        assert_eq!(
            detect_terminal_from_env(&env),
            "kitty",
            "kitty_term_over_alacritty"
        );
    }

    #[test]
    fn detects_alacritty() {
        let env = FakeEnvironment::new().with_var("ALACRITTY_SOCKET", "/tmp/alacritty");
        assert_eq!(
            detect_terminal_from_env(&env),
            "Alacritty",
            "alacritty_socket"
        );

        let env = FakeEnvironment::new().with_var("TERM", "alacritty");
        assert_eq!(
            detect_terminal_from_env(&env),
            "Alacritty",
            "alacritty_term"
        );
    }

    #[test]
    fn detects_konsole() {
        let env = FakeEnvironment::new().with_var("KONSOLE_VERSION", "230800");
        assert_eq!(
            detect_terminal_from_env(&env),
            "Konsole/230800",
            "konsole_version"
        );

        let env = FakeEnvironment::new().with_var("KONSOLE_VERSION", "");
        assert_eq!(detect_terminal_from_env(&env), "Konsole", "konsole_empty");
    }

    #[test]
    fn detects_gnome_terminal() {
        let env = FakeEnvironment::new().with_var("GNOME_TERMINAL_SCREEN", "1");
        assert_eq!(
            detect_terminal_from_env(&env),
            "gnome-terminal",
            "gnome_terminal_screen"
        );
    }

    #[test]
    fn detects_vte() {
        let env = FakeEnvironment::new().with_var("VTE_VERSION", "7000");
        assert_eq!(detect_terminal_from_env(&env), "VTE/7000", "vte_version");

        let env = FakeEnvironment::new().with_var("VTE_VERSION", "");
        assert_eq!(detect_terminal_from_env(&env), "VTE", "vte_empty");
    }

    #[test]
    fn detects_windows_terminal() {
        let env = FakeEnvironment::new().with_var("WT_SESSION", "1");
        assert_eq!(
            detect_terminal_from_env(&env),
            "WindowsTerminal",
            "wt_session"
        );
    }

    #[test]
    fn detects_term_fallbacks() {
        let env = FakeEnvironment::new().with_var("TERM", "xterm-256color");
        assert_eq!(
            detect_terminal_from_env(&env),
            "xterm-256color",
            "term_fallback"
        );

        let env = FakeEnvironment::new();
        assert_eq!(detect_terminal_from_env(&env), "unknown", "unknown");
    }
}
