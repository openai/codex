use pin_project_lite::pin_project;
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

pin_project! {
    /// One of two possible futures that have the same output type.
    #[project = EitherProj]
    pub(crate) enum Either<F1, F2> {
        Left {
            #[pin]
            fut: F1
        },
        Right {
            #[pin]
            fut: F2,
        },
    }
}

impl<F1, F2> Either<F1, F2> {
    pub(crate) fn left(fut: F1) -> Self {
        Either::Left { fut }
    }

    pub(crate) fn right(fut: F2) -> Self {
        Either::Right { fut }
    }
}

impl<F1, F2> Future for Either<F1, F2>
where
    F1: Future,
    F2: Future<Output = F1::Output>,
{
    type Output = F1::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.project() {
            EitherProj::Left { fut } => fut.poll(cx),
            EitherProj::Right { fut } => fut.poll(cx),
        }
    }
}
