//! Functionality that is only available for `unix` platforms.

use std::os::unix::io::BorrowedFd;

/// Get a file descriptor that can be used to wait for readiness in an external runtime.
///
/// This file descriptor is equivalent to the one used by the underlying epoll/kqueue/event ports
/// instance for polling. The intention is that this file descriptor can be registered into an
/// external runtime (like [`calloop`] or [GLib]) so that `async-io` can be seamlessly polled
/// alongside the other runtime.
///
/// Not every backend used on `unix` has an associated file descriptor, however. While epoll,
/// kqueue and event ports have a file descriptor as a backend, on some Unix systems `async-io`
/// will use the `poll()` system call instead. Since there are no file descriptors intrinsically
/// associated with `poll()`, this function will return `None`.
///
/// There is presently no way to stop the "`async-io`" thread from being launched, so the reactor
/// will still be continuously polled on that thread. This fact should be kept in mind by anyone
/// looking to integrate `async-io` into another runtime using this function.
///
/// It is possible to use this function to call raw system calls on the underlying event source.
/// This is generally not recommended, since registered event sources may conflict with `async-io`'s
/// existing scheme for managing sources. The behavior resulting from this is not specified, but
/// will not result in undefined behavior. This could include panics, incorrect results, aborts,
/// memory leaks, and non-termination.
///
/// [`calloop`]: https://docs.rs/calloop
/// [GLib]: https://en.wikipedia.org/wiki/GLib
///
/// ## Example
///
/// ```
/// #![cfg(unix)]
///
/// use async_io::os::unix::reactor_fd;
///
/// my_runtime::register(reactor_fd().unwrap());
/// # mod my_runtime {
/// #     use std::os::unix::io::BorrowedFd;
/// #     pub fn register(_: BorrowedFd<'_>) {}
/// # }
/// ```
pub fn reactor_fd() -> Option<BorrowedFd<'static>> {
    cfg_if::cfg_if! {
        if #[cfg(all(
            any(
                target_os = "linux",
                target_os = "android",
                target_os = "illumos",
                target_os = "solaris",
                target_vendor = "apple",
                target_os = "freebsd",
                target_os = "netbsd",
                target_os = "openbsd",
                target_os = "dragonfly",
            ),
            not(polling_test_poll_backend),
        ))] {
            use std::os::unix::io::AsFd;
            Some(crate::Reactor::get().poller.as_fd())
        } else {
            None
        }
    }
}
