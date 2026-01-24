#[cfg(any(all(target_os = "macos", target_arch = "aarch64"), target_os = "linux"))]
mod accept_elicitation;
// exec-server/tests/suite/mod.rs
#[cfg(any(all(target_os = "macos", target_arch = "aarch64"), target_os = "linux"))]
mod list_tools;
