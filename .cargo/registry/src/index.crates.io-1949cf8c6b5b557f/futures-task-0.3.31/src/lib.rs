//! Tools for working with tasks.

#![no_std]
#![doc(test(
    no_crate_inject,
    attr(
        deny(warnings, rust_2018_idioms, single_use_lifetimes),
        allow(dead_code, unused_assignments, unused_variables)
    )
))]
#![warn(missing_docs, unsafe_op_in_unsafe_fn)]

#[cfg(feature = "alloc")]
extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

mod spawn;
pub use crate::spawn::{LocalSpawn, Spawn, SpawnError};

#[cfg_attr(target_os = "none", cfg(target_has_atomic = "ptr"))]
#[cfg(feature = "alloc")]
mod arc_wake;
#[cfg_attr(target_os = "none", cfg(target_has_atomic = "ptr"))]
#[cfg(feature = "alloc")]
pub use crate::arc_wake::ArcWake;

#[cfg_attr(target_os = "none", cfg(target_has_atomic = "ptr"))]
#[cfg(feature = "alloc")]
mod waker;
#[cfg_attr(target_os = "none", cfg(target_has_atomic = "ptr"))]
#[cfg(feature = "alloc")]
pub use crate::waker::waker;

#[cfg_attr(target_os = "none", cfg(target_has_atomic = "ptr"))]
#[cfg(feature = "alloc")]
mod waker_ref;
#[cfg_attr(target_os = "none", cfg(target_has_atomic = "ptr"))]
#[cfg(feature = "alloc")]
pub use crate::waker_ref::{waker_ref, WakerRef};

mod future_obj;
pub use crate::future_obj::{FutureObj, LocalFutureObj, UnsafeFutureObj};

mod noop_waker;
pub use crate::noop_waker::noop_waker;
pub use crate::noop_waker::noop_waker_ref;

#[doc(no_inline)]
pub use core::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
