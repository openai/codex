//! Async synchronization primitives.
//!
//! This crate provides the following primitives:
//!
//! * [`Barrier`] - enables tasks to synchronize all together at the same time.
//! * [`Mutex`] - a mutual exclusion lock.
//! * [`RwLock`] - a reader-writer lock, allowing any number of readers or a single writer.
//! * [`Semaphore`] - limits the number of concurrent operations.
//!
//! ## Relationship with `std::sync`
//!
//! In general, you should consider using [`std::sync`] types over types from this crate.
//!
//! There are two primary use cases for types from this crate:
//!
//! - You need to use a synchronization primitive in a `no_std` environment.
//! - You need to hold a lock across an `.await` point.
//!   (Holding an [`std::sync`] lock guard across an `.await` will make your future non-`Send`,
//!   and is also highly likely to cause deadlocks.)
//!
//! If you already use `libstd` and you aren't holding locks across await points (there is a
//! Clippy lint called [`await_holding_lock`] that emits warnings for this scenario), you should
//! consider [`std::sync`] instead of this crate. Those types are optimized for the currently
//! running operating system, are less complex and are generally much faster.
//!
//! In contrast, `async-lock`'s notification system uses `std::sync::Mutex` under the hood if
//! the `std` feature is enabled, and will fall back to a significantly slower strategy if it is
//! not. So, there are few cases where `async-lock` is a win for performance over [`std::sync`].
//!
//! [`std::sync`]: https://doc.rust-lang.org/std/sync/index.html
//! [`await_holding_lock`]: https://rust-lang.github.io/rust-clippy/stable/index.html#/await_holding_lock

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs, missing_debug_implementations, rust_2018_idioms)]
#![doc(
    html_favicon_url = "https://raw.githubusercontent.com/smol-rs/smol/master/assets/images/logo_fullsize_transparent.png"
)]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/smol-rs/smol/master/assets/images/logo_fullsize_transparent.png"
)]

extern crate alloc;

/// Simple macro to extract the value of `Poll` or return `Pending`.
///
/// TODO: Drop in favor of `core::task::ready`, once MSRV is bumped to 1.64.
macro_rules! ready {
    ($e:expr) => {{
        use ::core::task::Poll;

        match $e {
            Poll::Ready(v) => v,
            Poll::Pending => return Poll::Pending,
        }
    }};
}

/// Pins a variable on the stack.
///
/// TODO: Drop in favor of `core::pin::pin`, once MSRV is bumped to 1.68.
#[cfg(all(feature = "std", not(target_family = "wasm")))]
macro_rules! pin {
    ($($x:ident),* $(,)?) => {
        $(
            let mut $x = $x;
            #[allow(unused_mut)]
            let mut $x = unsafe {
                core::pin::Pin::new_unchecked(&mut $x)
            };
        )*
    }
}

/// Make the given function const if the given condition is true.
macro_rules! const_fn {
    (
        const_if: #[cfg($($cfg:tt)+)];
        $(#[$($attr:tt)*])*
        $vis:vis const fn $($rest:tt)*
    ) => {
        #[cfg($($cfg)+)]
        $(#[$($attr)*])*
        $vis const fn $($rest)*
        #[cfg(not($($cfg)+))]
        $(#[$($attr)*])*
        $vis fn $($rest)*
    };
}

mod barrier;
mod mutex;
mod once_cell;
mod rwlock;
mod semaphore;

pub use barrier::{Barrier, BarrierWaitResult};
pub use mutex::{Mutex, MutexGuard, MutexGuardArc};
pub use once_cell::OnceCell;
pub use rwlock::{
    RwLock, RwLockReadGuard, RwLockReadGuardArc, RwLockUpgradableReadGuard,
    RwLockUpgradableReadGuardArc, RwLockWriteGuard, RwLockWriteGuardArc,
};
pub use semaphore::{Semaphore, SemaphoreGuard, SemaphoreGuardArc};

pub mod futures {
    //! Named futures for use with `async_lock` primitives.

    pub use crate::barrier::BarrierWait;
    pub use crate::mutex::{Lock, LockArc};
    pub use crate::rwlock::futures::{
        Read, ReadArc, UpgradableRead, UpgradableReadArc, Upgrade, UpgradeArc, Write, WriteArc,
    };
    pub use crate::semaphore::{Acquire, AcquireArc};
}

#[cfg(not(loom))]
/// Synchronization primitive implementation.
mod sync {
    pub(super) use core::sync::atomic;

    pub(super) trait WithMut {
        type Output;

        fn with_mut<F, R>(&mut self, f: F) -> R
        where
            F: FnOnce(&mut Self::Output) -> R;
    }

    impl WithMut for atomic::AtomicUsize {
        type Output = usize;

        #[inline]
        fn with_mut<F, R>(&mut self, f: F) -> R
        where
            F: FnOnce(&mut Self::Output) -> R,
        {
            f(self.get_mut())
        }
    }
}

#[cfg(loom)]
/// Synchronization primitive implementation.
mod sync {
    pub(super) use loom::sync::atomic;
}

#[cold]
fn abort() -> ! {
    // For no_std targets, panicking while panicking is defined as an abort
    #[cfg(not(feature = "std"))]
    {
        struct Bomb;

        impl Drop for Bomb {
            fn drop(&mut self) {
                panic!("Panicking while panicking to abort")
            }
        }

        let _bomb = Bomb;
        panic!("Panicking while panicking to abort")
    }

    // For libstd targets, abort using std::process::abort
    #[cfg(feature = "std")]
    std::process::abort()
}
