use crate::task::{self as task03, ArcWake as ArcWake03, WakerRef};
use futures_01::{
    task as task01, Async as Async01, Future as Future01, Poll as Poll01, Stream as Stream01,
};
#[cfg(feature = "sink")]
use futures_01::{AsyncSink as AsyncSink01, Sink as Sink01, StartSend as StartSend01};
use futures_core::{
    future::TryFuture as TryFuture03,
    stream::TryStream as TryStream03,
    task::{RawWaker, RawWakerVTable},
};
#[cfg(feature = "sink")]
use futures_sink::Sink as Sink03;
#[cfg(feature = "sink")]
use std::marker::PhantomData;
use std::{mem, pin::Pin, sync::Arc, task::Context};

#[allow(clippy::too_long_first_doc_paragraph)] // clippy bug, see https://github.com/rust-lang/rust-clippy/issues/13315
/// Converts a futures 0.3 [`TryFuture`](futures_core::future::TryFuture) or
/// [`TryStream`](futures_core::stream::TryStream) into a futures 0.1
/// [`Future`](futures_01::future::Future) or
/// [`Stream`](futures_01::stream::Stream).
#[derive(Debug, Clone, Copy)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct Compat<T> {
    pub(crate) inner: T,
}

/// Converts a futures 0.3 [`Sink`](futures_sink::Sink) into a futures 0.1
/// [`Sink`](futures_01::sink::Sink).
#[cfg(feature = "sink")]
#[cfg_attr(docsrs, doc(cfg(feature = "sink")))]
#[derive(Debug)]
#[must_use = "sinks do nothing unless polled"]
pub struct CompatSink<T, Item> {
    inner: T,
    _phantom: PhantomData<fn(Item)>,
}

impl<T> Compat<T> {
    /// Creates a new [`Compat`].
    ///
    /// For types which implement appropriate futures `0.3`
    /// traits, the result will be a type which implements
    /// the corresponding futures 0.1 type.
    pub fn new(inner: T) -> Self {
        Self { inner }
    }

    /// Get a reference to 0.3 Future, Stream, AsyncRead, or AsyncWrite object
    /// contained within.
    pub fn get_ref(&self) -> &T {
        &self.inner
    }

    /// Get a mutable reference to 0.3 Future, Stream, AsyncRead, or AsyncWrite object
    /// contained within.
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.inner
    }

    /// Returns the inner item.
    pub fn into_inner(self) -> T {
        self.inner
    }
}

#[cfg(feature = "sink")]
impl<T, Item> CompatSink<T, Item> {
    /// Creates a new [`CompatSink`].
    pub fn new(inner: T) -> Self {
        Self { inner, _phantom: PhantomData }
    }

    /// Get a reference to 0.3 Sink contained within.
    pub fn get_ref(&self) -> &T {
        &self.inner
    }

    /// Get a mutable reference to 0.3 Sink contained within.
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.inner
    }

    /// Returns the inner item.
    pub fn into_inner(self) -> T {
        self.inner
    }
}

fn poll_03_to_01<T, E>(x: task03::Poll<Result<T, E>>) -> Result<Async01<T>, E> {
    match x? {
        task03::Poll::Ready(t) => Ok(Async01::Ready(t)),
        task03::Poll::Pending => Ok(Async01::NotReady),
    }
}

impl<Fut> Future01 for Compat<Fut>
where
    Fut: TryFuture03 + Unpin,
{
    type Item = Fut::Ok;
    type Error = Fut::Error;

    fn poll(&mut self) -> Poll01<Self::Item, Self::Error> {
        with_context(self, |inner, cx| poll_03_to_01(inner.try_poll(cx)))
    }
}

impl<St> Stream01 for Compat<St>
where
    St: TryStream03 + Unpin,
{
    type Item = St::Ok;
    type Error = St::Error;

    fn poll(&mut self) -> Poll01<Option<Self::Item>, Self::Error> {
        with_context(self, |inner, cx| match inner.try_poll_next(cx)? {
            task03::Poll::Ready(None) => Ok(Async01::Ready(None)),
            task03::Poll::Ready(Some(t)) => Ok(Async01::Ready(Some(t))),
            task03::Poll::Pending => Ok(Async01::NotReady),
        })
    }
}

#[cfg(feature = "sink")]
impl<T, Item> Sink01 for CompatSink<T, Item>
where
    T: Sink03<Item> + Unpin,
{
    type SinkItem = Item;
    type SinkError = T::Error;

    fn start_send(&mut self, item: Self::SinkItem) -> StartSend01<Self::SinkItem, Self::SinkError> {
        with_sink_context(self, |mut inner, cx| match inner.as_mut().poll_ready(cx)? {
            task03::Poll::Ready(()) => inner.start_send(item).map(|()| AsyncSink01::Ready),
            task03::Poll::Pending => Ok(AsyncSink01::NotReady(item)),
        })
    }

    fn poll_complete(&mut self) -> Poll01<(), Self::SinkError> {
        with_sink_context(self, |inner, cx| poll_03_to_01(inner.poll_flush(cx)))
    }

    fn close(&mut self) -> Poll01<(), Self::SinkError> {
        with_sink_context(self, |inner, cx| poll_03_to_01(inner.poll_close(cx)))
    }
}

#[derive(Clone)]
struct Current(task01::Task);

impl Current {
    fn new() -> Self {
        Self(task01::current())
    }

    fn as_waker(&self) -> WakerRef<'_> {
        unsafe fn ptr_to_current<'a>(ptr: *const ()) -> &'a Current {
            unsafe { &*(ptr as *const Current) }
        }
        fn current_to_ptr(current: &Current) -> *const () {
            current as *const Current as *const ()
        }

        unsafe fn clone(ptr: *const ()) -> RawWaker {
            // Lazily create the `Arc` only when the waker is actually cloned.
            // FIXME: remove `transmute` when a `Waker` -> `RawWaker` conversion
            // function is landed in `core`.
            unsafe {
                mem::transmute::<task03::Waker, RawWaker>(task03::waker(Arc::new(
                    ptr_to_current(ptr).clone(),
                )))
            }
        }
        unsafe fn drop(_: *const ()) {}
        unsafe fn wake(ptr: *const ()) {
            unsafe { ptr_to_current(ptr).0.notify() }
        }

        let ptr = current_to_ptr(self);
        let vtable = &RawWakerVTable::new(clone, wake, wake, drop);
        WakerRef::new_unowned(std::mem::ManuallyDrop::new(unsafe {
            task03::Waker::from_raw(RawWaker::new(ptr, vtable))
        }))
    }
}

impl ArcWake03 for Current {
    fn wake_by_ref(arc_self: &Arc<Self>) {
        arc_self.0.notify();
    }
}

fn with_context<T, R, F>(compat: &mut Compat<T>, f: F) -> R
where
    T: Unpin,
    F: FnOnce(Pin<&mut T>, &mut Context<'_>) -> R,
{
    let current = Current::new();
    let waker = current.as_waker();
    let mut cx = Context::from_waker(&waker);
    f(Pin::new(&mut compat.inner), &mut cx)
}

#[cfg(feature = "sink")]
fn with_sink_context<T, Item, R, F>(compat: &mut CompatSink<T, Item>, f: F) -> R
where
    T: Unpin,
    F: FnOnce(Pin<&mut T>, &mut Context<'_>) -> R,
{
    let current = Current::new();
    let waker = current.as_waker();
    let mut cx = Context::from_waker(&waker);
    f(Pin::new(&mut compat.inner), &mut cx)
}

#[cfg(feature = "io-compat")]
#[cfg_attr(docsrs, doc(cfg(feature = "io-compat")))]
mod io {
    use super::*;
    use futures_io::{AsyncRead as AsyncRead03, AsyncWrite as AsyncWrite03};
    use tokio_io::{AsyncRead as AsyncRead01, AsyncWrite as AsyncWrite01};

    fn poll_03_to_io<T>(x: task03::Poll<Result<T, std::io::Error>>) -> Result<T, std::io::Error> {
        match x {
            task03::Poll::Ready(Ok(t)) => Ok(t),
            task03::Poll::Pending => Err(std::io::ErrorKind::WouldBlock.into()),
            task03::Poll::Ready(Err(e)) => Err(e),
        }
    }

    impl<R: AsyncRead03 + Unpin> std::io::Read for Compat<R> {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            let current = Current::new();
            let waker = current.as_waker();
            let mut cx = Context::from_waker(&waker);
            poll_03_to_io(Pin::new(&mut self.inner).poll_read(&mut cx, buf))
        }
    }

    impl<R: AsyncRead03 + Unpin> AsyncRead01 for Compat<R> {}

    impl<W: AsyncWrite03 + Unpin> std::io::Write for Compat<W> {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            let current = Current::new();
            let waker = current.as_waker();
            let mut cx = Context::from_waker(&waker);
            poll_03_to_io(Pin::new(&mut self.inner).poll_write(&mut cx, buf))
        }

        fn flush(&mut self) -> std::io::Result<()> {
            let current = Current::new();
            let waker = current.as_waker();
            let mut cx = Context::from_waker(&waker);
            poll_03_to_io(Pin::new(&mut self.inner).poll_flush(&mut cx))
        }
    }

    impl<W: AsyncWrite03 + Unpin> AsyncWrite01 for Compat<W> {
        fn shutdown(&mut self) -> std::io::Result<Async01<()>> {
            let current = Current::new();
            let waker = current.as_waker();
            let mut cx = Context::from_waker(&waker);
            poll_03_to_01(Pin::new(&mut self.inner).poll_close(&mut cx))
        }
    }
}
