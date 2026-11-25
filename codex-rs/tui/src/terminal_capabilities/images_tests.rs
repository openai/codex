#![cfg(test)]
use super::*;
use serial_test::serial;

pub(crate) struct EnvVarGuard {
    name: &'static str,
    previous: Option<std::ffi::OsString>,
}

impl EnvVarGuard {
    pub(crate) fn new(name: &'static str, value: Option<&str>) -> Self {
        let previous = env::var_os(name);
        match value {
            Some(value) => unsafe { env::set_var(name, value) },
            None => unsafe { env::remove_var(name) },
        }
        Self { name, previous }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        match self.previous.take() {
            Some(value) => unsafe { env::set_var(self.name, value) },
            None => unsafe { env::remove_var(self.name) },
        }
    }
}

#[test]
fn pet_image_support_prefers_multiplexer_safety() {
    assert_eq!(
        pet_image_support_for_terminal(&terminal_info_for_test(
            TerminalName::Ghostty,
            Some(Multiplexer::Tmux { version: None }),
            Some("Ghostty"),
            /*term*/ None,
        )),
        PetImageSupport::Unsupported(PetImageUnsupportedReason::Tmux)
    );
    assert_eq!(
        pet_image_support_for_terminal(&terminal_info_for_test(
            TerminalName::Kitty,
            Some(Multiplexer::Zellij { version: None }),
            Some("kitty"),
            /*term*/ None,
        )),
        PetImageSupport::Unsupported(PetImageUnsupportedReason::Zellij)
    );
}

#[test]
fn pet_image_support_detects_iterm2_kitty_file_graphics() {
    for info in [
        terminal_info_with_version_for_test(
            TerminalName::Iterm2,
            /*multiplexer*/ None,
            Some("iTerm.app"),
            Some("3.6.10"),
            /*term*/ None,
        ),
        terminal_info_with_version_for_test(
            TerminalName::Unknown,
            /*multiplexer*/ None,
            Some("iTerm.app"),
            Some("3.6.10"),
            Some("xterm-256color"),
        ),
    ] {
        assert_eq!(
            pet_image_support_for_terminal(&info),
            PetImageSupport::Supported(ImageProtocol::KittyLocalFile)
        );
    }
}

#[test]
fn pet_image_support_rejects_old_iterm2_versions() {
    for info in [
        terminal_info_with_version_for_test(
            TerminalName::Iterm2,
            /*multiplexer*/ None,
            Some("iTerm.app"),
            Some("3.5.14"),
            /*term*/ None,
        ),
        terminal_info_with_version_for_test(
            TerminalName::Unknown,
            /*multiplexer*/ None,
            Some("iTerm.app"),
            /*version*/ None,
            Some("xterm-256color"),
        ),
        terminal_info_with_version_for_test(
            TerminalName::Iterm2,
            /*multiplexer*/ None,
            Some("iTerm.app"),
            Some("3.5"),
            /*term*/ None,
        ),
    ] {
        assert_eq!(
            pet_image_support_for_terminal(&info),
            PetImageSupport::Unsupported(PetImageUnsupportedReason::Iterm2TooOld)
        );
    }
}

#[test]
fn pet_image_support_old_iterm2_message_mentions_upgrade() {
    let message =
        PetImageSupport::Unsupported(PetImageUnsupportedReason::Iterm2TooOld).unsupported_message();

    assert_eq!(
        message,
        Some("Pets require iTerm2 3.6 or newer. Upgrade iTerm2 to use terminal pets.")
    );
}

#[test]
fn pet_image_support_detects_kitty_graphics_terminals() {
    for info in [
        terminal_info_for_test(
            TerminalName::Ghostty,
            /*multiplexer*/ None,
            Some("Ghostty"),
            /*term*/ None,
        ),
        terminal_info_for_test(
            TerminalName::Kitty,
            /*multiplexer*/ None,
            Some("kitty"),
            /*term*/ None,
        ),
        terminal_info_for_test(
            TerminalName::Rex,
            /*multiplexer*/ None,
            Some("rex"),
            /*term*/ None,
        ),
        terminal_info_for_test(
            TerminalName::WezTerm,
            /*multiplexer*/ None,
            Some("WezTerm"),
            /*term*/ None,
        ),
        terminal_info_for_test(
            TerminalName::Unknown,
            /*multiplexer*/ None,
            /*term_program*/ None,
            Some("xterm-kitty"),
        ),
        terminal_info_for_test(
            TerminalName::Unknown,
            /*multiplexer*/ None,
            /*term_program*/ None,
            Some("wezterm"),
        ),
        terminal_info_for_test(
            TerminalName::Unknown,
            /*multiplexer*/ None,
            Some("WezTerm"),
            Some("xterm-256color"),
        ),
    ] {
        assert_eq!(
            pet_image_support_for_terminal(&info),
            PetImageSupport::Supported(ImageProtocol::Kitty)
        );
    }
}

#[test]
fn orca_graphics_require_a_direct_terminal_and_advertised_protocol() {
    let mut info = terminal_info_for_test(
        TerminalName::Orca,
        /*multiplexer*/ None,
        Some("Orca"),
        /*term*/ None,
    );
    for (protocol, term, sty_present, expected) in [
        (Some("kitty"), Some("xterm-256color"), false, true),
        (None, Some("xterm-256color"), false, false),
        (Some("kitty"), Some("screen-256color"), false, false),
        (Some("kitty"), Some("tmux-256color"), false, false),
        (Some("kitty"), Some("dumb"), false, false),
        (Some("kitty"), Some("xterm-256color"), true, false),
    ] {
        assert_eq!(
            supports_orca_kitty_graphics(&info, protocol, term, sty_present),
            expected
        );
    }
    info.multiplexer = Some(Multiplexer::Tmux { version: None });
    assert!(!supports_orca_kitty_graphics(
        &info,
        Some("kitty"),
        Some("xterm-256color"),
        /*sty_present*/ false
    ));
}

#[test]
fn pet_image_support_detects_sixel_terminals() {
    for info in [
        terminal_info_for_test(
            TerminalName::Unknown,
            /*multiplexer*/ None,
            /*term_program*/ None,
            Some("xterm-sixel"),
        ),
        terminal_info_for_test(
            TerminalName::Unknown,
            /*multiplexer*/ None,
            /*term_program*/ None,
            Some("foot"),
        ),
        terminal_info_for_test(
            TerminalName::Unknown,
            /*multiplexer*/ None,
            /*term_program*/ None,
            Some("mlterm"),
        ),
        terminal_info_for_test(
            TerminalName::WindowsTerminal,
            /*multiplexer*/ None,
            Some("WindowsTerminal"),
            Some("xterm-256color"),
        ),
    ] {
        assert_eq!(
            pet_image_support_for_terminal(&info),
            PetImageSupport::Supported(ImageProtocol::Sixel)
        );
    }
}

#[test]
#[serial]
fn wezterm_env_uses_kitty_graphics_for_ambient_pets() {
    let _tmux = EnvVarGuard::new("TMUX", /*value*/ None);
    let _tmux_pane = EnvVarGuard::new("TMUX_PANE", /*value*/ None);
    let _zellij = EnvVarGuard::new("ZELLIJ", /*value*/ None);
    let _zellij_session = EnvVarGuard::new("ZELLIJ_SESSION_NAME", /*value*/ None);
    let _zellij_version = EnvVarGuard::new("ZELLIJ_VERSION", /*value*/ None);
    let _kitty = EnvVarGuard::new("KITTY_WINDOW_ID", /*value*/ None);
    let _wezterm = EnvVarGuard::new("WEZTERM_VERSION", Some("20240203"));
    let _wezterm_executable = EnvVarGuard::new("WEZTERM_EXECUTABLE", /*value*/ None);
    let _orca_protocol = EnvVarGuard::new("ORCA_IMAGE_PROTOCOL", /*value*/ None);

    assert_eq!(
        image_support_for_terminal_environment(&terminal_info_for_test(
            TerminalName::Unknown,
            /*multiplexer*/ None,
            /*term_program*/ None,
            /*term*/ None,
        )),
        PetImageSupport::Supported(ImageProtocol::Kitty)
    );
    assert_eq!(
        image_support_for_terminal_environment(&terminal_info_for_test(
            TerminalName::Orca,
            /*multiplexer*/ None,
            Some("Orca"),
            /*term*/ None,
        )),
        PetImageSupport::Unsupported(PetImageUnsupportedReason::Terminal)
    );
}

#[test]
fn pet_image_support_rejects_unknown_terminals() {
    assert_eq!(
        pet_image_support_for_terminal(&terminal_info_for_test(
            TerminalName::Unknown,
            /*multiplexer*/ None,
            /*term_program*/ None,
            Some("xterm-256color"),
        )),
        PetImageSupport::Unsupported(PetImageUnsupportedReason::Terminal)
    );
}

fn terminal_info_for_test(
    name: TerminalName,
    multiplexer: Option<Multiplexer>,
    term_program: Option<&str>,
    term: Option<&str>,
) -> TerminalInfo {
    terminal_info_with_version_for_test(
        name,
        multiplexer,
        term_program,
        /*version*/ None,
        term,
    )
}

fn terminal_info_with_version_for_test(
    name: TerminalName,
    multiplexer: Option<Multiplexer>,
    term_program: Option<&str>,
    version: Option<&str>,
    term: Option<&str>,
) -> TerminalInfo {
    TerminalInfo {
        backend: None,
        name,
        term_program: term_program.map(str::to_string),
        version: version.map(str::to_string),
        term: term.map(str::to_string),
        multiplexer,
    }
}

#[test]
fn parse_dotted_version_requires_simple_numeric_components() {
    assert_eq!(parse_dotted_version(Some("3.6.10")), Some((3, 6, 10)));
    assert_eq!(parse_dotted_version(Some("3.6")), Some((3, 6, 0)));
    assert_eq!(parse_dotted_version(Some("3")), Some((3, 0, 0)));
    assert_eq!(parse_dotted_version(Some("3.6.10.1")), None);
    assert_eq!(parse_dotted_version(Some("3.6beta")), None);
    assert_eq!(parse_dotted_version(/*version*/ None), None);
}
