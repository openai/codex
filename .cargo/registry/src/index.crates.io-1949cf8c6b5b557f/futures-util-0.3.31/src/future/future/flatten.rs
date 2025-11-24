use core::pin::Pin;
use futures_core::future::{FusedFuture, Future};
use futures_core::ready;
use futures_core::stream::{FusedStream, Stream};
use futures_core::task::{Context, Poll};
#[cfg(feature = "sink")]
use futures_sink::Sink;
use pin_project_lite::pin_project;

pin_project! {
    #[project = FlattenProj]
    #[derive(Debug)]
    pub enum Flatten<Fut1, Fut2> {
        First { #[pin] f: Fut1 },
        Second { #[pin] f: Fut2 },
        Empty,
    }
}

impl<Fut1, Fut2> Flatten<Fut1, Fut2> {
    pub(crate) fn new(future: Fut1) -> Self {
        Self::First { f: future }
    }
}

impl<Fut> FusedFuture for Flatten<Fut, Fut::Output>
where
    Fut: Future,
    Fut::Output: Future,
{
    fn is_terminated(&self) -> bool {
        match self {
            Self::Empty => true,
            _ => false,
        }
    }
}

impl<Fut> Future for Flatten<Fut, Fut::Output>
where
    Fut: Future,
    Fut::Output: Future,
{
    type Output = <Fut::Output as Future>::Output;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Poll::Ready(loop {
            match self.as_mut().project() {
                FlattenProj::First { f } => {
                    let f = ready!(f.poll(cx));
                    self.set(Self::Second { f });
                }
                FlattenProj::Second { f } => {
                    let output = ready!(f.poll(cx));
                    self.set(Self::Empty);
                    break output;
                }
                FlattenProj::Empty => panic!("Flatten polled after completion"),
            }
        })
    }
}

impl<Fut> FusedStream for Flatten<Fut, Fut::Output>
where
    Fut: Future,
    Fut::Output: Stream,
{
    fn is_terminated(&self) -> bool {
        match self {
            Self::Empty => true,
            _ => false,
        }
    }
}

impl<Fut> Stream for Flatten<Fut, Fut::Output>
where
    Fut: Future,
    Fut::Output: Stream,
{
    type Item = <Fut::Output as Stream>::Item;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Poll::Ready(loop {
            match self.as_mut().project() {
                FlattenProj::First { f } => {
                    let f = ready!(f.poll(cx));
                    self.set(Self::Second { f });
                }
                FlattenProj::Second { f } => {
                    let output = ready!(f.poll_next(cx));
                    if output.is_none() {
                        self.set(Self::Empty);
                    }
                    break output;
                }
                FlattenProj::Empty => break None,
            }
        })
    }
}

#[cfg(feature = "sink")]
impl<Fut, Item> Sink<Item> for Flatten<Fut, Fut::Output>
where
    Fut: Future,
    Fut::Output: Sink<Item>,
{
    type Error = <Fut::Output as Sink<Item>>::Error;

    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(loop {
            match self.as_mut().project() {
                FlattenProj::First { f } => {
                    let f = ready!(f.poll(cx));
                    self.set(Self::Second { f });
                }
                FlattenProj::Second { f } => {
                    break ready!(f.poll_ready(cx));
                }
                FlattenProj::Empty => panic!("poll_ready called after eof"),
            }
        })
    }

    fn start_send(self: Pin<&mut Self>, item: Item) -> Result<(), Self::Error> {
        match self.project() {
            FlattenProj::First { .. } => panic!("poll_ready not called first"),
            FlattenProj::Second { f } => f.start_send(item),
            FlattenProj::Empty => panic!("start_send called after eof"),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        match self.project() {
            FlattenProj::First { .. } => Poll::Ready(Ok(())),
            FlattenProj::Second { f } => f.poll_flush(cx),
            FlattenProj::Empty => panic!("poll_flush called after eof"),
        }
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let res = match self.as_mut().project() {
            FlattenProj::Second { f } => f.poll_close(cx),
            _ => Poll::Ready(Ok(())),
        };
        if res.is_ready() {
            self.set(Self::Empty);
        }
        res
    }
}
