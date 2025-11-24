#[cfg(linux_kernel)]
pub(crate) mod futex;
#[cfg(not(windows))]
pub(crate) mod syscalls;
