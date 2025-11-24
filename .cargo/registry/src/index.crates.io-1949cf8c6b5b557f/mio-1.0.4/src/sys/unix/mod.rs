/// Helper macro to execute a system call that returns an `io::Result`.
//
// Macro must be defined before any modules that uses them.
#[allow(unused_macros)]
macro_rules! syscall {
    ($fn: ident ( $($arg: expr),* $(,)* ) ) => {{
        #[allow(unused_unsafe)]
        let res = unsafe { libc::$fn($($arg, )*) };
        if res < 0 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(res)
        }
    }};
}

cfg_os_poll! {
    #[cfg_attr(all(
        not(mio_unsupported_force_poll_poll),
        any(
            target_os = "android",
            target_os = "illumos",
            target_os = "linux",
            target_os = "redox",
        )
    ), path = "selector/epoll.rs")]
    #[cfg_attr(all(
        not(mio_unsupported_force_poll_poll),
        any(
            target_os = "dragonfly",
            target_os = "freebsd",
            target_os = "ios",
            target_os = "macos",
            target_os = "netbsd",
            target_os = "openbsd",
            target_os = "tvos",
            target_os = "visionos",
            target_os = "watchos",
        )
    ), path = "selector/kqueue.rs")]
    #[cfg_attr(any(
        mio_unsupported_force_poll_poll,
        target_os = "aix",
        target_os = "espidf",
        target_os = "fuchsia",
        target_os = "haiku",
        target_os = "hermit",
        target_os = "hurd",
        target_os = "nto",
        target_os = "solaris",
        target_os = "vita",
    ), path = "selector/poll.rs")]
    mod selector;
    pub(crate) use self::selector::*;

    #[cfg_attr(all(
        not(mio_unsupported_force_waker_pipe),
        any(
            target_os = "android",
            target_os = "espidf",
            target_os = "fuchsia",
            target_os = "hermit",
            target_os = "illumos",
            target_os = "linux",
        )
    ), path = "waker/eventfd.rs")]
    #[cfg_attr(all(
        not(mio_unsupported_force_waker_pipe),
        not(mio_unsupported_force_poll_poll), // `kqueue(2)` based waker doesn't work with `poll(2)`.
        any(
            target_os = "freebsd",
            target_os = "ios",
            target_os = "macos",
            target_os = "tvos",
            target_os = "visionos",
            target_os = "watchos",
        )
    ), path = "waker/kqueue.rs")]
    #[cfg_attr(any(
        // NOTE: also add to the list list for the `pipe` module below.
        mio_unsupported_force_waker_pipe,
        all(
            // `kqueue(2)` based waker doesn't work with `poll(2)`.
            mio_unsupported_force_poll_poll,
            any(
                target_os = "freebsd",
                target_os = "ios",
                target_os = "macos",
                target_os = "tvos",
                target_os = "visionos",
                target_os = "watchos",
            ),
        ),
        target_os = "aix",
        target_os = "dragonfly",
        target_os = "haiku",
        target_os = "hurd",
        target_os = "netbsd",
        target_os = "nto",
        target_os = "openbsd",
        target_os = "redox",
        target_os = "solaris",
        target_os = "vita",
    ), path = "waker/pipe.rs")]
    mod waker;
    // NOTE: the `Waker` type is expected in the selector module as the
    // `poll(2)` implementation needs to do some special stuff.

    mod sourcefd;
    #[cfg(feature = "os-ext")]
    pub use self::sourcefd::SourceFd;

    cfg_net! {
        mod net;

        pub(crate) mod tcp;
        pub(crate) mod udp;
        #[cfg(not(target_os = "hermit"))]
        pub(crate) mod uds;
    }

    #[cfg(all(
        any(
            // For the public `pipe` module, must match `cfg_os_ext` macro.
            feature = "os-ext",
            // For the `Waker` type based on a pipe.
            mio_unsupported_force_waker_pipe,
            all(
                // `kqueue(2)` based waker doesn't work with `poll(2)`.
                mio_unsupported_force_poll_poll,
                any(
                    target_os = "freebsd",
                    target_os = "ios",
                    target_os = "macos",
                    target_os = "tvos",
                    target_os = "visionos",
                    target_os = "watchos",
                ),
            ),
            // NOTE: also add to the list list for the `pipe` module below.
            target_os = "aix",
            target_os = "dragonfly",
            target_os = "haiku",
            target_os = "hurd",
            target_os = "netbsd",
            target_os = "nto",
            target_os = "openbsd",
            target_os = "redox",
            target_os = "solaris",
            target_os = "vita",
        ),
        // Hermit doesn't support pipes.
        not(target_os = "hermit"),
    ))]
    pub(crate) mod pipe;
}

cfg_not_os_poll! {
    cfg_any_os_ext! {
        mod sourcefd;
        #[cfg(feature = "os-ext")]
        pub use self::sourcefd::SourceFd;
    }
}
