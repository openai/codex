#![allow(non_snake_case)]

use super::assert_future;
use crate::future::{maybe_done, MaybeDone};
use core::fmt;
use core::pin::Pin;
use futures_core::future::{FusedFuture, Future};
use futures_core::task::{Context, Poll};
use pin_project_lite::pin_project;

macro_rules! generate {
    ($(
        $(#[$doc:meta])*
        ($Join:ident, <$($Fut:ident),*>),
    )*) => ($(
        pin_project! {
            $(#[$doc])*
            #[must_use = "futures do nothing unless you `.await` or poll them"]
            pub struct $Join<$($Fut: Future),*> {
                $(#[pin] $Fut: MaybeDone<$Fut>,)*
            }
        }

        impl<$($Fut),*> fmt::Debug for $Join<$($Fut),*>
        where
            $(
                $Fut: Future + fmt::Debug,
                $Fut::Output: fmt::Debug,
            )*
        {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_struct(stringify!($Join))
                    $(.field(stringify!($Fut), &self.$Fut))*
                    .finish()
            }
        }

        impl<$($Fut: Future),*> $Join<$($Fut),*> {
            fn new($($Fut: $Fut),*) -> Self {
                Self {
                    $($Fut: maybe_done($Fut)),*
                }
            }
        }

        impl<$($Fut: Future),*> Future for $Join<$($Fut),*> {
            type Output = ($($Fut::Output),*);

            fn poll(
                self: Pin<&mut Self>, cx: &mut Context<'_>
            ) -> Poll<Self::Output> {
                let mut all_done = true;
                let mut futures = self.project();
                $(
                    all_done &= futures.$Fut.as_mut().poll(cx).is_ready();
                )*

                if all_done {
                    Poll::Ready(($(futures.$Fut.take_output().unwrap()), *))
                } else {
                    Poll::Pending
                }
            }
        }

        impl<$($Fut: FusedFuture),*> FusedFuture for $Join<$($Fut),*> {
            fn is_terminated(&self) -> bool {
                $(
                    self.$Fut.is_terminated()
                ) && *
            }
        }
    )*)
}

generate! {
    /// Future for the [`join`](join()) function.
    (Join, <Fut1, Fut2>),

    /// Future for the [`join3`] function.
    (Join3, <Fut1, Fut2, Fut3>),

    /// Future for the [`join4`] function.
    (Join4, <Fut1, Fut2, Fut3, Fut4>),

    /// Future for the [`join5`] function.
    (Join5, <Fut1, Fut2, Fut3, Fut4, Fut5>),
}

/// Joins the result of two futures, waiting for them both to complete.
///
/// This function will return a new future which awaits both futures to
/// complete. The returned future will finish with a tuple of both results.
///
/// Note that this function consumes the passed futures and returns a
/// wrapped version of it.
///
/// # Examples
///
/// ```
/// # futures::executor::block_on(async {
/// use futures::future;
///
/// let a = async { 1 };
/// let b = async { 2 };
/// let pair = future::join(a, b);
///
/// assert_eq!(pair.await, (1, 2));
/// # });
/// ```
pub fn join<Fut1, Fut2>(future1: Fut1, future2: Fut2) -> Join<Fut1, Fut2>
where
    Fut1: Future,
    Fut2: Future,
{
    let f = Join::new(future1, future2);
    assert_future::<(Fut1::Output, Fut2::Output), _>(f)
}

/// Same as [`join`](join()), but with more futures.
///
/// # Examples
///
/// ```
/// # futures::executor::block_on(async {
/// use futures::future;
///
/// let a = async { 1 };
/// let b = async { 2 };
/// let c = async { 3 };
/// let tuple = future::join3(a, b, c);
///
/// assert_eq!(tuple.await, (1, 2, 3));
/// # });
/// ```
pub fn join3<Fut1, Fut2, Fut3>(
    future1: Fut1,
    future2: Fut2,
    future3: Fut3,
) -> Join3<Fut1, Fut2, Fut3>
where
    Fut1: Future,
    Fut2: Future,
    Fut3: Future,
{
    let f = Join3::new(future1, future2, future3);
    assert_future::<(Fut1::Output, Fut2::Output, Fut3::Output), _>(f)
}

/// Same as [`join`](join()), but with more futures.
///
/// # Examples
///
/// ```
/// # futures::executor::block_on(async {
/// use futures::future;
///
/// let a = async { 1 };
/// let b = async { 2 };
/// let c = async { 3 };
/// let d = async { 4 };
/// let tuple = future::join4(a, b, c, d);
///
/// assert_eq!(tuple.await, (1, 2, 3, 4));
/// # });
/// ```
pub fn join4<Fut1, Fut2, Fut3, Fut4>(
    future1: Fut1,
    future2: Fut2,
    future3: Fut3,
    future4: Fut4,
) -> Join4<Fut1, Fut2, Fut3, Fut4>
where
    Fut1: Future,
    Fut2: Future,
    Fut3: Future,
    Fut4: Future,
{
    let f = Join4::new(future1, future2, future3, future4);
    assert_future::<(Fut1::Output, Fut2::Output, Fut3::Output, Fut4::Output), _>(f)
}

/// Same as [`join`](join()), but with more futures.
///
/// # Examples
///
/// ```
/// # futures::executor::block_on(async {
/// use futures::future;
///
/// let a = async { 1 };
/// let b = async { 2 };
/// let c = async { 3 };
/// let d = async { 4 };
/// let e = async { 5 };
/// let tuple = future::join5(a, b, c, d, e);
///
/// assert_eq!(tuple.await, (1, 2, 3, 4, 5));
/// # });
/// ```
pub fn join5<Fut1, Fut2, Fut3, Fut4, Fut5>(
    future1: Fut1,
    future2: Fut2,
    future3: Fut3,
    future4: Fut4,
    future5: Fut5,
) -> Join5<Fut1, Fut2, Fut3, Fut4, Fut5>
where
    Fut1: Future,
    Fut2: Future,
    Fut3: Future,
    Fut4: Future,
    Fut5: Future,
{
    let f = Join5::new(future1, future2, future3, future4, future5);
    assert_future::<(Fut1::Output, Fut2::Output, Fut3::Output, Fut4::Output, Fut5::Output), _>(f)
}
