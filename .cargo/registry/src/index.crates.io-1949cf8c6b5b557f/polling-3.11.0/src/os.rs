//! Platform-specific functionality.

#[cfg(all(
    any(
        target_vendor = "apple",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "dragonfly",
    ),
    not(polling_test_poll_backend),
))]
pub mod kqueue;

#[cfg(target_os = "windows")]
pub mod iocp;

mod __private {
    #[doc(hidden)]
    #[allow(dead_code)]
    pub trait PollerSealed {}

    impl PollerSealed for crate::Poller {}
}
