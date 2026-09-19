//! Explicit local launch settings shared by native and Tokio process creation.
//!
//! The wrapped Tokio command is private: callers cannot install callbacks or
//! change settings that the native backend cannot inspect. Children receive only
//! explicitly supplied environment variables, piped stdio, and kill-on-drop.

use std::ffi::OsStr;
use std::io;
use std::path::Path;
use std::process::Stdio;

use crate::child::Child;
use crate::child::ChildKind;

/// Relationship between a child and its parent's process group.
#[derive(Clone, Copy)]
pub enum ProcessMode {
    Inherit,
    NewGroup,
}

/// A local command whose complete launch contract is known to both backends.
pub struct Command {
    inner: tokio::process::Command,
    process_mode: ProcessMode,
}

impl Command {
    pub fn new(program: impl AsRef<OsStr>) -> Self {
        let mut inner = tokio::process::Command::new(program);
        inner
            .env_clear()
            .kill_on_drop(true)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        Self {
            inner,
            process_mode: ProcessMode::Inherit,
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
            if Path::new(program).is_relative()
                && !program.is_empty()
                && let Some((child, stdin, stdout, stderr)) =
                    crate::child::macos::NativeChild::spawn(command, self.process_mode)?
            {
                return Ok(Child {
                    inner: ChildKind::Native(child),
                    stdin: Some(stdin),
                    stdout: Some(stdout),
                    stderr: Some(stderr),
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
