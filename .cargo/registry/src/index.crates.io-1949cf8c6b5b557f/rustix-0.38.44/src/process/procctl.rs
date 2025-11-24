//! Bindings for the FreeBSD `procctl` system call.
//!
//! There are similarities (but also differences) with Linux's `prctl` system
//! call, whose interface is located in the `prctl.rs` file.

#![allow(unsafe_code)]

#[cfg(feature = "alloc")]
use alloc::{vec, vec::Vec};
use core::mem::MaybeUninit;
use core::ptr;

use bitflags::bitflags;

use crate::backend::c::{c_int, c_uint, c_void};
use crate::backend::process::syscalls;
use crate::backend::process::types::RawId;
use crate::io;
use crate::process::{Pid, RawPid};
use crate::signal::Signal;
use crate::utils::{as_mut_ptr, as_ptr};

//
// Helper functions.
//

/// Subset of `idtype_t` C enum, with only the values allowed by `procctl`.
#[repr(i32)]
pub enum IdType {
    /// Process id.
    Pid = 0,
    /// Process group id.
    Pgid = 2,
}

/// A process selector for use with the `procctl` interface.
///
/// `None` represents the current process. `Some((IdType::Pid, pid))`
/// represents the process with pid `pid`. `Some((IdType::Pgid, pgid))`
/// represents the control processes belonging to the process group with id
/// `pgid`.
pub type ProcSelector = Option<(IdType, Pid)>;
fn proc_selector_to_raw(selector: ProcSelector) -> (IdType, RawPid) {
    match selector {
        Some((idtype, id)) => (idtype, id.as_raw_nonzero().get()),
        None => (IdType::Pid, 0),
    }
}

#[inline]
pub(crate) unsafe fn procctl(
    option: c_int,
    process: ProcSelector,
    data: *mut c_void,
) -> io::Result<()> {
    let (idtype, id) = proc_selector_to_raw(process);
    syscalls::procctl(idtype as c_uint, id as RawId, option, data)
}

#[inline]
pub(crate) unsafe fn procctl_set<P>(
    option: c_int,
    process: ProcSelector,
    data: &P,
) -> io::Result<()> {
    procctl(option, process, (as_ptr(data) as *mut P).cast())
}

#[inline]
pub(crate) unsafe fn procctl_get_optional<P>(
    option: c_int,
    process: ProcSelector,
) -> io::Result<P> {
    let mut value: MaybeUninit<P> = MaybeUninit::uninit();
    procctl(option, process, value.as_mut_ptr().cast())?;
    Ok(value.assume_init())
}

//
// PROC_PDEATHSIG_STATUS/PROC_PDEATHSIG_CTL
//

const PROC_PDEATHSIG_STATUS: c_int = 12;

/// Get the current value of the parent process death signal.
///
/// # References
///  - [Linux: `prctl(PR_GET_PDEATHSIG,…)`]
///  - [FreeBSD: `procctl(PROC_PDEATHSIG_STATUS,…)`]
///
/// [Linux: `prctl(PR_GET_PDEATHSIG,…)`]: https://man7.org/linux/man-pages/man2/prctl.2.html
/// [FreeBSD: `procctl(PROC_PDEATHSIG_STATUS,…)`]: https://man.freebsd.org/cgi/man.cgi?query=procctl&sektion=2
#[inline]
pub fn parent_process_death_signal() -> io::Result<Option<Signal>> {
    unsafe { procctl_get_optional::<c_int>(PROC_PDEATHSIG_STATUS, None) }.map(Signal::from_raw)
}

const PROC_PDEATHSIG_CTL: c_int = 11;

/// Set the parent-death signal of the calling process.
///
/// # References
///  - [Linux: `prctl(PR_SET_PDEATHSIG,…)`]
///  - [FreeBSD: `procctl(PROC_PDEATHSIG_CTL,…)`]
///
/// [Linux: `prctl(PR_SET_PDEATHSIG,…)`]: https://man7.org/linux/man-pages/man2/prctl.2.html
/// [FreeBSD: `procctl(PROC_PDEATHSIG_CTL,…)`]: https://man.freebsd.org/cgi/man.cgi?query=procctl&sektion=2
#[inline]
pub fn set_parent_process_death_signal(signal: Option<Signal>) -> io::Result<()> {
    let signal = signal.map_or(0, |signal| signal as c_int);
    unsafe { procctl_set::<c_int>(PROC_PDEATHSIG_CTL, None, &signal) }
}

//
// PROC_TRACE_CTL
//

const PROC_TRACE_CTL: c_int = 7;

const PROC_TRACE_CTL_ENABLE: i32 = 1;
const PROC_TRACE_CTL_DISABLE: i32 = 2;
const PROC_TRACE_CTL_DISABLE_EXEC: i32 = 3;

/// `PROC_TRACE_CTL_*`.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum DumpableBehavior {
    /// Not dumpable.
    NotDumpable = PROC_TRACE_CTL_DISABLE,
    /// Dumpable.
    Dumpable = PROC_TRACE_CTL_ENABLE,
    /// Not dumpable, and this behaviour is preserved across `execve` calls.
    NotDumpableExecPreserved = PROC_TRACE_CTL_DISABLE_EXEC,
}

/// Set the state of the `dumpable` attribute for the process indicated by
/// `idtype` and `id`.
///
/// This determines whether the process can be traced and whether core dumps
/// are produced for the process upon delivery of a signal whose default
/// behavior is to produce a core dump.
///
/// This is similar to `set_dumpable_behavior` on Linux, with the exception
/// that on FreeBSD there is an extra argument `process`. When `process` is set
/// to `None`, the operation is performed for the current process, like on
/// Linux.
///
/// # References
///  - [FreeBSD `procctl(PROC_TRACE_CTL,…)`]
///
/// [FreeBSD `procctl(PROC_TRACE_CTL,…)`]: https://man.freebsd.org/cgi/man.cgi?query=procctl&sektion=2
#[inline]
pub fn set_dumpable_behavior(process: ProcSelector, config: DumpableBehavior) -> io::Result<()> {
    unsafe { procctl(PROC_TRACE_CTL, process, config as usize as *mut _) }
}

//
// PROC_TRACE_STATUS
//

const PROC_TRACE_STATUS: c_int = 8;

/// Tracing status as returned by [`trace_status`].
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum TracingStatus {
    /// Tracing is disabled for the process.
    NotTraceble,
    /// Tracing is not disabled for the process, but not debugger/tracer is
    /// attached.
    Tracable,
    /// The process is being traced by the process whose pid is stored in the
    /// first component of this variant.
    BeingTraced(Pid),
}

/// Get the tracing status of the process indicated by `idtype` and `id`.
///
/// # References
///  - [FreeBSD `procctl(PROC_TRACE_STATUS,…)`]
///
/// [FreeBSD `procctl(PROC_TRACE_STATUS,…)`]: https://man.freebsd.org/cgi/man.cgi?query=procctl&sektion=2
#[inline]
pub fn trace_status(process: ProcSelector) -> io::Result<TracingStatus> {
    let val = unsafe { procctl_get_optional::<c_int>(PROC_TRACE_STATUS, process) }?;
    match val {
        -1 => Ok(TracingStatus::NotTraceble),
        0 => Ok(TracingStatus::Tracable),
        pid => {
            let pid = Pid::from_raw(pid as RawPid).ok_or(io::Errno::RANGE)?;
            Ok(TracingStatus::BeingTraced(pid))
        }
    }
}

//
// PROC_REAP_*
//

const PROC_REAP_ACQUIRE: c_int = 2;
const PROC_REAP_RELEASE: c_int = 3;

/// Acquire or release the reaper status of the calling process.
///
/// # References
///  - [FreeBSD: `procctl(PROC_REAP_ACQUIRE/RELEASE,…)`]
///
/// [FreeBSD: `procctl(PROC_REAP_ACQUIRE/RELEASE,…)`]: https://man.freebsd.org/cgi/man.cgi?query=procctl&sektion=2
#[inline]
pub fn set_reaper_status(reaper: bool) -> io::Result<()> {
    unsafe {
        procctl(
            if reaper {
                PROC_REAP_ACQUIRE
            } else {
                PROC_REAP_RELEASE
            },
            None,
            ptr::null_mut(),
        )
    }
}

const PROC_REAP_STATUS: c_int = 4;

bitflags! {
    /// `REAPER_STATUS_*`.
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct ReaperStatusFlags: c_uint {
        /// The process has acquired reaper status.
        const OWNED = 1;
        /// The process is the root of the reaper tree (pid 1).
        const REALINIT = 2;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

#[repr(C)]
struct procctl_reaper_status {
    rs_flags: c_uint,
    rs_children: c_uint,
    rs_descendants: c_uint,
    rs_reaper: RawPid,
    rs_pid: RawPid,
    rs_pad0: [c_uint; 15],
}

/// Reaper status as returned by [`get_reaper_status`].
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct ReaperStatus {
    /// The flags.
    pub flags: ReaperStatusFlags,
    /// The number of children of the reaper among the descendants.
    pub children: usize,
    /// The total number of descendants of the reaper(s), not counting
    /// descendants of the reaper in the subtree.
    pub descendants: usize,
    /// The pid of the reaper for the specified process id.
    pub reaper: Pid,
    /// The pid of one reaper child if there are any descendants.
    pub pid: Option<Pid>,
}

/// Get information about the reaper of the specified process (or the process
/// itself if it is a reaper).
///
/// # References
///  - [FreeBSD: `procctl(PROC_REAP_STATUS,…)`]
///
/// [FreeBSD: `procctl(PROC_REAP_STATUS,…)`]: https://man.freebsd.org/cgi/man.cgi?query=procctl&sektion=2
#[inline]
pub fn get_reaper_status(process: ProcSelector) -> io::Result<ReaperStatus> {
    let raw = unsafe { procctl_get_optional::<procctl_reaper_status>(PROC_REAP_STATUS, process) }?;
    Ok(ReaperStatus {
        flags: ReaperStatusFlags::from_bits_retain(raw.rs_flags),
        children: raw.rs_children as _,
        descendants: raw.rs_descendants as _,
        reaper: Pid::from_raw(raw.rs_reaper).ok_or(io::Errno::RANGE)?,
        pid: if raw.rs_pid == -1 {
            None
        } else {
            Some(Pid::from_raw(raw.rs_pid).ok_or(io::Errno::RANGE)?)
        },
    })
}

#[cfg(feature = "alloc")]
const PROC_REAP_GETPIDS: c_int = 5;

bitflags! {
    /// `REAPER_PIDINFO_*`.
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct PidInfoFlags: c_uint {
        /// This structure was filled by the kernel.
        const VALID = 1;
        /// The pid field identifies a direct child of the reaper.
        const CHILD = 2;
        /// The reported process is itself a reaper. Descendants of a
        /// subordinate reaper are not reported.
        const REAPER = 4;
        /// The reported process is in the zombie state.
        const ZOMBIE = 8;
        /// The reported process is stopped by
        /// [`Signal::Stop`]/[`Signal::Tstp`].
        const STOPPED = 16;
        /// The reported process is in the process of exiting.
        const EXITING = 32;
    }
}

#[repr(C)]
#[derive(Default, Clone)]
struct procctl_reaper_pidinfo {
    pi_pid: RawPid,
    pi_subtree: RawPid,
    pi_flags: c_uint,
    pi_pad0: [c_uint; 15],
}

#[repr(C)]
struct procctl_reaper_pids {
    rp_count: c_uint,
    rp_pad0: [c_uint; 15],
    rp_pids: *mut procctl_reaper_pidinfo,
}

/// A child process of a reaper.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct PidInfo {
    /// The flags of the process.
    pub flags: PidInfoFlags,
    /// The pid of the process.
    pub pid: Pid,
    /// The pid of the child of the reaper which is the (grand-…)parent of the
    /// process.
    pub subtree: Pid,
}

/// Get the list of descendants of the specified reaper process.
///
/// # References
///  - [FreeBSD: `procctl(PROC_REAP_GETPIDS,…)`]
///
/// [FreeBSD: `procctl(PROC_REAP_GETPIDS,…)`]: https://man.freebsd.org/cgi/man.cgi?query=procctl&sektion=2
#[cfg(feature = "alloc")]
pub fn get_reaper_pids(process: ProcSelector) -> io::Result<Vec<PidInfo>> {
    // Sadly no better way to guarantee that we get all the results than to
    // allocate ≈8MB of memory…
    const PID_MAX: usize = 99999;
    let mut pids: Vec<procctl_reaper_pidinfo> = vec![Default::default(); PID_MAX];
    let mut pinfo = procctl_reaper_pids {
        rp_count: PID_MAX as _,
        rp_pad0: [0; 15],
        rp_pids: pids.as_mut_slice().as_mut_ptr(),
    };
    unsafe { procctl(PROC_REAP_GETPIDS, process, as_mut_ptr(&mut pinfo).cast())? };
    let mut result = Vec::new();
    for raw in pids.into_iter() {
        let flags = PidInfoFlags::from_bits_retain(raw.pi_flags);
        if !flags.contains(PidInfoFlags::VALID) {
            break;
        }
        result.push(PidInfo {
            flags,
            subtree: Pid::from_raw(raw.pi_subtree).ok_or(io::Errno::RANGE)?,
            pid: Pid::from_raw(raw.pi_pid).ok_or(io::Errno::RANGE)?,
        });
    }
    Ok(result)
}

const PROC_REAP_KILL: c_int = 6;

bitflags! {
    /// `REAPER_KILL_*`.
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    struct KillFlags: c_uint {
        const CHILDREN = 1;
        const SUBTREE = 2;
    }
}

#[repr(C)]
struct procctl_reaper_kill {
    rk_sig: c_int,
    rk_flags: c_uint,
    rk_subtree: RawPid,
    rk_killed: c_uint,
    rk_fpid: RawPid,
    rk_pad0: [c_uint; 15],
}

/// Reaper status as returned by [`get_reaper_status`].
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct KillResult {
    /// The number of processes that were signalled.
    pub killed: usize,
    /// The pid of the first process that wasn't successfully signalled.
    pub first_failed: Option<Pid>,
}

/// Deliver a signal to some subset of the descendants of the reaper.
///
/// # References
///  - [FreeBSD: `procctl(PROC_REAP_KILL,…)`]
///
/// [FreeBSD: `procctl(PROC_REAP_KILL,…)`]: https://man.freebsd.org/cgi/man.cgi?query=procctl&sektion=2
pub fn reaper_kill(
    process: ProcSelector,
    signal: Signal,
    direct_children: bool,
    subtree: Option<Pid>,
) -> io::Result<KillResult> {
    let mut flags = KillFlags::empty();
    flags.set(KillFlags::CHILDREN, direct_children);
    flags.set(KillFlags::SUBTREE, subtree.is_some());
    let mut req = procctl_reaper_kill {
        rk_sig: signal as c_int,
        rk_flags: flags.bits(),
        rk_subtree: subtree.map(|p| p.as_raw_nonzero().into()).unwrap_or(0),
        rk_killed: 0,
        rk_fpid: 0,
        rk_pad0: [0; 15],
    };
    unsafe { procctl(PROC_REAP_KILL, process, as_mut_ptr(&mut req).cast())? };
    Ok(KillResult {
        killed: req.rk_killed as _,
        first_failed: Pid::from_raw(req.rk_fpid),
    })
}

//
// PROC_TRAPCAP_STATUS/PROC_TRAPCAP_CTL
//

const PROC_TRAPCAP_CTL: c_int = 9;

const PROC_TRAPCAP_CTL_ENABLE: i32 = 1;
const PROC_TRAPCAP_CTL_DISABLE: i32 = 2;

/// `PROC_TRAPCAP_CTL_*`.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum TrapCapBehavior {
    /// Disable the [`Signal::Trap`] signal delivery on capability mode access
    /// violations.
    Disable = PROC_TRAPCAP_CTL_DISABLE,
    /// Enable the [`Signal::Trap`] signal delivery on capability mode access
    /// violations.
    Enable = PROC_TRAPCAP_CTL_ENABLE,
}

/// Set the current value of the capability mode violation trapping behavior.
///
/// If this behavior is enabled, the kernel would deliver a [`Signal::Trap`]
/// signal on any return from a system call that would result in a
/// [`io::Errno::NOTCAPABLE`] or [`io::Errno::CAPMODE`] error.
///
/// This behavior is inherited by the children of the process and is kept
/// across `execve` calls.
///
/// # References
///  - [FreeBSD: `procctl(PROC_TRAPCAP_CTL,…)`]
///
/// [FreeBSD: `procctl(PROC_TRAPCAP_CTL,…)`]: https://man.freebsd.org/cgi/man.cgi?query=procctl&sektion=2
#[inline]
pub fn set_trap_cap_behavior(process: ProcSelector, config: TrapCapBehavior) -> io::Result<()> {
    let config = config as c_int;
    unsafe { procctl_set::<c_int>(PROC_TRAPCAP_CTL, process, &config) }
}

const PROC_TRAPCAP_STATUS: c_int = 10;

/// Get the current value of the capability mode violation trapping behavior.
///
/// # References
///  - [FreeBSD: `procctl(PROC_TRAPCAP_STATUS,…)`]
///
/// [FreeBSD: `procctl(PROC_TRAPCAP_STATUS,…)`]: https://man.freebsd.org/cgi/man.cgi?query=procctl&sektion=2
#[inline]
pub fn trap_cap_behavior(process: ProcSelector) -> io::Result<TrapCapBehavior> {
    let val = unsafe { procctl_get_optional::<c_int>(PROC_TRAPCAP_STATUS, process) }?;
    match val {
        PROC_TRAPCAP_CTL_DISABLE => Ok(TrapCapBehavior::Disable),
        PROC_TRAPCAP_CTL_ENABLE => Ok(TrapCapBehavior::Enable),
        _ => Err(io::Errno::RANGE),
    }
}

//
// PROC_NO_NEW_PRIVS_STATUS/PROC_NO_NEW_PRIVS_CTL
//

const PROC_NO_NEW_PRIVS_CTL: c_int = 19;

const PROC_NO_NEW_PRIVS_ENABLE: c_int = 1;

/// Enable the `no_new_privs` mode that ignores SUID and SGID bits on `execve`
/// in the specified process and its future descendants.
///
/// This is similar to `set_no_new_privs` on Linux, with the exception that on
/// FreeBSD there is no argument `no_new_privs` argument as it's only possible
/// to enable this mode and there's no going back.
///
/// # References
///  - [Linux: `prctl(PR_SET_NO_NEW_PRIVS,…)`]
///  - [FreeBSD: `procctl(PROC_NO_NEW_PRIVS_CTL,…)`]
///
/// [Linux: `prctl(PR_SET_NO_NEW_PRIVS,…)`]: https://man7.org/linux/man-pages/man2/prctl.2.html
/// [FreeBSD: `procctl(PROC_NO_NEW_PRIVS_CTL,…)`]: https://man.freebsd.org/cgi/man.cgi?query=procctl&sektion=2
#[inline]
pub fn set_no_new_privs(process: ProcSelector) -> io::Result<()> {
    unsafe { procctl_set::<c_int>(PROC_NO_NEW_PRIVS_CTL, process, &PROC_NO_NEW_PRIVS_ENABLE) }
}

const PROC_NO_NEW_PRIVS_STATUS: c_int = 20;

/// Check the `no_new_privs` mode of the specified process.
///
/// # References
///  - [Linux: `prctl(PR_GET_NO_NEW_PRIVS,…)`]
///  - [FreeBSD: `procctl(PROC_NO_NEW_PRIVS_STATUS,…)`]
///
/// [Linux: `prctl(PR_GET_NO_NEW_PRIVS,…)`]: https://man7.org/linux/man-pages/man2/prctl.2.html
/// [FreeBSD: `procctl(PROC_NO_NEW_PRIVS_STATUS,…)`]: https://man.freebsd.org/cgi/man.cgi?query=procctl&sektion=2
#[inline]
pub fn no_new_privs(process: ProcSelector) -> io::Result<bool> {
    unsafe { procctl_get_optional::<c_int>(PROC_NO_NEW_PRIVS_STATUS, process) }
        .map(|x| x == PROC_NO_NEW_PRIVS_ENABLE)
}
