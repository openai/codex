//! Epoch-based memory reclamation.
//!
//! An interesting problem concurrent collections deal with comes from the remove operation.
//! Suppose that a thread removes an element from a lock-free map, while another thread is reading
//! that same element at the same time. The first thread must wait until the second thread stops
//! reading the element. Only then it is safe to destruct it.
//!
//! Programming languages that come with garbage collectors solve this problem trivially. The
//! garbage collector will destruct the removed element when no thread can hold a reference to it
//! anymore.
//!
//! This crate implements a basic memory reclamation mechanism, which is based on epochs. When an
//! element gets removed from a concurrent collection, it is inserted into a pile of garbage and
//! marked with the current epoch. Every time a thread accesses a collection, it checks the current
//! epoch, attempts to increment it, and destructs some garbage that became so old that no thread
//! can be referencing it anymore.
//!
//! That is the general mechanism behind epoch-based memory reclamation, but the details are a bit
//! more complicated. Anyhow, memory reclamation is designed to be fully automatic and something
//! users of concurrent collections don't have to worry much about.
//!
//! # Pointers
//!
//! Concurrent collections are built using atomic pointers. This module provides [`Atomic`], which
//! is just a shared atomic pointer to a heap-allocated object. Loading an [`Atomic`] yields a
//! [`Shared`], which is an epoch-protected pointer through which the loaded object can be safely
//! read.
//!
//! # Pinning
//!
//! Before an [`Atomic`] can be loaded, a participant must be [`pin`]ned. By pinning a participant
//! we declare that any object that gets removed from now on must not be destructed just
//! yet. Garbage collection of newly removed objects is suspended until the participant gets
//! unpinned.
//!
//! # Garbage
//!
//! Objects that get removed from concurrent collections must be stashed away until all currently
//! pinned participants get unpinned. Such objects can be stored into a thread-local or global
//! storage, where they are kept until the right time for their destruction comes.
//!
//! There is a global shared instance of garbage queue. You can [`defer`](Guard::defer) the execution of an
//! arbitrary function until the global epoch is advanced enough. Most notably, concurrent data
//! structures may defer the deallocation of an object.
//!
//! # APIs
//!
//! For majority of use cases, just use the default garbage collector by invoking [`pin`]. If you
//! want to create your own garbage collector, use the [`Collector`] API.

#![doc(test(
    no_crate_inject,
    attr(
        deny(warnings, rust_2018_idioms),
        allow(dead_code, unused_assignments, unused_variables)
    )
))]
#![warn(
    missing_docs,
    missing_debug_implementations,
    rust_2018_idioms,
    unreachable_pub
)]
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(crossbeam_loom)]
extern crate loom_crate as loom;

#[cfg(crossbeam_loom)]
#[allow(unused_imports, dead_code)]
mod primitive {
    pub(crate) mod cell {
        pub(crate) use loom::cell::UnsafeCell;
    }
    pub(crate) mod sync {
        pub(crate) mod atomic {
            pub(crate) use loom::sync::atomic::{fence, AtomicPtr, AtomicUsize, Ordering};

            // FIXME: loom does not support compiler_fence at the moment.
            // https://github.com/tokio-rs/loom/issues/117
            // we use fence as a stand-in for compiler_fence for the time being.
            // this may miss some races since fence is stronger than compiler_fence,
            // but it's the best we can do for the time being.
            pub(crate) use self::fence as compiler_fence;
        }
        pub(crate) use loom::sync::Arc;
    }
    pub(crate) use loom::thread_local;
}
#[cfg(target_has_atomic = "ptr")]
#[cfg(not(crossbeam_loom))]
#[allow(unused_imports, dead_code)]
mod primitive {
    pub(crate) mod cell {
        #[derive(Debug)]
        #[repr(transparent)]
        pub(crate) struct UnsafeCell<T>(::core::cell::UnsafeCell<T>);

        // loom's UnsafeCell has a slightly different API than the standard library UnsafeCell.
        // Since we want the rest of the code to be agnostic to whether it's running under loom or
        // not, we write this small wrapper that provides the loom-supported API for the standard
        // library UnsafeCell. This is also what the loom documentation recommends:
        // https://github.com/tokio-rs/loom#handling-loom-api-differences
        impl<T> UnsafeCell<T> {
            #[inline]
            pub(crate) const fn new(data: T) -> UnsafeCell<T> {
                UnsafeCell(::core::cell::UnsafeCell::new(data))
            }

            #[inline]
            pub(crate) fn with<R>(&self, f: impl FnOnce(*const T) -> R) -> R {
                f(self.0.get())
            }

            #[inline]
            pub(crate) fn with_mut<R>(&self, f: impl FnOnce(*mut T) -> R) -> R {
                f(self.0.get())
            }
        }
    }
    pub(crate) mod sync {
        pub(crate) mod atomic {
            pub(crate) use core::sync::atomic::{
                compiler_fence, fence, AtomicPtr, AtomicUsize, Ordering,
            };
        }
        #[cfg(feature = "alloc")]
        pub(crate) use alloc::sync::Arc;
    }

    #[cfg(feature = "std")]
    pub(crate) use std::thread_local;
}

#[cfg(all(feature = "alloc", target_has_atomic = "ptr"))]
extern crate alloc;

#[cfg(all(feature = "alloc", target_has_atomic = "ptr"))]
mod atomic;
#[cfg(all(feature = "alloc", target_has_atomic = "ptr"))]
mod collector;
#[cfg(all(feature = "alloc", target_has_atomic = "ptr"))]
mod deferred;
#[cfg(all(feature = "alloc", target_has_atomic = "ptr"))]
mod epoch;
#[cfg(all(feature = "alloc", target_has_atomic = "ptr"))]
mod guard;
#[cfg(all(feature = "alloc", target_has_atomic = "ptr"))]
mod internal;
#[cfg(all(feature = "alloc", target_has_atomic = "ptr"))]
mod sync;

#[cfg(all(feature = "alloc", target_has_atomic = "ptr"))]
#[allow(deprecated)]
pub use crate::atomic::{CompareAndSetError, CompareAndSetOrdering};
#[cfg(all(feature = "alloc", target_has_atomic = "ptr"))]
pub use crate::{
    atomic::{Atomic, CompareExchangeError, Owned, Pointable, Pointer, Shared},
    collector::{Collector, LocalHandle},
    guard::{unprotected, Guard},
};

#[cfg(feature = "std")]
mod default;
#[cfg(feature = "std")]
pub use crate::default::{default_collector, is_pinned, pin};
