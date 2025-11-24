//! Wait for a process to change status
use crate::errno::Errno;
use crate::sys::signal::Signal;
use crate::unistd::Pid;
use crate::Result;
use cfg_if::cfg_if;
use libc::{self, c_int};
use std::convert::TryFrom;
#[cfg(any(
    target_os = "android",
    all(target_os = "linux", not(target_env = "uclibc")),
))]
use std::os::unix::io::{AsRawFd, BorrowedFd};

libc_bitflags!(
    /// Controls the behavior of [`waitpid`].
    pub struct WaitPidFlag: c_int {
        /// Do not block when there are no processes wishing to report status.
        WNOHANG;
        /// Report the status of selected processes which are stopped due to a
        /// [`SIGTTIN`](crate::sys::signal::Signal::SIGTTIN),
        /// [`SIGTTOU`](crate::sys::signal::Signal::SIGTTOU),
        /// [`SIGTSTP`](crate::sys::signal::Signal::SIGTSTP), or
        /// [`SIGSTOP`](crate::sys::signal::Signal::SIGSTOP) signal.
        WUNTRACED;
        /// Report the status of selected processes which have terminated.
        #[cfg(any(linux_android,
                  apple_targets,
                  target_os = "freebsd",
                  target_os = "haiku",
                  target_os = "redox",
                  target_os = "netbsd"))]
        WEXITED;
        /// Report the status of selected processes that have continued from a
        /// job control stop by receiving a
        /// [`SIGCONT`](crate::sys::signal::Signal::SIGCONT) signal.
        WCONTINUED;
        /// An alias for WUNTRACED.
        #[cfg(any(linux_android,
                  apple_targets,
                  target_os = "freebsd",
                  target_os = "haiku",
                  target_os = "redox",
                  target_os = "netbsd"))]
        WSTOPPED;
        /// Don't reap, just poll status.
        #[cfg(any(linux_android,
                  apple_targets,
                  target_os = "freebsd",
                  target_os = "haiku",
                  target_os = "redox",
                  target_os = "netbsd"))]
        WNOWAIT;
        /// Don't wait on children of other threads in this group
        #[cfg(any(linux_android, target_os = "redox"))]
        __WNOTHREAD;
        /// Wait on all children, regardless of type
        #[cfg(any(linux_android, target_os = "redox"))]
        __WALL;
        /// Wait for "clone" children only.
        #[cfg(any(linux_android, target_os = "redox"))]
        __WCLONE;
    }
);

/// Possible return values from `wait()` or `waitpid()`.
///
/// Each status (other than `StillAlive`) describes a state transition
/// in a child process `Pid`, such as the process exiting or stopping,
/// plus additional data about the transition if any.
///
/// Note that there are two Linux-specific enum variants, `PtraceEvent`
/// and `PtraceSyscall`. Portable code should avoid exhaustively
/// matching on `WaitStatus`.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum WaitStatus {
    /// The process exited normally (as with `exit()` or returning from
    /// `main`) with the given exit code. This case matches the C macro
    /// `WIFEXITED(status)`; the second field is `WEXITSTATUS(status)`.
    Exited(Pid, i32),
    /// The process was killed by the given signal. The third field
    /// indicates whether the signal generated a core dump. This case
    /// matches the C macro `WIFSIGNALED(status)`; the last two fields
    /// correspond to `WTERMSIG(status)` and `WCOREDUMP(status)`.
    Signaled(Pid, Signal, bool),
    /// The process is alive, but was stopped by the given signal. This
    /// is only reported if `WaitPidFlag::WUNTRACED` was passed. This
    /// case matches the C macro `WIFSTOPPED(status)`; the second field
    /// is `WSTOPSIG(status)`.
    Stopped(Pid, Signal),
    /// The traced process was stopped by a `PTRACE_EVENT_*` event. See
    /// [`nix::sys::ptrace`] and [`ptrace`(2)] for more information. All
    /// currently-defined events use `SIGTRAP` as the signal; the third
    /// field is the `PTRACE_EVENT_*` value of the event.
    ///
    /// [`nix::sys::ptrace`]: ../ptrace/index.html
    /// [`ptrace`(2)]: https://man7.org/linux/man-pages/man2/ptrace.2.html
    #[cfg(linux_android)]
    PtraceEvent(Pid, Signal, c_int),
    /// The traced process was stopped by execution of a system call,
    /// and `PTRACE_O_TRACESYSGOOD` is in effect. See [`ptrace`(2)] for
    /// more information.
    ///
    /// [`ptrace`(2)]: https://man7.org/linux/man-pages/man2/ptrace.2.html
    #[cfg(linux_android)]
    PtraceSyscall(Pid),
    /// The process was previously stopped but has resumed execution
    /// after receiving a `SIGCONT` signal. This is only reported if
    /// `WaitPidFlag::WCONTINUED` was passed. This case matches the C
    /// macro `WIFCONTINUED(status)`.
    Continued(Pid),
    /// There are currently no state changes to report in any awaited
    /// child process. This is only returned if `WaitPidFlag::WNOHANG`
    /// was used (otherwise `wait()` or `waitpid()` would block until
    /// there was something to report).
    StillAlive,
}

impl WaitStatus {
    /// Extracts the PID from the WaitStatus unless it equals StillAlive.
    pub fn pid(&self) -> Option<Pid> {
        use self::WaitStatus::*;
        match *self {
            Exited(p, _) | Signaled(p, _, _) | Stopped(p, _) | Continued(p) => {
                Some(p)
            }
            StillAlive => None,
            #[cfg(linux_android)]
            PtraceEvent(p, _, _) | PtraceSyscall(p) => Some(p),
        }
    }
}

fn exited(status: i32) -> bool {
    libc::WIFEXITED(status)
}

fn exit_status(status: i32) -> i32 {
    libc::WEXITSTATUS(status)
}

fn signaled(status: i32) -> bool {
    libc::WIFSIGNALED(status)
}

fn term_signal(status: i32) -> Result<Signal> {
    Signal::try_from(libc::WTERMSIG(status))
}

fn dumped_core(status: i32) -> bool {
    libc::WCOREDUMP(status)
}

fn stopped(status: i32) -> bool {
    libc::WIFSTOPPED(status)
}

fn stop_signal(status: i32) -> Result<Signal> {
    Signal::try_from(libc::WSTOPSIG(status))
}

#[cfg(linux_android)]
fn syscall_stop(status: i32) -> bool {
    // From ptrace(2), setting PTRACE_O_TRACESYSGOOD has the effect
    // of delivering SIGTRAP | 0x80 as the signal number for syscall
    // stops. This allows easily distinguishing syscall stops from
    // genuine SIGTRAP signals.
    libc::WSTOPSIG(status) == libc::SIGTRAP | 0x80
}

#[cfg(linux_android)]
fn stop_additional(status: i32) -> c_int {
    (status >> 16) as c_int
}

fn continued(status: i32) -> bool {
    libc::WIFCONTINUED(status)
}

impl WaitStatus {
    /// Convert a raw `wstatus` as returned by `waitpid`/`wait` into a `WaitStatus`
    ///
    /// # Errors
    ///
    /// Returns an `Error` corresponding to `EINVAL` for invalid status values.
    ///
    /// # Examples
    ///
    /// Convert a `wstatus` obtained from `libc::waitpid` into a `WaitStatus`:
    ///
    /// ```
    /// use nix::sys::wait::WaitStatus;
    /// use nix::sys::signal::Signal;
    /// let pid = nix::unistd::Pid::from_raw(1);
    /// let status = WaitStatus::from_raw(pid, 0x0002);
    /// assert_eq!(status, Ok(WaitStatus::Signaled(pid, Signal::SIGINT, false)));
    /// ```
    pub fn from_raw(pid: Pid, status: i32) -> Result<WaitStatus> {
        Ok(if exited(status) {
            WaitStatus::Exited(pid, exit_status(status))
        } else if signaled(status) {
            WaitStatus::Signaled(pid, term_signal(status)?, dumped_core(status))
        } else if stopped(status) {
            cfg_if! {
                if #[cfg(linux_android)] {
                    fn decode_stopped(pid: Pid, status: i32) -> Result<WaitStatus> {
                        let status_additional = stop_additional(status);
                        Ok(if syscall_stop(status) {
                            WaitStatus::PtraceSyscall(pid)
                        } else if status_additional == 0 {
                            WaitStatus::Stopped(pid, stop_signal(status)?)
                        } else {
                            WaitStatus::PtraceEvent(pid, stop_signal(status)?,
                                                    stop_additional(status))
                        })
                    }
                } else {
                    fn decode_stopped(pid: Pid, status: i32) -> Result<WaitStatus> {
                        Ok(WaitStatus::Stopped(pid, stop_signal(status)?))
                    }
                }
            }
            return decode_stopped(pid, status);
        } else {
            assert!(continued(status));
            WaitStatus::Continued(pid)
        })
    }

    /// Convert a `siginfo_t` as returned by `waitid` to a `WaitStatus`
    ///
    /// # Errors
    ///
    /// Returns an `Error` corresponding to `EINVAL` for invalid values.
    ///
    /// # Safety
    ///
    /// siginfo_t is actually a union, not all fields may be initialized.
    /// The functions si_pid() and si_status() must be valid to call on
    /// the passed siginfo_t.
    #[cfg(any(
        target_os = "android",
        target_os = "freebsd",
        target_os = "haiku",
        all(target_os = "linux", not(target_env = "uclibc")),
    ))]
    unsafe fn from_siginfo(siginfo: &libc::siginfo_t) -> Result<WaitStatus> {
        let si_pid = unsafe { siginfo.si_pid() };
        if si_pid == 0 {
            return Ok(WaitStatus::StillAlive);
        }

        assert_eq!(siginfo.si_signo, libc::SIGCHLD);

        let pid = Pid::from_raw(si_pid);
        let si_status = unsafe { siginfo.si_status() };

        let status = match siginfo.si_code {
            libc::CLD_EXITED => WaitStatus::Exited(pid, si_status),
            libc::CLD_KILLED | libc::CLD_DUMPED => WaitStatus::Signaled(
                pid,
                Signal::try_from(si_status)?,
                siginfo.si_code == libc::CLD_DUMPED,
            ),
            libc::CLD_STOPPED => {
                WaitStatus::Stopped(pid, Signal::try_from(si_status)?)
            }
            libc::CLD_CONTINUED => WaitStatus::Continued(pid),
            #[cfg(linux_android)]
            libc::CLD_TRAPPED => {
                if si_status == libc::SIGTRAP | 0x80 {
                    WaitStatus::PtraceSyscall(pid)
                } else {
                    WaitStatus::PtraceEvent(
                        pid,
                        Signal::try_from(si_status & 0xff)?,
                        (si_status >> 8) as c_int,
                    )
                }
            }
            _ => return Err(Errno::EINVAL),
        };

        Ok(status)
    }
}

/// Wait for a process to change status
///
/// See also [waitpid(2)](https://pubs.opengroup.org/onlinepubs/9699919799/functions/waitpid.html)
pub fn waitpid<P: Into<Option<Pid>>>(
    pid: P,
    options: Option<WaitPidFlag>,
) -> Result<WaitStatus> {
    use self::WaitStatus::*;

    let mut status: i32 = 0;

    let option_bits = match options {
        Some(bits) => bits.bits(),
        None => 0,
    };

    let res = unsafe {
        libc::waitpid(
            pid.into().unwrap_or_else(|| Pid::from_raw(-1)).into(),
            &mut status as *mut c_int,
            option_bits,
        )
    };

    match Errno::result(res)? {
        0 => Ok(StillAlive),
        res => WaitStatus::from_raw(Pid::from_raw(res), status),
    }
}

/// Wait for any child process to change status or a signal is received.
///
/// See also [wait(2)](https://pubs.opengroup.org/onlinepubs/9699919799/functions/wait.html)
pub fn wait() -> Result<WaitStatus> {
    waitpid(None, None)
}

/// The ID argument for `waitid`
#[cfg(any(
    target_os = "android",
    target_os = "freebsd",
    target_os = "haiku",
    all(target_os = "linux", not(target_env = "uclibc")),
))]
#[derive(Debug)]
pub enum Id<'fd> {
    /// Wait for any child
    All,
    /// Wait for the child whose process ID matches the given PID
    Pid(Pid),
    /// Wait for the child whose process group ID matches the given PID
    ///
    /// If the PID is zero, the caller's process group is used since Linux 5.4.
    PGid(Pid),
    /// Wait for the child referred to by the given PID file descriptor
    #[cfg(linux_android)]
    PIDFd(BorrowedFd<'fd>),
    /// A helper variant to resolve the unused parameter (`'fd`) problem on platforms
    /// other than Linux and Android.
    #[doc(hidden)]
    _Unreachable(std::marker::PhantomData<&'fd std::convert::Infallible>),
}

/// Wait for a process to change status
///
/// See also [waitid(2)](https://pubs.opengroup.org/onlinepubs/9699919799/functions/waitid.html)
#[cfg(any(
    target_os = "android",
    target_os = "freebsd",
    target_os = "haiku",
    all(target_os = "linux", not(target_env = "uclibc")),
))]
pub fn waitid(id: Id, flags: WaitPidFlag) -> Result<WaitStatus> {
    let (idtype, idval) = match id {
        Id::All => (libc::P_ALL, 0),
        Id::Pid(pid) => (libc::P_PID, pid.as_raw() as libc::id_t),
        Id::PGid(pid) => (libc::P_PGID, pid.as_raw() as libc::id_t),
        #[cfg(linux_android)]
        Id::PIDFd(fd) => (libc::P_PIDFD, fd.as_raw_fd() as libc::id_t),
        Id::_Unreachable(_) => {
            unreachable!("This variant could never be constructed")
        }
    };

    let siginfo = unsafe {
        // Memory is zeroed rather than uninitialized, as not all platforms
        // initialize the memory in the StillAlive case
        let mut siginfo: libc::siginfo_t = std::mem::zeroed();
        Errno::result(libc::waitid(idtype, idval, &mut siginfo, flags.bits()))?;
        siginfo
    };

    unsafe { WaitStatus::from_siginfo(&siginfo) }
}
