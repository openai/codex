//! Managed wrappers for [`tokio::process`].

use crate::DropBomb;
use ::tokio::process::Child as TokioChild;
use ::tokio::process::Command;
use std::io;
use std::ops::Deref;
use std::ops::DerefMut;
use std::process::ExitStatus;
use std::process::Output;

/// Extends [`tokio::process::Command`] with Codex-specific process spawning.
///
/// Callers should use [`CommandExt::spawn_managed`] when the returned child
/// must be joined before its handle is dropped.
pub trait CommandExt {
    /// Spawns this command and returns a child handle that must be joined.
    fn spawn_managed(&mut self) -> io::Result<Child>;
}

impl CommandExt for Command {
    fn spawn_managed(&mut self) -> io::Result<Child> {
        self.spawn().map(Child::new)
    }
}

/// An asynchronous child process handle that must be explicitly joined.
#[derive(Debug)]
pub struct Child {
    child: Option<TokioChild>,
    bomb: DropBomb,
}

impl Child {
    fn new(child: TokioChild) -> Self {
        Self {
            child: Some(child),
            bomb: DropBomb::new(),
        }
    }

    /// Waits for the child to exit and disarms the drop bomb on success.
    pub async fn wait(&mut self) -> io::Result<ExitStatus> {
        let result = self.child_mut().wait().await;
        if result.is_ok() {
            self.bomb.disarm();
        }
        result
    }

    /// Returns the child's exit status without blocking.
    ///
    /// The drop bomb is disarmed only when an exit status is available.
    pub fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        let result = self.child_mut().try_wait();
        if matches!(result, Ok(Some(_))) {
            self.bomb.disarm();
        }
        result
    }

    /// Waits for the child to exit and collects its output.
    ///
    /// The drop bomb remains armed until this consuming operation returns.
    pub async fn wait_with_output(mut self) -> io::Result<Output> {
        let child = self.take_child();
        let result = child.wait_with_output().await;
        self.bomb.disarm();
        result
    }

    fn child(&self) -> &TokioChild {
        match self.child.as_ref() {
            Some(child) => child,
            None => panic!("managed child is unavailable while joining"),
        }
    }

    fn child_mut(&mut self) -> &mut TokioChild {
        match self.child.as_mut() {
            Some(child) => child,
            None => panic!("managed child is unavailable while joining"),
        }
    }

    fn take_child(&mut self) -> TokioChild {
        match self.child.take() {
            Some(child) => child,
            None => panic!("managed child is unavailable while joining"),
        }
    }
}

impl Deref for Child {
    type Target = TokioChild;

    fn deref(&self) -> &Self::Target {
        self.child()
    }
}

impl DerefMut for Child {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.child_mut()
    }
}

#[cfg(test)]
#[path = "tokio_tests.rs"]
mod tests;
