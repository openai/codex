use super::*;
use pretty_assertions::assert_eq;
use std::collections::HashMap;

struct FakeEnvironment {
    vars: HashMap<String, String>,
    tmux_client_info: TmuxClientInfo,
}

impl FakeEnvironment {
    fn new() -> Self {
        Self {
            vars: HashMap::new(),
            tmux_client_info: TmuxClientInfo::default(),
        }
    }

    fn with_var(mut self, key: &str, value: &str) -> Self {
        self.vars.insert(key.to_string(), value.to_string());
        self
    }

    fn with_tmux_client_info(mut self, termtype: Option<&str>, termname: Option<&str>) -> Self {
        self.tmux_client_info = TmuxClientInfo {
            termtype: termtype.map(ToString::to_string),
            termname: termname.map(ToString::to_string),
        };
        self
    }
}

impl Environment for FakeEnvironment {
    fn var(&self, name: &str) -> Option<String> {
        self.vars.get(name).cloned()
    }

    fn tmux_client_info(&self) -> TmuxClientInfo {
        self.tmux_client_info.clone()
    }
}

fn terminal_info(
    name: TerminalName,
    term_program: Option<&str>,
    version: Option<&str>,
    term: Option<&str>,
    multiplexer: Option<Multiplexer>,
) -> TerminalInfo {
    TerminalInfo {
        name,
        term_program: term_program.map(ToString::to_string),
        version: version.map(ToString::to_string),
        term: term.map(ToString::to_string),
        multiplexer,
    }
}

#[test]
fn detects_term_program() {
    let env = FakeEnvironment::new()
        .with_var("TERM_PROGRAM", "iTerm.app")
        .with_var("TERM_PROGRAM_VERSION", "3.5.0")
        .with_var("WEZTERM_VERSION", "2024.2");
    let terminal = detect_terminal_info_from_env(&env);
    assert_eq!(
        terminal,
        terminal_info(
            TerminalName::Iterm2,
            Some("iTerm.app"),
            Some("3.5.0"),
            /*term*/ None,
            /*multiplexer*/ None,
        ),
        "term_program_with_version_info"
    );
    assert_eq!(
        terminal.user_agent_token(),
        "iTerm.app/3.5.0",
        "term_program_with_version_user_agent"
    );

    let env = FakeEnvironment::new()
        .with_var("TERM_PROGRAM", "iTerm.app")
        .with_var("TERM_PROGRAM_VERSION", "");
    let terminal = detect_terminal_info_from_env(&env);
    assert_eq!(
        terminal,
        terminal_info(
            TerminalName::Iterm2,
            Some("iTerm.app"),
            /*version*/ None,
            /*term*/ None,
            /*multiplexer*/ None
        ),
        "term_program_without_version_info"
    );
    assert_eq!(
        terminal.user_agent_token(),
        "iTerm.app",
        "term_program_without_version_user_agent"
    );

    let env = FakeEnvironment::new()
        .with_var("TERM_PROGRAM", "iTerm.app")
        .with_var("WEZTERM_VERSION", "2024.2");
    let terminal = detect_terminal_info_from_env(&env);
    assert_eq!(
        terminal,
        terminal_info(
            TerminalName::Iterm2,
            Some("iTerm.app"),
            /*version*/ None,
            /*term*/ None,
            /*multiplexer*/ None
        ),
        "term_program_overrides_wezterm_info"
    );
    assert_eq!(
        terminal.user_agent_token(),
        "iTerm.app",
        "term_program_overrides_wezterm_user_agent"
    );
}

#[test]
fn detects_iterm2() {
    let env = FakeEnvironment::new().with_var("ITERM_SESSION_ID", "w0t1p0");
    let terminal = detect_terminal_info_from_env(&env);
    assert_eq!(
        terminal,
        terminal_info(
            TerminalName::Iterm2,
            /*term_program*/ None,
            /*version*/ None,
            /*term*/ None,
            /*multiplexer*/ None
        ),
        "iterm_session_id_info"
    );
    assert_eq!(
        terminal.user_agent_token(),
        "iTerm.app",
        "iterm_session_id_user_agent"
    );
}

#[test]
fn detects_apple_terminal() {
    let env = FakeEnvironment::new().with_var("TERM_PROGRAM", "Apple_Terminal");
    let terminal = detect_terminal_info_from_env(&env);
    assert_eq!(
        terminal,
        terminal_info(
            TerminalName::AppleTerminal,
            Some("Apple_Terminal"),
            /*version*/ None,
            /*term*/ None,
            /*multiplexer*/ None,
        ),
        "apple_term_program_info"
    );
    assert_eq!(
        terminal.user_agent_token(),
        "Apple_Terminal",
        "apple_term_program_user_agent"
    );

    let env = FakeEnvironment::new().with_var("TERM_SESSION_ID", "A1B2C3");
    let terminal = detect_terminal_info_from_env(&env);
    assert_eq!(
        terminal,
        terminal_info(
            TerminalName::AppleTerminal,
            /*term_program*/ None,
            /*version*/ None,
            /*term*/ None,
            /*multiplexer*/ None
        ),
        "apple_term_session_id_info"
    );
    assert_eq!(
        terminal.user_agent_token(),
        "Apple_Terminal",
        "apple_term_session_id_user_agent"
    );
}

#[test]
fn detects_ghostty() {
    let env = FakeEnvironment::new().with_var("TERM_PROGRAM", "Ghostty");
    let terminal = detect_terminal_info_from_env(&env);
    assert_eq!(
        terminal,
        terminal_info(
            TerminalName::Ghostty,
            Some("Ghostty"),
            /*version*/ None,
            /*term*/ None,
            /*multiplexer*/ None
        ),
        "ghostty_term_program_info"
    );
    assert_eq!(
        terminal.user_agent_token(),
        "Ghostty",
        "ghostty_term_program_user_agent"
    );
}

#[test]
fn detects_vscode() {
    let env = FakeEnvironment::new()
        .with_var("TERM_PROGRAM", "vscode")
        .with_var("TERM_PROGRAM_VERSION", "1.86.0");
    let terminal = detect_terminal_info_from_env(&env);
    assert_eq!(
        terminal,
        terminal_info(
            TerminalName::VsCode,
            Some("vscode"),
            Some("1.86.0"),
            /*term*/ None,
            /*multiplexer*/ None
        ),
        "vscode_term_program_info"
    );
    assert_eq!(
        terminal.user_agent_token(),
        "vscode/1.86.0",
        "vscode_term_program_user_agent"
    );
}

#[test]
fn detects_warp_terminal() {
    let env = FakeEnvironment::new()
        .with_var("TERM_PROGRAM", "WarpTerminal")
        .with_var("TERM_PROGRAM_VERSION", "v0.2025.12.10.08.12.stable_03");
    let terminal = detect_terminal_info_from_env(&env);
    assert_eq!(
        terminal,
        terminal_info(
            TerminalName::WarpTerminal,
            Some("WarpTerminal"),
            Some("v0.2025.12.10.08.12.stable_03"),
            /*term*/ None,
            /*multiplexer*/ None,
        ),
        "warp_term_program_info"
    );
    assert_eq!(
        terminal.user_agent_token(),
        "WarpTerminal/v0.2025.12.10.08.12.stable_03",
        "warp_term_program_user_agent"
    );
}

#[test]
fn detects_tmux_multiplexer() {
    let env = FakeEnvironment::new()
        .with_var("TMUX", "/tmp/tmux-1000/default,123,0")
        .with_var("TERM_PROGRAM", "tmux")
        .with_tmux_client_info(Some("xterm-256color"), Some("screen-256color"));
    let terminal = detect_terminal_info_from_env(&env);
    assert_eq!(
        terminal,
        terminal_info(
            TerminalName::Unknown,
            Some("xterm-256color"),
            /*version*/ None,
            Some("screen-256color"),
            Some(Multiplexer::Tmux { version: None }),
        ),
        "tmux_multiplexer_info"
    );
    assert_eq!(
        terminal.user_agent_token(),
        "xterm-256color",
        "tmux_multiplexer_user_agent"
    );
}

#[test]
fn detects_zellij_multiplexer() {
    let env = FakeEnvironment::new().with_var("ZELLIJ", "1");
    let terminal = detect_terminal_info_from_env(&env);
    assert_eq!(
        terminal,
        TerminalInfo {
            name: TerminalName::Unknown,
            term_program: None,
            version: None,
            term: None,
            multiplexer: Some(Multiplexer::Zellij {}),
        },
        "zellij_multiplexer"
    );
}

#[test]
fn detects_tmux_client_termtype() {
    let env = FakeEnvironment::new()
        .with_var("TMUX", "/tmp/tmux-1000/default,123,0")
        .with_var("TERM_PROGRAM", "tmux")
        .with_tmux_client_info(Some("WezTerm"), /*termname*/ None);
    let terminal = detect_terminal_info_from_env(&env);
    assert_eq!(
        terminal,
        terminal_info(
            TerminalName::WezTerm,
            Some("WezTerm"),
            /*version*/ None,
            /*term*/ None,
            Some(Multiplexer::Tmux { version: None }),
        ),
        "tmux_client_termtype_info"
    );
    assert_eq!(
        terminal.user_agent_token(),
        "WezTerm",
        "tmux_client_termtype_user_agent"
    );
}

#[test]
fn detects_tmux_client_termname() {
    let env = FakeEnvironment::new()
        .with_var("TMUX", "/tmp/tmux-1000/default,123,0")
        .with_var("TERM_PROGRAM", "tmux")
        .with_tmux_client_info(/*termtype*/ None, Some("xterm-256color"));
    let terminal = detect_terminal_info_from_env(&env);
    assert_eq!(
        terminal,
        terminal_info(
            TerminalName::Unknown,
            /*term_program*/ None,
            /*version*/ None,
            Some("xterm-256color"),
            Some(Multiplexer::Tmux { version: None })
        ),
        "tmux_client_termname_info"
    );
    assert_eq!(
        terminal.user_agent_token(),
        "xterm-256color",
        "tmux_client_termname_user_agent"
    );
}

#[test]
fn detects_tmux_term_program_uses_client_termtype() {
    let env = FakeEnvironment::new()
        .with_var("TMUX", "/tmp/tmux-1000/default,123,0")
        .with_var("TERM_PROGRAM", "tmux")
        .with_var("TERM_PROGRAM_VERSION", "3.6a")
        .with_tmux_client_info(Some("ghostty 1.2.3"), Some("xterm-ghostty"));
    let terminal = detect_terminal_info_from_env(&env);
    assert_eq!(
        terminal,
        terminal_info(
            TerminalName::Ghostty,
            Some("ghostty"),
            Some("1.2.3"),
            Some("xterm-ghostty"),
            Some(Multiplexer::Tmux {
                version: Some("3.6a".to_string()),
            }),
        ),
        "tmux_term_program_client_termtype_info"
    );
    assert_eq!(
        terminal.user_agent_token(),
        "ghostty/1.2.3",
        "tmux_term_program_client_termtype_user_agent"
    );
}

#[test]
fn detects_wezterm() {
    let env = FakeEnvironment::new().with_var("WEZTERM_VERSION", "2024.2");
    let terminal = detect_terminal_info_from_env(&env);
    assert_eq!(
        terminal,
        terminal_info(
            TerminalName::WezTerm,
            /*term_program*/ None,
            Some("2024.2"),
            /*term*/ None,
            /*multiplexer*/ None
        ),
        "wezterm_version_info"
    );
    assert_eq!(
        terminal.user_agent_token(),
        "WezTerm/2024.2",
        "wezterm_version_user_agent"
    );

    let env = FakeEnvironment::new()
        .with_var("TERM_PROGRAM", "WezTerm")
        .with_var("TERM_PROGRAM_VERSION", "2024.2");
    let terminal = detect_terminal_info_from_env(&env);
    assert_eq!(
        terminal,
        terminal_info(
            TerminalName::WezTerm,
            Some("WezTerm"),
            Some("2024.2"),
            /*term*/ None,
            /*multiplexer*/ None
        ),
        "wezterm_term_program_info"
    );
    assert_eq!(
        terminal.user_agent_token(),
        "WezTerm/2024.2",
        "wezterm_term_program_user_agent"
    );

    let env = FakeEnvironment::new().with_var("WEZTERM_VERSION", "");
    let terminal = detect_terminal_info_from_env(&env);
    assert_eq!(
        terminal,
        terminal_info(
            TerminalName::WezTerm,
            /*term_program*/ None,
            /*version*/ None,
            /*term*/ None,
            /*multiplexer*/ None
        ),
        "wezterm_empty_info"
    );
    assert_eq!(
        terminal.user_agent_token(),
        "WezTerm",
        "wezterm_empty_user_agent"
    );
}

#[test]
fn detects_kitty() {
    let env = FakeEnvironment::new().with_var("KITTY_WINDOW_ID", "1");
    let terminal = detect_terminal_info_from_env(&env);
    assert_eq!(
        terminal,
        terminal_info(
            TerminalName::Kitty,
            /*term_program*/ None,
            /*version*/ None,
            /*term*/ None,
            /*multiplexer*/ None
        ),
        "kitty_window_id_info"
    );
    assert_eq!(
        terminal.user_agent_token(),
        "kitty",
        "kitty_window_id_user_agent"
    );

    let env = FakeEnvironment::new()
        .with_var("TERM_PROGRAM", "kitty")
        .with_var("TERM_PROGRAM_VERSION", "0.30.1");
    let terminal = detect_terminal_info_from_env(&env);
    assert_eq!(
        terminal,
        terminal_info(
            TerminalName::Kitty,
            Some("kitty"),
            Some("0.30.1"),
            /*term*/ None,
            /*multiplexer*/ None
        ),
        "kitty_term_program_info"
    );
    assert_eq!(
        terminal.user_agent_token(),
        "kitty/0.30.1",
        "kitty_term_program_user_agent"
    );

    let env = FakeEnvironment::new()
        .with_var("TERM", "xterm-kitty")
        .with_var("ALACRITTY_SOCKET", "/tmp/alacritty");
    let terminal = detect_terminal_info_from_env(&env);
    assert_eq!(
        terminal,
        terminal_info(
            TerminalName::Kitty,
            /*term_program*/ None,
            /*version*/ None,
            /*term*/ None,
            /*multiplexer*/ None
        ),
        "kitty_term_over_alacritty_info"
    );
    assert_eq!(
        terminal.user_agent_token(),
        "kitty",
        "kitty_term_over_alacritty_user_agent"
    );
}

#[test]
fn detects_alacritty() {
    let env = FakeEnvironment::new().with_var("ALACRITTY_SOCKET", "/tmp/alacritty");
    let terminal = detect_terminal_info_from_env(&env);
    assert_eq!(
        terminal,
        terminal_info(
            TerminalName::Alacritty,
            /*term_program*/ None,
            /*version*/ None,
            /*term*/ None,
            /*multiplexer*/ None
        ),
        "alacritty_socket_info"
    );
    assert_eq!(
        terminal.user_agent_token(),
        "Alacritty",
        "alacritty_socket_user_agent"
    );

    let env = FakeEnvironment::new()
        .with_var("TERM_PROGRAM", "Alacritty")
        .with_var("TERM_PROGRAM_VERSION", "0.13.2");
    let terminal = detect_terminal_info_from_env(&env);
    assert_eq!(
        terminal,
        terminal_info(
            TerminalName::Alacritty,
            Some("Alacritty"),
            Some("0.13.2"),
            /*term*/ None,
            /*multiplexer*/ None,
        ),
        "alacritty_term_program_info"
    );
    assert_eq!(
        terminal.user_agent_token(),
        "Alacritty/0.13.2",
        "alacritty_term_program_user_agent"
    );

    let env = FakeEnvironment::new().with_var("TERM", "alacritty");
    let terminal = detect_terminal_info_from_env(&env);
    assert_eq!(
        terminal,
        terminal_info(
            TerminalName::Alacritty,
            /*term_program*/ None,
            /*version*/ None,
            /*term*/ None,
            /*multiplexer*/ None
        ),
        "alacritty_term_info"
    );
    assert_eq!(
        terminal.user_agent_token(),
        "Alacritty",
        "alacritty_term_user_agent"
    );
}

#[test]
fn detects_konsole() {
    let env = FakeEnvironment::new().with_var("KONSOLE_VERSION", "230800");
    let terminal = detect_terminal_info_from_env(&env);
    assert_eq!(
        terminal,
        terminal_info(
            TerminalName::Konsole,
            /*term_program*/ None,
            Some("230800"),
            /*term*/ None,
            /*multiplexer*/ None
        ),
        "konsole_version_info"
    );
    assert_eq!(
        terminal.user_agent_token(),
        "Konsole/230800",
        "konsole_version_user_agent"
    );

    let env = FakeEnvironment::new()
        .with_var("TERM_PROGRAM", "Konsole")
        .with_var("TERM_PROGRAM_VERSION", "230800");
    let terminal = detect_terminal_info_from_env(&env);
    assert_eq!(
        terminal,
        terminal_info(
            TerminalName::Konsole,
            Some("Konsole"),
            Some("230800"),
            /*term*/ None,
            /*multiplexer*/ None
        ),
        "konsole_term_program_info"
    );
    assert_eq!(
        terminal.user_agent_token(),
        "Konsole/230800",
        "konsole_term_program_user_agent"
    );

    let env = FakeEnvironment::new().with_var("KONSOLE_VERSION", "");
    let terminal = detect_terminal_info_from_env(&env);
    assert_eq!(
        terminal,
        terminal_info(
            TerminalName::Konsole,
            /*term_program*/ None,
            /*version*/ None,
            /*term*/ None,
            /*multiplexer*/ None
        ),
        "konsole_empty_info"
    );
    assert_eq!(
        terminal.user_agent_token(),
        "Konsole",
        "konsole_empty_user_agent"
    );
}

#[test]
fn detects_gnome_terminal() {
    let env = FakeEnvironment::new().with_var("GNOME_TERMINAL_SCREEN", "1");
    let terminal = detect_terminal_info_from_env(&env);
    assert_eq!(
        terminal,
        terminal_info(
            TerminalName::GnomeTerminal,
            /*term_program*/ None,
            /*version*/ None,
            /*term*/ None,
            /*multiplexer*/ None
        ),
        "gnome_terminal_screen_info"
    );
    assert_eq!(
        terminal.user_agent_token(),
        "gnome-terminal",
        "gnome_terminal_screen_user_agent"
    );

    let env = FakeEnvironment::new()
        .with_var("TERM_PROGRAM", "gnome-terminal")
        .with_var("TERM_PROGRAM_VERSION", "3.50");
    let terminal = detect_terminal_info_from_env(&env);
    assert_eq!(
        terminal,
        terminal_info(
            TerminalName::GnomeTerminal,
            Some("gnome-terminal"),
            Some("3.50"),
            /*term*/ None,
            /*multiplexer*/ None,
        ),
        "gnome_terminal_term_program_info"
    );
    assert_eq!(
        terminal.user_agent_token(),
        "gnome-terminal/3.50",
        "gnome_terminal_term_program_user_agent"
    );
}

#[test]
fn detects_vte() {
    let env = FakeEnvironment::new().with_var("VTE_VERSION", "7000");
    let terminal = detect_terminal_info_from_env(&env);
    assert_eq!(
        terminal,
        terminal_info(
            TerminalName::Vte,
            /*term_program*/ None,
            Some("7000"),
            /*term*/ None,
            /*multiplexer*/ None
        ),
        "vte_version_info"
    );
    assert_eq!(
        terminal.user_agent_token(),
        "VTE/7000",
        "vte_version_user_agent"
    );

    let env = FakeEnvironment::new()
        .with_var("TERM_PROGRAM", "VTE")
        .with_var("TERM_PROGRAM_VERSION", "7000");
    let terminal = detect_terminal_info_from_env(&env);
    assert_eq!(
        terminal,
        terminal_info(
            TerminalName::Vte,
            Some("VTE"),
            Some("7000"),
            /*term*/ None,
            /*multiplexer*/ None
        ),
        "vte_term_program_info"
    );
    assert_eq!(
        terminal.user_agent_token(),
        "VTE/7000",
        "vte_term_program_user_agent"
    );

    let env = FakeEnvironment::new().with_var("VTE_VERSION", "");
    let terminal = detect_terminal_info_from_env(&env);
    assert_eq!(
        terminal,
        terminal_info(
            TerminalName::Vte,
            /*term_program*/ None,
            /*version*/ None,
            /*term*/ None,
            /*multiplexer*/ None
        ),
        "vte_empty_info"
    );
    assert_eq!(terminal.user_agent_token(), "VTE", "vte_empty_user_agent");
}

#[test]
fn detects_windows_terminal() {
    let env = FakeEnvironment::new().with_var("WT_SESSION", "1");
    let terminal = detect_terminal_info_from_env(&env);
    assert_eq!(
        terminal,
        terminal_info(
            TerminalName::WindowsTerminal,
            /*term_program*/ None,
            /*version*/ None,
            /*term*/ None,
            /*multiplexer*/ None
        ),
        "wt_session_info"
    );
    assert_eq!(
        terminal.user_agent_token(),
        "WindowsTerminal",
        "wt_session_user_agent"
    );

    let env = FakeEnvironment::new()
        .with_var("TERM_PROGRAM", "WindowsTerminal")
        .with_var("TERM_PROGRAM_VERSION", "1.21");
    let terminal = detect_terminal_info_from_env(&env);
    assert_eq!(
        terminal,
        terminal_info(
            TerminalName::WindowsTerminal,
            Some("WindowsTerminal"),
            Some("1.21"),
            /*term*/ None,
            /*multiplexer*/ None,
        ),
        "windows_terminal_term_program_info"
    );
    assert_eq!(
        terminal.user_agent_token(),
        "WindowsTerminal/1.21",
        "windows_terminal_term_program_user_agent"
    );
}

#[test]
fn detects_term_fallbacks() {
    let env = FakeEnvironment::new().with_var("TERM", "xterm-256color");
    let terminal = detect_terminal_info_from_env(&env);
    assert_eq!(
        terminal,
        terminal_info(
            TerminalName::Unknown,
            /*term_program*/ None,
            /*version*/ None,
            Some("xterm-256color"),
            /*multiplexer*/ None,
        ),
        "term_fallback_info"
    );
    assert_eq!(
        terminal.user_agent_token(),
        "xterm-256color",
        "term_fallback_user_agent"
    );

    let env = FakeEnvironment::new().with_var("TERM", "dumb");
    let terminal = detect_terminal_info_from_env(&env);
    assert_eq!(
        terminal,
        terminal_info(
            TerminalName::Dumb,
            /*term_program*/ None,
            /*version*/ None,
            Some("dumb"),
            /*multiplexer*/ None
        ),
        "dumb_term_info"
    );
    assert_eq!(terminal.user_agent_token(), "dumb", "dumb_term_user_agent");

    let env = FakeEnvironment::new();
    let terminal = detect_terminal_info_from_env(&env);
    assert_eq!(
        terminal,
        terminal_info(
            TerminalName::Unknown,
            /*term_program*/ None,
            /*version*/ None,
            /*term*/ None,
            /*multiplexer*/ None
        ),
        "unknown_info"
    );
    assert_eq!(terminal.user_agent_token(), "unknown", "unknown_user_agent");
}
