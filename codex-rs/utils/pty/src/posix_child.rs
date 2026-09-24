//! Native POSIX spawning without rewriting executable paths or argv[0].
//!
//! Spawn attributes and file actions implement the shared command's process-group
//! and descriptor policies. Bare commands search the child's PATH. Callers choose
//! whether incompatible executable formats and failed searches may retry through
//! Tokio. Linux also uses it to start the registered process-setup helper.
//! Each native child owns its PID until it has been reaped.

use std::ffi::CString;
use std::ffi::OsStr;
use std::io;
use std::mem::MaybeUninit;
use std::os::fd::AsRawFd;
use std::os::fd::FromRawFd;
use std::os::fd::OwnedFd;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::process::ExitStatusExt;
use std::process::ExitStatus;
use std::ptr;

use tokio::process::ChildStderr;
use tokio::process::ChildStdin;
use tokio::process::ChildStdout;
use tokio::signal::unix::Signal;
use tokio::signal::unix::SignalKind;
use tokio::signal::unix::signal;

use crate::child_command::ChildDropPolicy;

use crate::child::reaper;

#[cfg(target_os = "linux")]
use libc::POSIX_SPAWN_SETSID;
// Apple exposes this extension in spawn.h, but libc does not yet bind it.
#[cfg(target_os = "macos")]
const POSIX_SPAWN_SETSID: libc::c_int = 0x0400;

// Available since macOS 10.15 and in Rust's bundled musl. Older glibc needs
// runtime detection so this optimization does not raise our minimum libc version.
#[cfg(any(target_os = "macos", target_env = "musl"))]
unsafe extern "C" {
    fn posix_spawn_file_actions_addchdir_np(
        actions: *mut libc::posix_spawn_file_actions_t,
        path: *const libc::c_char,
    ) -> libc::c_int;
}

/// Owns a child PID until reaping, so cancellation cannot lose or reuse it.
/// Drop obeys the configured kill policy and reaps independently of Tokio.
pub(crate) struct NativeChild {
    pid: Option<libc::pid_t>,
    status: Option<ExitStatus>,
    sigchld: Signal,
    reaper: Option<std::sync::mpsc::Sender<reaper::ChildToReap>>,
}

impl NativeChild {
    /// Spawn the explicit command, retaining the caller's executable spelling and argv[0].
    /// Returns `Ok(None)` when native setup is unsupported or execution needs the
    /// caller's compatibility fallback.
    pub(crate) fn spawn(request: &crate::Command) -> io::Result<Option<crate::Child>> {
        #[cfg(target_os = "linux")]
        if request.descriptor_policy != crate::DescriptorPolicy::Inherit {
            return Ok(None);
        }
        let command = request.inner.as_std();
        let program = c_string(command.get_program())?;
        let search_path = !program.as_bytes().contains(&b'/');
        #[cfg(target_os = "linux")]
        if search_path
            && !command
                .get_envs()
                .any(|(key, value)| key == "PATH" && value.is_some())
        {
            // Preserve the libc-specific default search path in the fallback.
            return Ok(None);
        }
        let args = std::iter::once(request.arg0.as_deref().unwrap_or(command.get_program()))
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
        // Reserve the non-killing cleanup worker before creating a child. Drop
        // must not need a new thread or synchronously wait for a live process.
        let reaper = match request.drop_policy {
            ChildDropPolicy::KillAndReap => None,
            ChildDropPolicy::ReapOnly => Some(reaper::sender()?),
        };

        #[cfg(all(target_os = "linux", target_env = "gnu"))]
        let addchdir = {
            // SAFETY: dlsym returns the documented function signature when present.
            let symbol = unsafe {
                libc::dlsym(
                    libc::RTLD_DEFAULT,
                    c"posix_spawn_file_actions_addchdir_np".as_ptr(),
                )
            };
            if symbol.is_null() {
                return Ok(None);
            }
            unsafe {
                std::mem::transmute::<
                    *mut libc::c_void,
                    unsafe extern "C" fn(
                        *mut libc::posix_spawn_file_actions_t,
                        *const libc::c_char,
                    ) -> libc::c_int,
                >(symbol)
            }
        };
        #[cfg(any(target_os = "macos", target_env = "musl"))]
        let addchdir = posix_spawn_file_actions_addchdir_np;

        // Subscribe before spawning so a child that exits immediately cannot be missed.
        let sigchld = signal(SignalKind::child())?;
        let (stdin_read, stdin) = match &request.stdin {
            crate::ChildStdin::Piped => {
                let (reader, writer) = io::pipe()?;
                (
                    OwnedFd::from(reader),
                    Some(ChildStdin::from_std(OwnedFd::from(writer).into())?),
                )
            }
            crate::ChildStdin::Null => (std::fs::File::open("/dev/null")?.into(), None),
            crate::ChildStdin::File(fd) => (fd.try_clone()?, None),
        };
        let (stdout_read, stdout_write) = child_output(request.stdout_file.as_ref())?;
        let (stderr_read, stderr_write) = child_output(request.stderr_file.as_ref())?;
        let child_fds = [
            child_fd(stdin_read)?,
            child_fd(stdout_write)?,
            child_fd(stderr_write)?,
        ];
        let stdout = stdout_read
            .map(|fd| ChildStdout::from_std(fd.into()))
            .transpose()?;
        let stderr = stderr_read
            .map(|fd| ChildStderr::from_std(fd.into()))
            .transpose()?;

        let mut actions = MaybeUninit::uninit();
        // SAFETY: Successful initialization makes each object valid; RAII only
        // takes ownership after that success, including if the next init fails.
        cvt(unsafe { libc::posix_spawn_file_actions_init(actions.as_mut_ptr()) })?;
        let mut actions = FileActions(unsafe { actions.assume_init() });
        let mut attrs = MaybeUninit::uninit();
        cvt(unsafe { libc::posix_spawnattr_init(attrs.as_mut_ptr()) })?;
        let mut attrs = Attributes(unsafe { attrs.assume_init() });
        let mut pid = 0;
        // SAFETY: All C strings and pipe descriptors outlive this synchronous
        // spawn. The initialized action/attribute objects are destroyed by RAII.
        let result = unsafe {
            if let Some(cwd) = &cwd {
                cvt(addchdir(&mut actions.0, cwd.as_ptr()))?;
            }
            for (target, source) in child_fds.iter().enumerate() {
                cvt(libc::posix_spawn_file_actions_adddup2(
                    &mut actions.0,
                    source.as_raw_fd(),
                    target as i32,
                ))?;
            }
            for target in &request.inherited_fds {
                let result =
                    libc::posix_spawn_file_actions_adddup2(&mut actions.0, *target, *target);
                // macOS file actions can reject valid high descriptors below
                // RLIMIT_NOFILE. The compatibility backend can inherit them directly.
                if result == libc::EBADF && request.fallback == crate::SpawnFallback::Compatible {
                    return Ok(None);
                }
                cvt(result)?;
            }
            let group_flags = match request.process_mode {
                crate::ProcessMode::Inherit => 0,
                crate::ProcessMode::NewGroup => {
                    cvt(libc::posix_spawnattr_setpgroup(
                        &mut attrs.0,
                        /*pgroup*/ 0,
                    ))?;
                    libc::POSIX_SPAWN_SETPGROUP
                }
                crate::ProcessMode::NewSession => POSIX_SPAWN_SETSID as _,
            };
            let mut defaults = std::mem::zeroed();
            cvt_errno(libc::sigemptyset(&mut defaults))?;
            cvt_errno(libc::sigaddset(&mut defaults, libc::SIGPIPE))?;
            cvt(libc::posix_spawnattr_setsigdefault(&mut attrs.0, &defaults))?;
            #[cfg(target_os = "macos")]
            let descriptor_flags = match request.descriptor_policy {
                crate::DescriptorPolicy::Inherit => 0,
                crate::DescriptorPolicy::Explicit => libc::POSIX_SPAWN_CLOEXEC_DEFAULT,
            };
            #[cfg(target_os = "linux")]
            let descriptor_flags = 0;
            let result = libc::posix_spawnattr_setflags(
                &mut attrs.0,
                (group_flags | descriptor_flags | libc::POSIX_SPAWN_SETSIGDEF) as _,
            );
            // Unsupported attributes fail before posix_spawn can select its fallback.
            if result == libc::EINVAL && request.fallback == crate::SpawnFallback::Compatible {
                return Ok(None);
            }
            cvt(result)?;
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
                    #[cfg(target_os = "macos")]
                    if executable.is_empty() {
                        executable.push(".");
                    }
                    // Preserve the spelling execvp would pass to a shebang
                    // interpreter, including empty entries and trailing slashes.
                    if !executable.is_empty() {
                        executable.push("/");
                    }
                    executable.push(command.get_program());
                    if executable.as_bytes().len() >= libc::PATH_MAX as usize {
                        return if request.fallback == crate::SpawnFallback::Compatible {
                            Ok(None)
                        } else {
                            Err(io::Error::from_raw_os_error(libc::ENAMETOOLONG))
                        };
                    }
                    result = spawn(&c_string(&executable)?);
                    match result {
                        libc::ENOENT | libc::ENOTDIR | libc::EACCES => {}
                        #[cfg(target_os = "macos")]
                        libc::ELOOP | libc::ENAMETOOLONG => {}
                        #[cfg(all(target_os = "linux", target_env = "gnu"))]
                        libc::ESTALE | libc::ENODEV | libc::ETIMEDOUT => {}
                        _ => break,
                    }
                }
                result
            }
        };
        // Retain Command's shell fallback and exact PATH search errors.
        if request.fallback == crate::SpawnFallback::Compatible
            && (result == libc::ENOEXEC
                || (search_path && result != 0)
                || (matches!(request.process_mode, crate::ProcessMode::NewSession)
                    && matches!(result, libc::EINVAL | libc::EPERM)))
        {
            return Ok(None);
        }
        cvt(result)?;
        let child = Self {
            pid: Some(pid),
            status: None,
            sigchld,
            reaper,
        };
        Ok(Some(crate::Child {
            inner: super::ChildKind::Native(child),
            stdin,
            stdout,
            stderr,
        }))
    }

    pub(crate) fn id(&self) -> Option<u32> {
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
    pub(crate) async fn wait(&mut self) -> io::Result<ExitStatus> {
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
    pub(crate) async fn kill(&mut self) -> io::Result<()> {
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
        if let Some(reaper) = &self.reaper {
            // The shared sender keeps the worker alive for the process lifetime.
            let _ = reaper.send(reaper::ChildToReap::Native(pid));
            return;
        }
        // SAFETY: This child has not been reaped, so its PID cannot be reused.
        unsafe {
            libc::kill(pid, libc::SIGKILL);
        }
        // Drop may run during runtime shutdown. Reap independently of Tokio,
        // without blocking its worker threads while this process exits.
        if std::thread::Builder::new()
            .name("codex-child-reaper".into())
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

/// Keeps a child descriptor above stdio so `dup2` cannot clobber it when the
/// parent has closed standard descriptors. Spawn actions either duplicate it
/// onto stdio or explicitly preserve it across exec.
pub(crate) fn child_fd(fd: OwnedFd) -> io::Result<OwnedFd> {
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
        // SAFETY: This object was initialized by posix_spawn_file_actions_init.
        unsafe {
            libc::posix_spawn_file_actions_destroy(&mut self.0);
        }
    }
}

impl Drop for Attributes {
    fn drop(&mut self) {
        // SAFETY: This object was initialized by posix_spawnattr_init.
        unsafe {
            libc::posix_spawnattr_destroy(&mut self.0);
        }
    }
}

/// Wire a caller-owned output descriptor or return a new pipe for the parent.
fn child_output(file: Option<&OwnedFd>) -> io::Result<(Option<OwnedFd>, OwnedFd)> {
    if let Some(fd) = file {
        Ok((None, fd.try_clone()?))
    } else {
        let (reader, writer) = io::pipe()?;
        Ok((Some(reader.into()), writer.into()))
    }
}
