//! Terminal detection utilities.
//!
//! This module feeds terminal metadata into OpenTelemetry user-agent logging and into
//! terminal-specific configuration choices in the TUI. Detection only reads the
//! environment; it must not execute helpers selected by an untrusted PATH.

use std::sync::OnceLock;

/// Structured terminal identification data.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalInfo {
    /// The detected terminal name category.
    pub name: TerminalName,
    /// The `TERM_PROGRAM` value when provided by the terminal.
    pub term_program: Option<String>,
    /// The terminal version string when available.
    pub version: Option<String>,
    /// The `TERM` value when falling back to capability strings.
    pub term: Option<String>,
    /// Multiplexer metadata when a terminal multiplexer is active.
    pub multiplexer: Option<Multiplexer>,
    /// Backend metadata observed independently of the terminal application identity.
    pub backend: Option<TerminalBackend>,
}

/// Known terminal rendering backends derived from environment metadata.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalBackend {
    /// VTE, used by GNOME Terminal and other terminal applications.
    Vte {
        /// Encoded as major * 10_000 + minor * 100 + micro; unknown if malformed.
        version: Option<u32>,
    },
}

/// Known terminal name categories derived from environment variables.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalName {
    /// Apple Terminal (Terminal.app).
    AppleTerminal,
    /// Ghostty terminal emulator.
    Ghostty,
    /// iTerm2 terminal emulator.
    Iterm2,
    /// Warp terminal emulator.
    WarpTerminal,
    /// Visual Studio Code integrated terminal.
    VsCode,
    /// WezTerm terminal emulator.
    WezTerm,
    /// kitty terminal emulator.
    Kitty,
    /// Alacritty terminal emulator.
    Alacritty,
    /// KDE Konsole terminal emulator.
    Konsole,
    /// GNOME Terminal emulator.
    GnomeTerminal,
    /// VTE backend terminal.
    Vte,
    /// Windows Terminal emulator.
    WindowsTerminal,
    /// Dumb terminal (TERM=dumb).
    Dumb,
    /// Unknown or missing terminal identification.
    Unknown,
}

/// Detected terminal multiplexer metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Multiplexer {
    /// tmux terminal multiplexer.
    Tmux {
        /// tmux version string when `TERM_PROGRAM=tmux` is available.
        ///
        /// This is derived from `TERM_PROGRAM_VERSION`.
        version: Option<String>,
    },
    /// zellij terminal multiplexer.
    Zellij {
        /// Zellij version string when ZELLIJ_VERSION is available.
        version: Option<String>,
    },
}

impl TerminalInfo {
    /// Creates terminal metadata from detected fields.
    fn new(
        name: TerminalName,
        term_program: Option<String>,
        version: Option<String>,
        term: Option<String>,
        multiplexer: Option<Multiplexer>,
    ) -> Self {
        Self {
            name,
            term_program,
            version,
            term,
            multiplexer,
            backend: None,
        }
    }

    /// Creates terminal metadata from a `TERM_PROGRAM` match.
    fn from_term_program(
        name: TerminalName,
        term_program: String,
        version: Option<String>,
        multiplexer: Option<Multiplexer>,
    ) -> Self {
        Self::new(
            name,
            Some(term_program),
            version,
            /*term*/ None,
            multiplexer,
        )
    }

    /// Creates terminal metadata from a known terminal name and optional version.
    fn from_name(
        name: TerminalName,
        version: Option<String>,
        multiplexer: Option<Multiplexer>,
    ) -> Self {
        Self::new(
            name,
            /*term_program*/ None,
            version,
            /*term*/ None,
            multiplexer,
        )
    }

    /// Creates terminal metadata from a `TERM` capability value.
    fn from_term(term: String, multiplexer: Option<Multiplexer>) -> Self {
        let name = match term.as_str() {
            "dumb" => TerminalName::Dumb,
            "xterm-ghostty" => TerminalName::Ghostty,
            "wezterm" | "wezterm-mux" => TerminalName::WezTerm,
            _ => TerminalName::Unknown,
        };
        Self::new(
            name,
            /*term_program*/ None,
            /*version*/ None,
            Some(term),
            multiplexer,
        )
    }

    /// Creates terminal metadata for unknown terminals.
    fn unknown(multiplexer: Option<Multiplexer>) -> Self {
        Self::new(
            TerminalName::Unknown,
            /*term_program*/ None,
            /*version*/ None,
            /*term*/ None,
            multiplexer,
        )
    }

    /// Formats the terminal info as a User-Agent token.
    fn user_agent_token(&self) -> String {
        let raw = if let Some(program) = self.term_program.as_ref() {
            match self.version.as_ref().filter(|v| !v.is_empty()) {
                Some(version) => format!("{program}/{version}"),
                None => program.clone(),
            }
        } else if let Some(term) = self.term.as_ref().filter(|value| !value.is_empty()) {
            term.clone()
        } else {
            match self.name {
                TerminalName::AppleTerminal => {
                    format_terminal_version("Apple_Terminal", &self.version)
                }
                TerminalName::Ghostty => format_terminal_version("Ghostty", &self.version),
                TerminalName::Iterm2 => format_terminal_version("iTerm.app", &self.version),
                TerminalName::WarpTerminal => {
                    format_terminal_version("WarpTerminal", &self.version)
                }
                TerminalName::VsCode => format_terminal_version("vscode", &self.version),
                TerminalName::WezTerm => format_terminal_version("WezTerm", &self.version),
                TerminalName::Kitty => "kitty".to_string(),
                TerminalName::Alacritty => "Alacritty".to_string(),
                TerminalName::Konsole => format_terminal_version("Konsole", &self.version),
                TerminalName::GnomeTerminal => "gnome-terminal".to_string(),
                TerminalName::Vte => format_terminal_version("VTE", &self.version),
                TerminalName::WindowsTerminal => "WindowsTerminal".to_string(),
                TerminalName::Dumb => "dumb".to_string(),
                TerminalName::Unknown => "unknown".to_string(),
            }
        };

        sanitize_header_value(raw)
    }

    /// Returns whether the active terminal multiplexer is Zellij.
    pub fn is_zellij(&self) -> bool {
        matches!(self.multiplexer, Some(Multiplexer::Zellij { .. }))
    }
}

static TERMINAL_INFO: OnceLock<TerminalInfo> = OnceLock::new();

/// Environment variable access used by terminal detection.
///
/// This trait exists to allow faking the environment in tests.
trait Environment {
    /// Returns an environment variable when set.
    fn var(&self, name: &str) -> Option<String>;

    /// Returns whether an environment variable is set.
    fn has(&self, name: &str) -> bool {
        self.var(name).is_some()
    }

    /// Returns a non-empty environment variable.
    fn var_non_empty(&self, name: &str) -> Option<String> {
        self.var(name).and_then(none_if_whitespace)
    }

    /// Returns whether an environment variable is set and non-empty.
    fn has_non_empty(&self, name: &str) -> bool {
        self.var_non_empty(name).is_some()
    }
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
    terminal_info().user_agent_token()
}

/// Returns structured terminal metadata for the current process.
pub fn terminal_info() -> TerminalInfo {
    TERMINAL_INFO
        .get_or_init(|| detect_terminal_info_from_env(&ProcessEnvironment))
        .clone()
}

/// Detects structured terminal metadata from an injectable environment.
///
/// Detection order favors explicit identifiers before falling back to capability strings:
/// - `TERM_PROGRAM` (plus `TERM_PROGRAM_VERSION`) drives the detected terminal name,
///   except when it identifies tmux rather than the underlying terminal.
///   This means `TERM_PROGRAM` can mask later probes (for example `WT_SESSION`).
/// - Next, terminal-specific variables (WEZTERM, iTerm2, kitty, etc.) are checked.
///   `TERM_SESSION_ID` is not terminal-specific and cannot identify Apple Terminal.
/// - Finally, `TERM` is used as the capability fallback.
fn detect_terminal_info_from_env(env: &dyn Environment) -> TerminalInfo {
    let mut terminal = detect_terminal_identity_from_env(env);
    // Identity probes can return before VTE is considered. Keep its backend
    // metadata even when GNOME or another TERM_PROGRAM identifies the application.
    let vte_version = env.var("VTE_VERSION");
    if vte_version.is_some() || terminal.name == TerminalName::Vte {
        terminal.backend = Some(TerminalBackend::Vte {
            // Choose the raw value before parsing so malformed values retain
            // precedence over the terminal-version fallback.
            version: vte_version
                .as_deref()
                .or(terminal.version.as_deref())
                .and_then(|version| version.parse().ok()),
        });
    }
    terminal
}

fn detect_terminal_identity_from_env(env: &dyn Environment) -> TerminalInfo {
    let multiplexer = detect_multiplexer(env);

    if let Some(term_program) = env.var_non_empty("TERM_PROGRAM")
        && !is_tmux_term_program(&term_program)
    {
        let version = env.var_non_empty("TERM_PROGRAM_VERSION");
        let name = terminal_name_from_term_program(&term_program).unwrap_or(TerminalName::Unknown);
        return TerminalInfo::from_term_program(name, term_program, version, multiplexer);
    }

    if env.has_non_empty("GHOSTTY_RESOURCES_DIR") {
        return TerminalInfo::from_name(TerminalName::Ghostty, /*version*/ None, multiplexer);
    }

    if env.has("WEZTERM_VERSION") {
        let version = env.var_non_empty("WEZTERM_VERSION");
        return TerminalInfo::from_name(TerminalName::WezTerm, version, multiplexer);
    }

    if env.has("ITERM_SESSION_ID") || env.has("ITERM_PROFILE") || env.has("ITERM_PROFILE_NAME") {
        return TerminalInfo::from_name(TerminalName::Iterm2, /*version*/ None, multiplexer);
    }

    if env.has("KITTY_WINDOW_ID")
        || env
            .var("TERM")
            .map(|term| term.contains("kitty"))
            .unwrap_or(false)
    {
        return TerminalInfo::from_name(TerminalName::Kitty, /*version*/ None, multiplexer);
    }

    if env.has("ALACRITTY_SOCKET")
        || env
            .var("TERM")
            .map(|term| term == "alacritty")
            .unwrap_or(false)
    {
        return TerminalInfo::from_name(
            TerminalName::Alacritty,
            /*version*/ None,
            multiplexer,
        );
    }

    if env.has("KONSOLE_VERSION") {
        let version = env.var_non_empty("KONSOLE_VERSION");
        return TerminalInfo::from_name(TerminalName::Konsole, version, multiplexer);
    }

    if env.has("GNOME_TERMINAL_SCREEN") {
        return TerminalInfo::from_name(
            TerminalName::GnomeTerminal,
            /*version*/ None,
            multiplexer,
        );
    }

    if env.has("VTE_VERSION") {
        let version = env.var_non_empty("VTE_VERSION");
        return TerminalInfo::from_name(TerminalName::Vte, version, multiplexer);
    }

    if env.has("WT_SESSION") {
        return TerminalInfo::from_name(
            TerminalName::WindowsTerminal,
            /*version*/ None,
            multiplexer,
        );
    }

    if let Some(term) = env.var_non_empty("TERM") {
        return TerminalInfo::from_term(term, multiplexer);
    }

    TerminalInfo::unknown(multiplexer)
}

fn detect_multiplexer(env: &dyn Environment) -> Option<Multiplexer> {
    if env.has_non_empty("TMUX") || env.has_non_empty("TMUX_PANE") {
        return Some(Multiplexer::Tmux {
            version: tmux_version_from_env(env),
        });
    }

    if env.has_non_empty("ZELLIJ")
        || env.has_non_empty("ZELLIJ_SESSION_NAME")
        || env.has_non_empty("ZELLIJ_VERSION")
    {
        return Some(Multiplexer::Zellij {
            version: env.var_non_empty("ZELLIJ_VERSION"),
        });
    }

    None
}

fn is_tmux_term_program(value: &str) -> bool {
    value.eq_ignore_ascii_case("tmux")
}

fn tmux_version_from_env(env: &dyn Environment) -> Option<String> {
    let term_program = env.var("TERM_PROGRAM")?;
    if !is_tmux_term_program(&term_program) {
        return None;
    }

    env.var_non_empty("TERM_PROGRAM_VERSION")
}

/// Sanitizes a terminal token for use in User-Agent headers.
///
/// Invalid header characters are replaced with underscores.
fn sanitize_header_value(value: String) -> String {
    value.replace(|c| !is_valid_header_value_char(c), "_")
}

/// Returns whether a character is allowed in User-Agent header values.
fn is_valid_header_value_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' || c == '/'
}

fn terminal_name_from_term_program(value: &str) -> Option<TerminalName> {
    let normalized: String = value
        .trim()
        .chars()
        .filter(|c| !matches!(c, ' ' | '-' | '_' | '.'))
        .map(|c| c.to_ascii_lowercase())
        .collect();

    match normalized.as_str() {
        "appleterminal" => Some(TerminalName::AppleTerminal),
        "ghostty" => Some(TerminalName::Ghostty),
        "iterm" | "iterm2" | "itermapp" => Some(TerminalName::Iterm2),
        "warp" | "warpterminal" => Some(TerminalName::WarpTerminal),
        "vscode" => Some(TerminalName::VsCode),
        "wezterm" => Some(TerminalName::WezTerm),
        "kitty" => Some(TerminalName::Kitty),
        "alacritty" => Some(TerminalName::Alacritty),
        "konsole" => Some(TerminalName::Konsole),
        "gnometerminal" => Some(TerminalName::GnomeTerminal),
        "vte" => Some(TerminalName::Vte),
        "windowsterminal" => Some(TerminalName::WindowsTerminal),
        "dumb" => Some(TerminalName::Dumb),
        _ => None,
    }
}

fn format_terminal_version(name: &str, version: &Option<String>) -> String {
    match version.as_ref().filter(|value| !value.is_empty()) {
        Some(version) => format!("{name}/{version}"),
        None => name.to_string(),
    }
}

fn none_if_whitespace(value: String) -> Option<String> {
    (!value.trim().is_empty()).then_some(value)
}

#[cfg(test)]
#[path = "terminal_tests.rs"]
mod tests;
