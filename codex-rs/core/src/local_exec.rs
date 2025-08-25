use std::sync::Mutex;

#[cfg(unix)]
pub(crate) struct LocalExecRuntime {
    pgid: Mutex<Option<i32>>,
}

#[cfg(not(unix))]
pub(crate) struct LocalExecRuntime {
    running: Mutex<bool>,
}

impl LocalExecRuntime {
    pub(crate) fn new() -> Self {
        #[cfg(unix)]
        {
            Self {
                pgid: Mutex::new(None),
            }
        }
        #[cfg(not(unix))]
        {
            Self {
                running: Mutex::new(false),
            }
        }
    }
}

/// Configure child process before exec: on Unix, create a new process group so
/// we can signal the entire tree later.
#[cfg(unix)]
pub(crate) fn configure_child(cmd: &mut tokio::process::Command) {
    unsafe {
        cmd.pre_exec(|| {
            libc::setpgid(0, 0);
            Ok(())
        });
    }
}

/// No-op on non-Unix platforms.
#[cfg(not(unix))]
pub(crate) fn configure_child(_cmd: &mut tokio::process::Command) {}

/// Record the spawned child so future interrupts can target it.
#[cfg(unix)]
pub(crate) fn record_child(runtime: &LocalExecRuntime, pid_opt: Option<u32>) {
    if let Some(pid_u32) = pid_opt {
        let pid = pid_u32 as i32;
        // If getpgid fails, fall back to pid.
        let pgid = unsafe { libc::getpgid(pid) };
        let value = if pgid > 0 { pgid } else { pid };
        if let Ok(mut guard) = runtime.pgid.lock() {
            *guard = Some(value);
        }
    }
}

/// Minimal tracking on non-Unix platforms for in-progress state.
#[cfg(not(unix))]
pub(crate) fn record_child(runtime: &LocalExecRuntime, _pid_opt: Option<u32>) {
    if let Ok(mut guard) = runtime.running.lock() {
        *guard = true;
    }
}

/// Clear any recorded child state after it exits or upon spawn failure.
pub(crate) fn clear(runtime: &LocalExecRuntime) {
    #[cfg(unix)]
    {
        if let Ok(mut guard) = runtime.pgid.lock() {
            *guard = None;
        }
    }
    #[cfg(not(unix))]
    {
        if let Ok(mut guard) = runtime.running.lock() {
            *guard = false;
        }
    }
}

/// Attempt to interrupt a recorded child process tree.
pub(crate) fn interrupt(runtime: &LocalExecRuntime) {
    #[cfg(unix)]
    {
        if let Ok(mut guard) = runtime.pgid.lock()
            && let Some(pgid) = guard.take()
        {
            unsafe {
                let _ = libc::kill(-pgid, libc::SIGINT);
            }
        }
    }
    #[cfg(not(unix))]
    {
        if let Ok(mut guard) = runtime.running.lock() {
            *guard = false;
        }
    }
}
