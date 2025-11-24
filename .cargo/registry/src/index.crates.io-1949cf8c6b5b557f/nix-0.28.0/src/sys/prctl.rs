//! prctl is a Linux-only API for performing operations on a process or thread.
//!
//! Note that careless use of some prctl() operations can confuse the user-space run-time
//! environment, so these operations should be used with care.
//!
//! For more documentation, please read [prctl(2)](https://man7.org/linux/man-pages/man2/prctl.2.html).

use crate::errno::Errno;
use crate::sys::signal::Signal;
use crate::Result;

use libc::{c_int, c_ulong};
use std::convert::TryFrom;
use std::ffi::{CStr, CString};

libc_enum! {
    /// The type of hardware memory corruption kill policy for the thread.

    #[repr(i32)]
    #[non_exhaustive]
    #[allow(non_camel_case_types)]
    pub enum PrctlMCEKillPolicy {
        /// The thread will receive SIGBUS as soon as a memory corruption is detected.
        PR_MCE_KILL_EARLY,
        /// The process is killed only when it accesses a corrupted page.
        PR_MCE_KILL_LATE,
        /// Uses the system-wide default.
        PR_MCE_KILL_DEFAULT,
    }
    impl TryFrom<i32>
}

fn prctl_set_bool(option: c_int, status: bool) -> Result<()> {
    let res = unsafe { libc::prctl(option, status as c_ulong, 0, 0, 0) };
    Errno::result(res).map(drop)
}

fn prctl_get_bool(option: c_int) -> Result<bool> {
    let res = unsafe { libc::prctl(option, 0, 0, 0, 0) };
    Errno::result(res).map(|res| res != 0)
}

/// Set the "child subreaper" attribute for this process
pub fn set_child_subreaper(attribute: bool) -> Result<()> {
    prctl_set_bool(libc::PR_SET_CHILD_SUBREAPER, attribute)
}

/// Get the "child subreaper" attribute for this process
pub fn get_child_subreaper() -> Result<bool> {
    // prctl writes into this var
    let mut subreaper: c_int = 0;

    let res = unsafe {
        libc::prctl(libc::PR_GET_CHILD_SUBREAPER, &mut subreaper, 0, 0, 0)
    };

    Errno::result(res).map(|_| subreaper != 0)
}

/// Set the dumpable attribute which determines if core dumps are created for this process.
pub fn set_dumpable(attribute: bool) -> Result<()> {
    prctl_set_bool(libc::PR_SET_DUMPABLE, attribute)
}

/// Get the dumpable attribute for this process.
pub fn get_dumpable() -> Result<bool> {
    prctl_get_bool(libc::PR_GET_DUMPABLE)
}

/// Set the "keep capabilities" attribute for this process. This causes the thread to retain
/// capabilities even if it switches its UID to a nonzero value.
pub fn set_keepcaps(attribute: bool) -> Result<()> {
    prctl_set_bool(libc::PR_SET_KEEPCAPS, attribute)
}

/// Get the "keep capabilities" attribute for this process
pub fn get_keepcaps() -> Result<bool> {
    prctl_get_bool(libc::PR_GET_KEEPCAPS)
}

/// Clear the thread memory corruption kill policy and use the system-wide default
pub fn clear_mce_kill() -> Result<()> {
    let res = unsafe {
        libc::prctl(libc::PR_MCE_KILL, libc::PR_MCE_KILL_CLEAR, 0, 0, 0)
    };

    Errno::result(res).map(drop)
}

/// Set the thread memory corruption kill policy
pub fn set_mce_kill(policy: PrctlMCEKillPolicy) -> Result<()> {
    let res = unsafe {
        libc::prctl(
            libc::PR_MCE_KILL,
            libc::PR_MCE_KILL_SET,
            policy as c_ulong,
            0,
            0,
        )
    };

    Errno::result(res).map(drop)
}

/// Get the thread memory corruption kill policy
pub fn get_mce_kill() -> Result<PrctlMCEKillPolicy> {
    let res = unsafe { libc::prctl(libc::PR_MCE_KILL_GET, 0, 0, 0, 0) };

    match Errno::result(res) {
        Ok(val) => Ok(PrctlMCEKillPolicy::try_from(val)?),
        Err(e) => Err(e),
    }
}

/// Set the parent-death signal of the calling process. This is the signal that the calling process
/// will get when its parent dies.
pub fn set_pdeathsig<T: Into<Option<Signal>>>(signal: T) -> Result<()> {
    let sig = match signal.into() {
        Some(s) => s as c_int,
        None => 0,
    };

    let res = unsafe { libc::prctl(libc::PR_SET_PDEATHSIG, sig, 0, 0, 0) };

    Errno::result(res).map(drop)
}

/// Returns the current parent-death signal
pub fn get_pdeathsig() -> Result<Option<Signal>> {
    // prctl writes into this var
    let mut sig: c_int = 0;

    let res = unsafe { libc::prctl(libc::PR_GET_PDEATHSIG, &mut sig, 0, 0, 0) };

    match Errno::result(res) {
        Ok(_) => Ok(match sig {
            0 => None,
            _ => Some(Signal::try_from(sig)?),
        }),
        Err(e) => Err(e),
    }
}

/// Set the name of the calling thread. Strings longer than 15 bytes will be truncated.
pub fn set_name(name: &CStr) -> Result<()> {
    let res = unsafe { libc::prctl(libc::PR_SET_NAME, name.as_ptr(), 0, 0, 0) };

    Errno::result(res).map(drop)
}

/// Return the name of the calling thread
pub fn get_name() -> Result<CString> {
    // Size of buffer determined by linux/sched.h TASK_COMM_LEN
    let buf = [0u8; 16];

    let res = unsafe { libc::prctl(libc::PR_GET_NAME, &buf, 0, 0, 0) };

    Errno::result(res).and_then(|_| {
        CStr::from_bytes_until_nul(&buf)
            .map(CStr::to_owned)
            .map_err(|_| Errno::EINVAL)
    })
}

/// Sets the timer slack value for the calling thread. Timer slack is used by the kernel to group
/// timer expirations and make them the supplied amount of nanoseconds late.
pub fn set_timerslack(ns: u64) -> Result<()> {
    let res = unsafe { libc::prctl(libc::PR_SET_TIMERSLACK, ns, 0, 0, 0) };

    Errno::result(res).map(drop)
}

/// Get the timerslack for the calling thread.
pub fn get_timerslack() -> Result<i32> {
    let res = unsafe { libc::prctl(libc::PR_GET_TIMERSLACK, 0, 0, 0, 0) };

    Errno::result(res)
}

/// Disable all performance counters attached to the calling process.
pub fn task_perf_events_disable() -> Result<()> {
    let res =
        unsafe { libc::prctl(libc::PR_TASK_PERF_EVENTS_DISABLE, 0, 0, 0, 0) };

    Errno::result(res).map(drop)
}

/// Enable all performance counters attached to the calling process.
pub fn task_perf_events_enable() -> Result<()> {
    let res =
        unsafe { libc::prctl(libc::PR_TASK_PERF_EVENTS_ENABLE, 0, 0, 0, 0) };

    Errno::result(res).map(drop)
}

/// Set the calling threads "no new privs" attribute. Once set this option can not be unset.
pub fn set_no_new_privs() -> Result<()> {
    prctl_set_bool(libc::PR_SET_NO_NEW_PRIVS, true) // Cannot be unset
}

/// Get the "no new privs" attribute for the calling thread.
pub fn get_no_new_privs() -> Result<bool> {
    prctl_get_bool(libc::PR_GET_NO_NEW_PRIVS)
}

/// Set the state of the "THP disable" flag for the calling thread. Setting this disables
/// transparent huge pages.
pub fn set_thp_disable(flag: bool) -> Result<()> {
    prctl_set_bool(libc::PR_SET_THP_DISABLE, flag)
}

/// Get the "THP disable" flag for the calling thread.
pub fn get_thp_disable() -> Result<bool> {
    prctl_get_bool(libc::PR_GET_THP_DISABLE)
}
