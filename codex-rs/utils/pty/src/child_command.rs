//! Explicit local launch settings shared by native and Tokio process creation.
//!
//! The wrapped Tokio command is private: callers cannot install callbacks or
//! change settings that the native backend cannot inspect. Children receive only
//! explicitly supplied environment variables and default to kill-on-drop. Stdio,
//! descriptor inheritance, and compatibility fallbacks are configured independently.
//! Original Unix inputs retain their NUL validation even when std replaces them.

use std::ffi::OsStr;
#[cfg(unix)]
use std::ffi::OsString;
use std::io;
use std::path::Path;
use std::process::Stdio as TokioStdio;

use crate::child::Child;
use crate::child::ChildKind;

/// Relationship between a child and its parent's session and process group.
///
/// These settings apply only on Unix; other platforms use their default behavior.
#[derive(Clone, Copy)]
pub enum ProcessMode {
    /// Keep the parent's session, process group, and controlling terminal.
    Inherit,
    /// Create a process group led by the child, keeping the parent's session and
    /// controlling terminal. This allows signaling the child's group separately.
    NewGroup,
    /// Create a session and process group led by the child, detaching from the
    /// parent's controlling terminal.
    ///
    /// Native spawning is an optimization; compatibility fallbacks may fork.
    NewSession,
}

/// Whether Unix children inherit ambient descriptors or only explicit stdio and
/// the descriptors selected by `Command::preserve_fds`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DescriptorPolicy {
    Inherit,
    /// Exclude unrelated descriptors, allowing launch if best-effort cleanup fails.
    Explicit,
}

/// Whether a native launch may use Command's executable-text and PATH fallbacks.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SpawnFallback {
    Compatible,
    ReturnError,
}

/// Cleanup performed when a child handle is dropped before its exit is collected.
#[derive(Clone, Copy)]
pub(crate) enum ChildDropPolicy {
    /// Kill the direct child, then reap it. This is the default.
    KillAndReap,
    /// Reap the child when it exits, leaving termination to the caller.
    ReapOnly,
}

/// An explicit child stdin, including a socket used for bidirectional fd transfer.
pub enum ChildStdin {
    Piped,
    Null,
    #[cfg(unix)]
    File(std::os::fd::OwnedFd),
}

/// A local command whose complete launch contract is known to both backends.
pub struct Command {
    pub(crate) inner: tokio::process::Command,
    #[cfg(unix)]
    saw_nul: bool,
    pub(crate) process_mode: ProcessMode,
    pub(crate) descriptor_policy: DescriptorPolicy,
    pub(crate) fallback: SpawnFallback,
    pub(crate) stdin: ChildStdin,
    pub(crate) drop_policy: ChildDropPolicy,
    #[cfg(unix)]
    pub(crate) inherited_fds: Vec<std::os::fd::RawFd>,
    #[cfg(target_os = "linux")]
    parent_pid: Option<libc::pid_t>,
    #[cfg(unix)]
    pub(crate) stdout_file: Option<std::os::fd::OwnedFd>,
    #[cfg(unix)]
    pub(crate) stderr_file: Option<std::os::fd::OwnedFd>,
    #[cfg(unix)]
    pub(crate) arg0: Option<OsString>,
}

impl Command {
    pub fn new(program: impl AsRef<OsStr>) -> Self {
        let program = program.as_ref();
        let mut inner = tokio::process::Command::new(program);
        inner
            .env_clear()
            .kill_on_drop(true)
            .stdin(TokioStdio::piped())
            .stdout(TokioStdio::piped())
            .stderr(TokioStdio::piped());
        Self {
            inner,
            #[cfg(unix)]
            saw_nul: program.as_encoded_bytes().contains(&0),
            process_mode: ProcessMode::Inherit,
            descriptor_policy: DescriptorPolicy::Inherit,
            fallback: SpawnFallback::Compatible,
            stdin: ChildStdin::Piped,
            drop_policy: ChildDropPolicy::KillAndReap,
            #[cfg(unix)]
            inherited_fds: Vec::new(),
            #[cfg(target_os = "linux")]
            parent_pid: None,
            #[cfg(unix)]
            stdout_file: None,
            #[cfg(unix)]
            stderr_file: None,
            #[cfg(unix)]
            arg0: None,
        }
    }

    pub fn arg(&mut self, arg: impl AsRef<OsStr>) -> &mut Self {
        let arg = arg.as_ref();
        #[cfg(unix)]
        {
            self.saw_nul |= arg.as_encoded_bytes().contains(&0);
        }
        self.inner.arg(arg);
        self
    }

    pub fn args(&mut self, args: impl IntoIterator<Item = impl AsRef<OsStr>>) -> &mut Self {
        for arg in args {
            self.arg(arg);
        }
        self
    }

    pub fn env(&mut self, key: impl AsRef<OsStr>, value: impl AsRef<OsStr>) -> &mut Self {
        self.inner.env(key, value);
        self
    }

    pub fn envs<K, V>(&mut self, env: impl IntoIterator<Item = (K, V)>) -> &mut Self
    where
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        self.inner.envs(env);
        self
    }

    pub fn current_dir(&mut self, cwd: impl AsRef<Path>) -> &mut Self {
        let cwd = cwd.as_ref();
        #[cfg(unix)]
        {
            self.saw_nul |= cwd.as_os_str().as_encoded_bytes().contains(&0);
        }
        self.inner.current_dir(cwd);
        self
    }

    pub fn process_mode(&mut self, mode: ProcessMode) -> &mut Self {
        self.process_mode = mode;
        self
    }

    /// Choose who owns termination when the child handle is dropped.
    pub(crate) fn drop_policy(&mut self, policy: ChildDropPolicy) -> &mut Self {
        self.drop_policy = policy;
        self.inner.kill_on_drop(match policy {
            ChildDropPolicy::KillAndReap => true,
            ChildDropPolicy::ReapOnly => false,
        });
        self
    }

    pub fn stdin(&mut self, stdin: ChildStdin) -> &mut Self {
        self.stdin = stdin;
        self
    }

    pub fn descriptor_policy(&mut self, policy: DescriptorPolicy) -> &mut Self {
        self.descriptor_policy = policy;
        self
    }

    pub fn fallback(&mut self, fallback: SpawnFallback) -> &mut Self {
        self.fallback = fallback;
        self
    }

    /// Preserve live inheritable descriptors at their existing numbers.
    /// The caller must keep them open until spawning returns. We do not duplicate
    /// or close them: closing even a duplicate releases the parent's POSIX record
    /// locks. Closed and CLOEXEC descriptors remain excluded.
    #[cfg(unix)]
    pub fn preserve_fds(&mut self, fds: &[std::os::fd::RawFd]) -> &mut Self {
        self.inherited_fds = fds
            .iter()
            .copied()
            .filter(|fd| {
                // SAFETY: fcntl reports invalid descriptors without dereferencing memory.
                let flags = unsafe { libc::fcntl(*fd, libc::F_GETFD) };
                *fd > libc::STDERR_FILENO && flags >= 0 && flags & libc::FD_CLOEXEC == 0
            })
            .collect();
        self
    }

    /// Explicit launch attachments, including CLOEXEC descriptors. The caller
    /// owns them through spawn; only the child's flags are changed.
    #[cfg(unix)]
    pub(crate) fn inherit_fds(&mut self, fds: &[std::os::fd::RawFd]) -> &mut Self {
        self.inherited_fds = fds.to_vec();
        self
    }

    /// Keep the existing Linux pipe behavior when the spawning parent exits.
    #[cfg(target_os = "linux")]
    pub fn terminate_on_parent_death(&mut self) -> &mut Self {
        // SAFETY: getpid has no preconditions.
        self.parent_pid = Some(unsafe { libc::getpid() });
        self
    }

    #[cfg(unix)]
    pub fn arg0(&mut self, arg0: impl AsRef<OsStr>) -> &mut Self {
        self.saw_nul |= arg0.as_ref().as_encoded_bytes().contains(&0);
        self.inner.arg0(arg0.as_ref());
        self.arg0 = Some(arg0.as_ref().to_owned());
        self
    }

    /// Set the Windows process creation flags, replacing any previously selected flags.
    #[cfg(windows)]
    pub fn creation_flags(&mut self, flags: u32) -> &mut Self {
        self.inner.creation_flags(flags);
        self
    }
    /// Preserve Job Object assignment before the child begins executing on Windows.
    #[cfg(windows)]
    pub fn prepare_suspended_spawn(&mut self, job: &crate::JobObject) {
        job.prepare_suspended_spawn(&mut self.inner);
    }

    /// Reject original inputs that std replaced with a NUL-free placeholder.
    #[cfg(unix)]
    pub(crate) fn validate(&self) -> io::Result<()> {
        if self.saw_nul {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "nul byte found in provided data",
            ));
        }
        Ok(())
    }

    /// Launch with native platform settings and the existing compatibility fallback.
    pub fn spawn(mut self) -> io::Result<Child> {
        #[cfg(unix)]
        self.validate()?;
        #[cfg(unix)]
        if let ProcessMode::NewGroup = self.process_mode {
            self.inner.process_group(/*pgroup*/ 0);
        }
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        {
            let command = self.inner.as_std();
            let program = command.get_program();
            #[cfg(target_os = "macos")]
            let use_native = Path::new(program).is_relative()
                || self.descriptor_policy == DescriptorPolicy::Explicit
                || self.fallback == SpawnFallback::ReturnError
                || matches!(self.process_mode, ProcessMode::NewSession)
                || !self.inherited_fds.is_empty();
            #[cfg(target_os = "linux")]
            let use_native =
                matches!(self.process_mode, ProcessMode::NewSession) && self.parent_pid.is_none();
            if use_native
                && !program.is_empty()
                && let Some(child) = crate::child::posix::NativeChild::spawn(&self)?
            {
                return Ok(child);
            }
        }
        self.inner.stdin(match self.stdin {
            ChildStdin::Piped => TokioStdio::piped(),
            ChildStdin::Null => TokioStdio::null(),
            #[cfg(unix)]
            ChildStdin::File(fd) => TokioStdio::from(fd),
        });
        #[cfg(unix)]
        {
            let new_session = matches!(self.process_mode, ProcessMode::NewSession);
            let explicit_fds = self.descriptor_policy == DescriptorPolicy::Explicit;
            let targets = self.inherited_fds;
            #[cfg(target_os = "linux")]
            let parent_pid = self.parent_pid;
            #[cfg(not(target_os = "linux"))]
            let parent_pid: Option<i32> = None;
            if new_session || explicit_fds || !targets.is_empty() || parent_pid.is_some() {
                // SAFETY: The caller keeps the selected descriptors open. Session
                // and parent-death setup use system calls; Linux and macOS cleanup
                // avoid allocation after fork. Other Unix targets have an
                // allocation-after-fork risk documented on close_inherited_fds_except.
                unsafe {
                    self.inner.pre_exec(move || {
                        if new_session {
                            crate::process_group::detach_from_tty()?;
                        }
                        if let Some(parent_pid) = parent_pid {
                            crate::process_group::set_parent_death_signal(parent_pid)?;
                        }
                        if explicit_fds {
                            crate::pty::close_inherited_fds_except(&targets);
                        }
                        crate::pty::make_fds_inheritable(&targets)?;
                        Ok(())
                    });
                }
            }
        }
        #[cfg(unix)]
        {
            if let Some(fd) = self.stdout_file {
                self.inner.stdout(TokioStdio::from(fd));
            }
            if let Some(fd) = self.stderr_file {
                self.inner.stderr(TokioStdio::from(fd));
            }
        }
        #[cfg(unix)]
        let reaper = match self.drop_policy {
            ChildDropPolicy::KillAndReap => None,
            ChildDropPolicy::ReapOnly => Some(crate::child::reaper::sender()?),
        };
        let mut child = self.inner.spawn()?;
        Ok(Child {
            stdin: child.stdin.take(),
            stdout: child.stdout.take(),
            stderr: child.stderr.take(),
            inner: {
                #[cfg(unix)]
                {
                    match reaper {
                        Some(reaper) => ChildKind::TokioReapOnly {
                            child: Some(child),
                            reaper,
                        },
                        None => ChildKind::Tokio(child),
                    }
                }
                #[cfg(not(unix))]
                ChildKind::Tokio(child)
            },
        })
    }
}

#[cfg(all(test, unix))]
#[path = "command_validation_tests.rs"]
mod validation_tests;

#[cfg(all(test, target_os = "macos"))]
#[path = "macos_child_tests.rs"]
mod tests;

#[cfg(all(test, target_os = "macos"))]
#[path = "macos_descriptor_tests.rs"]
mod descriptor_tests;

#[cfg(all(test, target_os = "linux"))]
#[path = "linux_child_tests.rs"]
mod linux_tests;
