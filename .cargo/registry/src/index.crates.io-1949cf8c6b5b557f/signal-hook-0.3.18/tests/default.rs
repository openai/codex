//! Check the hack of SIG_DFL for windows.
//!
//! Libc doesn't export SIG_DFL on windows. It seems to be 0 on all platforms, though, but just to
//! make sure, we observe it is so. We try to read the previous signal on startup and it must be
//! the default.

extern crate libc;

use libc::{sighandler_t, signal, SIGTERM};

const SIG_DFL: sighandler_t = 0;

#[test]
fn sig_dfl() {
    unsafe {
        let prev = signal(SIGTERM, SIG_DFL);
        assert_eq!(SIG_DFL, prev);
    }
}

#[cfg(not(windows))]
#[test]
fn sig_dfl_static() {
    assert_eq!(::libc::SIG_DFL, SIG_DFL);
}
