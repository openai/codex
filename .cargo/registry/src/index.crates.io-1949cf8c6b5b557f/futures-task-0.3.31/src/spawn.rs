use crate::{FutureObj, LocalFutureObj};
use core::fmt;

/// The `Spawn` trait allows for pushing futures onto an executor that will
/// run them to completion.
pub trait Spawn {
    /// Spawns a future that will be run to completion.
    ///
    /// # Errors
    ///
    /// The executor may be unable to spawn tasks. Spawn errors should
    /// represent relatively rare scenarios, such as the executor
    /// having been shut down so that it is no longer able to accept
    /// tasks.
    fn spawn_obj(&self, future: FutureObj<'static, ()>) -> Result<(), SpawnError>;

    /// Determines whether the executor is able to spawn new tasks.
    ///
    /// This method will return `Ok` when the executor is *likely*
    /// (but not guaranteed) to accept a subsequent spawn attempt.
    /// Likewise, an `Err` return means that `spawn` is likely, but
    /// not guaranteed, to yield an error.
    #[inline]
    fn status(&self) -> Result<(), SpawnError> {
        Ok(())
    }
}

/// The `LocalSpawn` is similar to [`Spawn`], but allows spawning futures
/// that don't implement `Send`.
pub trait LocalSpawn {
    /// Spawns a future that will be run to completion.
    ///
    /// # Errors
    ///
    /// The executor may be unable to spawn tasks. Spawn errors should
    /// represent relatively rare scenarios, such as the executor
    /// having been shut down so that it is no longer able to accept
    /// tasks.
    fn spawn_local_obj(&self, future: LocalFutureObj<'static, ()>) -> Result<(), SpawnError>;

    /// Determines whether the executor is able to spawn new tasks.
    ///
    /// This method will return `Ok` when the executor is *likely*
    /// (but not guaranteed) to accept a subsequent spawn attempt.
    /// Likewise, an `Err` return means that `spawn` is likely, but
    /// not guaranteed, to yield an error.
    #[inline]
    fn status_local(&self) -> Result<(), SpawnError> {
        Ok(())
    }
}

/// An error that occurred during spawning.
pub struct SpawnError {
    _priv: (),
}

impl fmt::Debug for SpawnError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("SpawnError").field(&"shutdown").finish()
    }
}

impl fmt::Display for SpawnError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Executor is shutdown")
    }
}

#[cfg(feature = "std")]
impl std::error::Error for SpawnError {}

impl SpawnError {
    /// Spawning failed because the executor has been shut down.
    pub fn shutdown() -> Self {
        Self { _priv: () }
    }

    /// Check whether spawning failed to the executor being shut down.
    pub fn is_shutdown(&self) -> bool {
        true
    }
}

impl<Sp: ?Sized + Spawn> Spawn for &Sp {
    fn spawn_obj(&self, future: FutureObj<'static, ()>) -> Result<(), SpawnError> {
        Sp::spawn_obj(self, future)
    }

    fn status(&self) -> Result<(), SpawnError> {
        Sp::status(self)
    }
}

impl<Sp: ?Sized + Spawn> Spawn for &mut Sp {
    fn spawn_obj(&self, future: FutureObj<'static, ()>) -> Result<(), SpawnError> {
        Sp::spawn_obj(self, future)
    }

    fn status(&self) -> Result<(), SpawnError> {
        Sp::status(self)
    }
}

impl<Sp: ?Sized + LocalSpawn> LocalSpawn for &Sp {
    fn spawn_local_obj(&self, future: LocalFutureObj<'static, ()>) -> Result<(), SpawnError> {
        Sp::spawn_local_obj(self, future)
    }

    fn status_local(&self) -> Result<(), SpawnError> {
        Sp::status_local(self)
    }
}

impl<Sp: ?Sized + LocalSpawn> LocalSpawn for &mut Sp {
    fn spawn_local_obj(&self, future: LocalFutureObj<'static, ()>) -> Result<(), SpawnError> {
        Sp::spawn_local_obj(self, future)
    }

    fn status_local(&self) -> Result<(), SpawnError> {
        Sp::status_local(self)
    }
}

#[cfg(feature = "alloc")]
mod if_alloc {
    use super::*;
    use alloc::{boxed::Box, rc::Rc};

    impl<Sp: ?Sized + Spawn> Spawn for Box<Sp> {
        fn spawn_obj(&self, future: FutureObj<'static, ()>) -> Result<(), SpawnError> {
            (**self).spawn_obj(future)
        }

        fn status(&self) -> Result<(), SpawnError> {
            (**self).status()
        }
    }

    impl<Sp: ?Sized + LocalSpawn> LocalSpawn for Box<Sp> {
        fn spawn_local_obj(&self, future: LocalFutureObj<'static, ()>) -> Result<(), SpawnError> {
            (**self).spawn_local_obj(future)
        }

        fn status_local(&self) -> Result<(), SpawnError> {
            (**self).status_local()
        }
    }

    impl<Sp: ?Sized + Spawn> Spawn for Rc<Sp> {
        fn spawn_obj(&self, future: FutureObj<'static, ()>) -> Result<(), SpawnError> {
            (**self).spawn_obj(future)
        }

        fn status(&self) -> Result<(), SpawnError> {
            (**self).status()
        }
    }

    impl<Sp: ?Sized + LocalSpawn> LocalSpawn for Rc<Sp> {
        fn spawn_local_obj(&self, future: LocalFutureObj<'static, ()>) -> Result<(), SpawnError> {
            (**self).spawn_local_obj(future)
        }

        fn status_local(&self) -> Result<(), SpawnError> {
            (**self).status_local()
        }
    }

    #[cfg_attr(target_os = "none", cfg(target_has_atomic = "ptr"))]
    impl<Sp: ?Sized + Spawn> Spawn for alloc::sync::Arc<Sp> {
        fn spawn_obj(&self, future: FutureObj<'static, ()>) -> Result<(), SpawnError> {
            (**self).spawn_obj(future)
        }

        fn status(&self) -> Result<(), SpawnError> {
            (**self).status()
        }
    }

    #[cfg_attr(target_os = "none", cfg(target_has_atomic = "ptr"))]
    impl<Sp: ?Sized + LocalSpawn> LocalSpawn for alloc::sync::Arc<Sp> {
        fn spawn_local_obj(&self, future: LocalFutureObj<'static, ()>) -> Result<(), SpawnError> {
            (**self).spawn_local_obj(future)
        }

        fn status_local(&self) -> Result<(), SpawnError> {
            (**self).status_local()
        }
    }
}
