//! Managed wrappers for [`std::process`].

pub use crate::command_ext::CommandExt;
use crate::drop_bomb::DropBomb;
use either::Either;
use std::io;
use std::ops::Deref;
use std::ops::DerefMut;
use std::process::Child as StdChild;
use std::process::Command;
use std::process::ExitStatus;
use std::process::Output;

impl CommandExt for Command {
    type Child = Child;

    fn spawn_managed(&mut self) -> io::Result<Self::Child> {
        self.spawn().map(Child::new)
    }
}

/// A synchronous child process handle that must be explicitly joined.
#[derive(Debug)]
pub struct Child {
    // This is an Option only so DropBomb still runs after wait_with_output()
    // moves the native child into its consuming join.
    child: Option<StdChild>,
    bomb: DropBomb,
}

impl Child {
    fn new(child: StdChild) -> Self {
        Self {
            child: Some(child),
            bomb: DropBomb::new(),
        }
    }

    /// Waits for the child to exit and disarms the drop bomb on success.
    pub fn wait(mut self) -> io::Result<ExitStatus> {
        let result = self.child_mut().wait();
        if result.is_ok() {
            self.bomb.disarm();
        }
        result
    }

    /// Returns the child's exit status without blocking.
    ///
    /// Returns the still-armed child handle when an exit status is not yet
    /// available.
    pub fn try_wait(mut self) -> io::Result<Either<ExitStatus, Self>> {
        match self.child_mut().try_wait()? {
            Some(status) => {
                self.bomb.disarm();
                Ok(Either::Left(status))
            }
            None => Ok(Either::Right(self)),
        }
    }

    /// Waits for the child to exit and collects its output.
    ///
    /// The drop bomb remains armed until this consuming operation returns.
    pub fn wait_with_output(mut self) -> io::Result<Output> {
        let child = self.take_child();
        let result = child.wait_with_output();
        self.bomb.disarm();
        result
    }

    fn child(&self) -> &StdChild {
        match self.child.as_ref() {
            Some(child) => child,
            None => panic!("managed child was made None before its wrapper was dropped"),
        }
    }

    fn child_mut(&mut self) -> &mut StdChild {
        match self.child.as_mut() {
            Some(child) => child,
            None => panic!("managed child was made None before its wrapper was dropped"),
        }
    }

    fn take_child(&mut self) -> StdChild {
        match self.child.take() {
            Some(child) => child,
            None => panic!("managed child was made None before its wrapper was dropped"),
        }
    }
}

impl Deref for Child {
    type Target = StdChild;

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
#[path = "sync_tests.rs"]
mod tests;
