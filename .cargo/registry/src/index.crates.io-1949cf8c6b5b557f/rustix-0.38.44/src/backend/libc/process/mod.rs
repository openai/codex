#[cfg(any(freebsdlike, linux_kernel, target_os = "fuchsia"))]
pub(crate) mod cpu_set;
#[cfg(not(windows))]
pub(crate) mod syscalls;
pub(crate) mod types;
#[cfg(not(any(target_os = "espidf", target_os = "vita", target_os = "wasi")))]
pub(crate) mod wait;
