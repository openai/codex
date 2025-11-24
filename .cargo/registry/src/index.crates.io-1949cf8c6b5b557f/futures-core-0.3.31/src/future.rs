//! Futures.

use core::ops::DerefMut;
use core::pin::Pin;
use core::task::{Context, Poll};

#[doc(no_inline)]
pub use core::future::Future;

/// An owned dynamically typed [`Future`] for use in cases where you can't
/// statically type your result or need to add some indirection.
///
/// This type is often created by the [`boxed`] method on [`FutureExt`]. See its documentation for more.
///
/// [`boxed`]: https://docs.rs/futures/latest/futures/future/trait.FutureExt.html#method.boxed
/// [`FutureExt`]: https://docs.rs/futures/latest/futures/future/trait.FutureExt.html
#[cfg(feature = "alloc")]
pub type BoxFuture<'a, T> = Pin<alloc::boxed::Box<dyn Future<Output = T> + Send + 'a>>;

/// `BoxFuture`, but without the `Send` requirement.
///
/// This type is often created by the [`boxed_local`] method on [`FutureExt`]. See its documentation for more.
///
/// [`boxed_local`]: https://docs.rs/futures/latest/futures/future/trait.FutureExt.html#method.boxed_local
/// [`FutureExt`]: https://docs.rs/futures/latest/futures/future/trait.FutureExt.html
#[cfg(feature = "alloc")]
pub type LocalBoxFuture<'a, T> = Pin<alloc::boxed::Box<dyn Future<Output = T> + 'a>>;

/// A future which tracks whether or not the underlying future
/// should no longer be polled.
///
/// `is_terminated` will return `true` if a future should no longer be polled.
/// Usually, this state occurs after `poll` (or `try_poll`) returned
/// `Poll::Ready`. However, `is_terminated` may also return `true` if a future
/// has become inactive and can no longer make progress and should be ignored
/// or dropped rather than being `poll`ed again.
pub trait FusedFuture: Future {
    /// Returns `true` if the underlying future should no longer be polled.
    fn is_terminated(&self) -> bool;
}

impl<F: FusedFuture + ?Sized + Unpin> FusedFuture for &mut F {
    fn is_terminated(&self) -> bool {
        <F as FusedFuture>::is_terminated(&**self)
    }
}

impl<P> FusedFuture for Pin<P>
where
    P: DerefMut + Unpin,
    P::Target: FusedFuture,
{
    fn is_terminated(&self) -> bool {
        <P::Target as FusedFuture>::is_terminated(&**self)
    }
}

mod private_try_future {
    use super::Future;

    pub trait Sealed {}

    impl<F, T, E> Sealed for F where F: ?Sized + Future<Output = Result<T, E>> {}
}

/// A convenience for futures that return `Result` values that includes
/// a variety of adapters tailored to such futures.
pub trait TryFuture: Future + private_try_future::Sealed {
    /// The type of successful values yielded by this future
    type Ok;

    /// The type of failures yielded by this future
    type Error;

    /// Poll this `TryFuture` as if it were a `Future`.
    ///
    /// This method is a stopgap for a compiler limitation that prevents us from
    /// directly inheriting from the `Future` trait; in the future it won't be
    /// needed.
    fn try_poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<Self::Ok, Self::Error>>;
}

impl<F, T, E> TryFuture for F
where
    F: ?Sized + Future<Output = Result<T, E>>,
{
    type Ok = T;
    type Error = E;

    #[inline]
    fn try_poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.poll(cx)
    }
}

#[cfg(feature = "alloc")]
mod if_alloc {
    use super::*;
    use alloc::boxed::Box;

    impl<F: FusedFuture + ?Sized + Unpin> FusedFuture for Box<F> {
        fn is_terminated(&self) -> bool {
            <F as FusedFuture>::is_terminated(&**self)
        }
    }

    #[cfg(feature = "std")]
    impl<F: FusedFuture> FusedFuture for std::panic::AssertUnwindSafe<F> {
        fn is_terminated(&self) -> bool {
            <F as FusedFuture>::is_terminated(&**self)
        }
    }
}
