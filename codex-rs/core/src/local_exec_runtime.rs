use tokio::process::Command;

/// We don't support Windows yet, so we allow this stub trait for the Windows implementation.
#[cfg_attr(not(unix), allow(dead_code))]
/// Abstraction over platform-specific local exec runtime behavior.
pub(crate) trait LocalExecRuntime: Send + Sync {
    /// Configure the child process prior to exec/spawn (e.g., setpgid on Unix).
    fn configure_child(&self, cmd: &mut Command);

    /// Record a spawned child's pid so signals/cleanup can target it later.
    fn record_child(&self, pid_opt: Option<u32>);

    /// Clear any recorded state.
    fn clear(&self);

    /// Attempt to interrupt any recorded child process tree.
    fn interrupt(&self);
}

#[cfg(unix)]
pub(crate) struct UnixLocalExecRuntime {
    pgid: std::sync::Mutex<Option<i32>>,
}

#[cfg(unix)]
impl UnixLocalExecRuntime {
    pub(crate) fn new() -> Self {
        Self {
            pgid: std::sync::Mutex::new(None),
        }
    }
}

#[cfg(unix)]
impl LocalExecRuntime for UnixLocalExecRuntime {
    fn configure_child(&self, cmd: &mut Command) {
        unsafe {
            cmd.pre_exec(|| {
                libc::setpgid(0, 0);
                Ok(())
            });
        }
    }

    fn record_child(&self, pid_opt: Option<u32>) {
        if let Some(pid_u32) = pid_opt {
            let pid = pid_u32 as i32;
            // If getpgid fails, fall back to pid.
            let pgid = unsafe { libc::getpgid(pid) };
            let value = if pgid > 0 { pgid } else { pid };
            if let Ok(mut guard) = self.pgid.lock() {
                *guard = Some(value);
            }
        }
    }

    fn clear(&self) {
        if let Ok(mut guard) = self.pgid.lock() {
            *guard = None;
        }
    }

    fn interrupt(&self) {
        if let Ok(mut guard) = self.pgid.lock()
            && let Some(pgid) = guard.take()
        {
            unsafe {
                let _ = libc::kill(-pgid, libc::SIGINT);
            }
        }
    }
}

#[cfg(not(unix))]
pub(crate) struct WindowsLocalExecRuntime;

#[cfg(not(unix))]
impl WindowsLocalExecRuntime {
    pub(crate) fn new() -> Self {
        Self
    }
}

#[cfg(not(unix))]
impl LocalExecRuntime for WindowsLocalExecRuntime {
    fn configure_child(&self, _cmd: &mut Command) {}
    fn record_child(&self, _pid_opt: Option<u32>) {}
    fn clear(&self) {}
    fn interrupt(&self) {}
}
