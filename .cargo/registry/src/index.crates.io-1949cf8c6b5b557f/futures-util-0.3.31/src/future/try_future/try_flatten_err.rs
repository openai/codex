use core::pin::Pin;
use futures_core::future::{FusedFuture, Future, TryFuture};
use futures_core::ready;
use futures_core::task::{Context, Poll};
use pin_project_lite::pin_project;

pin_project! {
    #[project = TryFlattenErrProj]
    #[derive(Debug)]
    pub enum TryFlattenErr<Fut1, Fut2> {
        First { #[pin] f: Fut1 },
        Second { #[pin] f: Fut2 },
        Empty,
    }
}

impl<Fut1, Fut2> TryFlattenErr<Fut1, Fut2> {
    pub(crate) fn new(future: Fut1) -> Self {
        Self::First { f: future }
    }
}

impl<Fut> FusedFuture for TryFlattenErr<Fut, Fut::Error>
where
    Fut: TryFuture,
    Fut::Error: TryFuture<Ok = Fut::Ok>,
{
    fn is_terminated(&self) -> bool {
        match self {
            Self::Empty => true,
            _ => false,
        }
    }
}

impl<Fut> Future for TryFlattenErr<Fut, Fut::Error>
where
    Fut: TryFuture,
    Fut::Error: TryFuture<Ok = Fut::Ok>,
{
    type Output = Result<Fut::Ok, <Fut::Error as TryFuture>::Error>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Poll::Ready(loop {
            match self.as_mut().project() {
                TryFlattenErrProj::First { f } => match ready!(f.try_poll(cx)) {
                    Err(f) => self.set(Self::Second { f }),
                    Ok(e) => {
                        self.set(Self::Empty);
                        break Ok(e);
                    }
                },
                TryFlattenErrProj::Second { f } => {
                    let output = ready!(f.try_poll(cx));
                    self.set(Self::Empty);
                    break output;
                }
                TryFlattenErrProj::Empty => panic!("TryFlattenErr polled after completion"),
            }
        })
    }
}
