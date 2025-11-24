use super::assert_stream;
use crate::unfold_state::UnfoldState;
use core::fmt;
use core::pin::Pin;
use futures_core::future::Future;
use futures_core::ready;
use futures_core::stream::{FusedStream, Stream};
use futures_core::task::{Context, Poll};
use pin_project_lite::pin_project;

/// Creates a `Stream` from a seed and a closure returning a `Future`.
///
/// This function is the dual for the `Stream::fold()` adapter: while
/// `Stream::fold()` reduces a `Stream` to one single value, `unfold()` creates a
/// `Stream` from a seed value.
///
/// `unfold()` will call the provided closure with the provided seed, then wait
/// for the returned `Future` to complete with `(a, b)`. It will then yield the
/// value `a`, and use `b` as the next internal state.
///
/// If the closure returns `None` instead of `Some(Future)`, then the `unfold()`
/// will stop producing items and return `Poll::Ready(None)` in future
/// calls to `poll()`.
///
/// This function can typically be used when wanting to go from the "world of
/// futures" to the "world of streams": the provided closure can build a
/// `Future` using other library functions working on futures, and `unfold()`
/// will turn it into a `Stream` by repeating the operation.
///
/// # Example
///
/// ```
/// # futures::executor::block_on(async {
/// use futures::stream::{self, StreamExt};
///
/// let stream = stream::unfold(0, |state| async move {
///     if state <= 2 {
///         let next_state = state + 1;
///         let yielded = state * 2;
///         Some((yielded, next_state))
///     } else {
///         None
///     }
/// });
///
/// let result = stream.collect::<Vec<i32>>().await;
/// assert_eq!(result, vec![0, 2, 4]);
/// # });
/// ```
pub fn unfold<T, F, Fut, Item>(init: T, f: F) -> Unfold<T, F, Fut>
where
    F: FnMut(T) -> Fut,
    Fut: Future<Output = Option<(Item, T)>>,
{
    assert_stream::<Item, _>(Unfold { f, state: UnfoldState::Value { value: init } })
}

pin_project! {
    /// Stream for the [`unfold`] function.
    #[must_use = "streams do nothing unless polled"]
    pub struct Unfold<T, F, Fut> {
        f: F,
        #[pin]
        state: UnfoldState<T, Fut>,
    }
}

impl<T, F, Fut> fmt::Debug for Unfold<T, F, Fut>
where
    T: fmt::Debug,
    Fut: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Unfold").field("state", &self.state).finish()
    }
}

impl<T, F, Fut, Item> FusedStream for Unfold<T, F, Fut>
where
    F: FnMut(T) -> Fut,
    Fut: Future<Output = Option<(Item, T)>>,
{
    fn is_terminated(&self) -> bool {
        if let UnfoldState::Empty = self.state {
            true
        } else {
            false
        }
    }
}

impl<T, F, Fut, Item> Stream for Unfold<T, F, Fut>
where
    F: FnMut(T) -> Fut,
    Fut: Future<Output = Option<(Item, T)>>,
{
    type Item = Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();

        if let Some(state) = this.state.as_mut().take_value() {
            this.state.set(UnfoldState::Future { future: (this.f)(state) });
        }

        let step = match this.state.as_mut().project_future() {
            Some(fut) => ready!(fut.poll(cx)),
            None => panic!("Unfold must not be polled after it returned `Poll::Ready(None)`"),
        };

        if let Some((item, next_state)) = step {
            this.state.set(UnfoldState::Value { value: next_state });
            Poll::Ready(Some(item))
        } else {
            this.state.set(UnfoldState::Empty);
            Poll::Ready(None)
        }
    }
}
