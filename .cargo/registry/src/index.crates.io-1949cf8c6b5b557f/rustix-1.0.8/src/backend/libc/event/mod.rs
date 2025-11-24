pub(crate) mod poll_fd;
#[cfg(not(windows))]
pub(crate) mod types;

#[cfg_attr(windows, path = "windows_syscalls.rs")]
pub(crate) mod syscalls;

#[cfg(any(linux_kernel, target_os = "illumos", target_os = "redox"))]
pub mod epoll;
