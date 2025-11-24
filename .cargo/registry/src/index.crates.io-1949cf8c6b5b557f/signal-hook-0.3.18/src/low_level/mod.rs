//! Some low level utilities
//!
//! More often to build other abstractions than used directly.

use std::io::Error;

use libc::c_int;

#[cfg(feature = "channel")]
#[cfg_attr(docsrs, doc(cfg(feature = "channel")))]
pub mod channel;
#[cfg(not(windows))]
#[cfg_attr(docsrs, doc(cfg(not(windows))))]
pub mod pipe;
#[cfg(feature = "extended-siginfo-raw")]
#[cfg_attr(docsrs, doc(cfg(feature = "extended-siginfo-raw")))]
pub mod siginfo;
mod signal_details;

pub use signal_hook_registry::{register, unregister};

pub use self::signal_details::{emulate_default_handler, signal_name};

/// The usual raise, just the safe wrapper around it.
///
/// This is async-signal-safe.
pub fn raise(sig: c_int) -> Result<(), Error> {
    let result = unsafe { libc::raise(sig) };
    if result == -1 {
        Err(Error::last_os_error())
    } else {
        Ok(())
    }
}

/// A bare libc abort.
///
/// Unlike the [std::process::abort], this one is guaranteed to contain no additions or wrappers
/// and therefore is async-signal-safe. You can use this to terminate the application from within a
/// signal handler.
pub fn abort() -> ! {
    unsafe {
        libc::abort();
    }
}

/// A bare libc exit.
///
/// Unlike the [std::process::exit], this one is guaranteed to contain no additions or wrappers and
/// therefore is async-signal-safe. You can use this to terminate the application from within a
/// signal handler.
///
/// Also, see [`register_conditional_shutdown`][crate::flag::register_conditional_shutdown].
pub fn exit(status: c_int) -> ! {
    unsafe {
        // Yes, the one with underscore. That one doesn't call the at-exit hooks.
        libc::_exit(status);
    }
}
