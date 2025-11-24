//! Tools for working with tasks.
//!
//! This module contains:
//!
//! - [`Spawn`], a trait for spawning new tasks.
//! - [`Context`], a context of an asynchronous task,
//!   including a handle for waking up the task.
//! - [`Waker`], a handle for waking up a task.
//!
//! The remaining types and traits in the module are used for implementing
//! executors or dealing with synchronization issues around task wakeup.

#[doc(no_inline)]
pub use core::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

pub use futures_task::{FutureObj, LocalFutureObj, LocalSpawn, Spawn, SpawnError, UnsafeFutureObj};

pub use futures_task::noop_waker;
pub use futures_task::noop_waker_ref;

#[cfg_attr(target_os = "none", cfg(target_has_atomic = "ptr"))]
#[cfg(feature = "alloc")]
pub use futures_task::ArcWake;

#[cfg_attr(target_os = "none", cfg(target_has_atomic = "ptr"))]
#[cfg(feature = "alloc")]
pub use futures_task::waker;

#[cfg_attr(target_os = "none", cfg(target_has_atomic = "ptr"))]
#[cfg(feature = "alloc")]
pub use futures_task::{waker_ref, WakerRef};

#[cfg_attr(
    target_os = "none",
    cfg(any(target_has_atomic = "ptr", feature = "portable-atomic"))
)]
pub use futures_core::task::__internal::AtomicWaker;

mod spawn;
pub use self::spawn::{LocalSpawnExt, SpawnExt};
