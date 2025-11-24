//! Event operations.

#[cfg(any(linux_kernel, target_os = "illumos", target_os = "redox"))]
pub mod epoll;
#[cfg(any(
    linux_kernel,
    target_os = "freebsd",
    target_os = "illumos",
    target_os = "espidf"
))]
mod eventfd;
#[cfg(bsd)]
pub mod kqueue;
#[cfg(not(any(windows, target_os = "redox", target_os = "wasi")))]
mod pause;
mod poll;
#[cfg(solarish)]
pub mod port;
#[cfg(any(bsd, linux_kernel, windows, target_os = "wasi"))]
mod select;

pub use crate::timespec::{Nsecs, Secs, Timespec};
#[cfg(any(
    linux_kernel,
    target_os = "freebsd",
    target_os = "illumos",
    target_os = "espidf"
))]
pub use eventfd::{eventfd, EventfdFlags};
#[cfg(not(any(windows, target_os = "redox", target_os = "wasi")))]
pub use pause::*;
pub use poll::{poll, PollFd, PollFlags};
#[cfg(any(bsd, linux_kernel, windows, target_os = "wasi"))]
pub use select::*;
