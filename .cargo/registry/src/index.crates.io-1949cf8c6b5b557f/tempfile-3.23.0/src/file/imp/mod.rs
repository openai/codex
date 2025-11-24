#[cfg_attr(any(unix, target_os = "redox", target_os = "wasi"), path = "unix.rs")]
#[cfg_attr(windows, path = "windows.rs")]
#[cfg_attr(
    not(any(unix, target_os = "redox", target_os = "wasi", windows)),
    path = "other.rs"
)]
mod platform;

pub use self::platform::*;
