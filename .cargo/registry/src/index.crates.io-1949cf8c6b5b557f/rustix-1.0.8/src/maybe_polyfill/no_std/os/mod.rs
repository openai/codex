#[cfg(any(unix, target_os = "wasi"))]
pub mod fd;
#[cfg(windows)]
pub mod windows;
