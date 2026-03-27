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

#[cfg(windows)]
use std::ffi::OsString;
use std::io;
#[cfg(windows)]
use std::os::windows::ffi::OsStringExt;
#[cfg(windows)]
use std::path::PathBuf;

use tokio::process::Child;
#[cfg(windows)]
use winapi::um::sysinfoapi::GetSystemDirectoryW;

#[cfg(windows)]
fn trusted_taskkill_path() -> PathBuf {
    let mut buffer = [0_u16; 260];
    let len = unsafe { GetSystemDirectoryW(buffer.as_mut_ptr(), buffer.len() as u32) };
    if len > 0 && (len as usize) < buffer.len() {
        let mut path = PathBuf::from(OsString::from_wide(&buffer[..len as usize]));
        path.push("taskkill.exe");
        return path;
    }

    PathBuf::from(r"C:\Windows\System32\taskkill.exe")
}

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

#[cfg(windows)]
/// Best-effort termination of a process tree rooted at `pid`.
///
/// Uses `taskkill /PID <pid> /T /F` so descendants are terminated as well.
pub fn kill_process_group_by_pid(pid: u32) -> io::Result<()> {
    use std::process::Command;

    let command_result = Command::new(trusted_taskkill_path())
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .output();

    // Best-effort cleanup path: timeout/interrupt handling should continue to
    // the direct-child kill path even if taskkill launch or execution fails.
    let _ = command_result;
    Ok(())
}

#[cfg(not(any(unix, windows)))]
/// No-op on non-Unix platforms without a process-group primitive.
pub fn kill_process_group_by_pid(_pid: u32) -> io::Result<()> {
    Ok(())
}

#[cfg(unix)]
/// Send SIGTERM to a specific process group ID (best-effort).
///
/// Returns `Ok(true)` when SIGTERM was delivered to an existing group and
/// `Ok(false)` when the group no longer exists.
pub fn terminate_process_group(process_group_id: u32) -> io::Result<bool> {
    use std::io::ErrorKind;

    let pgid = process_group_id as libc::pid_t;
    let result = unsafe { libc::killpg(pgid, libc::SIGTERM) };
    if result == -1 {
        let err = io::Error::last_os_error();
        if err.kind() == ErrorKind::NotFound {
            return Ok(false);
        }
        return Err(err);
    }

    Ok(true)
}

#[cfg(not(unix))]
/// No-op on non-Unix platforms.
pub fn terminate_process_group(_process_group_id: u32) -> io::Result<bool> {
    Ok(false)
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

#[cfg(windows)]
/// Best-effort termination for a process-tree root id.
pub fn kill_process_group(process_group_id: u32) -> io::Result<()> {
    kill_process_group_by_pid(process_group_id)
}

#[cfg(not(any(unix, windows)))]
/// No-op on non-Unix platforms without a process-group primitive.
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

#[cfg(windows)]
/// Kill the Windows process tree rooted at a tokio child pid (best-effort).
pub fn kill_child_process_group(child: &mut Child) -> io::Result<()> {
    if let Some(pid) = child.id() {
        return kill_process_group_by_pid(pid);
    }

    Ok(())
}

#[cfg(not(any(unix, windows)))]
/// No-op on non-Unix platforms without a process-group primitive.
pub fn kill_child_process_group(_child: &mut Child) -> io::Result<()> {
    Ok(())
}
