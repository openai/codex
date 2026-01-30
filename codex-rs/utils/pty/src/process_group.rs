//! Process-group helpers shared by pipe/pty and shell command execution.
//!
//! This module centralizes the OS-specific pieces that ensure a spawned
//! command can be cleaned up reliably:
//! - `set_process_group` is called in `pre_exec` so the child starts its own
//!   process group.
//! - `detach_from_tty` starts a new session so non-interactive children do not
//!   inherit the controlling TTY.
//! - `kill_process_group_by_pid` targets the whole group (children/grandchildren)
//! - `kill_process_group` targets a known process group ID directly
//!   instead of a single PID.
//! - `set_parent_death_signal` (Linux only) arranges for the child to receive a
//!   `SIGTERM` when the parent exits, and re-checks the parent PID to avoid
//!   races during fork/exec.
//!
//! On non-Unix platforms these helpers are no-ops.

use std::io;

use tokio::process::Child;

#[cfg(target_os = "macos")]
use libc::c_char;
#[cfg(target_os = "macos")]
use libc::c_int;
#[cfg(target_os = "macos")]
use libc::c_uint;

#[cfg(target_os = "linux")]
/// Ensure the child receives SIGTERM when the original parent dies.
///
/// This should run in `pre_exec` and uses `parent_pid` captured before spawn to
/// avoid a race where the parent exits between fork and exec.
pub fn set_parent_death_signal(parent_pid: libc::pid_t) -> io::Result<()> {
    if unsafe { libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM) } == -1 {
        return Err(io::Error::last_os_error());
    }

    if unsafe { libc::getppid() } != parent_pid {
        unsafe {
            libc::raise(libc::SIGTERM);
        }
    }

    Ok(())
}

#[cfg(not(target_os = "linux"))]
/// No-op on non-Linux platforms.
pub fn set_parent_death_signal(_parent_pid: i32) -> io::Result<()> {
    Ok(())
}

#[cfg(target_os = "macos")]
/// Reset exception ports to the system crash reporter.
///
/// This breaks any inherited connection to a wrapping crash handler before
/// `exec`, so crashes in the spawned process are reported by the system.
pub fn reset_to_system_crash_reporter() -> bool {
    type MachPort = c_uint;
    type KernReturn = c_int;
    type ExceptionMask = c_uint;
    type ExceptionBehavior = c_int;
    type ThreadStateFlavor = c_int;

    const KERN_SUCCESS: KernReturn = 0;
    const MACH_PORT_NULL: MachPort = 0;
    const MACH_PORT_DEAD: MachPort = MachPort::MAX;

    const EXC_CRASH: u32 = 10;
    const EXC_RESOURCE: u32 = 11;
    const EXC_GUARD: u32 = 12;

    const EXC_MASK_CRASH: ExceptionMask = 1 << EXC_CRASH;
    const EXC_MASK_RESOURCE: ExceptionMask = 1 << EXC_RESOURCE;
    const EXC_MASK_GUARD: ExceptionMask = 1 << EXC_GUARD;

    const EXCEPTION_STATE_IDENTITY: ExceptionBehavior = 3;
    const MACH_EXCEPTION_CODES: ExceptionBehavior = 0x8000_0000u32 as ExceptionBehavior;

    #[cfg(target_arch = "aarch64")]
    const MACHINE_THREAD_STATE: ThreadStateFlavor = 1;
    #[cfg(target_arch = "x86_64")]
    const MACHINE_THREAD_STATE: ThreadStateFlavor = 7;
    #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
    const MACHINE_THREAD_STATE: ThreadStateFlavor = 1;

    unsafe extern "C" {
        static bootstrap_port: MachPort;
        fn bootstrap_look_up(
            bp: MachPort,
            service_name: *const c_char,
            sp: *mut MachPort,
        ) -> KernReturn;
        fn mach_task_self() -> MachPort;
        fn task_set_exception_ports(
            task: MachPort,
            exception_mask: ExceptionMask,
            new_port: MachPort,
            behavior: ExceptionBehavior,
            new_flavor: ThreadStateFlavor,
        ) -> KernReturn;
    }

    const REPORT_CRASH_SERVICE: &[u8] = b"com.apple.ReportCrash\0";
    let service_name = REPORT_CRASH_SERVICE.as_ptr().cast::<c_char>();

    let mut report_crash: MachPort = MACH_PORT_NULL;
    let lookup = unsafe {
        bootstrap_look_up(
            bootstrap_port,
            service_name,
            &mut report_crash as *mut MachPort,
        )
    };
    if lookup != KERN_SUCCESS || report_crash == MACH_PORT_DEAD || report_crash == MACH_PORT_NULL {
        return false;
    }

    // Best-effort: include resource/guard masks, then fall back to crash-only
    // when those exceptions are unsupported in this context.
    let mask = EXC_MASK_CRASH | EXC_MASK_RESOURCE | EXC_MASK_GUARD;
    let behavior = EXCEPTION_STATE_IDENTITY | MACH_EXCEPTION_CODES;

    let task = unsafe { mach_task_self() };
    let primary = unsafe {
        task_set_exception_ports(task, mask, report_crash, behavior, MACHINE_THREAD_STATE)
    };
    if primary == KERN_SUCCESS {
        return true;
    }

    let fallback = unsafe {
        task_set_exception_ports(
            task,
            EXC_MASK_CRASH,
            report_crash,
            behavior,
            MACHINE_THREAD_STATE,
        )
    };
    fallback == KERN_SUCCESS
}

#[cfg(not(target_os = "macos"))]
/// No-op on non-macOS platforms.
pub fn reset_to_system_crash_reporter() -> bool {
    true
}

#[cfg(unix)]
/// Detach from the controlling TTY by starting a new session.
pub fn detach_from_tty() -> io::Result<()> {
    let result = unsafe { libc::setsid() };
    if result == -1 {
        let err = io::Error::last_os_error();
        if err.raw_os_error() == Some(libc::EPERM) {
            return set_process_group();
        }
        return Err(err);
    }
    Ok(())
}

#[cfg(not(unix))]
/// No-op on non-Unix platforms.
pub fn detach_from_tty() -> io::Result<()> {
    Ok(())
}

#[cfg(unix)]
/// Put the calling process into its own process group.
///
/// Intended for use in `pre_exec` so the child becomes the group leader.
pub fn set_process_group() -> io::Result<()> {
    let result = unsafe { libc::setpgid(0, 0) };
    if result == -1 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(not(unix))]
/// No-op on non-Unix platforms.
pub fn set_process_group() -> io::Result<()> {
    Ok(())
}

#[cfg(unix)]
/// Kill the process group for the given PID (best-effort).
///
/// This resolves the PGID for `pid` and sends SIGKILL to the whole group.
pub fn kill_process_group_by_pid(pid: u32) -> io::Result<()> {
    use std::io::ErrorKind;

    let pid = pid as libc::pid_t;
    let pgid = unsafe { libc::getpgid(pid) };
    if pgid == -1 {
        let err = io::Error::last_os_error();
        if err.kind() != ErrorKind::NotFound {
            return Err(err);
        }
        return Ok(());
    }

    let result = unsafe { libc::killpg(pgid, libc::SIGKILL) };
    if result == -1 {
        let err = io::Error::last_os_error();
        if err.kind() != ErrorKind::NotFound {
            return Err(err);
        }
    }

    Ok(())
}

#[cfg(not(unix))]
/// No-op on non-Unix platforms.
pub fn kill_process_group_by_pid(_pid: u32) -> io::Result<()> {
    Ok(())
}

#[cfg(unix)]
/// Kill a specific process group ID (best-effort).
pub fn kill_process_group(process_group_id: u32) -> io::Result<()> {
    use std::io::ErrorKind;

    let pgid = process_group_id as libc::pid_t;
    let result = unsafe { libc::killpg(pgid, libc::SIGKILL) };
    if result == -1 {
        let err = io::Error::last_os_error();
        if err.kind() != ErrorKind::NotFound {
            return Err(err);
        }
    }

    Ok(())
}

#[cfg(not(unix))]
/// No-op on non-Unix platforms.
pub fn kill_process_group(_process_group_id: u32) -> io::Result<()> {
    Ok(())
}

#[cfg(unix)]
/// Kill the process group for a tokio child (best-effort).
pub fn kill_child_process_group(child: &mut Child) -> io::Result<()> {
    if let Some(pid) = child.id() {
        return kill_process_group_by_pid(pid);
    }

    Ok(())
}

#[cfg(not(unix))]
/// No-op on non-Unix platforms.
pub fn kill_child_process_group(_child: &mut Child) -> io::Result<()> {
    Ok(())
}
