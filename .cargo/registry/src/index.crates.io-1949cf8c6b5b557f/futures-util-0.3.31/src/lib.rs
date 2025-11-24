//! Combinators and utilities for working with `Future`s, `Stream`s, `Sink`s,
//! and the `AsyncRead` and `AsyncWrite` traits.

#![no_std]
#![doc(test(
    no_crate_inject,
    attr(
        deny(warnings, rust_2018_idioms, single_use_lifetimes),
        allow(dead_code, unused_assignments, unused_variables)
    )
))]
#![warn(missing_docs, unsafe_op_in_unsafe_fn)]
#![cfg_attr(docsrs, feature(doc_cfg))]

#[cfg(all(feature = "bilock", not(feature = "unstable")))]
compile_error!("The `bilock` feature requires the `unstable` feature as an explicit opt-in to unstable features");

#[cfg(feature = "alloc")]
extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

// Macro re-exports
pub use futures_core::ready;
pub use pin_utils::pin_mut;

#[cfg(feature = "async-await")]
#[macro_use]
mod async_await;
#[cfg(feature = "async-await")]
#[doc(hidden)]
pub use self::async_await::*;

// Not public API.
#[cfg(feature = "async-await")]
#[doc(hidden)]
pub mod __private {
    pub use crate::*;
    pub use core::{
        option::Option::{self, None, Some},
        pin::Pin,
        result::Result::{Err, Ok},
    };

    pub mod async_await {
        pub use crate::async_await::*;
    }
}

#[cfg(feature = "sink")]
macro_rules! delegate_sink {
    ($field:ident, $item:ty) => {
        fn poll_ready(
            self: core::pin::Pin<&mut Self>,
            cx: &mut core::task::Context<'_>,
        ) -> core::task::Poll<Result<(), Self::Error>> {
            self.project().$field.poll_ready(cx)
        }

        fn start_send(self: core::pin::Pin<&mut Self>, item: $item) -> Result<(), Self::Error> {
            self.project().$field.start_send(item)
        }

        fn poll_flush(
            self: core::pin::Pin<&mut Self>,
            cx: &mut core::task::Context<'_>,
        ) -> core::task::Poll<Result<(), Self::Error>> {
            self.project().$field.poll_flush(cx)
        }

        fn poll_close(
            self: core::pin::Pin<&mut Self>,
            cx: &mut core::task::Context<'_>,
        ) -> core::task::Poll<Result<(), Self::Error>> {
            self.project().$field.poll_close(cx)
        }
    };
}

macro_rules! delegate_future {
    ($field:ident) => {
        fn poll(
            self: core::pin::Pin<&mut Self>,
            cx: &mut core::task::Context<'_>,
        ) -> core::task::Poll<Self::Output> {
            self.project().$field.poll(cx)
        }
    };
}

macro_rules! delegate_stream {
    ($field:ident) => {
        fn poll_next(
            self: core::pin::Pin<&mut Self>,
            cx: &mut core::task::Context<'_>,
        ) -> core::task::Poll<Option<Self::Item>> {
            self.project().$field.poll_next(cx)
        }
        fn size_hint(&self) -> (usize, Option<usize>) {
            self.$field.size_hint()
        }
    };
}

#[cfg(feature = "io")]
#[cfg(feature = "std")]
macro_rules! delegate_async_write {
    ($field:ident) => {
        fn poll_write(
            self: core::pin::Pin<&mut Self>,
            cx: &mut core::task::Context<'_>,
            buf: &[u8],
        ) -> core::task::Poll<std::io::Result<usize>> {
            self.project().$field.poll_write(cx, buf)
        }
        fn poll_write_vectored(
            self: core::pin::Pin<&mut Self>,
            cx: &mut core::task::Context<'_>,
            bufs: &[std::io::IoSlice<'_>],
        ) -> core::task::Poll<std::io::Result<usize>> {
            self.project().$field.poll_write_vectored(cx, bufs)
        }
        fn poll_flush(
            self: core::pin::Pin<&mut Self>,
            cx: &mut core::task::Context<'_>,
        ) -> core::task::Poll<std::io::Result<()>> {
            self.project().$field.poll_flush(cx)
        }
        fn poll_close(
            self: core::pin::Pin<&mut Self>,
            cx: &mut core::task::Context<'_>,
        ) -> core::task::Poll<std::io::Result<()>> {
            self.project().$field.poll_close(cx)
        }
    };
}

#[cfg(feature = "io")]
#[cfg(feature = "std")]
macro_rules! delegate_async_read {
    ($field:ident) => {
        fn poll_read(
            self: core::pin::Pin<&mut Self>,
            cx: &mut core::task::Context<'_>,
            buf: &mut [u8],
        ) -> core::task::Poll<std::io::Result<usize>> {
            self.project().$field.poll_read(cx, buf)
        }

        fn poll_read_vectored(
            self: core::pin::Pin<&mut Self>,
            cx: &mut core::task::Context<'_>,
            bufs: &mut [std::io::IoSliceMut<'_>],
        ) -> core::task::Poll<std::io::Result<usize>> {
            self.project().$field.poll_read_vectored(cx, bufs)
        }
    };
}

#[cfg(feature = "io")]
#[cfg(feature = "std")]
macro_rules! delegate_async_buf_read {
    ($field:ident) => {
        fn poll_fill_buf(
            self: core::pin::Pin<&mut Self>,
            cx: &mut core::task::Context<'_>,
        ) -> core::task::Poll<std::io::Result<&[u8]>> {
            self.project().$field.poll_fill_buf(cx)
        }

        fn consume(self: core::pin::Pin<&mut Self>, amt: usize) {
            self.project().$field.consume(amt)
        }
    };
}

macro_rules! delegate_access_inner {
    ($field:ident, $inner:ty, ($($ind:tt)*)) => {
        /// Acquires a reference to the underlying sink or stream that this combinator is
        /// pulling from.
        pub fn get_ref(&self) -> &$inner {
            (&self.$field) $($ind get_ref())*
        }

        /// Acquires a mutable reference to the underlying sink or stream that this
        /// combinator is pulling from.
        ///
        /// Note that care must be taken to avoid tampering with the state of the
        /// sink or stream which may otherwise confuse this combinator.
        pub fn get_mut(&mut self) -> &mut $inner {
            (&mut self.$field) $($ind get_mut())*
        }

        /// Acquires a pinned mutable reference to the underlying sink or stream that this
        /// combinator is pulling from.
        ///
        /// Note that care must be taken to avoid tampering with the state of the
        /// sink or stream which may otherwise confuse this combinator.
        pub fn get_pin_mut(self: core::pin::Pin<&mut Self>) -> core::pin::Pin<&mut $inner> {
            self.project().$field $($ind get_pin_mut())*
        }

        /// Consumes this combinator, returning the underlying sink or stream.
        ///
        /// Note that this may discard intermediate state of this combinator, so
        /// care should be taken to avoid losing resources when this is called.
        pub fn into_inner(self) -> $inner {
            self.$field $($ind into_inner())*
        }
    }
}

macro_rules! delegate_all {
    (@trait Future $name:ident < $($arg:ident),* > ($t:ty) $(where $($bound:tt)*)*) => {
        impl<$($arg),*> futures_core::future::Future for $name<$($arg),*> where $t: futures_core::future::Future $(, $($bound)*)* {
            type Output = <$t as futures_core::future::Future>::Output;

            delegate_future!(inner);
        }
    };
    (@trait FusedFuture $name:ident < $($arg:ident),* > ($t:ty) $(where $($bound:tt)*)*) => {
        impl<$($arg),*> futures_core::future::FusedFuture for $name<$($arg),*> where $t: futures_core::future::FusedFuture $(, $($bound)*)* {
            fn is_terminated(&self) -> bool {
                self.inner.is_terminated()
            }
        }
    };
    (@trait Stream $name:ident < $($arg:ident),* > ($t:ty) $(where $($bound:tt)*)*) => {
        impl<$($arg),*> futures_core::stream::Stream for $name<$($arg),*> where $t: futures_core::stream::Stream $(, $($bound)*)* {
            type Item = <$t as futures_core::stream::Stream>::Item;

            delegate_stream!(inner);
        }
    };
    (@trait FusedStream $name:ident < $($arg:ident),* > ($t:ty) $(where $($bound:tt)*)*) => {
        impl<$($arg),*> futures_core::stream::FusedStream for $name<$($arg),*> where $t: futures_core::stream::FusedStream $(, $($bound)*)* {
            fn is_terminated(&self) -> bool {
                self.inner.is_terminated()
            }
        }
    };
    (@trait Sink $name:ident < $($arg:ident),* > ($t:ty) $(where $($bound:tt)*)*) => {
        #[cfg(feature = "sink")]
        impl<_Item, $($arg),*> futures_sink::Sink<_Item> for $name<$($arg),*> where $t: futures_sink::Sink<_Item> $(, $($bound)*)* {
            type Error = <$t as futures_sink::Sink<_Item>>::Error;

            delegate_sink!(inner, _Item);
        }
    };
    (@trait Debug $name:ident < $($arg:ident),* > ($t:ty) $(where $($bound:tt)*)*) => {
        impl<$($arg),*> core::fmt::Debug for $name<$($arg),*> where $t: core::fmt::Debug $(, $($bound)*)* {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                core::fmt::Debug::fmt(&self.inner, f)
            }
        }
    };
    (@trait AccessInner[$inner:ty, ($($ind:tt)*)] $name:ident < $($arg:ident),* > ($t:ty) $(where $($bound:tt)*)*) => {
        impl<$($arg),*> $name<$($arg),*> $(where $($bound)*)* {
            delegate_access_inner!(inner, $inner, ($($ind)*));
        }
    };
    (@trait New[|$($param:ident: $paramt:ty),*| $cons:expr] $name:ident < $($arg:ident),* > ($t:ty) $(where $($bound:tt)*)*) => {
        impl<$($arg),*> $name<$($arg),*> $(where $($bound)*)* {
            pub(crate) fn new($($param: $paramt),*) -> Self {
                Self { inner: $cons }
            }
        }
    };
    ($(#[$attr:meta])* $name:ident<$($arg:ident),*>($t:ty) : $ftrait:ident $([$($targs:tt)*])* $({$($item:tt)*})* $(where $($bound:tt)*)*) => {
        pin_project_lite::pin_project! {
            #[must_use = "futures/streams/sinks do nothing unless you `.await` or poll them"]
            $(#[$attr])*
            pub struct $name< $($arg),* > $(where $($bound)*)* { #[pin] inner: $t }
        }

        impl<$($arg),*> $name< $($arg),* > $(where $($bound)*)* {
            $($($item)*)*
        }

        delegate_all!(@trait $ftrait $([$($targs)*])* $name<$($arg),*>($t) $(where $($bound)*)*);
    };
    ($(#[$attr:meta])* $name:ident<$($arg:ident),*>($t:ty) : $ftrait:ident $([$($ftargs:tt)*])* + $strait:ident $([$($stargs:tt)*])* $(+ $trait:ident $([$($targs:tt)*])*)* $({$($item:tt)*})* $(where $($bound:tt)*)*) => {
        delegate_all!($(#[$attr])* $name<$($arg),*>($t) : $strait $([$($stargs)*])* $(+ $trait $([$($targs)*])*)* $({$($item)*})* $(where $($bound)*)*);

        delegate_all!(@trait $ftrait $([$($ftargs)*])* $name<$($arg),*>($t) $(where $($bound)*)*);
    };
}

pub mod future;
#[doc(no_inline)]
pub use crate::future::{Future, FutureExt, TryFuture, TryFutureExt};

pub mod stream;
#[doc(no_inline)]
pub use crate::stream::{Stream, StreamExt, TryStream, TryStreamExt};

#[cfg(feature = "sink")]
#[cfg_attr(docsrs, doc(cfg(feature = "sink")))]
pub mod sink;
#[cfg(feature = "sink")]
#[doc(no_inline)]
pub use crate::sink::{Sink, SinkExt};

pub mod task;

pub mod never;

#[cfg(feature = "compat")]
#[cfg_attr(docsrs, doc(cfg(feature = "compat")))]
pub mod compat;

#[cfg(feature = "io")]
#[cfg_attr(docsrs, doc(cfg(feature = "io")))]
#[cfg(feature = "std")]
pub mod io;
#[cfg(feature = "io")]
#[cfg(feature = "std")]
#[doc(no_inline)]
pub use crate::io::{
    AsyncBufRead, AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncSeek, AsyncSeekExt, AsyncWrite,
    AsyncWriteExt,
};

#[cfg(feature = "alloc")]
pub mod lock;

#[cfg_attr(target_os = "none", cfg(target_has_atomic = "ptr"))]
#[cfg(feature = "alloc")]
mod abortable;

mod fns;
mod unfold_state;
