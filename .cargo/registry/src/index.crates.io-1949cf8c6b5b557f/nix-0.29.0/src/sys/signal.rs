// Portions of this file are Copyright 2014 The Rust Project Developers.
// See https://www.rust-lang.org/policies/licenses.

//! Operating system signals.

use crate::errno::Errno;
use crate::{Error, Result};
use cfg_if::cfg_if;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::mem;
use std::ops::BitOr;
#[cfg(freebsdlike)]
use std::os::unix::io::RawFd;
use std::ptr;
use std::str::FromStr;

#[cfg(not(any(
    target_os = "fuchsia",
    target_os = "hurd",
    target_os = "openbsd",
    target_os = "redox"
)))]
#[cfg(any(feature = "aio", feature = "signal"))]
pub use self::sigevent::*;

#[cfg(any(feature = "aio", feature = "process", feature = "signal"))]
libc_enum! {
    /// Types of operating system signals
    // Currently there is only one definition of c_int in libc, as well as only one
    // type for signal constants.
    // We would prefer to use the libc::c_int alias in the repr attribute. Unfortunately
    // this is not (yet) possible.
    #[repr(i32)]
    #[non_exhaustive]
    #[cfg_attr(docsrs, doc(cfg(any(feature = "aio", feature = "signal"))))]
    pub enum Signal {
        /// Hangup
        SIGHUP,
        /// Interrupt
        SIGINT,
        /// Quit
        SIGQUIT,
        /// Illegal instruction (not reset when caught)
        SIGILL,
        /// Trace trap (not reset when caught)
        SIGTRAP,
        /// Abort
        SIGABRT,
        /// Bus error
        SIGBUS,
        /// Floating point exception
        SIGFPE,
        /// Kill (cannot be caught or ignored)
        SIGKILL,
        /// User defined signal 1
        SIGUSR1,
        /// Segmentation violation
        SIGSEGV,
        /// User defined signal 2
        SIGUSR2,
        /// Write on a pipe with no one to read it
        SIGPIPE,
        /// Alarm clock
        SIGALRM,
        /// Software termination signal from kill
        SIGTERM,
        /// Stack fault (obsolete)
        #[cfg(all(any(linux_android, target_os = "emscripten",
                      target_os = "fuchsia"),
                  not(any(target_arch = "mips",
                          target_arch = "mips32r6",
                          target_arch = "mips64",
                          target_arch = "mips64r6",
                          target_arch = "sparc64"))))]
        SIGSTKFLT,
        /// To parent on child stop or exit
        SIGCHLD,
        /// Continue a stopped process
        SIGCONT,
        /// Sendable stop signal not from tty
        SIGSTOP,
        /// Stop signal from tty
        SIGTSTP,
        /// To readers pgrp upon background tty read
        SIGTTIN,
        /// Like TTIN if (tp->t_local&LTOSTOP)
        SIGTTOU,
        /// Urgent condition on IO channel
        SIGURG,
        /// Exceeded CPU time limit
        SIGXCPU,
        /// Exceeded file size limit
        SIGXFSZ,
        /// Virtual time alarm
        SIGVTALRM,
        /// Profiling time alarm
        SIGPROF,
        /// Window size changes
        SIGWINCH,
        /// Input/output possible signal
        #[cfg(not(target_os = "haiku"))]
        SIGIO,
        #[cfg(any(linux_android, target_os = "emscripten",
                  target_os = "fuchsia", target_os = "aix"))]
        /// Power failure imminent.
        SIGPWR,
        /// Bad system call
        SIGSYS,
        #[cfg(not(any(linux_android, target_os = "emscripten",
                      target_os = "fuchsia",
                      target_os = "redox", target_os = "haiku")))]
        /// Emulator trap
        SIGEMT,
        #[cfg(not(any(linux_android, target_os = "emscripten",
                      target_os = "fuchsia", target_os = "redox",
                      target_os = "haiku", target_os = "aix")))]
        /// Information request
        SIGINFO,
    }
    impl TryFrom<i32>
}

#[cfg(feature = "signal")]
impl FromStr for Signal {
    type Err = Error;
    fn from_str(s: &str) -> Result<Signal> {
        Ok(match s {
            "SIGHUP" => Signal::SIGHUP,
            "SIGINT" => Signal::SIGINT,
            "SIGQUIT" => Signal::SIGQUIT,
            "SIGILL" => Signal::SIGILL,
            "SIGTRAP" => Signal::SIGTRAP,
            "SIGABRT" => Signal::SIGABRT,
            "SIGBUS" => Signal::SIGBUS,
            "SIGFPE" => Signal::SIGFPE,
            "SIGKILL" => Signal::SIGKILL,
            "SIGUSR1" => Signal::SIGUSR1,
            "SIGSEGV" => Signal::SIGSEGV,
            "SIGUSR2" => Signal::SIGUSR2,
            "SIGPIPE" => Signal::SIGPIPE,
            "SIGALRM" => Signal::SIGALRM,
            "SIGTERM" => Signal::SIGTERM,
            #[cfg(all(
                any(
                    linux_android,
                    target_os = "emscripten",
                    target_os = "fuchsia",
                ),
                not(any(
                    target_arch = "mips",
                    target_arch = "mips32r6",
                    target_arch = "mips64",
                    target_arch = "mips64r6",
                    target_arch = "sparc64"
                ))
            ))]
            "SIGSTKFLT" => Signal::SIGSTKFLT,
            "SIGCHLD" => Signal::SIGCHLD,
            "SIGCONT" => Signal::SIGCONT,
            "SIGSTOP" => Signal::SIGSTOP,
            "SIGTSTP" => Signal::SIGTSTP,
            "SIGTTIN" => Signal::SIGTTIN,
            "SIGTTOU" => Signal::SIGTTOU,
            "SIGURG" => Signal::SIGURG,
            "SIGXCPU" => Signal::SIGXCPU,
            "SIGXFSZ" => Signal::SIGXFSZ,
            "SIGVTALRM" => Signal::SIGVTALRM,
            "SIGPROF" => Signal::SIGPROF,
            "SIGWINCH" => Signal::SIGWINCH,
            #[cfg(not(target_os = "haiku"))]
            "SIGIO" => Signal::SIGIO,
            #[cfg(any(
                linux_android,
                target_os = "emscripten",
                target_os = "fuchsia",
            ))]
            "SIGPWR" => Signal::SIGPWR,
            "SIGSYS" => Signal::SIGSYS,
            #[cfg(not(any(
                linux_android,
                target_os = "emscripten",
                target_os = "fuchsia",
                target_os = "redox",
                target_os = "haiku"
            )))]
            "SIGEMT" => Signal::SIGEMT,
            #[cfg(not(any(
                linux_android,
                target_os = "emscripten",
                target_os = "fuchsia",
                target_os = "redox",
                target_os = "aix",
                target_os = "haiku"
            )))]
            "SIGINFO" => Signal::SIGINFO,
            _ => return Err(Errno::EINVAL),
        })
    }
}

#[cfg(feature = "signal")]
impl Signal {
    /// Returns name of signal.
    ///
    /// This function is equivalent to `<Signal as AsRef<str>>::as_ref()`,
    /// with difference that returned string is `'static`
    /// and not bound to `self`'s lifetime.
    pub const fn as_str(self) -> &'static str {
        match self {
            Signal::SIGHUP => "SIGHUP",
            Signal::SIGINT => "SIGINT",
            Signal::SIGQUIT => "SIGQUIT",
            Signal::SIGILL => "SIGILL",
            Signal::SIGTRAP => "SIGTRAP",
            Signal::SIGABRT => "SIGABRT",
            Signal::SIGBUS => "SIGBUS",
            Signal::SIGFPE => "SIGFPE",
            Signal::SIGKILL => "SIGKILL",
            Signal::SIGUSR1 => "SIGUSR1",
            Signal::SIGSEGV => "SIGSEGV",
            Signal::SIGUSR2 => "SIGUSR2",
            Signal::SIGPIPE => "SIGPIPE",
            Signal::SIGALRM => "SIGALRM",
            Signal::SIGTERM => "SIGTERM",
            #[cfg(all(
                any(
                    linux_android,
                    target_os = "emscripten",
                    target_os = "fuchsia",
                ),
                not(any(
                    target_arch = "mips",
                    target_arch = "mips32r6",
                    target_arch = "mips64",
                    target_arch = "mips64r6",
                    target_arch = "sparc64"
                ))
            ))]
            Signal::SIGSTKFLT => "SIGSTKFLT",
            Signal::SIGCHLD => "SIGCHLD",
            Signal::SIGCONT => "SIGCONT",
            Signal::SIGSTOP => "SIGSTOP",
            Signal::SIGTSTP => "SIGTSTP",
            Signal::SIGTTIN => "SIGTTIN",
            Signal::SIGTTOU => "SIGTTOU",
            Signal::SIGURG => "SIGURG",
            Signal::SIGXCPU => "SIGXCPU",
            Signal::SIGXFSZ => "SIGXFSZ",
            Signal::SIGVTALRM => "SIGVTALRM",
            Signal::SIGPROF => "SIGPROF",
            Signal::SIGWINCH => "SIGWINCH",
            #[cfg(not(target_os = "haiku"))]
            Signal::SIGIO => "SIGIO",
            #[cfg(any(
                linux_android,
                target_os = "emscripten",
                target_os = "fuchsia",
                target_os = "aix",
            ))]
            Signal::SIGPWR => "SIGPWR",
            Signal::SIGSYS => "SIGSYS",
            #[cfg(not(any(
                linux_android,
                target_os = "emscripten",
                target_os = "fuchsia",
                target_os = "redox",
                target_os = "haiku"
            )))]
            Signal::SIGEMT => "SIGEMT",
            #[cfg(not(any(
                linux_android,
                target_os = "emscripten",
                target_os = "fuchsia",
                target_os = "redox",
                target_os = "aix",
                target_os = "haiku"
            )))]
            Signal::SIGINFO => "SIGINFO",
        }
    }
}

#[cfg(feature = "signal")]
impl AsRef<str> for Signal {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

#[cfg(feature = "signal")]
impl fmt::Display for Signal {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.as_ref())
    }
}

#[cfg(feature = "signal")]
pub use self::Signal::*;

#[cfg(target_os = "redox")]
#[cfg(feature = "signal")]
const SIGNALS: [Signal; 29] = [
    SIGHUP, SIGINT, SIGQUIT, SIGILL, SIGTRAP, SIGABRT, SIGBUS, SIGFPE, SIGKILL,
    SIGUSR1, SIGSEGV, SIGUSR2, SIGPIPE, SIGALRM, SIGTERM, SIGCHLD, SIGCONT,
    SIGSTOP, SIGTSTP, SIGTTIN, SIGTTOU, SIGURG, SIGXCPU, SIGXFSZ, SIGVTALRM,
    SIGPROF, SIGWINCH, SIGIO, SIGSYS,
];
#[cfg(target_os = "haiku")]
#[cfg(feature = "signal")]
const SIGNALS: [Signal; 28] = [
    SIGHUP, SIGINT, SIGQUIT, SIGILL, SIGTRAP, SIGABRT, SIGBUS, SIGFPE, SIGKILL,
    SIGUSR1, SIGSEGV, SIGUSR2, SIGPIPE, SIGALRM, SIGTERM, SIGCHLD, SIGCONT,
    SIGSTOP, SIGTSTP, SIGTTIN, SIGTTOU, SIGURG, SIGXCPU, SIGXFSZ, SIGVTALRM,
    SIGPROF, SIGWINCH, SIGSYS,
];
#[cfg(all(
    any(linux_android, target_os = "emscripten", target_os = "fuchsia"),
    not(any(
        target_arch = "mips",
        target_arch = "mips32r6",
        target_arch = "mips64",
        target_arch = "mips64r6",
        target_arch = "sparc64"
    ))
))]
#[cfg(feature = "signal")]
const SIGNALS: [Signal; 31] = [
    SIGHUP, SIGINT, SIGQUIT, SIGILL, SIGTRAP, SIGABRT, SIGBUS, SIGFPE, SIGKILL,
    SIGUSR1, SIGSEGV, SIGUSR2, SIGPIPE, SIGALRM, SIGTERM, SIGSTKFLT, SIGCHLD,
    SIGCONT, SIGSTOP, SIGTSTP, SIGTTIN, SIGTTOU, SIGURG, SIGXCPU, SIGXFSZ,
    SIGVTALRM, SIGPROF, SIGWINCH, SIGIO, SIGPWR, SIGSYS,
];
#[cfg(all(
    any(linux_android, target_os = "emscripten", target_os = "fuchsia"),
    any(
        target_arch = "mips",
        target_arch = "mips32r6",
        target_arch = "mips64",
        target_arch = "mips64r6",
        target_arch = "sparc64"
    )
))]
#[cfg(feature = "signal")]
const SIGNALS: [Signal; 30] = [
    SIGHUP, SIGINT, SIGQUIT, SIGILL, SIGTRAP, SIGABRT, SIGBUS, SIGFPE, SIGKILL,
    SIGUSR1, SIGSEGV, SIGUSR2, SIGPIPE, SIGALRM, SIGTERM, SIGCHLD, SIGCONT,
    SIGSTOP, SIGTSTP, SIGTTIN, SIGTTOU, SIGURG, SIGXCPU, SIGXFSZ, SIGVTALRM,
    SIGPROF, SIGWINCH, SIGIO, SIGPWR, SIGSYS,
];
#[cfg(target_os = "aix")]
#[cfg(feature = "signal")]
const SIGNALS: [Signal; 30] = [
    SIGHUP, SIGINT, SIGQUIT, SIGILL, SIGABRT, SIGEMT, SIGFPE, SIGKILL, SIGSEGV,
    SIGSYS, SIGPIPE, SIGALRM, SIGTERM, SIGUSR1, SIGUSR2, SIGPWR, SIGWINCH,
    SIGURG, SIGPOLL, SIGIO, SIGSTOP, SIGTSTP, SIGCONT, SIGTTIN, SIGTTOU,
    SIGVTALRM, SIGPROF, SIGXCPU, SIGXFSZ, SIGTRAP,
];
#[cfg(not(any(
    linux_android,
    target_os = "fuchsia",
    target_os = "emscripten",
    target_os = "aix",
    target_os = "redox",
    target_os = "haiku"
)))]
#[cfg(feature = "signal")]
const SIGNALS: [Signal; 31] = [
    SIGHUP, SIGINT, SIGQUIT, SIGILL, SIGTRAP, SIGABRT, SIGBUS, SIGFPE, SIGKILL,
    SIGUSR1, SIGSEGV, SIGUSR2, SIGPIPE, SIGALRM, SIGTERM, SIGCHLD, SIGCONT,
    SIGSTOP, SIGTSTP, SIGTTIN, SIGTTOU, SIGURG, SIGXCPU, SIGXFSZ, SIGVTALRM,
    SIGPROF, SIGWINCH, SIGIO, SIGSYS, SIGEMT, SIGINFO,
];

feature! {
#![feature = "signal"]

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
/// Iterate through all signals defined by this operating system
pub struct SignalIterator {
    next: usize,
}

impl Iterator for SignalIterator {
    type Item = Signal;

    fn next(&mut self) -> Option<Signal> {
        if self.next < SIGNALS.len() {
            let next_signal = SIGNALS[self.next];
            self.next += 1;
            Some(next_signal)
        } else {
            None
        }
    }
}

impl Signal {
    /// Iterate through all signals defined by this OS
    pub const fn iterator() -> SignalIterator {
        SignalIterator{next: 0}
    }
}

/// Alias for [`SIGABRT`]
pub const SIGIOT : Signal = SIGABRT;
/// Alias for [`SIGIO`]
#[cfg(not(target_os = "haiku"))]
pub const SIGPOLL : Signal = SIGIO;
/// Alias for [`SIGSYS`]
pub const SIGUNUSED : Signal = SIGSYS;

cfg_if! {
    if #[cfg(target_os = "redox")] {
        type SaFlags_t = libc::c_ulong;
    } else if #[cfg(target_env = "uclibc")] {
        type SaFlags_t = libc::c_ulong;
    } else {
        type SaFlags_t = libc::c_int;
    }
}
}

#[cfg(feature = "signal")]
libc_bitflags! {
    /// Controls the behavior of a [`SigAction`]
    #[cfg_attr(docsrs, doc(cfg(feature = "signal")))]
    pub struct SaFlags: SaFlags_t {
        /// When catching a [`Signal::SIGCHLD`] signal, the signal will be
        /// generated only when a child process exits, not when a child process
        /// stops.
        SA_NOCLDSTOP;
        /// When catching a [`Signal::SIGCHLD`] signal, the system will not
        /// create zombie processes when children of the calling process exit.
        #[cfg(not(target_os = "hurd"))]
        SA_NOCLDWAIT;
        /// Further occurrences of the delivered signal are not masked during
        /// the execution of the handler.
        SA_NODEFER;
        /// The system will deliver the signal to the process on a signal stack,
        /// specified by each thread with sigaltstack(2).
        SA_ONSTACK;
        /// The handler is reset back to the default at the moment the signal is
        /// delivered.
        SA_RESETHAND;
        /// Requests that certain system calls restart if interrupted by this
        /// signal.  See the man page for complete details.
        SA_RESTART;
        /// This flag is controlled internally by Nix.
        SA_SIGINFO;
    }
}

#[cfg(feature = "signal")]
libc_enum! {
    /// Specifies how certain functions should manipulate a signal mask
    #[repr(i32)]
    #[non_exhaustive]
    #[cfg_attr(docsrs, doc(cfg(feature = "signal")))]
    pub enum SigmaskHow {
        /// The new mask is the union of the current mask and the specified set.
        SIG_BLOCK,
        /// The new mask is the intersection of the current mask and the
        /// complement of the specified set.
        SIG_UNBLOCK,
        /// The current mask is replaced by the specified set.
        SIG_SETMASK,
    }
}

feature! {
#![feature = "signal"]

use crate::unistd::Pid;
use std::iter::Extend;
use std::iter::FromIterator;
use std::iter::IntoIterator;

/// Specifies a set of [`Signal`]s that may be blocked, waited for, etc.
// We are using `transparent` here to be super sure that `SigSet`
// is represented exactly like the `sigset_t` struct from C.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq)]
pub struct SigSet {
    sigset: libc::sigset_t
}

impl SigSet {
    /// Initialize to include all signals.
    #[doc(alias("sigfillset"))]
    pub fn all() -> SigSet {
        let mut sigset = mem::MaybeUninit::uninit();
        let _ = unsafe { libc::sigfillset(sigset.as_mut_ptr()) };

        unsafe{ SigSet { sigset: sigset.assume_init() } }
    }

    /// Initialize to include nothing.
    #[doc(alias("sigemptyset"))]
    pub fn empty() -> SigSet {
        let mut sigset = mem::MaybeUninit::uninit();
        let _ = unsafe { libc::sigemptyset(sigset.as_mut_ptr()) };

        unsafe{ SigSet { sigset: sigset.assume_init() } }
    }

    /// Add the specified signal to the set.
    #[doc(alias("sigaddset"))]
    pub fn add(&mut self, signal: Signal) {
        unsafe { libc::sigaddset(&mut self.sigset as *mut libc::sigset_t, signal as libc::c_int) };
    }

    /// Remove all signals from this set.
    #[doc(alias("sigemptyset"))]
    pub fn clear(&mut self) {
        unsafe { libc::sigemptyset(&mut self.sigset as *mut libc::sigset_t) };
    }

    /// Remove the specified signal from this set.
    #[doc(alias("sigdelset"))]
    pub fn remove(&mut self, signal: Signal) {
        unsafe { libc::sigdelset(&mut self.sigset as *mut libc::sigset_t, signal as libc::c_int) };
    }

    /// Return whether this set includes the specified signal.
    #[doc(alias("sigismember"))]
    pub fn contains(&self, signal: Signal) -> bool {
        let res = unsafe { libc::sigismember(&self.sigset as *const libc::sigset_t, signal as libc::c_int) };

        match res {
            1 => true,
            0 => false,
            _ => unreachable!("unexpected value from sigismember"),
        }
    }

    /// Returns an iterator that yields the signals contained in this set.
    pub fn iter(&self) -> SigSetIter<'_> {
        self.into_iter()
    }

    /// Gets the currently blocked (masked) set of signals for the calling thread.
    pub fn thread_get_mask() -> Result<SigSet> {
        let mut oldmask = mem::MaybeUninit::uninit();
        do_pthread_sigmask(SigmaskHow::SIG_SETMASK, None, Some(oldmask.as_mut_ptr()))?;
        Ok(unsafe{ SigSet{sigset: oldmask.assume_init()}})
    }

    /// Sets the set of signals as the signal mask for the calling thread.
    pub fn thread_set_mask(&self) -> Result<()> {
        pthread_sigmask(SigmaskHow::SIG_SETMASK, Some(self), None)
    }

    /// Adds the set of signals to the signal mask for the calling thread.
    pub fn thread_block(&self) -> Result<()> {
        pthread_sigmask(SigmaskHow::SIG_BLOCK, Some(self), None)
    }

    /// Removes the set of signals from the signal mask for the calling thread.
    pub fn thread_unblock(&self) -> Result<()> {
        pthread_sigmask(SigmaskHow::SIG_UNBLOCK, Some(self), None)
    }

    /// Sets the set of signals as the signal mask, and returns the old mask.
    pub fn thread_swap_mask(&self, how: SigmaskHow) -> Result<SigSet> {
        let mut oldmask = mem::MaybeUninit::uninit();
        do_pthread_sigmask(how, Some(self), Some(oldmask.as_mut_ptr()))?;
        Ok(unsafe{ SigSet{sigset: oldmask.assume_init()}})
    }

    /// Suspends execution of the calling thread until one of the signals in the
    /// signal mask becomes pending, and returns the accepted signal.
    #[cfg(not(target_os = "redox"))] // RedoxFS does not yet support sigwait
    pub fn wait(&self) -> Result<Signal> {
        use std::convert::TryFrom;

        let mut signum = mem::MaybeUninit::uninit();
        let res = unsafe { libc::sigwait(&self.sigset as *const libc::sigset_t, signum.as_mut_ptr()) };

        Errno::result(res).map(|_| unsafe {
            Signal::try_from(signum.assume_init()).unwrap()
        })
    }

    /// Wait for a signal
    ///
    /// # Return value
    /// If `sigsuspend(2)` is interrupted (EINTR), this function returns `Ok`.
    /// If `sigsuspend(2)` set other error, this function returns `Err`.
    ///
    /// For more information see the
    /// [`sigsuspend(2)`](https://pubs.opengroup.org/onlinepubs/9699919799/functions/sigsuspend.html).
    #[cfg(any(
        bsd,
        linux_android,
        solarish,
        target_os = "haiku",
        target_os = "hurd",
        target_os = "aix",
        target_os = "fuchsia"
    ))]
    #[doc(alias("sigsuspend"))]
    pub fn suspend(&self) -> Result<()> {
        let res = unsafe {
            libc::sigsuspend(&self.sigset as *const libc::sigset_t)
        };
        match Errno::result(res).map(drop) {
            Err(Errno::EINTR) => Ok(()),
            Err(e) => Err(e),
            Ok(_) => unreachable!("because this syscall always returns -1 if returns"),
        }
    }

    /// Converts a `libc::sigset_t` object to a [`SigSet`] without checking  whether the
    /// `libc::sigset_t` is already initialized.
    ///
    /// # Safety
    ///
    /// The `sigset` passed in must be a valid an initialized `libc::sigset_t` by calling either
    /// [`sigemptyset(3)`](https://man7.org/linux/man-pages/man3/sigemptyset.3p.html) or
    /// [`sigfillset(3)`](https://man7.org/linux/man-pages/man3/sigfillset.3p.html).
    /// Otherwise, the results are undefined.
    pub unsafe fn from_sigset_t_unchecked(sigset: libc::sigset_t) -> SigSet {
        SigSet { sigset }
    }
}

impl From<Signal> for SigSet {
    fn from(signal: Signal) -> SigSet {
        let mut sigset = SigSet::empty();
        sigset.add(signal);
        sigset
    }
}

impl BitOr for Signal {
    type Output = SigSet;

    fn bitor(self, rhs: Self) -> Self::Output {
        let mut sigset = SigSet::empty();
        sigset.add(self);
        sigset.add(rhs);
        sigset
    }
}

impl BitOr<Signal> for SigSet {
    type Output = SigSet;

    fn bitor(mut self, rhs: Signal) -> Self::Output {
        self.add(rhs);
        self
    }
}

impl BitOr for SigSet {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        self.iter().chain(rhs.iter()).collect()
    }
}

impl AsRef<libc::sigset_t> for SigSet {
    fn as_ref(&self) -> &libc::sigset_t {
        &self.sigset
    }
}

// TODO: Consider specialization for the case where T is &SigSet and libc::sigorset is available.
impl Extend<Signal> for SigSet {
    fn extend<T>(&mut self, iter: T)
    where T: IntoIterator<Item = Signal> {
        for signal in iter {
            self.add(signal);
        }
    }
}

impl FromIterator<Signal> for SigSet {
    fn from_iter<T>(iter: T) -> Self
    where T: IntoIterator<Item = Signal> {
        let mut sigset = SigSet::empty();
        sigset.extend(iter);
        sigset
    }
}

impl PartialEq for SigSet {
    fn eq(&self, other: &Self) -> bool {
        for signal in Signal::iterator() {
            if self.contains(signal) != other.contains(signal) {
                return false;
            }
        }
        true
    }
}

impl Hash for SigSet {
    fn hash<H: Hasher>(&self, state: &mut H) {
        for signal in Signal::iterator() {
            if self.contains(signal) {
                signal.hash(state);
            }
        }
    }
}

/// Iterator for a [`SigSet`].
///
/// Call [`SigSet::iter`] to create an iterator.
#[derive(Clone, Debug)]
pub struct SigSetIter<'a> {
    sigset: &'a SigSet,
    inner: SignalIterator,
}

impl Iterator for SigSetIter<'_> {
    type Item = Signal;
    fn next(&mut self) -> Option<Signal> {
        loop {
            match self.inner.next() {
                None => return None,
                Some(signal) if self.sigset.contains(signal) => return Some(signal),
                Some(_signal) => continue,
            }
        }
    }
}

impl<'a> IntoIterator for &'a SigSet {
    type Item = Signal;
    type IntoIter = SigSetIter<'a>;
    fn into_iter(self) -> Self::IntoIter {
        SigSetIter { sigset: self, inner: Signal::iterator() }
    }
}

/// A signal handler.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SigHandler {
    /// Default signal handling.
    SigDfl,
    /// Request that the signal be ignored.
    SigIgn,
    /// Use the given signal-catching function, which takes in the signal.
    Handler(extern fn(libc::c_int)),
    /// Use the given signal-catching function, which takes in the signal, information about how
    /// the signal was generated, and a pointer to the threads `ucontext_t`.
    #[cfg(not(target_os = "redox"))]
    SigAction(extern fn(libc::c_int, *mut libc::siginfo_t, *mut libc::c_void))
}

/// Action to take on receipt of a signal. Corresponds to `sigaction`.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SigAction {
    sigaction: libc::sigaction
}

impl From<SigAction> for libc::sigaction {
    fn from(value: SigAction) -> libc::sigaction {
        value.sigaction
    }
}

impl SigAction {
    /// Creates a new action.
    ///
    /// The `SA_SIGINFO` bit in the `flags` argument is ignored (it will be set only if `handler`
    /// is the `SigAction` variant). `mask` specifies other signals to block during execution of
    /// the signal-catching function.
    pub fn new(handler: SigHandler, flags: SaFlags, mask: SigSet) -> SigAction {
        #[cfg(not(target_os = "aix"))]
        unsafe fn install_sig(p: *mut libc::sigaction, handler: SigHandler) {
            unsafe {
                 (*p).sa_sigaction = match handler {
                    SigHandler::SigDfl => libc::SIG_DFL,
                    SigHandler::SigIgn => libc::SIG_IGN,
                    SigHandler::Handler(f) => f as *const extern fn(libc::c_int) as usize,
                    #[cfg(not(target_os = "redox"))]
                    SigHandler::SigAction(f) => f as *const extern fn(libc::c_int, *mut libc::siginfo_t, *mut libc::c_void) as usize,
                };
            }
        }

        #[cfg(target_os = "aix")]
        unsafe fn install_sig(p: *mut libc::sigaction, handler: SigHandler) {
            unsafe {
                (*p).sa_union.__su_sigaction = match handler {
                    SigHandler::SigDfl => unsafe { mem::transmute::<usize, extern "C" fn(libc::c_int, *mut libc::siginfo_t, *mut libc::c_void)>(libc::SIG_DFL) },
                    SigHandler::SigIgn => unsafe { mem::transmute::<usize, extern "C" fn(libc::c_int, *mut libc::siginfo_t, *mut libc::c_void)>(libc::SIG_IGN) },
                    SigHandler::Handler(f) => unsafe { mem::transmute::<extern "C" fn(i32), extern "C" fn(i32, *mut libc::siginfo_t, *mut libc::c_void)>(f) },
                    SigHandler::SigAction(f) => f,
                };
            }
        }

        let mut s = mem::MaybeUninit::<libc::sigaction>::uninit();
        unsafe {
            let p = s.as_mut_ptr();
            install_sig(p, handler);
            (*p).sa_flags = match handler {
                #[cfg(not(target_os = "redox"))]
                SigHandler::SigAction(_) => (flags | SaFlags::SA_SIGINFO).bits(),
                _ => (flags - SaFlags::SA_SIGINFO).bits(),
            };
            (*p).sa_mask = mask.sigset;

            SigAction { sigaction: s.assume_init() }
        }
    }

    /// Returns the flags set on the action.
    pub fn flags(&self) -> SaFlags {
        SaFlags::from_bits_truncate(self.sigaction.sa_flags)
    }

    /// Returns the set of signals that are blocked during execution of the action's
    /// signal-catching function.
    pub fn mask(&self) -> SigSet {
        SigSet { sigset: self.sigaction.sa_mask }
    }

    /// Returns the action's handler.
    #[cfg(not(target_os = "aix"))]
    pub fn handler(&self) -> SigHandler {
        match self.sigaction.sa_sigaction {
            libc::SIG_DFL => SigHandler::SigDfl,
            libc::SIG_IGN => SigHandler::SigIgn,
            #[cfg(not(target_os = "redox"))]
            p if self.flags().contains(SaFlags::SA_SIGINFO) =>
                SigHandler::SigAction(
                // Safe for one of two reasons:
                // * The SigHandler was created by SigHandler::new, in which
                //   case the pointer is correct, or
                // * The SigHandler was created by signal or sigaction, which
                //   are unsafe functions, so the caller should've somehow
                //   ensured that it is correctly initialized.
                unsafe{
                    *(&p as *const usize
                         as *const extern fn(_, _, _))
                }
                as extern fn(_, _, _)),
            p => SigHandler::Handler(
                // Safe for one of two reasons:
                // * The SigHandler was created by SigHandler::new, in which
                //   case the pointer is correct, or
                // * The SigHandler was created by signal or sigaction, which
                //   are unsafe functions, so the caller should've somehow
                //   ensured that it is correctly initialized.
                unsafe{
                    *(&p as *const usize
                         as *const extern fn(libc::c_int))
                }
                as extern fn(libc::c_int)),
        }
    }

    /// Returns the action's handler.
    #[cfg(target_os = "aix")]
    pub fn handler(&self) -> SigHandler {
        unsafe {
        match self.sigaction.sa_union.__su_sigaction as usize {
            libc::SIG_DFL => SigHandler::SigDfl,
            libc::SIG_IGN => SigHandler::SigIgn,
            p if self.flags().contains(SaFlags::SA_SIGINFO) =>
                SigHandler::SigAction(
                    *(&p as *const usize
                         as *const extern fn(_, _, _))
                as extern fn(_, _, _)),
            p => SigHandler::Handler(
                    *(&p as *const usize
                         as *const extern fn(libc::c_int))
                as extern fn(libc::c_int)),
        }
        }
    }
}

/// Changes the action taken by a process on receipt of a specific signal.
///
/// `signal` can be any signal except `SIGKILL` or `SIGSTOP`. On success, it returns the previous
/// action for the given signal. If `sigaction` fails, no new signal handler is installed.
///
/// # Safety
///
/// * Signal handlers may be called at any point during execution, which limits
///   what is safe to do in the body of the signal-catching function. Be certain
///   to only make syscalls that are explicitly marked safe for signal handlers
///   and only share global data using atomics.
///
/// * There is also no guarantee that the old signal handler was installed
///   correctly.  If it was installed by this crate, it will be.  But if it was
///   installed by, for example, C code, then there is no guarantee its function
///   pointer is valid.  In that case, this function effectively dereferences a
///   raw pointer of unknown provenance.
pub unsafe fn sigaction(signal: Signal, sigaction: &SigAction) -> Result<SigAction> {
    let mut oldact = mem::MaybeUninit::<libc::sigaction>::uninit();

    let res = unsafe { libc::sigaction(signal as libc::c_int,
                              &sigaction.sigaction as *const libc::sigaction,
                              oldact.as_mut_ptr()) };

    Errno::result(res).map(|_| SigAction { sigaction: unsafe { oldact.assume_init() } })
}

/// Signal management (see [signal(3p)](https://pubs.opengroup.org/onlinepubs/9699919799/functions/signal.html))
///
/// Installs `handler` for the given `signal`, returning the previous signal
/// handler. `signal` should only be used following another call to `signal` or
/// if the current handler is the default. The return value of `signal` is
/// undefined after setting the handler with [`sigaction`][SigActionFn].
///
/// # Safety
///
/// If the pointer to the previous signal handler is invalid, undefined
/// behavior could be invoked when casting it back to a [`SigAction`][SigActionStruct].
///
/// # Examples
///
/// Ignore `SIGINT`:
///
/// ```no_run
/// # use nix::sys::signal::{self, Signal, SigHandler};
/// unsafe { signal::signal(Signal::SIGINT, SigHandler::SigIgn) }.unwrap();
/// ```
///
/// Use a signal handler to set a flag variable:
///
/// ```no_run
/// # use std::convert::TryFrom;
/// # use std::sync::atomic::{AtomicBool, Ordering};
/// # use nix::sys::signal::{self, Signal, SigHandler};
/// static SIGNALED: AtomicBool = AtomicBool::new(false);
///
/// extern fn handle_sigint(signal: libc::c_int) {
///     let signal = Signal::try_from(signal).unwrap();
///     SIGNALED.store(signal == Signal::SIGINT, Ordering::Relaxed);
/// }
///
/// fn main() {
///     let handler = SigHandler::Handler(handle_sigint);
///     unsafe { signal::signal(Signal::SIGINT, handler) }.unwrap();
/// }
/// ```
///
/// # Errors
///
/// Returns [`Error(Errno::EOPNOTSUPP)`] if `handler` is
/// [`SigAction`][SigActionStruct]. Use [`sigaction`][SigActionFn] instead.
///
/// `signal` also returns any error from `libc::signal`, such as when an attempt
/// is made to catch a signal that cannot be caught or to ignore a signal that
/// cannot be ignored.
///
/// [`Error::UnsupportedOperation`]: ../../enum.Error.html#variant.UnsupportedOperation
/// [SigActionStruct]: struct.SigAction.html
/// [sigactionFn]: fn.sigaction.html
pub unsafe fn signal(signal: Signal, handler: SigHandler) -> Result<SigHandler> {
    let signal = signal as libc::c_int;
    let res = match handler {
        SigHandler::SigDfl => unsafe { libc::signal(signal, libc::SIG_DFL) },
        SigHandler::SigIgn => unsafe { libc::signal(signal, libc::SIG_IGN) },
        SigHandler::Handler(handler) => unsafe { libc::signal(signal, handler as libc::sighandler_t) },
        #[cfg(not(target_os = "redox"))]
        SigHandler::SigAction(_) => return Err(Errno::ENOTSUP),
    };
    Errno::result(res).map(|oldhandler| {
        match oldhandler {
            libc::SIG_DFL => SigHandler::SigDfl,
            libc::SIG_IGN => SigHandler::SigIgn,
            p => SigHandler::Handler(
                unsafe { *(&p as *const usize as *const extern fn(libc::c_int)) } as extern fn(libc::c_int)),
        }
    })
}

fn do_pthread_sigmask(how: SigmaskHow,
                       set: Option<&SigSet>,
                       oldset: Option<*mut libc::sigset_t>) -> Result<()> {
    if set.is_none() && oldset.is_none() {
        return Ok(())
    }

    let res = unsafe {
        // if set or oldset is None, pass in null pointers instead
        libc::pthread_sigmask(how as libc::c_int,
                             set.map_or_else(ptr::null::<libc::sigset_t>,
                                             |s| &s.sigset as *const libc::sigset_t),
                             oldset.unwrap_or(ptr::null_mut())
                             )
    };

    Errno::result(res).map(drop)
}

/// Manages the signal mask (set of blocked signals) for the calling thread.
///
/// If the `set` parameter is `Some(..)`, then the signal mask will be updated with the signal set.
/// The `how` flag decides the type of update. If `set` is `None`, `how` will be ignored,
/// and no modification will take place.
///
/// If the 'oldset' parameter is `Some(..)` then the current signal mask will be written into it.
///
/// If both `set` and `oldset` is `Some(..)`, the current signal mask will be written into oldset,
/// and then it will be updated with `set`.
///
/// If both `set` and `oldset` is None, this function is a no-op.
///
/// For more information, visit the [`pthread_sigmask`](https://pubs.opengroup.org/onlinepubs/9699919799/functions/pthread_sigmask.html),
/// or [`sigprocmask`](https://pubs.opengroup.org/onlinepubs/9699919799/functions/sigprocmask.html) man pages.
pub fn pthread_sigmask(how: SigmaskHow,
                       set: Option<&SigSet>,
                       oldset: Option<&mut SigSet>) -> Result<()>
{
    do_pthread_sigmask(how, set, oldset.map(|os| &mut os.sigset as *mut _ ))
}

/// Examine and change blocked signals.
///
/// For more information see the [`sigprocmask` man
/// pages](https://pubs.opengroup.org/onlinepubs/9699919799/functions/sigprocmask.html).
pub fn sigprocmask(how: SigmaskHow, set: Option<&SigSet>, oldset: Option<&mut SigSet>) -> Result<()> {
    if set.is_none() && oldset.is_none() {
        return Ok(())
    }

    let res = unsafe {
        // if set or oldset is None, pass in null pointers instead
        libc::sigprocmask(how as libc::c_int,
                          set.map_or_else(ptr::null::<libc::sigset_t>,
                                          |s| &s.sigset as *const libc::sigset_t),
                          oldset.map_or_else(ptr::null_mut::<libc::sigset_t>,
                                             |os| &mut os.sigset as *mut libc::sigset_t))
    };

    Errno::result(res).map(drop)
}

/// Send a signal to a process
///
/// # Arguments
///
/// * `pid` -    Specifies which processes should receive the signal.
///   - If positive, specifies an individual process.
///   - If zero, the signal will be sent to all processes whose group
///     ID is equal to the process group ID of the sender.  This is a
#[cfg_attr(target_os = "fuchsia", doc = "variant of `killpg`.")]
#[cfg_attr(not(target_os = "fuchsia"), doc = "variant of [`killpg`].")]
///   - If `-1` and the process has super-user privileges, the signal
///     is sent to all processes exclusing system processes.
///   - If less than `-1`, the signal is sent to all processes whose
///     process group ID is equal to the absolute value of `pid`.
/// * `signal` - Signal to send. If `None`, error checking is performed
///              but no signal is actually sent.
///
/// See Also
/// [`kill(2)`](https://pubs.opengroup.org/onlinepubs/9699919799/functions/kill.html)
pub fn kill<T: Into<Option<Signal>>>(pid: Pid, signal: T) -> Result<()> {
    let res = unsafe { libc::kill(pid.into(),
                                  match signal.into() {
                                      Some(s) => s as libc::c_int,
                                      None => 0,
                                  }) };

    Errno::result(res).map(drop)
}

/// Send a signal to a process group
///
/// # Arguments
///
/// * `pgrp` -   Process group to signal.  If less then or equal 1, the behavior
///              is platform-specific.
/// * `signal` - Signal to send. If `None`, `killpg` will only preform error
///              checking and won't send any signal.
///
/// See Also [killpg(3)](https://pubs.opengroup.org/onlinepubs/9699919799/functions/killpg.html).
#[cfg(not(target_os = "fuchsia"))]
pub fn killpg<T: Into<Option<Signal>>>(pgrp: Pid, signal: T) -> Result<()> {
    let res = unsafe { libc::killpg(pgrp.into(),
                                  match signal.into() {
                                      Some(s) => s as libc::c_int,
                                      None => 0,
                                  }) };

    Errno::result(res).map(drop)
}

/// Send a signal to the current thread
///
/// See Also [raise(3)](https://pubs.opengroup.org/onlinepubs/9699919799/functions/raise.html)
pub fn raise(signal: Signal) -> Result<()> {
    let res = unsafe { libc::raise(signal as libc::c_int) };

    Errno::result(res).map(drop)
}
}

feature! {
#![any(feature = "aio", feature = "signal")]

/// Identifies a thread for [`SigevNotify::SigevThreadId`]
#[cfg(target_os = "freebsd")]
pub type type_of_thread_id = libc::lwpid_t;
/// Identifies a thread for [`SigevNotify::SigevThreadId`]
#[cfg(all(not(target_os = "hurd"), any(target_env = "gnu", target_env = "uclibc")))]
pub type type_of_thread_id = libc::pid_t;

/// Specifies the notification method used by a [`SigEvent`]
// sigval is actually a union of a int and a void*.  But it's never really used
// as a pointer, because neither libc nor the kernel ever dereference it.  nix
// therefore presents it as an intptr_t, which is how kevent uses it.
#[cfg(not(any(target_os = "fuchsia", target_os = "hurd", target_os = "openbsd", target_os = "redox")))]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SigevNotify {
    /// No notification will be delivered
    SigevNone,
    /// Notify by delivering a signal to the process.
    SigevSignal {
        /// Signal to deliver
        signal: Signal,
        /// Will be present in the `si_value` field of the [`libc::siginfo_t`]
        /// structure of the queued signal.
        si_value: libc::intptr_t
    },
    // Note: SIGEV_THREAD is not implemented, but could be if desired.
    /// Notify by delivering an event to a kqueue.
    #[cfg(freebsdlike)]
    SigevKevent {
        /// File descriptor of the kqueue to notify.
        kq: RawFd,
        /// Will be contained in the kevent's `udata` field.
        udata: libc::intptr_t
    },
    /// Notify by delivering an event to a kqueue, with optional event flags set
    #[cfg(target_os = "freebsd")]
    #[cfg(feature = "event")]
    SigevKeventFlags {
        /// File descriptor of the kqueue to notify.
        kq: RawFd,
        /// Will be contained in the kevent's `udata` field.
        udata: libc::intptr_t,
        /// Flags that will be set on the delivered event.  See `kevent(2)`.
        flags: crate::sys::event::EventFlag
    },
    /// Notify by delivering a signal to a thread.
    #[cfg(any(
            target_os = "freebsd",
            target_env = "gnu",
            target_env = "uclibc",
    ))]
    SigevThreadId {
        /// Signal to send
        signal: Signal,
        /// LWP ID of the thread to notify
        thread_id: type_of_thread_id,
        /// Will be present in the `si_value` field of the [`libc::siginfo_t`]
        /// structure of the queued signal.
        si_value: libc::intptr_t
    },
}
}

#[cfg(not(any(
    target_os = "fuchsia",
    target_os = "hurd",
    target_os = "openbsd",
    target_os = "redox"
)))]
mod sigevent {
    feature! {
    #![any(feature = "aio", feature = "signal")]

    use std::mem;
    use super::SigevNotify;

    #[cfg(target_os = "freebsd")]
    pub(crate) use ffi::sigevent as libc_sigevent;
    #[cfg(not(target_os = "freebsd"))]
    pub(crate) use libc::sigevent as libc_sigevent;

    // For FreeBSD only, we define the C structure here.  Because the structure
    // defined in libc isn't correct.  The real sigevent contains union fields,
    // but libc could not represent those when sigevent was originally added, so
    // instead libc simply defined the most useful field.  Now that Rust can
    // represent unions, there's a PR to libc to fix it.  However, it's stuck
    // forever due to backwards compatibility concerns.  Even though there's a
    // workaround, libc refuses to merge it.  I think it's just too complicated
    // for them to want to think about right now, because that project is
    // short-staffed.  So we define it here instead, so we won't have to wait on
    // libc.
    // https://github.com/rust-lang/libc/pull/2813
    #[cfg(target_os = "freebsd")]
    mod ffi {
        use std::{fmt, hash};

        #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
        #[repr(C)]
        pub struct __c_anonymous_sigev_thread {
            pub _function: *mut libc::c_void,   // Actually a function pointer
            pub _attribute: *mut libc::pthread_attr_t,
        }
        #[derive(Clone, Copy)]
        // This will never be used on its own, and its parent has a Debug impl,
        // so it doesn't need one.
        #[allow(missing_debug_implementations)]
        #[repr(C)]
        pub union __c_anonymous_sigev_un {
            pub _threadid: libc::__lwpid_t,
            pub _sigev_thread: __c_anonymous_sigev_thread,
            pub _kevent_flags: libc::c_ushort,
            __spare__: [libc::c_long; 8],
        }

        #[derive(Clone, Copy)]
        #[repr(C)]
        pub struct sigevent {
            pub sigev_notify: libc::c_int,
            pub sigev_signo: libc::c_int,
            pub sigev_value: libc::sigval,
            pub _sigev_un: __c_anonymous_sigev_un,
        }

        impl fmt::Debug for sigevent {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                let mut ds = f.debug_struct("sigevent");
                ds.field("sigev_notify", &self.sigev_notify)
                    .field("sigev_signo", &self.sigev_signo)
                    .field("sigev_value", &self.sigev_value);
                // Safe because we check the sigev_notify discriminant
                unsafe {
                    match self.sigev_notify {
                        libc::SIGEV_KEVENT => {
                            ds.field("sigev_notify_kevent_flags", &self._sigev_un._kevent_flags);
                        }
                        libc::SIGEV_THREAD_ID => {
                            ds.field("sigev_notify_thread_id", &self._sigev_un._threadid);
                        }
                        libc::SIGEV_THREAD => {
                            ds.field("sigev_notify_function", &self._sigev_un._sigev_thread._function);
                            ds.field("sigev_notify_attributes", &self._sigev_un._sigev_thread._attribute);
                        }
                        _ => ()
                    };
                }
                ds.finish()
            }
        }

        impl PartialEq for sigevent {
            fn eq(&self, other: &Self) -> bool {
                let mut equals = self.sigev_notify == other.sigev_notify;
                equals &= self.sigev_signo == other.sigev_signo;
                equals &= self.sigev_value == other.sigev_value;
                // Safe because we check the sigev_notify discriminant
                unsafe {
                    match self.sigev_notify {
                        libc::SIGEV_KEVENT => {
                            equals &= self._sigev_un._kevent_flags == other._sigev_un._kevent_flags;
                        }
                        libc::SIGEV_THREAD_ID => {
                            equals &= self._sigev_un._threadid == other._sigev_un._threadid;
                        }
                        libc::SIGEV_THREAD => {
                            equals &= self._sigev_un._sigev_thread == other._sigev_un._sigev_thread;
                        }
                        _ => /* The union field is don't care */ ()
                    }
                }
                equals
            }
        }

        impl Eq for sigevent {}

        impl hash::Hash for sigevent {
            fn hash<H: hash::Hasher>(&self, s: &mut H) {
                self.sigev_notify.hash(s);
                self.sigev_signo.hash(s);
                self.sigev_value.hash(s);
                // Safe because we check the sigev_notify discriminant
                unsafe {
                    match self.sigev_notify {
                        libc::SIGEV_KEVENT => {
                            self._sigev_un._kevent_flags.hash(s);
                        }
                        libc::SIGEV_THREAD_ID => {
                            self._sigev_un._threadid.hash(s);
                        }
                        libc::SIGEV_THREAD => {
                            self._sigev_un._sigev_thread.hash(s);
                        }
                        _ => /* The union field is don't care */ ()
                    }
                }
            }
        }
    }

    /// Used to request asynchronous notification of the completion of certain
    /// events, such as POSIX AIO and timers.
    #[repr(C)]
    #[derive(Clone, Debug, Eq, Hash, PartialEq)]
    // It can't be Copy on all platforms.
    #[allow(missing_copy_implementations)]
    pub struct SigEvent {
        sigevent: libc_sigevent
    }

    impl SigEvent {
        /// **Note:** this constructor does not allow the user to set the
        /// `sigev_notify_kevent_flags` field.  That's considered ok because on FreeBSD
        /// at least those flags don't do anything useful.  That field is part of a
        /// union that shares space with the more genuinely useful fields.
        ///
        /// **Note:** This constructor also doesn't allow the caller to set the
        /// `sigev_notify_function` or `sigev_notify_attributes` fields, which are
        /// required for `SIGEV_THREAD`.  That's considered ok because on no operating
        /// system is `SIGEV_THREAD` the most efficient way to deliver AIO
        /// notification.  FreeBSD and DragonFly BSD programs should prefer `SIGEV_KEVENT`.
        /// Linux, Solaris, and portable programs should prefer `SIGEV_THREAD_ID` or
        /// `SIGEV_SIGNAL`.  That field is part of a union that shares space with the
        /// more genuinely useful `sigev_notify_thread_id`
        pub fn new(sigev_notify: SigevNotify) -> SigEvent {
            let mut sev: libc_sigevent = unsafe { mem::zeroed() };
            match sigev_notify {
                SigevNotify::SigevNone => {
                    sev.sigev_notify = libc::SIGEV_NONE;
                },
                SigevNotify::SigevSignal{signal, si_value} => {
                    sev.sigev_notify = libc::SIGEV_SIGNAL;
                    sev.sigev_signo = signal as libc::c_int;
                    sev.sigev_value.sival_ptr = si_value as *mut libc::c_void
                },
                #[cfg(freebsdlike)]
                SigevNotify::SigevKevent{kq, udata} => {
                    sev.sigev_notify = libc::SIGEV_KEVENT;
                    sev.sigev_signo = kq;
                    sev.sigev_value.sival_ptr = udata as *mut libc::c_void;
                },
                #[cfg(target_os = "freebsd")]
                #[cfg(feature = "event")]
                SigevNotify::SigevKeventFlags{kq, udata, flags} => {
                    sev.sigev_notify = libc::SIGEV_KEVENT;
                    sev.sigev_signo = kq;
                    sev.sigev_value.sival_ptr = udata as *mut libc::c_void;
                    sev._sigev_un._kevent_flags = flags.bits();
                },
                #[cfg(target_os = "freebsd")]
                SigevNotify::SigevThreadId{signal, thread_id, si_value} => {
                    sev.sigev_notify = libc::SIGEV_THREAD_ID;
                    sev.sigev_signo = signal as libc::c_int;
                    sev.sigev_value.sival_ptr = si_value as *mut libc::c_void;
                    sev._sigev_un._threadid = thread_id;
                }
                #[cfg(any(target_env = "gnu", target_env = "uclibc"))]
                SigevNotify::SigevThreadId{signal, thread_id, si_value} => {
                    sev.sigev_notify = libc::SIGEV_THREAD_ID;
                    sev.sigev_signo = signal as libc::c_int;
                    sev.sigev_value.sival_ptr = si_value as *mut libc::c_void;
                    sev.sigev_notify_thread_id = thread_id;
                }
            }
            SigEvent{sigevent: sev}
        }

        /// Return a copy of the inner structure
        #[cfg(target_os = "freebsd")]
        pub fn sigevent(&self) -> libc::sigevent {
            // Safe because they're really the same structure.  See
            // https://github.com/rust-lang/libc/pull/2813
            unsafe {
                mem::transmute::<libc_sigevent, libc::sigevent>(self.sigevent)
            }
        }

        /// Return a copy of the inner structure
        #[cfg(not(target_os = "freebsd"))]
        pub fn sigevent(&self) -> libc::sigevent {
            self.sigevent
        }

        /// Returns a mutable pointer to the `sigevent` wrapped by `self`
        #[cfg(target_os = "freebsd")]
        pub fn as_mut_ptr(&mut self) -> *mut libc::sigevent {
            // Safe because they're really the same structure.  See
            // https://github.com/rust-lang/libc/pull/2813
            &mut self.sigevent as *mut libc_sigevent as *mut libc::sigevent
        }

        /// Returns a mutable pointer to the `sigevent` wrapped by `self`
        #[cfg(not(target_os = "freebsd"))]
        pub fn as_mut_ptr(&mut self) -> *mut libc::sigevent {
            &mut self.sigevent
        }
    }

    impl<'a> From<&'a libc::sigevent> for SigEvent {
        #[cfg(target_os = "freebsd")]
        fn from(sigevent: &libc::sigevent) -> Self {
            // Safe because they're really the same structure.  See
            // https://github.com/rust-lang/libc/pull/2813
            let sigevent = unsafe {
                mem::transmute::<libc::sigevent, libc_sigevent>(*sigevent)
            };
            SigEvent{ sigevent }
        }
        #[cfg(not(target_os = "freebsd"))]
        fn from(sigevent: &libc::sigevent) -> Self {
            SigEvent{ sigevent: *sigevent }
        }
    }
    }
}
