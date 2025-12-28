mod osc9;
mod windows_toast;

use std::env;
use std::io;

use codex_core::env::is_wsl;
use osc9::Osc9Backend;
use windows_toast::WindowsToastBackend;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationBackendKind {
    Osc9,
    WindowsToast,
}

#[derive(Debug)]
pub enum DesktopNotificationBackend {
    Osc9(Osc9Backend),
    WindowsToast(WindowsToastBackend),
}

impl DesktopNotificationBackend {
    pub fn osc9() -> Self {
        Self::Osc9(Osc9Backend)
    }

    pub fn windows_toast() -> Self {
        Self::WindowsToast(WindowsToastBackend::default())
    }

    pub fn kind(&self) -> NotificationBackendKind {
        match self {
            DesktopNotificationBackend::Osc9(_) => NotificationBackendKind::Osc9,
            DesktopNotificationBackend::WindowsToast(_) => NotificationBackendKind::WindowsToast,
        }
    }

    pub fn notify(&mut self, message: &str) -> io::Result<()> {
        match self {
            DesktopNotificationBackend::Osc9(backend) => backend.notify(message),
            DesktopNotificationBackend::WindowsToast(backend) => backend.notify(message),
        }
    }
}

pub fn detect_backend() -> DesktopNotificationBackend {
    if should_use_windows_toasts() {
        tracing::info!(
            "Windows Terminal session detected under WSL; using Windows toast notifications"
        );
        DesktopNotificationBackend::windows_toast()
    } else {
        DesktopNotificationBackend::osc9()
    }
}

fn should_use_windows_toasts() -> bool {
    is_wsl() && env::var_os("WT_SESSION").is_some()
}

#[cfg(test)]
mod tests {
    use super::NotificationBackendKind;
    use super::detect_backend;
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
    #[serial]
    fn defaults_to_osc9_outside_wsl() {
        let _wsl_guard = EnvVarGuard::remove("WSL_DISTRO_NAME");
        let _wt_guard = EnvVarGuard::remove("WT_SESSION");
        assert_eq!(detect_backend().kind(), NotificationBackendKind::Osc9);
    }

    #[test]
    #[serial]
    fn waits_for_windows_terminal() {
        let _wsl_guard = EnvVarGuard::set("WSL_DISTRO_NAME", "Ubuntu");
        let _wt_guard = EnvVarGuard::remove("WT_SESSION");
        assert_eq!(detect_backend().kind(), NotificationBackendKind::Osc9);
    }

    #[cfg(target_os = "linux")]
    #[test]
    #[serial]
    fn selects_windows_toast_in_wsl_windows_terminal() {
        let _wsl_guard = EnvVarGuard::set("WSL_DISTRO_NAME", "Ubuntu");
        let _wt_guard = EnvVarGuard::set("WT_SESSION", "abc");
        assert_eq!(
            detect_backend().kind(),
            NotificationBackendKind::WindowsToast
        );
    }

    #[cfg(not(target_os = "linux"))]
    #[test]
    #[serial]
    fn stays_on_osc9_outside_linux_even_with_wsl_env() {
        let _wsl_guard = EnvVarGuard::set("WSL_DISTRO_NAME", "Ubuntu");
        let _wt_guard = EnvVarGuard::set("WT_SESSION", "abc");
        assert_eq!(detect_backend().kind(), NotificationBackendKind::Osc9);
    }
}
