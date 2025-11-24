/*
 * Copyright 2020 Actyx AG
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */
//! A mutual exclusion primitive that relies on static type information only
//!
//! This library is inspired by [this discussion](https://internals.rust-lang.org/t/what-shall-sync-mean-across-an-await/12020/2).
#![doc(html_logo_url = "https://developer.actyx.com/img/logo.svg")]
#![doc(html_favicon_url = "https://developer.actyx.com/img/favicon.ico")]
#![no_std]

use core::{
    fmt::{self, Debug, Formatter},
    pin::Pin,
    future::Future,
    task::{Context, Poll},
};

/// A mutual exclusion primitive that relies on static type information only
///
/// In some cases synchronization can be proven statically: whenever you hold an exclusive `&mut`
/// reference, the Rust type system ensures that no other part of the program can hold another
/// reference to the data. Therefore it is safe to access it even if the current thread obtained
/// this reference via a channel. Whenever this is the case, the overhead of allocating and locking
/// a [`Mutex`] can be avoided by using this static version.
///
/// One example where this is often applicable is [`Future`], which requires an exclusive reference
/// for its [`poll`] method: While a given `Future` implementation may not be safe to access by
/// multiple threads concurrently, the executor can only run the `Future` on one thread at any
/// given time, making it [`Sync`] in practice as long as the implementation is `Send`. You can
/// therefore use the static mutex to prove that your data structure is `Sync` even though it
/// contains such a `Future`.
///
/// # Example
///
/// ```
/// use sync_wrapper::SyncWrapper;
/// use std::future::Future;
///
/// struct MyThing {
///     future: SyncWrapper<Box<dyn Future<Output = String> + Send>>,
/// }
///
/// impl MyThing {
///     // all accesses to `self.future` now require an exclusive reference or ownership
/// }
///
/// fn assert_sync<T: Sync>() {}
///
/// assert_sync::<MyThing>();
/// ```
///
/// [`Mutex`]: https://doc.rust-lang.org/std/sync/struct.Mutex.html
/// [`Future`]: https://doc.rust-lang.org/std/future/trait.Future.html
/// [`poll`]: https://doc.rust-lang.org/std/future/trait.Future.html#method.poll
/// [`Sync`]: https://doc.rust-lang.org/std/marker/trait.Sync.html
#[repr(transparent)]
pub struct SyncWrapper<T>(T);

impl<T> SyncWrapper<T> {
    /// Creates a new static mutex containing the given value.
    ///
    /// # Examples
    ///
    /// ```
    /// use sync_wrapper::SyncWrapper;
    ///
    /// let mutex = SyncWrapper::new(42);
    /// ```
    pub const fn new(value: T) -> Self {
        Self(value)
    }

    /// Acquires a reference to the protected value.
    ///
    /// This is safe because it requires an exclusive reference to the mutex. Therefore this method
    /// neither panics nor does it return an error. This is in contrast to [`Mutex::get_mut`] which
    /// returns an error if another thread panicked while holding the lock. It is not recommended
    /// to send an exclusive reference to a potentially damaged value to another thread for further
    /// processing.
    ///
    /// [`Mutex::get_mut`]: https://doc.rust-lang.org/std/sync/struct.Mutex.html#method.get_mut
    ///
    /// # Examples
    ///
    /// ```
    /// use sync_wrapper::SyncWrapper;
    ///
    /// let mut mutex = SyncWrapper::new(42);
    /// let value = mutex.get_mut();
    /// *value = 0;
    /// assert_eq!(*mutex.get_mut(), 0);
    /// ```
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.0
    }

    /// Acquires a pinned reference to the protected value.
    ///
    /// See [`Self::get_mut`] for why this method is safe.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::future::Future;
    /// use std::pin::Pin;
    /// use std::task::{Context, Poll};
    ///
    /// use pin_project_lite::pin_project;
    /// use sync_wrapper::SyncWrapper;
    ///
    /// pin_project! {
    ///     struct FutureWrapper<F> {
    ///         #[pin]
    ///         inner: SyncWrapper<F>,
    ///     }
    /// }
    ///
    /// impl<F: Future> Future for FutureWrapper<F> {
    ///     type Output = F::Output;
    ///
    ///     fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    ///         self.project().inner.get_pin_mut().poll(cx)
    ///     }
    /// }
    /// ```
    pub fn get_pin_mut(self: Pin<&mut Self>) -> Pin<&mut T> {
        unsafe { Pin::map_unchecked_mut(self, |this| &mut this.0) }
    }

    /// Consumes this mutex, returning the underlying data.
    ///
    /// This is safe because it requires ownership of the mutex, therefore this method will neither
    /// panic nor does it return an error. This is in contrast to [`Mutex::into_inner`] which
    /// returns an error if another thread panicked while holding the lock. It is not recommended
    /// to send an exclusive reference to a potentially damaged value to another thread for further
    /// processing.
    ///
    /// [`Mutex::into_inner`]: https://doc.rust-lang.org/std/sync/struct.Mutex.html#method.into_inner
    ///
    /// # Examples
    ///
    /// ```
    /// use sync_wrapper::SyncWrapper;
    ///
    /// let mut mutex = SyncWrapper::new(42);
    /// assert_eq!(mutex.into_inner(), 42);
    /// ```
    pub fn into_inner(self) -> T {
        self.0
    }
}

// this is safe because the only operations permitted on this data structure require exclusive
// access or ownership
unsafe impl<T> Sync for SyncWrapper<T> {}

impl<T> Debug for SyncWrapper<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.pad("SyncWrapper")
    }
}

impl<T: Default> Default for SyncWrapper<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T> From<T> for SyncWrapper<T> {
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

/// `Future` which is `Sync`.
///
/// # Examples
///
/// ```
/// use sync_wrapper::{SyncWrapper, SyncFuture};
///
/// let fut = async { 1 };
/// let fut = SyncFuture::new(fut);
/// ```
pub struct SyncFuture<F> {
    inner: SyncWrapper<F>
}
impl <F: Future> SyncFuture<F> {
    pub fn new(inner: F) -> Self {
        Self { inner: SyncWrapper::new(inner) }
    }
    pub fn into_inner(self) -> F {
        self.inner.into_inner()
    }
}
impl <F: Future> Future for SyncFuture<F> {
    type Output = F::Output;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let inner = unsafe { self.map_unchecked_mut(|x| x.inner.get_mut()) };
        inner.poll(cx)
    }
}
impl<T> Debug for SyncFuture<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.pad("SyncFuture")
    }
}

/// `Stream` which is `Sync`.
///
/// # Examples
///
/// ```
/// use sync_wrapper::SyncStream;
/// use futures::stream;
///
/// let st = stream::iter(vec![1]);
/// let st = SyncStream::new(st);
/// ```
#[cfg(feature = "futures")]
pub struct SyncStream<S> {
    inner: SyncWrapper<S>
}
#[cfg(feature = "futures")]
impl <S: futures_core::Stream> SyncStream<S> {
    pub fn new(inner: S) -> Self {
        Self { inner: SyncWrapper::new(inner) }
    }
    pub fn into_inner(self) -> S {
        self.inner.into_inner()
    }
}
#[cfg(feature = "futures")]
impl <S: futures_core::Stream> futures_core::Stream for SyncStream<S> {
    type Item = S::Item;
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let inner = unsafe { self.map_unchecked_mut(|x| x.inner.get_mut()) };
        inner.poll_next(cx)
    }
}
#[cfg(feature = "futures")]
impl<T> Debug for SyncStream<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.pad("SyncStream")
    }
}
