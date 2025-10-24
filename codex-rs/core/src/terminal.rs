use std::sync::OnceLock;

static TERMINAL: OnceLock<String> = OnceLock::new();

pub fn user_agent() -> String {
    TERMINAL.get_or_init(detect_terminal).to_string()
}

/// Sanitize a header value to be used in a User-Agent string.
///
/// This function replaces any characters that are not allowed in a User-Agent string with an underscore.
///
/// # Arguments
///
/// * `value` - The value to sanitize.
fn is_valid_header_value_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' || c == '/'
}

fn sanitize_header_value(value: String) -> String {
    value.replace(|c| !is_valid_header_value_char(c), "_")
}

fn detect_terminal() -> String {
    sanitize_header_value(
        if let Ok(tp) = std::env::var("TERM_PROGRAM")
            && !tp.trim().is_empty()
        {
            let ver = std::env::var("TERM_PROGRAM_VERSION").ok();
            match ver {
                Some(v) if !v.trim().is_empty() => format!("{tp}/{v}"),
                _ => tp,
            }
        } else if let Ok(v) = std::env::var("WEZTERM_VERSION") {
            if !v.trim().is_empty() {
                format!("WezTerm/{v}")
            } else {
                "WezTerm".to_string()
            }
        } else if std::env::var("KITTY_WINDOW_ID").is_ok()
            || std::env::var("TERM")
                .map(|t| t.contains("kitty"))
                .unwrap_or(false)
        {
            "kitty".to_string()
        } else if std::env::var("ALACRITTY_SOCKET").is_ok()
            || std::env::var("TERM")
                .map(|t| t == "alacritty")
                .unwrap_or(false)
        {
            "Alacritty".to_string()
        } else if let Ok(v) = std::env::var("KONSOLE_VERSION") {
            if !v.trim().is_empty() {
                format!("Konsole/{v}")
            } else {
                "Konsole".to_string()
            }
        } else if std::env::var("GNOME_TERMINAL_SCREEN").is_ok() {
            return "gnome-terminal".to_string();
        } else if let Ok(v) = std::env::var("VTE_VERSION") {
            if !v.trim().is_empty() {
                format!("VTE/{v}")
            } else {
                "VTE".to_string()
            }
        } else if std::env::var("WT_SESSION").is_ok() {
            return "WindowsTerminal".to_string();
        } else {
            std::env::var("TERM").unwrap_or_else(|_| "unknown".to_string())
        },
    )
}
