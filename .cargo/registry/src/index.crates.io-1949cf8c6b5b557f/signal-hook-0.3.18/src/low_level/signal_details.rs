//! Providing auxiliary information for signals.

use std::io::Error;
use std::mem;
use std::ptr;

use libc::{c_int, EINVAL};
#[cfg(not(windows))]
use libc::{sigset_t, SIG_UNBLOCK};

use crate::consts::signal::*;
use crate::low_level;

#[derive(Clone, Copy, Debug)]
enum DefaultKind {
    Ignore,
    #[cfg(not(windows))]
    Stop,
    Term,
}

struct Details {
    signal: c_int,
    name: &'static str,
    default_kind: DefaultKind,
}

macro_rules! s {
    ($name: expr, $kind: ident) => {
        Details {
            signal: $name,
            name: stringify!($name),
            default_kind: DefaultKind::$kind,
        }
    };
}

#[cfg(not(windows))]
const DETAILS: &[Details] = &[
    s!(SIGABRT, Term),
    s!(SIGALRM, Term),
    s!(SIGBUS, Term),
    s!(SIGCHLD, Ignore),
    // Technically, continue the process... but this is not done *by* the process.
    s!(SIGCONT, Ignore),
    s!(SIGFPE, Term),
    s!(SIGHUP, Term),
    s!(SIGILL, Term),
    s!(SIGINT, Term),
    #[cfg(any(
        target_os = "freebsd",
        target_os = "dragonfly",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "macos"
    ))]
    s!(SIGINFO, Ignore),
    #[cfg(not(target_os = "haiku"))]
    s!(SIGIO, Ignore),
    // Can't override anyway, but...
    s!(SIGKILL, Term),
    s!(SIGPIPE, Term),
    s!(SIGPROF, Term),
    s!(SIGQUIT, Term),
    s!(SIGSEGV, Term),
    // Can't override anyway, but...
    s!(SIGSTOP, Stop),
    s!(SIGSYS, Term),
    s!(SIGTERM, Term),
    s!(SIGTRAP, Term),
    s!(SIGTSTP, Stop),
    s!(SIGTTIN, Stop),
    s!(SIGTTOU, Stop),
    s!(SIGURG, Ignore),
    s!(SIGUSR1, Term),
    s!(SIGUSR2, Term),
    s!(SIGVTALRM, Term),
    s!(SIGWINCH, Ignore),
    s!(SIGXCPU, Term),
    s!(SIGXFSZ, Term),
];

#[cfg(windows)]
const DETAILS: &[Details] = &[
    s!(SIGABRT, Term),
    s!(SIGFPE, Term),
    s!(SIGILL, Term),
    s!(SIGINT, Term),
    s!(SIGSEGV, Term),
    s!(SIGTERM, Term),
];

/// Provides a human-readable name of a signal.
///
/// Note that the name does not have to be known (in case it is some less common, or non-standard
/// signal).
///
/// # Examples
///
/// ```
/// # use signal_hook::low_level::signal_name;
/// assert_eq!("SIGKILL", signal_name(9).unwrap());
/// assert!(signal_name(142).is_none());
/// ```
pub fn signal_name(signal: c_int) -> Option<&'static str> {
    DETAILS.iter().find(|d| d.signal == signal).map(|d| d.name)
}

#[cfg(not(windows))]
fn restore_default(signal: c_int) -> Result<(), Error> {
    unsafe {
        // A C structure, supposed to be memset to 0 before use.
        let mut action: libc::sigaction = mem::zeroed();
        action.sa_sigaction = libc::SIG_DFL as _;
        if libc::sigaction(signal, &action, ptr::null_mut()) == 0 {
            Ok(())
        } else {
            Err(Error::last_os_error())
        }
    }
}

#[cfg(windows)]
fn restore_default(signal: c_int) -> Result<(), Error> {
    unsafe {
        // SIG_DFL = 0, but not in libc :-(
        if libc::signal(signal, 0) == 0 {
            Ok(())
        } else {
            Err(Error::last_os_error())
        }
    }
}

/// Emulates the behaviour of a default handler for the provided signal.
///
/// This function does its best to provide the same action as the default handler would do, without
/// disrupting the rest of the handling of such signal in the application. It is also
/// async-signal-safe.
///
/// This function necessarily looks up the appropriate action in a table. That means it is possible
/// your system has a signal that is not known to this function. In such case an error is returned
/// (equivalent of `EINVAL`).
///
/// See also the [`register_conditional_default`][crate::flag::register_conditional_default].
///
/// # Warning
///
/// There's a short race condition in case of signals that terminate (either with or without a core
/// dump). The emulation first resets the signal handler back to default (as the application is
/// going to end, it's not a problem) and invokes it. But if some other thread installs a signal
/// handler in the meantime (without assistance from `signal-hook`), it can happen this will be
/// invoked by the re-raised signal.
///
/// This function will still terminate the application (there's a fallback on `abort`), the risk is
/// invoking the newly installed signal handler. Note that manipulating the low-level signals is
/// always racy in a multi-threaded program, therefore the described situation is already
/// discouraged.
///
/// If you are uneasy about such race condition, the recommendation is to run relevant termination
/// routine manually ([`exit`][super::exit] or [`abort`][super::abort]); they always do what they
/// say, but slightly differ in externally observable behaviour from termination by a signal (the
/// exit code will specify that the application exited, not that it terminated with a signal in the
/// first case, and `abort` terminates on `SIGABRT`, so the detected termination signal may be
/// different).
pub fn emulate_default_handler(signal: c_int) -> Result<(), Error> {
    #[cfg(not(windows))]
    {
        if signal == SIGSTOP || signal == SIGKILL {
            return low_level::raise(signal);
        }
    }
    let kind = DETAILS
        .iter()
        .find(|d| d.signal == signal)
        .map(|d| d.default_kind)
        .ok_or_else(|| Error::from_raw_os_error(EINVAL))?;
    match kind {
        DefaultKind::Ignore => Ok(()),
        #[cfg(not(windows))]
        DefaultKind::Stop => low_level::raise(SIGSTOP),
        DefaultKind::Term => {
            if let Ok(()) = restore_default(signal) {
                #[cfg(not(windows))]
                unsafe {
                    #[allow(deprecated)]
                    let mut newsigs: sigset_t = mem::zeroed();

                    // Some android versions don't have the sigemptyset and sigaddset.
                    // Unfortunately, we don't have an access to the android _version_. We just
                    // know that 64bit versions are all OK, so this is a best-effort guess.
                    //
                    // For the affected/guessed versions, we provide our own implementation. We
                    // hope it to be correct (it's inspired by a libc implementation and we assume
                    // the kernel uses the same format ‒ it's unlikely to be different both because
                    // of compatibility and because there's really nothing to invent about a
                    // bitarray).
                    //
                    // We use the proper way for other systems.
                    #[cfg(all(target_os = "android", target_pointer_width = "32"))]
                    unsafe fn prepare_sigset(set: *mut sigset_t, mut signal: c_int) {
                        signal -= 1;
                        let set_raw: *mut libc::c_ulong = set.cast();
                        let size = mem::size_of::<libc::c_ulong>();
                        assert_eq!(set_raw as usize % mem::align_of::<libc::c_ulong>(), 0);
                        let pos = signal as usize / size;
                        assert!(pos < mem::size_of::<sigset_t>() / size);
                        let bit = 1 << (signal as usize % size);
                        set_raw.add(pos).write(bit);
                    }

                    #[cfg(not(all(target_os = "android", target_pointer_width = "32")))]
                    unsafe fn prepare_sigset(set: *mut sigset_t, signal: c_int) {
                        libc::sigemptyset(set);
                        libc::sigaddset(set, signal);
                    }

                    prepare_sigset(&mut newsigs, signal);
                    // Ignore the result, if it doesn't work, we try anyway
                    // Also, sigprocmask is unspecified, but available on more systems. And we want
                    // to just enable _something_. And if it doesn't work, we'll terminate
                    // anyway... It's not UB, so we are good.
                    libc::sigprocmask(SIG_UNBLOCK, &newsigs, ptr::null_mut());
                }
                let _ = low_level::raise(signal);
            }
            // Fallback if anything failed or someone managed to put some other action in in
            // between.
            unsafe { libc::abort() }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn existing() {
        assert_eq!("SIGTERM", signal_name(SIGTERM).unwrap());
    }

    #[test]
    fn unknown() {
        assert!(signal_name(128).is_none());
    }
}
