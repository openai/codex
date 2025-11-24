//! Signal numbers.
//!
//! # Safety
//!
//! Some signal numbers are reserved by the libc.
//! [`Signal::from_raw_unchecked`] and [`Signal::from_raw_nonzero_unchecked`]
//! allow constructing `Signal` values with arbitrary values. Users must avoid
//! using reserved values to send, consume, or block any signals or alter any
//! signal handlers.
//!
//! See the individual functions' safety comments for more details.
#![allow(unsafe_code)]

use crate::backend::c;
use core::fmt;
use core::num::NonZeroI32;

/// A signal number for use with [`kill_process`], [`kill_process_group`], and
/// [`kill_current_process_group`].
///
/// For additional constructors such as [`Signal::rt_min`]
/// (aka `libc::SIGRTMIN`), [`Signal::rt_max`] (aka `libc::SIGRTMAX`),
/// [`Signal::rt`] (aka `|n| libc::SIGRTMIN() + n`), [`Signal::from_raw`], and
/// [`Signal::from_raw_nonzero`], see [rustix-libc-wrappers].
///
/// # References
///  - [POSIX]
///  - [Linux]
///  - [glibc]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/basedefs/signal.h.html
/// [Linux]: https://man7.org/linux/man-pages/man7/signal.7.html
/// [glibc]: https://sourceware.org/glibc/manual/latest/html_node/Standard-Signals.html
///
/// [`kill_process`]: crate::process::kill_process
/// [`kill_process_group`]: crate::process::kill_process_group
/// [`kill_current_process_group`]: crate::process::kill_current_process_group
/// [`Signal::rt_min`]: https://docs.rs/rustix-libc-wrappers/*/rustix_libc_wrappers/trait.SignalExt.html#tymethod.rt_min
/// [`Signal::rt_max`]: https://docs.rs/rustix-libc-wrappers/*/rustix_libc_wrappers/trait.SignalExt.html#tymethod.rt_max
/// [`Signal::rt`]: https://docs.rs/rustix-libc-wrappers/*/rustix_libc_wrappers/trait.SignalExt.html#tymethod.rt
/// [`Signal::from_raw`]: https://docs.rs/rustix-libc-wrappers/*/rustix_libc_wrappers/trait.SignalExt.html#tymethod.from_raw
/// [`Signal::from_raw_nonzero`]: https://docs.rs/rustix-libc-wrappers/*/rustix_libc_wrappers/trait.SignalExt.html#tymethod.from_raw_nonzero
/// [rustix-libc-wrappers]: https://docs.rs/rustix-libc-wrappers
#[doc(alias = "SIGRTMIN")]
#[doc(alias = "SIGRTMAX")]
#[derive(Copy, Clone, Eq, PartialEq)]
#[repr(transparent)]
pub struct Signal(NonZeroI32);

// SAFETY: The libc-defined signal values are all non-zero.
#[rustfmt::skip]
impl Signal {
    /// `SIGHUP`
    pub const HUP: Self = Self(unsafe { NonZeroI32::new_unchecked(c::SIGHUP) });
    /// `SIGINT`
    pub const INT: Self = Self(unsafe { NonZeroI32::new_unchecked(c::SIGINT) });
    /// `SIGQUIT`
    pub const QUIT: Self = Self(unsafe { NonZeroI32::new_unchecked(c::SIGQUIT) });
    /// `SIGILL`
    pub const ILL: Self = Self(unsafe { NonZeroI32::new_unchecked(c::SIGILL) });
    /// `SIGTRAP`
    pub const TRAP: Self = Self(unsafe { NonZeroI32::new_unchecked(c::SIGTRAP) });
    /// `SIGABRT`, aka `SIGIOT`
    #[doc(alias = "IOT")]
    #[doc(alias = "ABRT")]
    pub const ABORT: Self = Self(unsafe { NonZeroI32::new_unchecked(c::SIGABRT) });
    /// `SIGBUS`
    pub const BUS: Self = Self(unsafe { NonZeroI32::new_unchecked(c::SIGBUS) });
    /// `SIGFPE`
    pub const FPE: Self = Self(unsafe { NonZeroI32::new_unchecked(c::SIGFPE) });
    /// `SIGKILL`
    pub const KILL: Self = Self(unsafe { NonZeroI32::new_unchecked(c::SIGKILL) });
    /// `SIGUSR1`
    #[cfg(not(target_os = "vita"))]
    pub const USR1: Self = Self(unsafe { NonZeroI32::new_unchecked(c::SIGUSR1) });
    /// `SIGSEGV`
    pub const SEGV: Self = Self(unsafe { NonZeroI32::new_unchecked(c::SIGSEGV) });
    /// `SIGUSR2`
    #[cfg(not(target_os = "vita"))]
    pub const USR2: Self = Self(unsafe { NonZeroI32::new_unchecked(c::SIGUSR2) });
    /// `SIGPIPE`
    pub const PIPE: Self = Self(unsafe { NonZeroI32::new_unchecked(c::SIGPIPE) });
    /// `SIGALRM`
    #[doc(alias = "ALRM")]
    pub const ALARM: Self = Self(unsafe { NonZeroI32::new_unchecked(c::SIGALRM) });
    /// `SIGTERM`
    pub const TERM: Self = Self(unsafe { NonZeroI32::new_unchecked(c::SIGTERM) });
    /// `SIGSTKFLT`
    #[cfg(not(any(
        bsd,
        solarish,
        target_os = "aix",
        target_os = "cygwin",
        target_os = "haiku",
        target_os = "horizon",
        target_os = "hurd",
        target_os = "nto",
        target_os = "vita",
        all(
            linux_kernel,
            any(
                target_arch = "mips",
                target_arch = "mips32r6",
                target_arch = "mips64",
                target_arch = "mips64r6",
                target_arch = "sparc",
                target_arch = "sparc64"
            ),
        ),
    )))]
    pub const STKFLT: Self = Self(unsafe { NonZeroI32::new_unchecked(c::SIGSTKFLT) });
    /// `SIGCHLD`
    #[cfg(not(target_os = "vita"))]
    #[doc(alias = "CHLD")]
    pub const CHILD: Self = Self(unsafe { NonZeroI32::new_unchecked(c::SIGCHLD) });
    /// `SIGCONT`
    #[cfg(not(target_os = "vita"))]
    pub const CONT: Self = Self(unsafe { NonZeroI32::new_unchecked(c::SIGCONT) });
    /// `SIGSTOP`
    #[cfg(not(target_os = "vita"))]
    pub const STOP: Self = Self(unsafe { NonZeroI32::new_unchecked(c::SIGSTOP) });
    /// `SIGTSTP`
    #[cfg(not(target_os = "vita"))]
    pub const TSTP: Self = Self(unsafe { NonZeroI32::new_unchecked(c::SIGTSTP) });
    /// `SIGTTIN`
    #[cfg(not(target_os = "vita"))]
    pub const TTIN: Self = Self(unsafe { NonZeroI32::new_unchecked(c::SIGTTIN) });
    /// `SIGTTOU`
    #[cfg(not(target_os = "vita"))]
    pub const TTOU: Self = Self(unsafe { NonZeroI32::new_unchecked(c::SIGTTOU) });
    /// `SIGURG`
    #[cfg(not(target_os = "vita"))]
    pub const URG: Self = Self(unsafe { NonZeroI32::new_unchecked(c::SIGURG) });
    /// `SIGXCPU`
    #[cfg(not(target_os = "vita"))]
    pub const XCPU: Self = Self(unsafe { NonZeroI32::new_unchecked(c::SIGXCPU) });
    /// `SIGXFSZ`
    #[cfg(not(target_os = "vita"))]
    pub const XFSZ: Self = Self(unsafe { NonZeroI32::new_unchecked(c::SIGXFSZ) });
    /// `SIGVTALRM`
    #[cfg(not(target_os = "vita"))]
    #[doc(alias = "VTALRM")]
    pub const VTALARM: Self = Self(unsafe { NonZeroI32::new_unchecked(c::SIGVTALRM) });
    /// `SIGPROF`
    #[cfg(not(target_os = "vita"))]
    pub const PROF: Self = Self(unsafe { NonZeroI32::new_unchecked(c::SIGPROF) });
    /// `SIGWINCH`
    #[cfg(not(target_os = "vita"))]
    pub const WINCH: Self = Self(unsafe { NonZeroI32::new_unchecked(c::SIGWINCH) });
    /// `SIGIO`, aka `SIGPOLL`
    #[doc(alias = "POLL")]
    #[cfg(not(any(target_os = "haiku", target_os = "vita")))]
    pub const IO: Self = Self(unsafe { NonZeroI32::new_unchecked(c::SIGIO) });
    /// `SIGPWR`
    #[cfg(not(any(
        bsd,
        target_os = "haiku",
        target_os = "horizon",
        target_os = "hurd",
        target_os = "vita"
    )))]
    #[doc(alias = "PWR")]
    pub const POWER: Self = Self(unsafe { NonZeroI32::new_unchecked(c::SIGPWR) });
    /// `SIGSYS`, aka `SIGUNUSED`
    #[doc(alias = "UNUSED")]
    pub const SYS: Self = Self(unsafe { NonZeroI32::new_unchecked(c::SIGSYS) });
    /// `SIGEMT`
    #[cfg(any(
        bsd,
        solarish,
        target_os = "aix",
        target_os = "hermit",
        all(
            linux_kernel,
            any(
                target_arch = "mips",
                target_arch = "mips32r6",
                target_arch = "mips64",
                target_arch = "mips64r6",
                target_arch = "sparc",
                target_arch = "sparc64"
            )
        )
    ))]
    pub const EMT: Self = Self(unsafe { NonZeroI32::new_unchecked(c::SIGEMT) });
    /// `SIGINFO`
    #[cfg(bsd)]
    pub const INFO: Self = Self(unsafe { NonZeroI32::new_unchecked(c::SIGINFO) });
    /// `SIGTHR`
    #[cfg(target_os = "freebsd")]
    #[doc(alias = "LWP")]
    pub const THR: Self = Self(unsafe { NonZeroI32::new_unchecked(c::SIGTHR) });
    /// `SIGLIBRT`
    #[cfg(target_os = "freebsd")]
    pub const LIBRT: Self = Self(unsafe { NonZeroI32::new_unchecked(c::SIGLIBRT) });
}

impl Signal {
    /// Convert a `Signal` to a raw signal number.
    ///
    /// To convert to a `NonZeroI32`, use [`Signal::as_raw_nonzero`].
    #[inline]
    pub const fn as_raw(self) -> i32 {
        self.0.get()
    }

    /// Convert a `Signal` to a raw non-zero signal number.
    #[inline]
    pub const fn as_raw_nonzero(self) -> NonZeroI32 {
        self.0
    }

    /// Convert a raw signal number into a `Signal` without checks.
    ///
    /// For a safe checked version, see [`Signal::from_raw`] in
    /// [rustix-libc-wrappers].
    ///
    /// # Safety
    ///
    /// `sig` must be a valid and non-zero signal number.
    ///
    /// And, if `sig` is a signal number reserved by the libc, such as a value
    /// from the libc [`SIGRTMIN`] to the libc [`SIGRTMAX`], inclusive, then
    /// the resulting `Signal` must not be used to send, consume, or block any
    /// signals or alter any signal handlers.
    ///
    /// [`Signal::from_raw`]: https://docs.rs/rustix-libc-wrappers/*/rustix_libc_wrappers/trait.SignalExt.html#tymethod.from_raw
    /// [rustix-libc-wrappers]: https://docs.rs/rustix-libc-wrappers
    /// [`SIGRTMIN`]: https://docs.rs/libc/*/libc/fn.SIGRTMIN.html
    /// [`SIGRTMAX`]: https://docs.rs/libc/*/libc/fn.SIGRTMAX.html
    #[inline]
    pub const unsafe fn from_raw_unchecked(sig: i32) -> Self {
        Self::from_raw_nonzero_unchecked(NonZeroI32::new_unchecked(sig))
    }

    /// Convert a raw non-zero signal number into a `Signal` without checks.
    ///
    /// For a safe checked version, see [`Signal::from_raw_nonzero`] in
    /// [rustix-libc-wrappers].
    ///
    /// # Safety
    ///
    /// `sig` must be a valid signal number.
    ///
    /// And, if `sig` is a signal number reserved by the libc, such as a value
    /// from [`SIGRTMIN`] to [`SIGRTMAX`] inclusive, then the resulting
    /// `Signal` must not be used to send, consume, or block any signals or
    /// alter any signal handlers.
    ///
    /// [`Signal::from_raw_nonzero`]: https://docs.rs/rustix-libc-wrappers/*/rustix_libc_wrappers/trait.SignalExt.html#tymethod.from_raw_nonzero
    /// [rustix-libc-wrappers]: https://docs.rs/rustix-libc-wrappers
    /// [`SIGRTMIN`]: https://docs.rs/libc/*/libc/fn.SIGRTMIN.html
    /// [`SIGRTMAX`]: https://docs.rs/libc/*/libc/fn.SIGRTMAX.html
    #[inline]
    pub const unsafe fn from_raw_nonzero_unchecked(sig: NonZeroI32) -> Self {
        Self(sig)
    }

    /// Convert a raw named signal number into a `Signal`.
    ///
    /// If the given signal number corresponds to one of the named constant
    /// signal values, such as [`Signal::HUP`] or [`Signal::INT`], return the
    /// `Signal` value. Otherwise return `None`.
    ///
    /// Signals in the range `SIGRTMIN` through `SIGRTMAX` are not supported by
    /// this function. For a constructor that does recognize those values, see
    /// [`Signal::from_raw`] in [rustix-libc-wrappers].
    ///
    /// [`Signal::from_raw`]: https://docs.rs/rustix-libc-wrappers/*/rustix_libc_wrappers/trait.SignalExt.html#tymethod.from_raw
    /// [rustix-libc-wrappers]: https://docs.rs/rustix-libc-wrappers
    pub const fn from_named_raw(sig: i32) -> Option<Self> {
        if let Some(sig) = NonZeroI32::new(sig) {
            Self::from_named_raw_nonzero(sig)
        } else {
            None
        }
    }

    /// Convert a raw non-zero named signal number into a `Signal`.
    ///
    /// If the given signal number corresponds to one of the constant signal
    /// values, such as [`Signal::HUP`] or [`Signal::INT`], return the
    /// `Signal` value. Otherwise return `None`.
    ///
    /// Signals in the range `SIGRTMIN` through `SIGRTMAX` are not supported by
    /// this function. For a constructor that does recognize those values, see
    /// [`Signal::from_raw_nonzero`] in [rustix-libc-wrappers].
    ///
    /// [`Signal::from_raw_nonzero`]: https://docs.rs/rustix-libc-wrappers/*/rustix_libc_wrappers/trait.SignalExt.html#tymethod.from_raw_nonzero
    /// [rustix-libc-wrappers]: https://docs.rs/rustix-libc-wrappers
    pub const fn from_named_raw_nonzero(sig: NonZeroI32) -> Option<Self> {
        match sig.get() {
            c::SIGHUP => Some(Self::HUP),
            c::SIGINT => Some(Self::INT),
            c::SIGQUIT => Some(Self::QUIT),
            c::SIGILL => Some(Self::ILL),
            c::SIGTRAP => Some(Self::TRAP),
            c::SIGABRT => Some(Self::ABORT),
            c::SIGBUS => Some(Self::BUS),
            c::SIGFPE => Some(Self::FPE),
            c::SIGKILL => Some(Self::KILL),
            #[cfg(not(target_os = "vita"))]
            c::SIGUSR1 => Some(Self::USR1),
            c::SIGSEGV => Some(Self::SEGV),
            #[cfg(not(target_os = "vita"))]
            c::SIGUSR2 => Some(Self::USR2),
            c::SIGPIPE => Some(Self::PIPE),
            c::SIGALRM => Some(Self::ALARM),
            c::SIGTERM => Some(Self::TERM),
            #[cfg(not(any(
                bsd,
                solarish,
                target_os = "aix",
                target_os = "cygwin",
                target_os = "haiku",
                target_os = "horizon",
                target_os = "hurd",
                target_os = "nto",
                target_os = "vita",
                all(
                    linux_kernel,
                    any(
                        target_arch = "mips",
                        target_arch = "mips32r6",
                        target_arch = "mips64",
                        target_arch = "mips64r6",
                        target_arch = "sparc",
                        target_arch = "sparc64"
                    ),
                )
            )))]
            c::SIGSTKFLT => Some(Self::STKFLT),
            #[cfg(not(target_os = "vita"))]
            c::SIGCHLD => Some(Self::CHILD),
            #[cfg(not(target_os = "vita"))]
            c::SIGCONT => Some(Self::CONT),
            #[cfg(not(target_os = "vita"))]
            c::SIGSTOP => Some(Self::STOP),
            #[cfg(not(target_os = "vita"))]
            c::SIGTSTP => Some(Self::TSTP),
            #[cfg(not(target_os = "vita"))]
            c::SIGTTIN => Some(Self::TTIN),
            #[cfg(not(target_os = "vita"))]
            c::SIGTTOU => Some(Self::TTOU),
            #[cfg(not(target_os = "vita"))]
            c::SIGURG => Some(Self::URG),
            #[cfg(not(target_os = "vita"))]
            c::SIGXCPU => Some(Self::XCPU),
            #[cfg(not(target_os = "vita"))]
            c::SIGXFSZ => Some(Self::XFSZ),
            #[cfg(not(target_os = "vita"))]
            c::SIGVTALRM => Some(Self::VTALARM),
            #[cfg(not(target_os = "vita"))]
            c::SIGPROF => Some(Self::PROF),
            #[cfg(not(target_os = "vita"))]
            c::SIGWINCH => Some(Self::WINCH),
            #[cfg(not(any(target_os = "haiku", target_os = "vita")))]
            c::SIGIO => Some(Self::IO),
            #[cfg(not(any(
                bsd,
                target_os = "haiku",
                target_os = "horizon",
                target_os = "hurd",
                target_os = "vita"
            )))]
            c::SIGPWR => Some(Self::POWER),
            c::SIGSYS => Some(Self::SYS),
            #[cfg(any(
                bsd,
                solarish,
                target_os = "aix",
                target_os = "hermit",
                all(
                    linux_kernel,
                    any(
                        target_arch = "mips",
                        target_arch = "mips32r6",
                        target_arch = "mips64",
                        target_arch = "mips64r6",
                        target_arch = "sparc",
                        target_arch = "sparc64"
                    )
                )
            ))]
            c::SIGEMT => Some(Self::EMT),
            #[cfg(bsd)]
            c::SIGINFO => Some(Self::INFO),
            #[cfg(target_os = "freebsd")]
            c::SIGTHR => Some(Self::THR),
            #[cfg(target_os = "freebsd")]
            c::SIGLIBRT => Some(Self::LIBRT),

            _ => None,
        }
    }
}

impl fmt::Debug for Signal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::HUP => "Signal::HUP".fmt(f),
            Self::INT => "Signal::INT".fmt(f),
            Self::QUIT => "Signal::QUIT".fmt(f),
            Self::ILL => "Signal::ILL".fmt(f),
            Self::TRAP => "Signal::TRAP".fmt(f),
            Self::ABORT => "Signal::ABORT".fmt(f),
            Self::BUS => "Signal::BUS".fmt(f),
            Self::FPE => "Signal::FPE".fmt(f),
            Self::KILL => "Signal::KILL".fmt(f),
            #[cfg(not(target_os = "vita"))]
            Self::USR1 => "Signal::USR1".fmt(f),
            Self::SEGV => "Signal::SEGV".fmt(f),
            #[cfg(not(target_os = "vita"))]
            Self::USR2 => "Signal::USR2".fmt(f),
            Self::PIPE => "Signal::PIPE".fmt(f),
            Self::ALARM => "Signal::ALARM".fmt(f),
            Self::TERM => "Signal::TERM".fmt(f),
            #[cfg(not(any(
                bsd,
                solarish,
                target_os = "aix",
                target_os = "cygwin",
                target_os = "haiku",
                target_os = "horizon",
                target_os = "hurd",
                target_os = "nto",
                target_os = "vita",
                all(
                    linux_kernel,
                    any(
                        target_arch = "mips",
                        target_arch = "mips32r6",
                        target_arch = "mips64",
                        target_arch = "mips64r6",
                        target_arch = "sparc",
                        target_arch = "sparc64"
                    ),
                ),
            )))]
            Self::STKFLT => "Signal::STKFLT".fmt(f),
            #[cfg(not(target_os = "vita"))]
            Self::CHILD => "Signal::CHILD".fmt(f),
            #[cfg(not(target_os = "vita"))]
            Self::CONT => "Signal::CONT".fmt(f),
            #[cfg(not(target_os = "vita"))]
            Self::STOP => "Signal::STOP".fmt(f),
            #[cfg(not(target_os = "vita"))]
            Self::TSTP => "Signal::TSTP".fmt(f),
            #[cfg(not(target_os = "vita"))]
            Self::TTIN => "Signal::TTIN".fmt(f),
            #[cfg(not(target_os = "vita"))]
            Self::TTOU => "Signal::TTOU".fmt(f),
            #[cfg(not(target_os = "vita"))]
            Self::URG => "Signal::URG".fmt(f),
            #[cfg(not(target_os = "vita"))]
            Self::XCPU => "Signal::XCPU".fmt(f),
            #[cfg(not(target_os = "vita"))]
            Self::XFSZ => "Signal::XFSZ".fmt(f),
            #[cfg(not(target_os = "vita"))]
            Self::VTALARM => "Signal::VTALARM".fmt(f),
            #[cfg(not(target_os = "vita"))]
            Self::PROF => "Signal::PROF".fmt(f),
            #[cfg(not(target_os = "vita"))]
            Self::WINCH => "Signal::WINCH".fmt(f),
            #[cfg(not(any(target_os = "haiku", target_os = "vita")))]
            Self::IO => "Signal::IO".fmt(f),
            #[cfg(not(any(
                bsd,
                target_os = "haiku",
                target_os = "horizon",
                target_os = "hurd",
                target_os = "vita"
            )))]
            Self::POWER => "Signal::POWER".fmt(f),
            Self::SYS => "Signal::SYS".fmt(f),
            #[cfg(any(
                bsd,
                solarish,
                target_os = "aix",
                target_os = "hermit",
                all(
                    linux_kernel,
                    any(
                        target_arch = "mips",
                        target_arch = "mips32r6",
                        target_arch = "mips64",
                        target_arch = "mips64r6",
                        target_arch = "sparc",
                        target_arch = "sparc64"
                    )
                )
            ))]
            Self::EMT => "Signal::EMT".fmt(f),
            #[cfg(bsd)]
            Self::INFO => "Signal::INFO".fmt(f),
            #[cfg(target_os = "freebsd")]
            Self::THR => "Signal::THR".fmt(f),
            #[cfg(target_os = "freebsd")]
            Self::LIBRT => "Signal::LIBRT".fmt(f),

            n => {
                "Signal::from_raw(".fmt(f)?;
                n.as_raw().fmt(f)?;
                ")".fmt(f)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basics() {
        assert_eq!(Signal::HUP.as_raw(), libc::SIGHUP);
        unsafe {
            assert_eq!(Signal::from_raw_unchecked(libc::SIGHUP), Signal::HUP);
            assert_eq!(
                Signal::from_raw_nonzero_unchecked(NonZeroI32::new(libc::SIGHUP).unwrap()),
                Signal::HUP
            );
        }
    }

    #[test]
    fn test_named() {
        assert_eq!(Signal::from_named_raw(-1), None);
        assert_eq!(Signal::from_named_raw(0), None);
        assert_eq!(Signal::from_named_raw(c::SIGHUP), Some(Signal::HUP));
        assert_eq!(Signal::from_named_raw(c::SIGSEGV), Some(Signal::SEGV));
        assert_eq!(Signal::from_named_raw(c::SIGSYS), Some(Signal::SYS));
        #[cfg(any(linux_like, solarish, target_os = "hurd"))]
        {
            assert_eq!(Signal::from_named_raw(libc::SIGRTMIN()), None);
            assert_eq!(Signal::from_named_raw(libc::SIGRTMAX()), None);
        }
    }
}
