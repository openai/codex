//! Module for actions setting flags.
//!
//! This contains helper functions to set flags whenever a signal happens. The flags are atomic
//! bools or numbers and the library manipulates them with the `SeqCst` ordering, in case someone
//! cares about relative order to some *other* atomic variables. If you don't care about the
//! relative order, you are free to use `Ordering::Relaxed` when reading and resetting the flags.
//!
//! # When to use
//!
//! The flags in this module allow for polling if a signal arrived since the previous poll. The do
//! not allow blocking until something arrives.
//!
//! Therefore, the natural way to use them is in applications that have some kind of iterative work
//! with both some upper and lower time limit on one iteration. If one iteration could block for
//! arbitrary time, the handling of the signal would be postponed for a long time. If the iteration
//! didn't block at all, the checking for the signal would turn into a busy-loop.
//!
//! If what you need is blocking until a signal comes, you might find better tools in the
//! [`pipe`][crate::low_level::pipe] and [`iterator`][crate::iterator] modules.
//!
//! # Examples
//!
//! Doing something until terminated. This also knows by which signal it was terminated. In case
//! multiple termination signals arrive before it is handled, it recognizes the last one.
//!
//! ```rust
//! use std::io::Error;
//! use std::sync::Arc;
//! use std::sync::atomic::{AtomicUsize, Ordering};
//!
//! use signal_hook::consts::signal::*;
//! use signal_hook::flag as signal_flag;
//!
//! fn main() -> Result<(), Error> {
//!     let term = Arc::new(AtomicUsize::new(0));
//!     const SIGTERM_U: usize = SIGTERM as usize;
//!     const SIGINT_U: usize = SIGINT as usize;
//! #   #[cfg(not(windows))]
//!     const SIGQUIT_U: usize = SIGQUIT as usize;
//!     signal_flag::register_usize(SIGTERM, Arc::clone(&term), SIGTERM_U)?;
//!     signal_flag::register_usize(SIGINT, Arc::clone(&term), SIGINT_U)?;
//! #   #[cfg(not(windows))]
//!     signal_flag::register_usize(SIGQUIT, Arc::clone(&term), SIGQUIT_U)?;
//!
//! #   // Hack to terminate the example when run as a doc-test.
//! #   term.store(SIGTERM_U, Ordering::Relaxed);
//!     loop {
//!         match term.load(Ordering::Relaxed) {
//!             0 => {
//!                 // Do some useful stuff here
//!             }
//!             SIGTERM_U => {
//!                 eprintln!("Terminating on the TERM signal");
//!                 break;
//!             }
//!             SIGINT_U => {
//!                 eprintln!("Terminating on the INT signal");
//!                 break;
//!             }
//! #           #[cfg(not(windows))]
//!             SIGQUIT_U => {
//!                 eprintln!("Terminating on the QUIT signal");
//!                 break;
//!             }
//!             _ => unreachable!(),
//!         }
//!     }
//!
//!     Ok(())
//! }
//! ```
//!
//! Sending a signal to self and seeing it arrived (not of a practical usage on itself):
//!
//! ```rust
//! use std::io::Error;
//! use std::sync::Arc;
//! use std::sync::atomic::{AtomicBool, Ordering};
//! use std::thread;
//! use std::time::Duration;
//!
//! use signal_hook::consts::signal::*;
//! use signal_hook::low_level::raise;
//!
//! fn main() -> Result<(), Error> {
//!     let got = Arc::new(AtomicBool::new(false));
//! #   #[cfg(not(windows))]
//!     signal_hook::flag::register(SIGUSR1, Arc::clone(&got))?;
//! #   #[cfg(windows)]
//! #   signal_hook::flag::register(SIGTERM, Arc::clone(&got))?;
//! #   #[cfg(not(windows))]
//!     raise(SIGUSR1).unwrap();
//! #   #[cfg(windows)]
//! #   raise(SIGTERM).unwrap();
//!     // A sleep here, because it could run the signal handler in another thread and we may not
//!     // see the flag right away. This is still a hack and not guaranteed to work, it is just an
//!     // example!
//!     thread::sleep(Duration::from_secs(1));
//!     assert!(got.load(Ordering::Relaxed));
//!     Ok(())
//! }
//! ```
//!
//! Reloading a configuration on `SIGHUP` (which is a common behaviour of many UNIX daemons,
//! together with reopening the log file).
//!
//! ```rust
//! use std::io::Error;
//! use std::sync::Arc;
//! use std::sync::atomic::{AtomicBool, Ordering};
//!
//! use signal_hook::consts::signal::*;
//! use signal_hook::flag as signal_flag;
//!
//! fn main() -> Result<(), Error> {
//!     // We start with true, to load the configuration in the very first iteration too.
//!     let reload = Arc::new(AtomicBool::new(true));
//!     let term = Arc::new(AtomicBool::new(false));
//! #   #[cfg(not(windows))]
//!     signal_flag::register(SIGHUP, Arc::clone(&reload))?;
//!     signal_flag::register(SIGINT, Arc::clone(&term))?;
//!     signal_flag::register(SIGTERM, Arc::clone(&term))?;
//! #   #[cfg(not(windows))]
//!     signal_flag::register(SIGQUIT, Arc::clone(&term))?;
//!     while !term.load(Ordering::Relaxed) {
//!         // Using swap here, not load, to reset it back to false once it is reloaded.
//!         if reload.swap(false, Ordering::Relaxed) {
//!             // Reload the config here
//! #
//! #           // Hiden hack to make the example terminate when run as doc-test. Not part of the
//! #           // real code.
//! #           term.store(true, Ordering::Relaxed);
//!         }
//!         // Serve one request
//!     }
//!     Ok(())
//! }
//! ```

use std::io::Error;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use libc::{c_int, EINVAL};

use crate::{low_level, SigId};

/// Registers an action to set the flag to `true` whenever the given signal arrives.
///
/// # Panics
///
/// If the signal is one of the forbidden.
pub fn register(signal: c_int, flag: Arc<AtomicBool>) -> Result<SigId, Error> {
    // We use SeqCst for two reasons:
    // * Signals should not come very often, so the performance does not really matter.
    // * We promise the order of actions, but setting different atomics with Relaxed or similar
    //   would not guarantee the effective order.
    unsafe { low_level::register(signal, move || flag.store(true, Ordering::SeqCst)) }
}

/// Registers an action to set the flag to the given value whenever the signal arrives.
pub fn register_usize(signal: c_int, flag: Arc<AtomicUsize>, value: usize) -> Result<SigId, Error> {
    unsafe { low_level::register(signal, move || flag.store(value, Ordering::SeqCst)) }
}

/// Terminate the application on a signal if the given condition is true.
///
/// This can be used for different use cases. One of them (with the condition being always true) is
/// just unconditionally terminate on the given signal.
///
/// Another is being able to turn on and off the behaviour by the shared flag.
///
/// The last one is handling double CTRL+C ‒ if the user presses CTRL+C, we would like to start a
/// graceful shutdown. But if anything ever gets stuck in the shutdown, second CTRL+C (or other
/// such termination signal) should terminate the application without further delay.
///
/// To do that, one can combine this with [`register`]. On the first run, the flag is `false` and
/// this doesn't terminate. But then the flag is set to true during the first run and „arms“ the
/// shutdown on the second run. Note that it matters in which order the actions are registered (the
/// shutdown must go first). And yes, this also allows asking the user „Do you want to terminate“
/// and disarming the abrupt shutdown if the user answers „No“.
///
/// # Panics
///
/// If the signal is one of the forbidden.
pub fn register_conditional_shutdown(
    signal: c_int,
    status: c_int,
    condition: Arc<AtomicBool>,
) -> Result<SigId, Error> {
    let action = move || {
        if condition.load(Ordering::SeqCst) {
            low_level::exit(status);
        }
    };
    unsafe { low_level::register(signal, action) }
}

/// Conditionally runs an emulation of the default action on the given signal.
///
/// If the provided condition is true at the time of invoking the signal handler, the equivalent of
/// the default action of the given signal is run. It is a bit similar to
/// [`register_conditional_shutdown`], except that it doesn't terminate for non-termination
/// signals, it runs their default handler.
///
/// # Panics
///
/// If the signal is one of the forbidden
///
/// # Errors
///
/// Similarly to the [`emulate_default_handler`][low_level::emulate_default_handler] function, this
/// one looks the signal up in a table. If it is unknown, an error is returned.
///
/// Additionally to that, any errors that can be caused by a registration of a handler can happen
/// too.
pub fn register_conditional_default(
    signal: c_int,
    condition: Arc<AtomicBool>,
) -> Result<SigId, Error> {
    // Verify we know about this particular signal.
    low_level::signal_name(signal).ok_or_else(|| Error::from_raw_os_error(EINVAL))?;
    let action = move || {
        if condition.load(Ordering::SeqCst) {
            let _ = low_level::emulate_default_handler(signal);
        }
    };
    unsafe { low_level::register(signal, action) }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic;
    use std::time::{Duration, Instant};

    use super::*;
    use crate::consts::signal::*;

    fn self_signal() {
        #[cfg(not(windows))]
        const SIG: c_int = SIGUSR1;
        #[cfg(windows)]
        const SIG: c_int = SIGTERM;
        crate::low_level::raise(SIG).unwrap();
    }

    fn wait_flag(flag: &AtomicBool) -> bool {
        let start = Instant::now();
        while !flag.load(Ordering::Relaxed) {
            // Replaced by hint::spin_loop, but we want to support older compiler
            #[allow(deprecated)]
            atomic::spin_loop_hint();
            if Instant::now() - start > Duration::from_secs(1) {
                // We reached a timeout and nothing happened yet.
                // In theory, using timeouts for thread-synchronization tests is wrong, but a
                // second should be enough in practice.
                return false;
            }
        }
        true
    }

    #[test]
    fn register_unregister() {
        // When we register the action, it is active.
        let flag = Arc::new(AtomicBool::new(false));
        #[cfg(not(windows))]
        let signal = register(SIGUSR1, Arc::clone(&flag)).unwrap();
        #[cfg(windows)]
        let signal = register(crate::SIGTERM, Arc::clone(&flag)).unwrap();
        self_signal();
        assert!(wait_flag(&flag));
        // But stops working after it is unregistered.
        assert!(crate::low_level::unregister(signal));
        flag.store(false, Ordering::Relaxed);
        self_signal();
        assert!(!wait_flag(&flag));
        // And the unregistration actually dropped its copy of the Arc
        assert_eq!(1, Arc::strong_count(&flag));
    }

    // The shutdown is tested in tests/shutdown.rs
}
