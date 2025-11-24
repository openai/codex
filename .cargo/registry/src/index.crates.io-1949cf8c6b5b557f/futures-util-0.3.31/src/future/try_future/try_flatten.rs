use core::pin::Pin;
use futures_core::future::{FusedFuture, Future, TryFuture};
use futures_core::ready;
use futures_core::stream::{FusedStream, Stream, TryStream};
use futures_core::task::{Context, Poll};
#[cfg(feature = "sink")]
use futures_sink::Sink;
use pin_project_lite::pin_project;

pin_project! {
    #[project = TryFlattenProj]
    #[derive(Debug)]
    pub enum TryFlatten<Fut1, Fut2> {
        First { #[pin] f: Fut1 },
        Second { #[pin] f: Fut2 },
        Empty,
    }
}

impl<Fut1, Fut2> TryFlatten<Fut1, Fut2> {
    pub(crate) fn new(future: Fut1) -> Self {
        Self::First { f: future }
    }
}

impl<Fut> FusedFuture for TryFlatten<Fut, Fut::Ok>
where
    Fut: TryFuture,
    Fut::Ok: TryFuture<Error = Fut::Error>,
{
    fn is_terminated(&self) -> bool {
        match self {
            Self::Empty => true,
            _ => false,
        }
    }
}

impl<Fut> Future for TryFlatten<Fut, Fut::Ok>
where
    Fut: TryFuture,
    Fut::Ok: TryFuture<Error = Fut::Error>,
{
    type Output = Result<<Fut::Ok as TryFuture>::Ok, Fut::Error>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Poll::Ready(loop {
            match self.as_mut().project() {
                TryFlattenProj::First { f } => match ready!(f.try_poll(cx)) {
                    Ok(f) => self.set(Self::Second { f }),
                    Err(e) => {
                        self.set(Self::Empty);
                        break Err(e);
                    }
                },
                TryFlattenProj::Second { f } => {
                    let output = ready!(f.try_poll(cx));
                    self.set(Self::Empty);
                    break output;
                }
                TryFlattenProj::Empty => panic!("TryFlatten polled after completion"),
            }
        })
    }
}

impl<Fut> FusedStream for TryFlatten<Fut, Fut::Ok>
where
    Fut: TryFuture,
    Fut::Ok: TryStream<Error = Fut::Error>,
{
    fn is_terminated(&self) -> bool {
        match self {
            Self::Empty => true,
            _ => false,
        }
    }
}

impl<Fut> Stream for TryFlatten<Fut, Fut::Ok>
where
    Fut: TryFuture,
    Fut::Ok: TryStream<Error = Fut::Error>,
{
    type Item = Result<<Fut::Ok as TryStream>::Ok, Fut::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Poll::Ready(loop {
            match self.as_mut().project() {
                TryFlattenProj::First { f } => match ready!(f.try_poll(cx)) {
                    Ok(f) => self.set(Self::Second { f }),
                    Err(e) => {
                        self.set(Self::Empty);
                        break Some(Err(e));
                    }
                },
                TryFlattenProj::Second { f } => {
                    let output = ready!(f.try_poll_next(cx));
                    if output.is_none() {
                        self.set(Self::Empty);
                    }
                    break output;
                }
                TryFlattenProj::Empty => break None,
            }
        })
    }
}

#[cfg(feature = "sink")]
impl<Fut, Item> Sink<Item> for TryFlatten<Fut, Fut::Ok>
where
    Fut: TryFuture,
    Fut::Ok: Sink<Item, Error = Fut::Error>,
{
    type Error = Fut::Error;

    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(loop {
            match self.as_mut().project() {
                TryFlattenProj::First { f } => match ready!(f.try_poll(cx)) {
                    Ok(f) => self.set(Self::Second { f }),
                    Err(e) => {
                        self.set(Self::Empty);
                        break Err(e);
                    }
                },
                TryFlattenProj::Second { f } => {
                    break ready!(f.poll_ready(cx));
                }
                TryFlattenProj::Empty => panic!("poll_ready called after eof"),
            }
        })
    }

    fn start_send(self: Pin<&mut Self>, item: Item) -> Result<(), Self::Error> {
        match self.project() {
            TryFlattenProj::First { .. } => panic!("poll_ready not called first"),
            TryFlattenProj::Second { f } => f.start_send(item),
            TryFlattenProj::Empty => panic!("start_send called after eof"),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        match self.project() {
            TryFlattenProj::First { .. } => Poll::Ready(Ok(())),
            TryFlattenProj::Second { f } => f.poll_flush(cx),
            TryFlattenProj::Empty => panic!("poll_flush called after eof"),
        }
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let res = match self.as_mut().project() {
            TryFlattenProj::Second { f } => f.poll_close(cx),
            _ => Poll::Ready(Ok(())),
        };
        if res.is_ready() {
            self.set(Self::Empty);
        }
        res
    }
}
