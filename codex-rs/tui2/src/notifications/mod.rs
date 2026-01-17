//! Chooses and drives the desktop notification backend for the TUI.
//!
//! The TUI prefers OSC 9 terminal notifications everywhere, but when running in
//! WSL inside Windows Terminal it switches to native Windows toast
//! notifications. The selection is intentionally conservative: it only opts
//! into toast notifications when both WSL is detected and the `WT_SESSION`
//! environment variable is present, avoiding surprises on other hosts.
//!
//! This module owns backend selection and dispatch. It does not format
//! notification messages, and it does not persist any user preferences. The
//! backend decision is based solely on environment detection each time
//! [`detect_backend`] is called.

mod osc9;
mod windows_toast;

use std::env;
use std::io;

use codex_core::env::is_wsl;
use osc9::Osc9Backend;
use windows_toast::WindowsToastBackend;

/// Identifies which notification backend is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationBackendKind {
    /// Terminal-based OSC 9 notifications.
    Osc9,
    /// Windows native toast notifications.
    WindowsToast,
}

/// Dispatches notifications to the selected backend.
///
/// The enum stores the backend value so it can carry any internal state or
/// configuration required by that backend implementation.
#[derive(Debug)]
pub enum DesktopNotificationBackend {
    /// Uses OSC 9 escape sequences for notifications.
    Osc9(Osc9Backend),
    /// Uses Windows toast notifications via PowerShell.
    WindowsToast(WindowsToastBackend),
}

impl DesktopNotificationBackend {
    /// Constructs the OSC 9 backend.
    pub fn osc9() -> Self {
        Self::Osc9(Osc9Backend)
    }

    /// Constructs the Windows toast backend with default configuration.
    pub fn windows_toast() -> Self {
        Self::WindowsToast(WindowsToastBackend::default())
    }

    /// Returns the backend kind for telemetry and tests.
    pub fn kind(&self) -> NotificationBackendKind {
        match self {
            DesktopNotificationBackend::Osc9(_) => NotificationBackendKind::Osc9,
            DesktopNotificationBackend::WindowsToast(_) => NotificationBackendKind::WindowsToast,
        }
    }

    /// Sends a notification message via the selected backend.
    pub fn notify(&mut self, message: &str) -> io::Result<()> {
        match self {
            DesktopNotificationBackend::Osc9(backend) => backend.notify(message),
            DesktopNotificationBackend::WindowsToast(backend) => backend.notify(message),
        }
    }
}

/// Detects the best notification backend for the current environment.
///
/// WSL sessions running inside Windows Terminal opt into Windows toasts for
/// higher reliability; all other environments use OSC 9 notifications.
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

/// Returns true when the Windows toast backend should be used.
///
/// This is limited to WSL sessions inside Windows Terminal to avoid enabling
/// PowerShell-based notifications in other environments.
fn should_use_windows_toasts() -> bool {
    is_wsl() && env::var_os("WT_SESSION").is_some()
}

#[cfg(test)]
mod tests {
    use super::NotificationBackendKind;
    use super::detect_backend;
    use serial_test::serial;
    use std::ffi::OsString;

    /// Restores a single environment variable when dropped.
    struct EnvVarGuard {
        /// The environment variable name to restore.
        key: &'static str,
        /// The original value of the variable, if any.
        original: Option<OsString>,
    }

    impl EnvVarGuard {
        /// Sets an environment variable for the duration of the guard.
        fn set(key: &'static str, value: &str) -> Self {
            let original = std::env::var_os(key);
            unsafe {
                std::env::set_var(key, value);
            }
            Self { key, original }
        }

        /// Removes an environment variable for the duration of the guard.
        fn remove(key: &'static str) -> Self {
            let original = std::env::var_os(key);
            unsafe {
                std::env::remove_var(key);
            }
            Self { key, original }
        }
    }

    impl Drop for EnvVarGuard {
        /// Restores the original environment variable state.
        fn drop(&mut self) {
            unsafe {
                match &self.original {
                    Some(value) => std::env::set_var(self.key, value),
                    None => std::env::remove_var(self.key),
                }
            }
        }
    }

    /// Ensures OSC 9 is selected when no WSL-related variables are set.
    #[test]
    #[serial]
    fn defaults_to_osc9_outside_wsl() {
        let _wsl_guard = EnvVarGuard::remove("WSL_DISTRO_NAME");
        let _wt_guard = EnvVarGuard::remove("WT_SESSION");
        assert_eq!(detect_backend().kind(), NotificationBackendKind::Osc9);
    }

    /// Requires Windows Terminal to opt into toast notifications.
    #[test]
    #[serial]
    fn waits_for_windows_terminal() {
        let _wsl_guard = EnvVarGuard::set("WSL_DISTRO_NAME", "Ubuntu");
        let _wt_guard = EnvVarGuard::remove("WT_SESSION");
        assert_eq!(detect_backend().kind(), NotificationBackendKind::Osc9);
    }

    /// Uses Windows toast notifications inside WSL on Linux hosts.
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

    /// Keeps OSC 9 on non-Linux hosts, even with WSL-related environment set.
    #[cfg(not(target_os = "linux"))]
    #[test]
    #[serial]
    fn stays_on_osc9_outside_linux_even_with_wsl_env() {
        let _wsl_guard = EnvVarGuard::set("WSL_DISTRO_NAME", "Ubuntu");
        let _wt_guard = EnvVarGuard::set("WT_SESSION", "abc");
        assert_eq!(detect_backend().kind(), NotificationBackendKind::Osc9);
    }
}
