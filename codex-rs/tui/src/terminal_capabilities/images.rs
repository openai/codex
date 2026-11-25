//! Detects image protocols and placement support, including fullscreen-only virtual placements.
use codex_terminal_detection::Multiplexer;
use codex_terminal_detection::TerminalInfo;
use codex_terminal_detection::TerminalName;
use codex_terminal_detection::terminal_info;
use std::env;

const ITERM2_KITTY_MIN_VERSION: (u64, u64, u64) = (3, 6, 0);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageProtocol {
    Kitty,
    KittyLocalFile,
    Sixel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PetImageSupport {
    Supported(ImageProtocol),
    Unsupported(PetImageUnsupportedReason),
}

impl PetImageSupport {
    pub(crate) fn protocol(self) -> Option<ImageProtocol> {
        match self {
            Self::Supported(protocol) => Some(protocol),
            Self::Unsupported(PetImageUnsupportedReason::Tmux) => {
                crate::tui::tmux_fullscreen_image_base().map(|_| ImageProtocol::Kitty)
            }
            Self::Unsupported(_) => None,
        }
    }

    pub(crate) fn unsupported_message(self) -> Option<&'static str> {
        match self {
            Self::Supported(_) => None,
            Self::Unsupported(reason) => self.protocol().is_none().then(|| reason.message()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PetImageUnsupportedReason {
    Tmux,
    Zellij,
    Iterm2TooOld,
    Terminal,
}

impl PetImageUnsupportedReason {
    fn message(self) -> &'static str {
        match self {
            Self::Tmux => {
                "Pets are unavailable in this tmux pane. Use fullscreen Codex with allow-passthrough all, or run Codex outside tmux."
            }
            Self::Zellij => {
                "Pets are disabled in Zellij. Terminal images don’t stay reliably pane-local in Zellij. Run Codex outside Zellij to use pets."
            }
            Self::Iterm2TooOld => {
                "Pets require iTerm2 3.6 or newer. Upgrade iTerm2 to use terminal pets."
            }
            Self::Terminal => {
                "Pets aren’t available in this terminal. Terminal pets need image support, and this terminal environment doesn’t expose a supported image protocol. Try a terminal with Kitty graphics or Sixel support, or run Codex outside tmux."
            }
        }
    }
}

pub(crate) fn detect_pet_image_support() -> PetImageSupport {
    if env::var_os("TMUX").is_some() || env::var_os("TMUX_PANE").is_some() {
        return PetImageSupport::Unsupported(PetImageUnsupportedReason::Tmux);
    }

    if env::var_os("ZELLIJ").is_some()
        || env::var_os("ZELLIJ_SESSION_NAME").is_some()
        || env::var_os("ZELLIJ_VERSION").is_some()
    {
        return PetImageSupport::Unsupported(PetImageUnsupportedReason::Zellij);
    }

    image_support_for_terminal_environment(&terminal_info())
}

fn image_support_for_terminal_environment(info: &TerminalInfo) -> PetImageSupport {
    if info.name == TerminalName::Orca {
        return if supports_orca_kitty_graphics(
            info,
            env::var("ORCA_IMAGE_PROTOCOL").ok().as_deref(),
            env::var("TERM").ok().as_deref(),
            /*sty_present*/ env::var_os("STY").is_some(),
        ) {
            PetImageSupport::Supported(ImageProtocol::Kitty)
        } else {
            PetImageSupport::Unsupported(PetImageUnsupportedReason::Terminal)
        };
    }

    if env::var_os("KITTY_WINDOW_ID").is_some() {
        return PetImageSupport::Supported(ImageProtocol::Kitty);
    }

    if env::var_os("WEZTERM_EXECUTABLE").is_some() || env::var_os("WEZTERM_VERSION").is_some() {
        return PetImageSupport::Supported(ImageProtocol::Kitty);
    }

    pet_image_support_for_terminal(info)
}

fn supports_orca_kitty_graphics(
    info: &TerminalInfo,
    protocol: Option<&str>,
    term: Option<&str>,
    sty_present: bool,
) -> bool {
    info.multiplexer.is_none()
        && protocol == Some("kitty")
        && !sty_present
        && !term.is_some_and(|term| {
            term == "dumb" || term.starts_with("screen") || term.starts_with("tmux")
        })
}

fn pet_image_support_for_terminal(info: &TerminalInfo) -> PetImageSupport {
    match info.multiplexer {
        Some(Multiplexer::Tmux { .. }) => {
            return PetImageSupport::Unsupported(PetImageUnsupportedReason::Tmux);
        }
        Some(Multiplexer::Zellij { .. }) => {
            return PetImageSupport::Unsupported(PetImageUnsupportedReason::Zellij);
        }
        None => {}
    }

    if supports_iterm2_kitty_graphics(info) {
        return PetImageSupport::Supported(ImageProtocol::KittyLocalFile);
    }

    if is_iterm2_terminal(info) {
        return PetImageSupport::Unsupported(PetImageUnsupportedReason::Iterm2TooOld);
    }

    if supports_kitty_graphics(info) {
        return PetImageSupport::Supported(ImageProtocol::Kitty);
    }

    if supports_sixel(info) {
        return PetImageSupport::Supported(ImageProtocol::Sixel);
    }

    PetImageSupport::Unsupported(PetImageUnsupportedReason::Terminal)
}

fn supports_iterm2_kitty_graphics(info: &TerminalInfo) -> bool {
    is_iterm2_terminal(info)
        && version_is_at_least(
            info.version.as_deref(),
            /*minimum*/ ITERM2_KITTY_MIN_VERSION,
        )
}

fn is_iterm2_terminal(info: &TerminalInfo) -> bool {
    matches!(info.name, TerminalName::Iterm2)
        || terminal_field_contains(info.term_program.as_deref(), "iterm")
}

fn supports_kitty_graphics(info: &TerminalInfo) -> bool {
    matches!(
        info.name,
        TerminalName::Ghostty | TerminalName::Kitty | TerminalName::Rex | TerminalName::WezTerm
    ) || terminal_field_contains(info.term.as_deref(), "kitty")
        || terminal_field_contains(info.term.as_deref(), "ghostty")
        || terminal_field_contains(info.term.as_deref(), "wezterm")
        || terminal_field_contains(info.term_program.as_deref(), "kitty")
        || terminal_field_contains(info.term_program.as_deref(), "ghostty")
        || terminal_field_contains(info.term_program.as_deref(), "wezterm")
}

fn supports_sixel(info: &TerminalInfo) -> bool {
    matches!(info.name, TerminalName::WindowsTerminal)
        || terminal_field_contains(info.term.as_deref(), "sixel")
        || terminal_field_contains(info.term.as_deref(), "mlterm")
        || terminal_field_contains(info.term.as_deref(), "foot")
}

fn terminal_field_contains(value: Option<&str>, needle: &str) -> bool {
    value.is_some_and(|value| value.to_ascii_lowercase().contains(needle))
}

fn version_is_at_least(version: Option<&str>, minimum: (u64, u64, u64)) -> bool {
    parse_dotted_version(version).is_some_and(|version| version >= minimum)
}

fn parse_dotted_version(version: Option<&str>) -> Option<(u64, u64, u64)> {
    let version = version?;
    let mut parts = version.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next().unwrap_or("0").parse().ok()?;
    let patch = parts.next().unwrap_or("0").parse().ok()?;

    if parts.next().is_some() {
        return None;
    }

    Some((major, minor, patch))
}

#[cfg(test)]
#[path = "images_tests.rs"]
pub(crate) mod tests;
