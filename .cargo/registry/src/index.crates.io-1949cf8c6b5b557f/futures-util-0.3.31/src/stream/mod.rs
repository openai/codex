//! Asynchronous streams.
//!
//! This module contains:
//!
//! - The [`Stream`] trait, for objects that can asynchronously produce a
//!   sequence of values.
//! - The [`StreamExt`] and [`TryStreamExt`] trait, which provides adapters for
//!   chaining and composing streams.
//! - Top-level stream constructors like [`iter`](iter()) which creates a
//!   stream from an iterator.

#[cfg(feature = "alloc")]
pub use futures_core::stream::{BoxStream, LocalBoxStream};
pub use futures_core::stream::{FusedStream, Stream, TryStream};

// Extension traits and combinators

#[allow(clippy::module_inception)]
mod stream;
pub use self::stream::{
    All, Any, Chain, Collect, Concat, Count, Cycle, Enumerate, Filter, FilterMap, FlatMap, Flatten,
    Fold, ForEach, Fuse, Inspect, Map, Next, NextIf, NextIfEq, Peek, PeekMut, Peekable, Scan,
    SelectNextSome, Skip, SkipWhile, StreamExt, StreamFuture, Take, TakeUntil, TakeWhile, Then,
    Unzip, Zip,
};

#[cfg(feature = "std")]
pub use self::stream::CatchUnwind;

#[cfg(feature = "alloc")]
pub use self::stream::Chunks;

#[cfg(feature = "alloc")]
pub use self::stream::ReadyChunks;

#[cfg(feature = "sink")]
#[cfg_attr(docsrs, doc(cfg(feature = "sink")))]
pub use self::stream::Forward;

#[cfg_attr(target_os = "none", cfg(target_has_atomic = "ptr"))]
#[cfg(feature = "alloc")]
pub use self::stream::{
    BufferUnordered, Buffered, FlatMapUnordered, FlattenUnordered, ForEachConcurrent,
};

#[cfg_attr(target_os = "none", cfg(target_has_atomic = "ptr"))]
#[cfg(feature = "sink")]
#[cfg_attr(docsrs, doc(cfg(feature = "sink")))]
#[cfg(feature = "alloc")]
pub use self::stream::{ReuniteError, SplitSink, SplitStream};

mod try_stream;
pub use self::try_stream::{
    try_unfold, AndThen, ErrInto, InspectErr, InspectOk, IntoStream, MapErr, MapOk, OrElse, TryAll,
    TryAny, TryCollect, TryConcat, TryFilter, TryFilterMap, TryFlatten, TryFold, TryForEach,
    TryNext, TrySkipWhile, TryStreamExt, TryTakeWhile, TryUnfold,
};

#[cfg(feature = "io")]
#[cfg_attr(docsrs, doc(cfg(feature = "io")))]
#[cfg(feature = "std")]
pub use self::try_stream::IntoAsyncRead;

#[cfg_attr(target_os = "none", cfg(target_has_atomic = "ptr"))]
#[cfg(feature = "alloc")]
pub use self::try_stream::{
    TryBufferUnordered, TryBuffered, TryFlattenUnordered, TryForEachConcurrent,
};

#[cfg(feature = "alloc")]
pub use self::try_stream::{TryChunks, TryChunksError, TryReadyChunks, TryReadyChunksError};

// Primitive streams

mod iter;
pub use self::iter::{iter, Iter};

mod repeat;
pub use self::repeat::{repeat, Repeat};

mod repeat_with;
pub use self::repeat_with::{repeat_with, RepeatWith};

mod empty;
pub use self::empty::{empty, Empty};

mod once;
pub use self::once::{once, Once};

mod pending;
pub use self::pending::{pending, Pending};

mod poll_fn;
pub use self::poll_fn::{poll_fn, PollFn};

mod poll_immediate;
pub use self::poll_immediate::{poll_immediate, PollImmediate};

mod select;
pub use self::select::{select, Select};

mod select_with_strategy;
pub use self::select_with_strategy::{select_with_strategy, PollNext, SelectWithStrategy};

mod unfold;
pub use self::unfold::{unfold, Unfold};

#[cfg_attr(target_os = "none", cfg(target_has_atomic = "ptr"))]
#[cfg(feature = "alloc")]
mod futures_ordered;
#[cfg_attr(target_os = "none", cfg(target_has_atomic = "ptr"))]
#[cfg(feature = "alloc")]
pub use self::futures_ordered::FuturesOrdered;

#[cfg_attr(target_os = "none", cfg(target_has_atomic = "ptr"))]
#[cfg(feature = "alloc")]
pub mod futures_unordered;
#[cfg_attr(target_os = "none", cfg(target_has_atomic = "ptr"))]
#[cfg(feature = "alloc")]
#[doc(inline)]
pub use self::futures_unordered::FuturesUnordered;

#[cfg_attr(target_os = "none", cfg(target_has_atomic = "ptr"))]
#[cfg(feature = "alloc")]
pub mod select_all;
#[cfg_attr(target_os = "none", cfg(target_has_atomic = "ptr"))]
#[cfg(feature = "alloc")]
#[doc(inline)]
pub use self::select_all::{select_all, SelectAll};

#[cfg_attr(target_os = "none", cfg(target_has_atomic = "ptr"))]
#[cfg(feature = "alloc")]
mod abortable;
#[cfg_attr(target_os = "none", cfg(target_has_atomic = "ptr"))]
#[cfg(feature = "alloc")]
pub use crate::abortable::{AbortHandle, AbortRegistration, Abortable, Aborted};
#[cfg_attr(target_os = "none", cfg(target_has_atomic = "ptr"))]
#[cfg(feature = "alloc")]
pub use abortable::abortable;

// Just a helper function to ensure the streams we're returning all have the
// right implementations.
pub(crate) fn assert_stream<T, S>(stream: S) -> S
where
    S: Stream<Item = T>,
{
    stream
}
