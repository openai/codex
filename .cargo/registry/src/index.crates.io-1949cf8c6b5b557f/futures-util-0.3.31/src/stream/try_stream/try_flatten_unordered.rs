use core::marker::PhantomData;
use core::pin::Pin;

use futures_core::ready;
use futures_core::stream::{FusedStream, Stream, TryStream};
use futures_core::task::{Context, Poll};
#[cfg(feature = "sink")]
use futures_sink::Sink;

use pin_project_lite::pin_project;

use crate::future::Either;
use crate::stream::stream::flatten_unordered::{
    FlattenUnorderedWithFlowController, FlowController, FlowStep,
};
use crate::stream::IntoStream;
use crate::TryStreamExt;

delegate_all!(
    /// Stream for the [`try_flatten_unordered`](super::TryStreamExt::try_flatten_unordered) method.
    TryFlattenUnordered<St>(
        FlattenUnorderedWithFlowController<NestedTryStreamIntoEitherTryStream<St>, PropagateBaseStreamError<St>>
    ): Debug + Sink + Stream + FusedStream + AccessInner[St, (. .)]
        + New[
            |stream: St, limit: impl Into<Option<usize>>|
                FlattenUnorderedWithFlowController::new(
                    NestedTryStreamIntoEitherTryStream::new(stream),
                    limit.into()
                )
        ]
    where
        St: TryStream,
        St::Ok: TryStream,
        St::Ok: Unpin,
        <St::Ok as TryStream>::Error: From<St::Error>
);

pin_project! {
    /// Emits either successful streams or single-item streams containing the underlying errors.
    /// This's a wrapper for `FlattenUnordered` to reuse its logic over `TryStream`.
    #[derive(Debug)]
    #[must_use = "streams do nothing unless polled"]
    pub struct NestedTryStreamIntoEitherTryStream<St>
        where
            St: TryStream,
            St::Ok: TryStream,
            St::Ok: Unpin,
            <St::Ok as TryStream>::Error: From<St::Error>
        {
            #[pin]
            stream: St
        }
}

impl<St> NestedTryStreamIntoEitherTryStream<St>
where
    St: TryStream,
    St::Ok: TryStream + Unpin,
    <St::Ok as TryStream>::Error: From<St::Error>,
{
    fn new(stream: St) -> Self {
        Self { stream }
    }

    delegate_access_inner!(stream, St, ());
}

/// Emits a single item immediately, then stream will be terminated.
#[derive(Debug, Clone)]
pub struct Single<T>(Option<T>);

impl<T> Single<T> {
    /// Constructs new `Single` with the given value.
    fn new(val: T) -> Self {
        Self(Some(val))
    }

    /// Attempts to take inner item immediately. Will always succeed if the stream isn't terminated.
    fn next_immediate(&mut self) -> Option<T> {
        self.0.take()
    }
}

impl<T> Unpin for Single<T> {}

impl<T> Stream for Single<T> {
    type Item = T;

    fn poll_next(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Poll::Ready(self.0.take())
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.as_ref().map_or((0, Some(0)), |_| (1, Some(1)))
    }
}

/// Immediately propagates errors occurred in the base stream.
#[derive(Debug, Clone, Copy)]
pub struct PropagateBaseStreamError<St>(PhantomData<St>);

type BaseStreamItem<St> = <NestedTryStreamIntoEitherTryStream<St> as Stream>::Item;
type InnerStreamItem<St> = <BaseStreamItem<St> as Stream>::Item;

impl<St> FlowController<BaseStreamItem<St>, InnerStreamItem<St>> for PropagateBaseStreamError<St>
where
    St: TryStream,
    St::Ok: TryStream + Unpin,
    <St::Ok as TryStream>::Error: From<St::Error>,
{
    fn next_step(item: BaseStreamItem<St>) -> FlowStep<BaseStreamItem<St>, InnerStreamItem<St>> {
        match item {
            // A new successful inner stream received
            st @ Either::Left(_) => FlowStep::Continue(st),
            // An error encountered
            Either::Right(mut err) => FlowStep::Return(err.next_immediate().unwrap()),
        }
    }
}

type SingleStreamResult<St> = Single<Result<<St as TryStream>::Ok, <St as TryStream>::Error>>;

impl<St> Stream for NestedTryStreamIntoEitherTryStream<St>
where
    St: TryStream,
    St::Ok: TryStream + Unpin,
    <St::Ok as TryStream>::Error: From<St::Error>,
{
    // Item is either an inner stream or a stream containing a single error.
    // This will allow using `Either`'s `Stream` implementation as both branches are actually streams of `Result`'s.
    type Item = Either<IntoStream<St::Ok>, SingleStreamResult<St::Ok>>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let item = ready!(self.project().stream.try_poll_next(cx));

        let out = match item {
            Some(res) => match res {
                // Emit successful inner stream as is
                Ok(stream) => Either::Left(stream.into_stream()),
                // Wrap an error into a stream containing a single item
                err @ Err(_) => {
                    let res = err.map(|_: St::Ok| unreachable!()).map_err(Into::into);

                    Either::Right(Single::new(res))
                }
            },
            None => return Poll::Ready(None),
        };

        Poll::Ready(Some(out))
    }
}

impl<St> FusedStream for NestedTryStreamIntoEitherTryStream<St>
where
    St: TryStream + FusedStream,
    St::Ok: TryStream + Unpin,
    <St::Ok as TryStream>::Error: From<St::Error>,
{
    fn is_terminated(&self) -> bool {
        self.stream.is_terminated()
    }
}

// Forwarding impl of Sink from the underlying stream
#[cfg(feature = "sink")]
impl<St, Item> Sink<Item> for NestedTryStreamIntoEitherTryStream<St>
where
    St: TryStream + Sink<Item>,
    St::Ok: TryStream + Unpin,
    <St::Ok as TryStream>::Error: From<<St as TryStream>::Error>,
{
    type Error = <St as Sink<Item>>::Error;

    delegate_sink!(stream, Item);
}
