use super::assert_sink;
use crate::unfold_state::UnfoldState;
use core::{future::Future, pin::Pin};
use futures_core::ready;
use futures_core::task::{Context, Poll};
use futures_sink::Sink;
use pin_project_lite::pin_project;

pin_project! {
    /// Sink for the [`unfold`] function.
    #[derive(Debug)]
    #[must_use = "sinks do nothing unless polled"]
    pub struct Unfold<T, F, R> {
        function: F,
        #[pin]
        state: UnfoldState<T, R>,
    }
}

/// Create a sink from a function which processes one item at a time.
///
/// # Examples
///
/// ```
/// # futures::executor::block_on(async {
/// use futures::sink::{self, SinkExt};
///
/// let unfold = sink::unfold(0, |mut sum, i: i32| {
///     async move {
///         sum += i;
///         eprintln!("{}", i);
///         Ok::<_, futures::never::Never>(sum)
///     }
/// });
/// futures::pin_mut!(unfold);
/// unfold.send(5).await?;
/// # Ok::<(), futures::never::Never>(()) }).unwrap();
/// ```
pub fn unfold<T, F, R, Item, E>(init: T, function: F) -> Unfold<T, F, R>
where
    F: FnMut(T, Item) -> R,
    R: Future<Output = Result<T, E>>,
{
    assert_sink::<Item, E, _>(Unfold { function, state: UnfoldState::Value { value: init } })
}

impl<T, F, R, Item, E> Sink<Item> for Unfold<T, F, R>
where
    F: FnMut(T, Item) -> R,
    R: Future<Output = Result<T, E>>,
{
    type Error = E;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.poll_flush(cx)
    }

    fn start_send(self: Pin<&mut Self>, item: Item) -> Result<(), Self::Error> {
        let mut this = self.project();
        let future = match this.state.as_mut().take_value() {
            Some(value) => (this.function)(value, item),
            None => panic!("start_send called without poll_ready being called first"),
        };
        this.state.set(UnfoldState::Future { future });
        Ok(())
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let mut this = self.project();
        Poll::Ready(if let Some(future) = this.state.as_mut().project_future() {
            match ready!(future.poll(cx)) {
                Ok(state) => {
                    this.state.set(UnfoldState::Value { value: state });
                    Ok(())
                }
                Err(err) => {
                    this.state.set(UnfoldState::Empty);
                    Err(err)
                }
            }
        } else {
            Ok(())
        })
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.poll_flush(cx)
    }
}
