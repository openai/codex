//! Asynchronous values.
//!
//! This module contains:
//!
//! - The [`Future`] trait.
//! - The [`FutureExt`] and [`TryFutureExt`] trait, which provides adapters for
//!   chaining and composing futures.
//! - Top-level future combinators like [`lazy`](lazy()) which creates a future
//!   from a closure that defines its return value, and [`ready`](ready()),
//!   which constructs a future with an immediate defined value.

#[doc(no_inline)]
pub use core::future::Future;

#[cfg(feature = "alloc")]
pub use futures_core::future::{BoxFuture, LocalBoxFuture};
pub use futures_core::future::{FusedFuture, TryFuture};
pub use futures_task::{FutureObj, LocalFutureObj, UnsafeFutureObj};

// Extension traits and combinators
#[allow(clippy::module_inception)]
mod future;
pub use self::future::{
    Flatten, Fuse, FutureExt, Inspect, IntoStream, Map, MapInto, NeverError, Then, UnitError,
};

#[deprecated(note = "This is now an alias for [Flatten](Flatten)")]
pub use self::future::FlattenStream;

#[cfg(feature = "std")]
pub use self::future::CatchUnwind;

#[cfg(feature = "channel")]
#[cfg_attr(docsrs, doc(cfg(feature = "channel")))]
#[cfg(feature = "std")]
pub use self::future::{Remote, RemoteHandle};

#[cfg(feature = "std")]
pub use self::future::{Shared, WeakShared};

mod try_future;
pub use self::try_future::{
    AndThen, ErrInto, InspectErr, InspectOk, IntoFuture, MapErr, MapOk, MapOkOrElse, OkInto,
    OrElse, TryFlatten, TryFlattenStream, TryFutureExt, UnwrapOrElse,
};

#[cfg(feature = "sink")]
#[cfg_attr(docsrs, doc(cfg(feature = "sink")))]
pub use self::try_future::FlattenSink;

// Primitive futures

mod lazy;
pub use self::lazy::{lazy, Lazy};

mod pending;
pub use self::pending::{pending, Pending};

mod maybe_done;
pub use self::maybe_done::{maybe_done, MaybeDone};

mod try_maybe_done;
pub use self::try_maybe_done::{try_maybe_done, TryMaybeDone};

mod option;
pub use self::option::OptionFuture;

mod poll_fn;
pub use self::poll_fn::{poll_fn, PollFn};

mod poll_immediate;
pub use self::poll_immediate::{poll_immediate, PollImmediate};

mod ready;
pub use self::ready::{err, ok, ready, Ready};

mod always_ready;
pub use self::always_ready::{always_ready, AlwaysReady};

mod join;
pub use self::join::{join, join3, join4, join5, Join, Join3, Join4, Join5};

#[cfg(feature = "alloc")]
mod join_all;
#[cfg(feature = "alloc")]
pub use self::join_all::{join_all, JoinAll};

mod select;
pub use self::select::{select, Select};

#[cfg(feature = "alloc")]
mod select_all;
#[cfg(feature = "alloc")]
pub use self::select_all::{select_all, SelectAll};

mod try_join;
pub use self::try_join::{
    try_join, try_join3, try_join4, try_join5, TryJoin, TryJoin3, TryJoin4, TryJoin5,
};

#[cfg(feature = "alloc")]
mod try_join_all;
#[cfg(feature = "alloc")]
pub use self::try_join_all::{try_join_all, TryJoinAll};

mod try_select;
pub use self::try_select::{try_select, TrySelect};

#[cfg(feature = "alloc")]
mod select_ok;
#[cfg(feature = "alloc")]
pub use self::select_ok::{select_ok, SelectOk};

mod either;
pub use self::either::Either;

#[cfg_attr(target_os = "none", cfg(target_has_atomic = "ptr"))]
#[cfg(feature = "alloc")]
mod abortable;
#[cfg_attr(target_os = "none", cfg(target_has_atomic = "ptr"))]
#[cfg(feature = "alloc")]
pub use crate::abortable::{AbortHandle, AbortRegistration, Abortable, Aborted};
#[cfg_attr(target_os = "none", cfg(target_has_atomic = "ptr"))]
#[cfg(feature = "alloc")]
pub use abortable::abortable;

// Just a helper function to ensure the futures we're returning all have the
// right implementations.
pub(crate) fn assert_future<T, F>(future: F) -> F
where
    F: Future<Output = T>,
{
    future
}
