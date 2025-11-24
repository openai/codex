#[cfg(any(feature = "time", target_arch = "x86"))]
pub(crate) mod syscalls;
pub(crate) mod types;
