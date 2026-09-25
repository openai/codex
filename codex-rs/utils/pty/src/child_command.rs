//! Explicit local launch settings shared by native and Tokio process creation.
//!
//! The wrapped Tokio command is private: callers cannot install callbacks or
//! change settings that the native backend cannot inspect. Children receive only
//! explicitly supplied environment variables and kill-on-drop. Stdio, descriptor
//! inheritance, and compatibility fallbacks are configured independently.

use std::ffi::OsStr;
#[cfg(unix)]
use std::ffi::OsString;
use std::io;
use std::path::Path;
use std::process::Stdio as TokioStdio;

use crate::child::Child;
use crate::child::ChildKind;

/// Relationship between a child and its parent's process group.
#[derive(Clone, Copy)]
pub enum ProcessMode {
    Inherit,
    NewGroup,
}

/// Descriptors visible to a Unix child beyond its explicit stdio.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DescriptorPolicy {
    Inherit,
    StdioOnly,
}

/// Whether a native launch may use Command's executable-text and PATH fallbacks.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SpawnFallback {
    Compatible,
    ReturnError,
}

/// An explicit child stdin, including a socket used for bidirectional fd transfer.
pub enum ChildStdin {
    Piped,
    #[cfg(unix)]
    File(std::os::fd::OwnedFd),
}

/// A local command whose complete launch contract is known to both backends.
pub struct Command {
    pub(crate) inner: tokio::process::Command,
    pub(crate) process_mode: ProcessMode,
    pub(crate) descriptor_policy: DescriptorPolicy,
    pub(crate) fallback: SpawnFallback,
    pub(crate) stdin: ChildStdin,
    #[cfg(unix)]
    pub(crate) arg0: Option<OsString>,
}

impl Command {
    pub fn new(program: impl AsRef<OsStr>) -> Self {
        let mut inner = tokio::process::Command::new(program);
        inner
            .env_clear()
            .kill_on_drop(true)
            .stdin(TokioStdio::piped())
            .stdout(TokioStdio::piped())
            .stderr(TokioStdio::piped());
        Self {
            inner,
            process_mode: ProcessMode::Inherit,
            descriptor_policy: DescriptorPolicy::Inherit,
            fallback: SpawnFallback::Compatible,
            stdin: ChildStdin::Piped,
            #[cfg(unix)]
            arg0: None,
        }
    }

    pub fn arg(&mut self, arg: impl AsRef<OsStr>) -> &mut Self {
        self.inner.arg(arg);
        self
    }

    pub fn args(&mut self, args: impl IntoIterator<Item = impl AsRef<OsStr>>) -> &mut Self {
        self.inner.args(args);
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
        self.inner.current_dir(cwd);
        self
    }

    pub fn process_mode(&mut self, mode: ProcessMode) -> &mut Self {
        self.process_mode = mode;
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

    #[cfg(unix)]
    pub fn arg0(&mut self, arg0: impl AsRef<OsStr>) -> &mut Self {
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

    /// Launch with native macOS path handling and the existing compatibility fallback.
    pub fn spawn(mut self) -> io::Result<Child> {
        #[cfg(unix)]
        if let ProcessMode::NewGroup = self.process_mode {
            self.inner.process_group(/*pgroup*/ 0);
        }
        #[cfg(target_os = "macos")]
        {
            let command = self.inner.as_std();
            let program = command.get_program();
            if (Path::new(program).is_relative()
                || self.descriptor_policy == DescriptorPolicy::StdioOnly
                || self.fallback == SpawnFallback::ReturnError)
                && !program.is_empty()
                && let Some(child) = crate::child::macos::NativeChild::spawn(&self)?
            {
                return Ok(child);
            }
        }
        self.inner.stdin(match self.stdin {
            ChildStdin::Piped => TokioStdio::piped(),
            #[cfg(unix)]
            ChildStdin::File(fd) => TokioStdio::from(fd),
        });
        #[cfg(unix)]
        if self.descriptor_policy == DescriptorPolicy::StdioOnly {
            // SAFETY: This preserves the existing Unix descriptor cleanup before exec.
            unsafe {
                self.inner.pre_exec(|| {
                    crate::pty::close_inherited_fds_except(&[]);
                    Ok(())
                });
            }
        }
        let mut child = self.inner.spawn()?;
        Ok(Child {
            stdin: child.stdin.take(),
            stdout: child.stdout.take(),
            stderr: child.stderr.take(),
            inner: ChildKind::Tokio(child),
        })
    }
}

#[cfg(all(test, target_os = "macos"))]
#[path = "macos_child_tests.rs"]
mod tests;

#[cfg(all(test, target_os = "macos"))]
#[path = "macos_descriptor_tests.rs"]
mod descriptor_tests;
