//! Native spawning for macOS MCP executables, without rewriting script paths.
//!
//! Rust falls back to fork for a historical relative-path/cwd bug in Apple's
//! `posix_spawnp`. Calling `posix_spawn` directly avoids that wrapper. This module only
//! accepts the launcher's cleared-environment command shape, with piped stdio,
//! a new process group, and default `argv[0]`. Bare commands search the child's
//! PATH. Unsuccessful searches and executable files without shebangs retain the
//! existing launcher. Each native child owns its PID until it has been reaped.

use std::ffi::CString;
use std::ffi::OsStr;
use std::io;
use std::os::fd::AsRawFd;
use std::os::fd::FromRawFd;
use std::os::fd::OwnedFd;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::process::ExitStatusExt;
use std::path::Path;
use std::process::ExitStatus;
use std::ptr;

use tokio::process::Child as TokioChild;
use tokio::process::ChildStderr;
use tokio::process::ChildStdin;
use tokio::process::ChildStdout;
use tokio::process::Command;
use tokio::signal::unix::Signal;
use tokio::signal::unix::SignalKind;
use tokio::signal::unix::signal;

/// Matches Tokio's child API while keeping native spawning private to macOS.
pub struct Child {
    inner: ChildKind,
    pub stdin: Option<ChildStdin>,
    pub stdout: Option<ChildStdout>,
    pub stderr: Option<ChildStderr>,
}

enum ChildKind {
    Tokio(TokioChild),
    Native(NativeChild),
}

impl Child {
    /// Uses native spawning for relative paths and bare names, retaining Tokio's
    /// fallback for unsuccessful PATH searches and executable text without a shebang.
    pub(super) fn spawn(mut command: Command) -> io::Result<Self> {
        let program = command.as_std().get_program();
        if Path::new(program).is_relative()
            && !program.is_empty()
            && let Some((child, stdin, stdout, stderr)) = NativeChild::spawn(command.as_std())?
        {
            return Ok(Self {
                inner: ChildKind::Native(child),
                stdin: Some(stdin),
                stdout: Some(stdout),
                stderr: Some(stderr),
            });
        }
        let mut child = command.spawn()?;
        Ok(Self {
            stdin: child.stdin.take(),
            stdout: child.stdout.take(),
            stderr: child.stderr.take(),
            inner: ChildKind::Tokio(child),
        })
    }

    pub fn id(&self) -> Option<u32> {
        match &self.inner {
            ChildKind::Tokio(child) => child.id(),
            ChildKind::Native(child) => child.id(),
        }
    }

    pub async fn wait(&mut self) -> io::Result<ExitStatus> {
        self.stdin.take();
        match &mut self.inner {
            ChildKind::Tokio(child) => child.wait().await,
            ChildKind::Native(child) => child.wait().await,
        }
    }

    pub async fn kill(&mut self) -> io::Result<()> {
        self.stdin.take();
        match &mut self.inner {
            ChildKind::Tokio(child) => child.kill().await,
            ChildKind::Native(child) => child.kill().await,
        }
    }
}

// libc does not expose this Apple extension. It is available since macOS 10.15,
// before Codex's minimum supported macOS version (12).
unsafe extern "C" {
    fn posix_spawn_file_actions_addchdir_np(
        actions: *mut libc::posix_spawn_file_actions_t,
        path: *const libc::c_char,
    ) -> libc::c_int;
}

/// Owns a child PID until reaping, so cancellation cannot lose or reuse it.
/// Dropping a live child kills it and reaps it independently of the Tokio runtime.
struct NativeChild {
    pid: Option<libc::pid_t>,
    status: Option<ExitStatus>,
    sigchld: Signal,
}

impl NativeChild {
    /// Spawns the MCP command without changing its executable path or `argv[0]`.
    /// The caller must clear inherited environment variables before setting the
    /// child's environment, because only explicit command entries are copied.
    fn spawn(
        command: &std::process::Command,
    ) -> io::Result<Option<(Self, ChildStdin, ChildStdout, ChildStderr)>> {
        let program = c_string(command.get_program())?;
        let search_path = !program.as_bytes().contains(&b'/');
        let args = std::iter::once(command.get_program())
            .chain(command.get_args())
            .map(c_string)
            .collect::<io::Result<Vec<_>>>()?;
        let argv = args
            .iter()
            .map(|arg| arg.as_ptr().cast_mut())
            .chain(std::iter::once(ptr::null_mut()))
            .collect::<Vec<_>>();
        let env = command
            .get_envs()
            .filter_map(|(key, value)| value.map(|value| (key, value)))
            .map(|(key, value)| {
                let mut entry = key.to_os_string();
                entry.push("=");
                entry.push(value);
                c_string(&entry)
            })
            .collect::<io::Result<Vec<_>>>()?;
        let envp = env
            .iter()
            .map(|entry| entry.as_ptr().cast_mut())
            .chain(std::iter::once(ptr::null_mut()))
            .collect::<Vec<_>>();
        let cwd = command
            .get_current_dir()
            .map(|cwd| c_string(cwd.as_os_str()))
            .transpose()?;

        // Subscribe before spawning so a child that exits immediately cannot be missed.
        let sigchld = signal(SignalKind::child())?;
        let (stdin_read, stdin_write) = io::pipe()?;
        let (stdout_read, stdout_write) = io::pipe()?;
        let (stderr_read, stderr_write) = io::pipe()?;
        let child_fds = [
            child_fd(stdin_read.into())?,
            child_fd(stdout_write.into())?,
            child_fd(stderr_write.into())?,
        ];
        let stdin = ChildStdin::from_std(OwnedFd::from(stdin_write).into())?;
        let stdout = ChildStdout::from_std(OwnedFd::from(stdout_read).into())?;
        let stderr = ChildStderr::from_std(OwnedFd::from(stderr_read).into())?;

        let mut actions = FileActions(ptr::null_mut());
        let mut attrs = Attributes(ptr::null_mut());
        let mut pid = 0;
        // SAFETY: All C strings and pipe descriptors outlive this synchronous
        // spawn. The initialized action/attribute objects are destroyed by RAII.
        let result = unsafe {
            cvt(libc::posix_spawn_file_actions_init(&mut actions.0))?;
            cvt(libc::posix_spawnattr_init(&mut attrs.0))?;
            if let Some(cwd) = &cwd {
                cvt(posix_spawn_file_actions_addchdir_np(
                    &mut actions.0,
                    cwd.as_ptr(),
                ))?;
            }
            for (target, source) in child_fds.iter().enumerate() {
                cvt(libc::posix_spawn_file_actions_adddup2(
                    &mut actions.0,
                    source.as_raw_fd(),
                    target as i32,
                ))?;
            }
            cvt(libc::posix_spawnattr_setpgroup(
                &mut attrs.0,
                /*pgroup*/ 0,
            ))?;
            let mut defaults = 0;
            cvt_errno(libc::sigemptyset(&mut defaults))?;
            cvt_errno(libc::sigaddset(&mut defaults, libc::SIGPIPE))?;
            cvt(libc::posix_spawnattr_setsigdefault(&mut attrs.0, &defaults))?;
            // Match Command's descriptor inheritance: honor FD_CLOEXEC rather
            // than introducing a different policy with CLOEXEC_DEFAULT.
            cvt(libc::posix_spawnattr_setflags(
                &mut attrs.0,
                (libc::POSIX_SPAWN_SETPGROUP | libc::POSIX_SPAWN_SETSIGDEF) as _,
            ))?;
            let mut spawn = |executable: &CString| {
                libc::posix_spawn(
                    &mut pid,
                    executable.as_ptr(),
                    &actions.0,
                    &attrs.0,
                    argv.as_ptr(),
                    envp.as_ptr(),
                )
            };
            if !search_path {
                spawn(&program)
            } else {
                // posix_spawnp searches the parent's PATH, not envp. Search the
                // child's PATH ourselves, using Apple's default when it is unset.
                let path = command
                    .get_envs()
                    .find(|(key, _)| *key == "PATH")
                    .and_then(|(_, value)| value)
                    .unwrap_or(OsStr::new("/usr/bin:/bin"));
                let mut result = libc::ENOENT;
                for directory in std::env::split_paths(path) {
                    let mut executable = directory.into_os_string();
                    if executable.is_empty() {
                        executable.push(".");
                    }
                    // Preserve the spelling execvp would pass to a shebang
                    // interpreter, including empty entries and trailing slashes.
                    executable.push("/");
                    executable.push(command.get_program());
                    if executable.as_bytes().len() >= libc::PATH_MAX as usize {
                        return Ok(None);
                    }
                    result = spawn(&c_string(&executable)?);
                    if !matches!(
                        result,
                        libc::ENOENT
                            | libc::ENOTDIR
                            | libc::EACCES
                            | libc::ELOOP
                            | libc::ENAMETOOLONG
                    ) {
                        break;
                    }
                }
                result
            }
        };
        // Retain Command's shell fallback and exact PATH search errors.
        if result == libc::ENOEXEC || (search_path && result != 0) {
            return Ok(None);
        }
        cvt(result)?;
        let child = Self {
            pid: Some(pid),
            status: None,
            sigchld,
        };
        Ok(Some((child, stdin, stdout, stderr)))
    }

    fn id(&self) -> Option<u32> {
        self.pid.map(|pid| pid as u32)
    }

    /// Polls and caches the exit status, relinquishing the PID once it is reaped.
    /// `ECHILD` also relinquishes it to prevent later signaling of a reused PID.
    fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        if self.status.is_some() {
            return Ok(self.status);
        }
        let pid = self
            .pid
            .ok_or_else(|| io::Error::from_raw_os_error(libc::ECHILD))?;
        let mut status = 0;
        // SAFETY: We own this child PID and provide writable status storage.
        match unsafe { libc::waitpid(pid, &mut status, libc::WNOHANG) } {
            0 => Ok(None),
            -1 => {
                let error = io::Error::last_os_error();
                if error.raw_os_error() == Some(libc::ECHILD) {
                    self.pid = None;
                }
                Err(error)
            }
            _ => {
                self.pid = None;
                self.status = Some(ExitStatus::from_raw(status));
                Ok(self.status)
            }
        }
    }

    /// Waits without transferring child ownership into the future, so callers
    /// may cancel a wait and then wait again or kill the same child.
    async fn wait(&mut self) -> io::Result<ExitStatus> {
        loop {
            match self.try_wait() {
                Ok(Some(status)) => return Ok(status),
                Ok(None) => {}
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error),
            }
            self.sigchld
                .recv()
                .await
                .ok_or_else(|| io::Error::other("SIGCHLD stream closed"))?;
        }
    }

    /// Sends SIGKILL if still owned, then waits for the child to be reaped.
    async fn kill(&mut self) -> io::Result<()> {
        if let Some(pid) = self.pid {
            // SAFETY: An unreaped child retains its PID, even after it exits.
            let result = unsafe { libc::kill(pid, libc::SIGKILL) };
            if result == -1 && io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH) {
                return Err(io::Error::last_os_error());
            }
        }
        self.wait().await.map(|_| ())
    }
}

impl Drop for NativeChild {
    fn drop(&mut self) {
        let _ = self.try_wait();
        let Some(pid) = self.pid.take() else { return };
        // SAFETY: This child has not been reaped, so its PID cannot be reused.
        unsafe {
            libc::kill(pid, libc::SIGKILL);
        }
        // Drop may run during runtime shutdown. Reap independently of Tokio,
        // without blocking its worker threads while the killed process exits.
        if std::thread::Builder::new()
            .name("mcp-child-reaper".into())
            .spawn(move || reap(pid))
            .is_err()
        {
            // Resource exhaustion must not turn a dropped child into a zombie.
            reap(pid);
        }
    }
}

/// Reaps the child transferred by `Drop`, retrying interrupted waits without
/// requiring a live Tokio runtime. The caller relinquishes ownership of `pid`.
fn reap(pid: libc::pid_t) {
    loop {
        // SAFETY: The caller transferred exclusive ownership of this unreaped PID.
        let result = unsafe {
            libc::waitpid(pid, ptr::null_mut(), /*options*/ 0)
        };
        if result != -1 || io::Error::last_os_error().kind() != io::ErrorKind::Interrupted {
            break;
        }
    }
}

fn c_string(value: &OsStr) -> io::Result<CString> {
    CString::new(value.as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "nul byte in MCP command"))
}

/// Keeps a child pipe source above stdio so `dup2` cannot clobber another source
/// when the parent has closed standard descriptors. Close-on-exec disposes of
/// this extra descriptor after the spawn actions duplicate it onto stdio.
fn child_fd(fd: OwnedFd) -> io::Result<OwnedFd> {
    // SAFETY: fcntl duplicates this live descriptor; the returned fd is newly owned.
    let duplicate = unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 3) };
    cvt_errno(duplicate)?;
    Ok(unsafe { OwnedFd::from_raw_fd(duplicate) })
}

/// Converts a spawn API's returned error number, which does not use `errno`.
fn cvt(result: libc::c_int) -> io::Result<()> {
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::from_raw_os_error(result))
    }
}

/// Converts a syscall's `-1` sentinel using the thread's current `errno`.
fn cvt_errno(result: libc::c_int) -> io::Result<()> {
    if result == -1 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

struct FileActions(libc::posix_spawn_file_actions_t);
struct Attributes(libc::posix_spawnattr_t);

impl Drop for FileActions {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: This object was initialized by posix_spawn_file_actions_init.
            unsafe {
                libc::posix_spawn_file_actions_destroy(&mut self.0);
            }
        }
    }
}

impl Drop for Attributes {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: This object was initialized by posix_spawnattr_init.
            unsafe {
                libc::posix_spawnattr_destroy(&mut self.0);
            }
        }
    }
}

#[cfg(test)]
#[path = "macos_child_tests.rs"]
mod tests;
