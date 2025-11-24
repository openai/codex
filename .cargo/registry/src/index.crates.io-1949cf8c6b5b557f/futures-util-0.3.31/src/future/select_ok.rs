use super::assert_future;
use crate::future::TryFutureExt;
use alloc::vec::Vec;
use core::iter::FromIterator;
use core::mem;
use core::pin::Pin;
use futures_core::future::{Future, TryFuture};
use futures_core::task::{Context, Poll};

/// Future for the [`select_ok`] function.
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct SelectOk<Fut> {
    inner: Vec<Fut>,
}

impl<Fut: Unpin> Unpin for SelectOk<Fut> {}

/// Creates a new future which will select the first successful future over a list of futures.
///
/// The returned future will wait for any future within `iter` to be ready and Ok. Unlike
/// `select_all`, this will only return the first successful completion, or the last
/// failure. This is useful in contexts where any success is desired and failures
/// are ignored, unless all the futures fail.
///
///  This function is only available when the `std` or `alloc` feature of this
/// library is activated, and it is activated by default.
///
/// # Panics
///
/// This function will panic if the iterator specified contains no items.
pub fn select_ok<I>(iter: I) -> SelectOk<I::Item>
where
    I: IntoIterator,
    I::Item: TryFuture + Unpin,
{
    let ret = SelectOk { inner: iter.into_iter().collect() };
    assert!(!ret.inner.is_empty(), "iterator provided to select_ok was empty");
    assert_future::<
        Result<(<I::Item as TryFuture>::Ok, Vec<I::Item>), <I::Item as TryFuture>::Error>,
        _,
    >(ret)
}

impl<Fut: TryFuture + Unpin> Future for SelectOk<Fut> {
    type Output = Result<(Fut::Ok, Vec<Fut>), Fut::Error>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // loop until we've either exhausted all errors, a success was hit, or nothing is ready
        loop {
            let item =
                self.inner.iter_mut().enumerate().find_map(|(i, f)| match f.try_poll_unpin(cx) {
                    Poll::Pending => None,
                    Poll::Ready(e) => Some((i, e)),
                });
            match item {
                Some((idx, res)) => {
                    // always remove Ok or Err, if it's not the last Err continue looping
                    drop(self.inner.remove(idx));
                    match res {
                        Ok(e) => {
                            let rest = mem::take(&mut self.inner);
                            return Poll::Ready(Ok((e, rest)));
                        }
                        Err(e) => {
                            if self.inner.is_empty() {
                                return Poll::Ready(Err(e));
                            }
                        }
                    }
                }
                None => {
                    // based on the filter above, nothing is ready, return
                    return Poll::Pending;
                }
            }
        }
    }
}

impl<Fut: TryFuture + Unpin> FromIterator<Fut> for SelectOk<Fut> {
    fn from_iter<T: IntoIterator<Item = Fut>>(iter: T) -> Self {
        select_ok(iter)
    }
}
