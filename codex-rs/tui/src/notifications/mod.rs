mod bel;
mod osc;

use std::io;

use crate::terminal_capabilities::NotificationCapabilities;
use bel::BelBackend;
use codex_config::types::NotificationMethod;
use osc::OscBackend;
use osc::OscProtocol;

#[derive(Debug)]
pub enum DesktopNotificationBackend {
    Osc(OscBackend),
    Bel(BelBackend),
}

impl DesktopNotificationBackend {
    pub fn for_method(method: NotificationMethod) -> Self {
        match method {
            NotificationMethod::Auto => {
                if NotificationCapabilities::detect().osc9 {
                    Self::Osc(OscBackend::new(OscProtocol::Osc9))
                } else {
                    Self::Bel(BelBackend)
                }
            }
            NotificationMethod::Osc9 => Self::Osc(OscBackend::new(OscProtocol::Osc9)),
            NotificationMethod::Osc777 => Self::Osc(OscBackend::new(OscProtocol::Osc777)),
            NotificationMethod::Bel => Self::Bel(BelBackend),
        }
    }

    pub fn method(&self) -> NotificationMethod {
        match self {
            DesktopNotificationBackend::Osc(backend) => match backend.protocol {
                OscProtocol::Osc9 => NotificationMethod::Osc9,
                OscProtocol::Osc777 => NotificationMethod::Osc777,
            },
            DesktopNotificationBackend::Bel(_) => NotificationMethod::Bel,
        }
    }

    pub fn notify(&mut self, message: &str) -> io::Result<()> {
        match self {
            DesktopNotificationBackend::Osc(backend) => backend.notify(message),
            DesktopNotificationBackend::Bel(backend) => backend.notify(message),
        }
    }
}

pub fn detect_backend(method: NotificationMethod) -> DesktopNotificationBackend {
    DesktopNotificationBackend::for_method(method)
}

#[cfg(test)]
mod tests {
    use super::detect_backend;
    use crate::terminal_capabilities::NotificationCapabilities;
    use codex_config::types::NotificationMethod;
    use codex_terminal_detection::TerminalInfo;
    use codex_terminal_detection::TerminalName;
    use pretty_assertions::assert_eq;

    fn test_terminal(name: TerminalName) -> TerminalInfo {
        TerminalInfo {
            backend: None,
            name,
            term_program: None,
            version: None,
            term: None,
            multiplexer: None,
        }
    }

    #[test]
    fn selects_explicit_notification_method() {
        for method in [
            NotificationMethod::Osc9,
            NotificationMethod::Osc777,
            NotificationMethod::Bel,
        ] {
            assert_eq!(detect_backend(method).method(), method);
        }
    }

    #[test]
    fn supports_osc9_for_supported_terminals() {
        for (name, limit) in [
            (TerminalName::Ghostty, 255),
            (TerminalName::Iterm2, usize::MAX),
            (TerminalName::Kitty, usize::MAX),
            (TerminalName::WarpTerminal, usize::MAX),
            (TerminalName::WezTerm, usize::MAX),
        ] {
            let caps = NotificationCapabilities::for_terminal(&test_terminal(name));
            assert_eq!((caps.osc9, caps.max_body_bytes), (true, limit), "{name:?}");
        }
    }

    #[test]
    fn supports_osc9_for_unsupported_terminals() {
        for name in [
            TerminalName::AppleTerminal,
            TerminalName::Alacritty,
            TerminalName::Dumb,
            TerminalName::GnomeTerminal,
            TerminalName::Konsole,
            TerminalName::Unknown,
            TerminalName::VsCode,
            TerminalName::Vte,
            TerminalName::WindowsTerminal,
        ] {
            assert_eq!(
                NotificationCapabilities::for_terminal(&test_terminal(name)).osc9,
                false,
                "{name:?} should not support OSC 9"
            );
        }
    }
}
