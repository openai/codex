//! Extracting more information from the C [`siginfo_t`] structure.
//!
//! See [`Origin`].

use std::fmt::{Debug, Formatter, Result as FmtResult};

use libc::{c_int, pid_t, siginfo_t, uid_t};

use crate::low_level;

// Careful: make sure the signature and the constants match the C source
extern "C" {
    fn sighook_signal_cause(info: &siginfo_t) -> ICause;
    fn sighook_signal_pid(info: &siginfo_t) -> pid_t;
    fn sighook_signal_uid(info: &siginfo_t) -> uid_t;
}

// Warning: must be in sync with the C code
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
#[repr(u8)]
// For some reason, the fact it comes from the C makes rustc emit warning that *some* of these are
// not constructed. No idea why only some of them.
#[allow(dead_code)]
enum ICause {
    Unknown = 0,
    Kernel = 1,
    User = 2,
    TKill = 3,
    Queue = 4,
    MesgQ = 5,
    Exited = 6,
    Killed = 7,
    Dumped = 8,
    Trapped = 9,
    Stopped = 10,
    Continued = 11,
}

impl ICause {
    // The MacOs doesn't use the SI_* constants and leaves si_code at 0. But it doesn't use an
    // union, it has a good-behaved struct with fields and therefore we *can* read the values,
    // even though they'd contain nonsense (zeroes). We wipe that out later.
    #[cfg(target_os = "macos")]
    fn has_process(self) -> bool {
        true
    }

    #[cfg(not(target_os = "macos"))]
    fn has_process(self) -> bool {
        use ICause::*;
        match self {
            Unknown | Kernel => false,
            User | TKill | Queue | MesgQ | Exited | Killed | Dumped | Trapped | Stopped
            | Continued => true,
        }
    }
}

/// Information about process, as presented in the signal metadata.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub struct Process {
    /// The process ID.
    pub pid: pid_t,

    /// The user owning the process.
    pub uid: uid_t,
}

impl Process {
    /**
     * Extract the process information.
     *
     * # Safety
     *
     * The `info` must have a `si_code` corresponding to some situation that has the `si_pid`
     * and `si_uid` filled in.
     */
    unsafe fn extract(info: &siginfo_t) -> Self {
        Self {
            pid: sighook_signal_pid(info),
            uid: sighook_signal_uid(info),
        }
    }
}

/// The means by which a signal was sent by other process.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Sent {
    /// The `kill` call.
    User,

    /// The `tkill` call.
    ///
    /// This is likely linux specific.
    TKill,

    /// `sigqueue`.
    Queue,

    /// `mq_notify`.
    MesgQ,
}

/// A child changed its state.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Chld {
    /// The child exited normally.
    Exited,

    /// It got killed by a signal.
    Killed,

    /// It got killed by a signal and dumped core.
    Dumped,

    /// The child was trapped by a `SIGTRAP` signal.
    Trapped,

    /// The child got stopped.
    Stopped,

    /// The child continued (after being stopped).
    Continued,
}

/// What caused a signal.
///
/// This is a best-effort (and possibly incomplete) representation of the C `siginfo_t::si_code`.
/// It may differ between OSes and may be extended in future versions.
///
/// Note that this doesn't contain all the „fault“ signals (`SIGILL`, `SIGSEGV` and similar).
/// There's no reasonable way to use the exfiltrators with them, since the handler either needs to
/// terminate the process or somehow recover from the situation. Things based on exfiltrators do
/// neither, which would cause an UB and therefore these values just don't make sense.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Cause {
    /// The cause is unknown.
    ///
    /// Some systems don't fill this in. Some systems have values we don't understand. Some signals
    /// don't have specific reasons to come to being.
    Unknown,

    /// Sent by the kernel.
    ///
    /// This probably exists only on Linux.
    Kernel,

    /// The signal was sent by other process.
    Sent(Sent),

    /// A `SIGCHLD`, caused by a child process changing state.
    Chld(Chld),
}

impl From<ICause> for Cause {
    fn from(c: ICause) -> Cause {
        match c {
            ICause::Kernel => Cause::Kernel,
            ICause::User => Cause::Sent(Sent::User),
            ICause::TKill => Cause::Sent(Sent::TKill),
            ICause::Queue => Cause::Sent(Sent::Queue),
            ICause::MesgQ => Cause::Sent(Sent::MesgQ),
            ICause::Exited => Cause::Chld(Chld::Exited),
            ICause::Killed => Cause::Chld(Chld::Killed),
            ICause::Dumped => Cause::Chld(Chld::Dumped),
            ICause::Trapped => Cause::Chld(Chld::Trapped),
            ICause::Stopped => Cause::Chld(Chld::Stopped),
            ICause::Continued => Cause::Chld(Chld::Continued),
            // Unknown and possibly others if the underlying lib is updated
            _ => Cause::Unknown,
        }
    }
}

/// Information about a signal and its origin.
///
/// This is produced by the [`WithOrigin`] exfiltrator (or can be [extracted][Origin::extract] from
/// `siginfo_t` by hand).
#[derive(Clone, Eq, PartialEq)]
#[non_exhaustive]
pub struct Origin {
    /// The signal that happened.
    pub signal: c_int,

    /// Information about the process that caused the signal.
    ///
    /// Note that not all signals are caused by a specific process or have the information
    /// available („fault“ signals like `SIGBUS` don't have, any signal may be sent by the kernel
    /// instead of a specific process).
    ///
    /// This is filled in whenever available. For most signals, this is the process that sent the
    /// signal (by `kill` or similar), for `SIGCHLD` it is the child that caused the signal.
    pub process: Option<Process>,

    /// How the signal happened.
    ///
    /// This is a best-effort value. In particular, some systems may have causes not known to this
    /// library. Some other systems (MacOS) does not fill the value in so there's no way to know.
    /// In all these cases, this will contain [`Cause::Unknown`].
    ///
    /// Some values are platform specific and not available on other systems.
    ///
    /// Future versions may enrich the enum by further values.
    pub cause: Cause,
}

impl Debug for Origin {
    fn fmt(&self, fmt: &mut Formatter) -> FmtResult {
        fn named_signal(sig: c_int) -> String {
            low_level::signal_name(sig)
                .map(|n| format!("{} ({})", n, sig))
                .unwrap_or_else(|| sig.to_string())
        }
        fmt.debug_struct("Origin")
            .field("signal", &named_signal(self.signal))
            .field("process", &self.process)
            .field("cause", &self.cause)
            .finish()
    }
}

impl Origin {
    /// Extracts the Origin from a raw `siginfo_t` structure.
    ///
    /// This function is async-signal-safe, can be called inside a signal handler.
    ///
    /// # Safety
    ///
    /// On systems where the structure is backed by an union on the C side, this requires the
    /// `si_code` and `si_signo` fields must be set properly according to what fields are
    /// available.
    ///
    /// The value passed by kernel satisfies this, care must be taken only when constructed
    /// manually.
    pub unsafe fn extract(info: &siginfo_t) -> Self {
        let cause = sighook_signal_cause(info);
        let process = if cause.has_process() {
            let process = Process::extract(info);
            // On macos we don't have the si_code to go by, but we can go by the values being
            // empty there.
            if cfg!(target_os = "macos") && process.pid == 0 && process.uid == 0 {
                None
            } else {
                Some(process)
            }
        } else {
            None
        };
        let signal = info.si_signo;
        Origin {
            cause: cause.into(),
            signal,
            process,
        }
    }
}
