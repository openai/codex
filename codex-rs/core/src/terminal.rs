//! Terminal detection utilities.
//!
//! This module feeds terminal metadata into OpenTelemetry user-agent logging and into
//! terminal-specific configuration choices in the TUI.

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
    Zellij {},
}

/// tmux client terminal identification captured via `tmux display-message`.
///
/// `termtype` corresponds to `#{client_termtype}` and typically reflects the
/// underlying terminal program (for example, `ghostty` or `wezterm`) with an
/// optional version suffix. `termname` comes from `#{client_termname}` and
/// preserves the TERM capability string exposed by the client (for example,
/// `xterm-256color`).
///
/// This information is only available when running under tmux and lets us
/// attribute the session to the underlying terminal rather than to tmux itself.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct TmuxClientInfo {
    termtype: Option<String>,
    termname: Option<String>,
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
        }
    }

    /// Creates terminal metadata from a `TERM_PROGRAM` match.
    fn from_term_program(
        name: TerminalName,
        term_program: String,
        version: Option<String>,
        multiplexer: Option<Multiplexer>,
    ) -> Self {
        Self::new(name, Some(term_program), version, None, multiplexer)
    }

    /// Creates terminal metadata from a `TERM_PROGRAM` match plus a `TERM` value.
    fn from_term_program_and_term(
        name: TerminalName,
        term_program: String,
        version: Option<String>,
        term: Option<String>,
        multiplexer: Option<Multiplexer>,
    ) -> Self {
        Self::new(name, Some(term_program), version, term, multiplexer)
    }

    /// Creates terminal metadata from a known terminal name and optional version.
    fn from_name(
        name: TerminalName,
        version: Option<String>,
        multiplexer: Option<Multiplexer>,
    ) -> Self {
        Self::new(name, None, version, None, multiplexer)
    }

    /// Creates terminal metadata from a `TERM` capability value.
    fn from_term(term: String, multiplexer: Option<Multiplexer>) -> Self {
        let name = if term == "dumb" {
            TerminalName::Dumb
        } else {
            TerminalName::Unknown
        };
        Self::new(name, None, None, Some(term), multiplexer)
    }

    /// Creates terminal metadata for unknown terminals.
    fn unknown(multiplexer: Option<Multiplexer>) -> Self {
        Self::new(TerminalName::Unknown, None, None, None, multiplexer)
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

    /// Returns tmux client details when available.
    fn tmux_client_info(&self) -> TmuxClientInfo;
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

    fn tmux_client_info(&self) -> TmuxClientInfo {
        tmux_client_info()
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
/// - If `TERM_PROGRAM=tmux`, the tmux client term type/name are used instead. The client term
///   type is split on whitespace to extract a program name plus optional version (for example,
///   `ghostty 1.2.3`), while the client term name becomes the `TERM` capability string.
/// - Otherwise, `TERM_PROGRAM` (plus `TERM_PROGRAM_VERSION`) drives the detected terminal name.
///   This means `TERM_PROGRAM` can mask later probes (for example `WT_SESSION`).
/// - Next, terminal-specific variables (WEZTERM, iTerm2, Apple Terminal, kitty, etc.) are checked.
/// - Finally, `TERM` is used as the capability fallback with `TerminalName::Unknown`.
///
/// tmux client term info is only consulted when a tmux multiplexer is detected, and it is
/// derived from `tmux display-message` to surface the underlying terminal program instead of
/// reporting tmux itself.
fn detect_terminal_info_from_env(env: &dyn Environment) -> TerminalInfo {
    let multiplexer = detect_multiplexer(env);

    if let Some(term_program) = env.var_non_empty("TERM_PROGRAM") {
        if is_tmux_term_program(&term_program)
            && matches!(multiplexer, Some(Multiplexer::Tmux { .. }))
            && let Some(terminal) =
                terminal_from_tmux_client_info(env.tmux_client_info(), multiplexer.clone())
        {
            return terminal;
        }

        let version = env.var_non_empty("TERM_PROGRAM_VERSION");
        let name = terminal_name_from_term_program(&term_program).unwrap_or(TerminalName::Unknown);
        return TerminalInfo::from_term_program(name, term_program, version, multiplexer);
    }

    if env.has("WEZTERM_VERSION") {
        let version = env.var_non_empty("WEZTERM_VERSION");
        return TerminalInfo::from_name(TerminalName::WezTerm, version, multiplexer);
    }

    if env.has("ITERM_SESSION_ID") || env.has("ITERM_PROFILE") || env.has("ITERM_PROFILE_NAME") {
        return TerminalInfo::from_name(TerminalName::Iterm2, None, multiplexer);
    }

    if env.has("TERM_SESSION_ID") {
        return TerminalInfo::from_name(TerminalName::AppleTerminal, None, multiplexer);
    }

    if env.has("KITTY_WINDOW_ID")
        || env
            .var("TERM")
            .map(|term| term.contains("kitty"))
            .unwrap_or(false)
    {
        return TerminalInfo::from_name(TerminalName::Kitty, None, multiplexer);
    }

    if env.has("ALACRITTY_SOCKET")
        || env
            .var("TERM")
            .map(|term| term == "alacritty")
            .unwrap_or(false)
    {
        return TerminalInfo::from_name(TerminalName::Alacritty, None, multiplexer);
    }

    if env.has("KONSOLE_VERSION") {
        let version = env.var_non_empty("KONSOLE_VERSION");
        return TerminalInfo::from_name(TerminalName::Konsole, version, multiplexer);
    }

    if env.has("GNOME_TERMINAL_SCREEN") {
        return TerminalInfo::from_name(TerminalName::GnomeTerminal, None, multiplexer);
    }

    if env.has("VTE_VERSION") {
        let version = env.var_non_empty("VTE_VERSION");
        return TerminalInfo::from_name(TerminalName::Vte, version, multiplexer);
    }

    if env.has("WT_SESSION") {
        return TerminalInfo::from_name(TerminalName::WindowsTerminal, None, multiplexer);
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
        return Some(Multiplexer::Zellij {});
    }

    None
}

fn is_tmux_term_program(value: &str) -> bool {
    value.eq_ignore_ascii_case("tmux")
}

fn terminal_from_tmux_client_info(
    client_info: TmuxClientInfo,
    multiplexer: Option<Multiplexer>,
) -> Option<TerminalInfo> {
    let termtype = client_info.termtype.and_then(none_if_whitespace);
    let termname = client_info.termname.and_then(none_if_whitespace);

    if let Some(termtype) = termtype.as_ref() {
        let (program, version) = split_term_program_and_version(termtype);
        let name = terminal_name_from_term_program(&program).unwrap_or(TerminalName::Unknown);
        return Some(TerminalInfo::from_term_program_and_term(
            name,
            program,
            version,
            termname,
            multiplexer,
        ));
    }

    termname
        .as_ref()
        .map(|termname| TerminalInfo::from_term(termname.to_string(), multiplexer))
}

fn tmux_version_from_env(env: &dyn Environment) -> Option<String> {
    let term_program = env.var("TERM_PROGRAM")?;
    if !is_tmux_term_program(&term_program) {
        return None;
    }

    env.var_non_empty("TERM_PROGRAM_VERSION")
}

fn split_term_program_and_version(value: &str) -> (String, Option<String>) {
    let mut parts = value.split_whitespace();
    let program = parts.next().unwrap_or_default().to_string();
    let version = parts.next().map(ToString::to_string);
    (program, version)
}

fn tmux_client_info() -> TmuxClientInfo {
    let termtype = tmux_display_message("#{client_termtype}");
    let termname = tmux_display_message("#{client_termname}");

    TmuxClientInfo { termtype, termname }
}

fn tmux_display_message(format: &str) -> Option<String> {
    let output = std::process::Command::new("tmux")
        .args(["display-message", "-p", format])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let value = String::from_utf8(output.stdout).ok()?;
    none_if_whitespace(value.trim().to_string())
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
