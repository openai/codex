mod bel;
mod osc9;

use std::env;
use std::io;

use bel::BelBackend;
use codex_core::config::types::NotificationMethod;
use osc9::Osc9Backend;

#[derive(Debug)]
pub enum DesktopNotificationBackend {
    Osc9(Osc9Backend),
    Bel(BelBackend),
}

impl DesktopNotificationBackend {
    pub fn for_method(method: NotificationMethod) -> Self {
        match method {
            NotificationMethod::Auto => {
                if supports_osc9() {
                    Self::Osc9(Osc9Backend)
                } else {
                    Self::Bel(BelBackend)
                }
            }
            NotificationMethod::Osc9 => Self::Osc9(Osc9Backend),
            NotificationMethod::Bel => Self::Bel(BelBackend),
        }
    }

    pub fn method(&self) -> NotificationMethod {
        match self {
            DesktopNotificationBackend::Osc9(_) => NotificationMethod::Osc9,
            DesktopNotificationBackend::Bel(_) => NotificationMethod::Bel,
        }
    }

    pub fn notify(&mut self, message: &str) -> io::Result<()> {
        match self {
            DesktopNotificationBackend::Osc9(backend) => backend.notify(message),
            DesktopNotificationBackend::Bel(backend) => backend.notify(message),
        }
    }
}

pub fn detect_backend(method: NotificationMethod) -> DesktopNotificationBackend {
    DesktopNotificationBackend::for_method(method)
}

fn supports_osc9() -> bool {
    if env::var_os("WT_SESSION").is_some() {
        return false;
    }
    // Prefer TERM_PROGRAM when present, but keep fallbacks for shells/launchers
    // that don't set it (e.g., tmux/ssh) to avoid regressing OSC 9 support.
    if matches!(
        env::var("TERM_PROGRAM").ok().as_deref(),
        Some("WezTerm" | "ghostty")
    ) {
        return true;
    }
    // iTerm still provides a strong session signal even when TERM_PROGRAM is missing.
    if env::var_os("ITERM_SESSION_ID").is_some() {
        return true;
    }
    // TERM-based hints cover kitty/wezterm setups without TERM_PROGRAM.
    matches!(
        env::var("TERM").ok().as_deref(),
        Some("xterm-kitty" | "wezterm" | "wezterm-mux")
    )
}

#[cfg(test)]
mod tests {
    use super::detect_backend;
    use codex_core::config::types::NotificationMethod;
    use serial_test::serial;
    use std::ffi::OsString;

    struct EnvVarGuard {
        key: &'static str,
        original: Option<OsString>,
    }

    impl EnvVarGuard {
        fn set(key: &'static str, value: &str) -> Self {
            let original = std::env::var_os(key);
            unsafe {
                std::env::set_var(key, value);
            }
            Self { key, original }
        }

        fn remove(key: &'static str) -> Self {
            let original = std::env::var_os(key);
            unsafe {
                std::env::remove_var(key);
            }
            Self { key, original }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            unsafe {
                match &self.original {
                    Some(value) => std::env::set_var(self.key, value),
                    None => std::env::remove_var(self.key),
                }
            }
        }
    }

    #[test]
    fn selects_osc9_method() {
        assert!(matches!(
            detect_backend(NotificationMethod::Osc9),
            super::DesktopNotificationBackend::Osc9(_)
        ));
    }

    #[test]
    fn selects_bel_method() {
        assert!(matches!(
            detect_backend(NotificationMethod::Bel),
            super::DesktopNotificationBackend::Bel(_)
        ));
    }

    #[test]
    #[serial]
    fn auto_prefers_bel_without_hints() {
        let _term = EnvVarGuard::remove("TERM");
        let _term_program = EnvVarGuard::remove("TERM_PROGRAM");
        let _iterm = EnvVarGuard::remove("ITERM_SESSION_ID");
        let _wt = EnvVarGuard::remove("WT_SESSION");
        assert!(matches!(
            detect_backend(NotificationMethod::Auto),
            super::DesktopNotificationBackend::Bel(_)
        ));
    }

    #[test]
    #[serial]
    fn auto_uses_osc9_for_iterm() {
        let _term = EnvVarGuard::remove("TERM");
        let _term_program = EnvVarGuard::remove("TERM_PROGRAM");
        let _iterm = EnvVarGuard::set("ITERM_SESSION_ID", "abc");
        let _wt = EnvVarGuard::remove("WT_SESSION");
        assert!(matches!(
            detect_backend(NotificationMethod::Auto),
            super::DesktopNotificationBackend::Osc9(_)
        ));
    }
}
