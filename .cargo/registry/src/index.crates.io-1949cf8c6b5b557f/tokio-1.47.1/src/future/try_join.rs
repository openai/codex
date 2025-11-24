use crate::future::maybe_done::{maybe_done, MaybeDone};

use pin_project_lite::pin_project;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

pub(crate) fn try_join3<T1, F1, T2, F2, T3, F3, E>(
    future1: F1,
    future2: F2,
    future3: F3,
) -> TryJoin3<F1, F2, F3>
where
    F1: Future<Output = Result<T1, E>>,
    F2: Future<Output = Result<T2, E>>,
    F3: Future<Output = Result<T3, E>>,
{
    TryJoin3 {
        future1: maybe_done(future1),
        future2: maybe_done(future2),
        future3: maybe_done(future3),
    }
}

pin_project! {
    pub(crate) struct TryJoin3<F1, F2, F3>
    where
        F1: Future,
        F2: Future,
        F3: Future,
    {
        #[pin]
        future1: MaybeDone<F1>,
        #[pin]
        future2: MaybeDone<F2>,
        #[pin]
        future3: MaybeDone<F3>,
    }
}

impl<T1, F1, T2, F2, T3, F3, E> Future for TryJoin3<F1, F2, F3>
where
    F1: Future<Output = Result<T1, E>>,
    F2: Future<Output = Result<T2, E>>,
    F3: Future<Output = Result<T3, E>>,
{
    type Output = Result<(T1, T2, T3), E>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut all_done = true;

        let mut me = self.project();

        if me.future1.as_mut().poll(cx).is_pending() {
            all_done = false;
        } else if me.future1.as_mut().output_mut().unwrap().is_err() {
            return Poll::Ready(Err(me.future1.take_output().unwrap().err().unwrap()));
        }

        if me.future2.as_mut().poll(cx).is_pending() {
            all_done = false;
        } else if me.future2.as_mut().output_mut().unwrap().is_err() {
            return Poll::Ready(Err(me.future2.take_output().unwrap().err().unwrap()));
        }

        if me.future3.as_mut().poll(cx).is_pending() {
            all_done = false;
        } else if me.future3.as_mut().output_mut().unwrap().is_err() {
            return Poll::Ready(Err(me.future3.take_output().unwrap().err().unwrap()));
        }

        if all_done {
            Poll::Ready(Ok((
                me.future1.take_output().unwrap().ok().unwrap(),
                me.future2.take_output().unwrap().ok().unwrap(),
                me.future3.take_output().unwrap().ok().unwrap(),
            )))
        } else {
            Poll::Pending
        }
    }
}
