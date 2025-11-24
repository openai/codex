use pin_project_lite::pin_project;

use std::future::Future;
use std::pin::Pin;
use std::task::{self, Poll};

pub(crate) trait Started: Future {
    fn started(&self) -> bool;
}

pub(crate) fn lazy<F, R>(func: F) -> Lazy<F, R>
where
    F: FnOnce() -> R,
    R: Future + Unpin,
{
    Lazy {
        inner: Inner::Init { func },
    }
}

// FIXME: allow() required due to `impl Trait` leaking types to this lint
pin_project! {
    #[allow(missing_debug_implementations)]
    pub(crate) struct Lazy<F, R> {
        #[pin]
        inner: Inner<F, R>,
    }
}

pin_project! {
    #[project = InnerProj]
    #[project_replace = InnerProjReplace]
    enum Inner<F, R> {
        Init { func: F },
        Fut { #[pin] fut: R },
        Empty,
    }
}

impl<F, R> Started for Lazy<F, R>
where
    F: FnOnce() -> R,
    R: Future,
{
    fn started(&self) -> bool {
        match self.inner {
            Inner::Init { .. } => false,
            Inner::Fut { .. } | Inner::Empty => true,
        }
    }
}

impl<F, R> Future for Lazy<F, R>
where
    F: FnOnce() -> R,
    R: Future,
{
    type Output = R::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();

        if let InnerProj::Fut { fut } = this.inner.as_mut().project() {
            return fut.poll(cx);
        }

        match this.inner.as_mut().project_replace(Inner::Empty) {
            InnerProjReplace::Init { func } => {
                this.inner.set(Inner::Fut { fut: func() });
                if let InnerProj::Fut { fut } = this.inner.project() {
                    return fut.poll(cx);
                }
                unreachable!()
            }
            _ => unreachable!("lazy state wrong"),
        }
    }
}
