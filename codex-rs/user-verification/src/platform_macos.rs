//! macOS Security and LocalAuthentication integration.

#[cfg(target_os = "macos")]
mod enrollment_diagnostics;
mod error;
#[cfg(target_os = "macos")]
mod key_protection;
#[cfg(target_os = "macos")]
mod provider;

#[cfg(target_os = "macos")]
pub(crate) use provider::NativeProvider;
#[cfg(target_os = "macos")]
pub(crate) use provider::device_supported;
