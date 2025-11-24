use futures_01::executor::{
    spawn as spawn01, Notify as Notify01, NotifyHandle as NotifyHandle01, Spawn as Spawn01,
    UnsafeNotify as UnsafeNotify01,
};
use futures_01::{Async as Async01, Future as Future01, Stream as Stream01};
#[cfg(feature = "sink")]
use futures_01::{AsyncSink as AsyncSink01, Sink as Sink01};
use futures_core::{future::Future as Future03, stream::Stream as Stream03, task as task03};
#[cfg(feature = "sink")]
use futures_sink::Sink as Sink03;
use std::boxed::Box;
use std::pin::Pin;
use std::task::Context;

#[cfg(feature = "io-compat")]
#[cfg_attr(docsrs, doc(cfg(feature = "io-compat")))]
#[allow(unreachable_pub)] // https://github.com/rust-lang/rust/issues/57411
pub use io::{AsyncRead01CompatExt, AsyncWrite01CompatExt};

/// Converts a futures 0.1 Future, Stream, AsyncRead, or AsyncWrite
/// object to a futures 0.3-compatible version,
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct Compat01As03<T> {
    pub(crate) inner: Spawn01<T>,
}

impl<T> Unpin for Compat01As03<T> {}

impl<T> Compat01As03<T> {
    /// Wraps a futures 0.1 Future, Stream, AsyncRead, or AsyncWrite
    /// object in a futures 0.3-compatible wrapper.
    pub fn new(object: T) -> Self {
        Self { inner: spawn01(object) }
    }

    fn in_notify<R>(&mut self, cx: &mut Context<'_>, f: impl FnOnce(&mut T) -> R) -> R {
        let notify = &WakerToHandle(cx.waker());
        self.inner.poll_fn_notify(notify, 0, f)
    }

    /// Get a reference to 0.1 Future, Stream, AsyncRead, or AsyncWrite object contained within.
    pub fn get_ref(&self) -> &T {
        self.inner.get_ref()
    }

    /// Get a mutable reference to 0.1 Future, Stream, AsyncRead or AsyncWrite object contained
    /// within.
    pub fn get_mut(&mut self) -> &mut T {
        self.inner.get_mut()
    }

    /// Consume this wrapper to return the underlying 0.1 Future, Stream, AsyncRead, or
    /// AsyncWrite object.
    pub fn into_inner(self) -> T {
        self.inner.into_inner()
    }
}

/// Extension trait for futures 0.1 [`Future`](futures_01::future::Future)
pub trait Future01CompatExt: Future01 {
    /// Converts a futures 0.1
    /// [`Future<Item = T, Error = E>`](futures_01::future::Future)
    /// into a futures 0.3
    /// [`Future<Output = Result<T, E>>`](futures_core::future::Future).
    ///
    /// ```
    /// # if cfg!(miri) { return; } // https://github.com/rust-lang/futures-rs/issues/2514
    /// # futures::executor::block_on(async {
    /// # // TODO: These should be all using `futures::compat`, but that runs up against Cargo
    /// # // feature issues
    /// use futures_util::compat::Future01CompatExt;
    ///
    /// let future = futures_01::future::ok::<u32, ()>(1);
    /// assert_eq!(future.compat().await, Ok(1));
    /// # });
    /// ```
    fn compat(self) -> Compat01As03<Self>
    where
        Self: Sized,
    {
        Compat01As03::new(self)
    }
}
impl<Fut: Future01> Future01CompatExt for Fut {}

/// Extension trait for futures 0.1 [`Stream`](futures_01::stream::Stream)
pub trait Stream01CompatExt: Stream01 {
    /// Converts a futures 0.1
    /// [`Stream<Item = T, Error = E>`](futures_01::stream::Stream)
    /// into a futures 0.3
    /// [`Stream<Item = Result<T, E>>`](futures_core::stream::Stream).
    ///
    /// ```
    /// # if cfg!(miri) { return; } // https://github.com/rust-lang/futures-rs/issues/2514
    /// # futures::executor::block_on(async {
    /// use futures::stream::StreamExt;
    /// use futures_util::compat::Stream01CompatExt;
    ///
    /// let stream = futures_01::stream::once::<u32, ()>(Ok(1));
    /// let mut stream = stream.compat();
    /// assert_eq!(stream.next().await, Some(Ok(1)));
    /// assert_eq!(stream.next().await, None);
    /// # });
    /// ```
    fn compat(self) -> Compat01As03<Self>
    where
        Self: Sized,
    {
        Compat01As03::new(self)
    }
}
impl<St: Stream01> Stream01CompatExt for St {}

/// Extension trait for futures 0.1 [`Sink`](futures_01::sink::Sink)
#[cfg(feature = "sink")]
#[cfg_attr(docsrs, doc(cfg(feature = "sink")))]
pub trait Sink01CompatExt: Sink01 {
    /// Converts a futures 0.1
    /// [`Sink<SinkItem = T, SinkError = E>`](futures_01::sink::Sink)
    /// into a futures 0.3
    /// [`Sink<T, Error = E>`](futures_sink::Sink).
    ///
    /// ```
    /// # if cfg!(miri) { return; } // https://github.com/rust-lang/futures-rs/issues/2514
    /// # futures::executor::block_on(async {
    /// use futures::{sink::SinkExt, stream::StreamExt};
    /// use futures_util::compat::{Stream01CompatExt, Sink01CompatExt};
    ///
    /// let (tx, rx) = futures_01::unsync::mpsc::channel(1);
    /// let (mut tx, mut rx) = (tx.sink_compat(), rx.compat());
    ///
    /// tx.send(1).await.unwrap();
    /// drop(tx);
    /// assert_eq!(rx.next().await, Some(Ok(1)));
    /// assert_eq!(rx.next().await, None);
    /// # });
    /// ```
    fn sink_compat(self) -> Compat01As03Sink<Self, Self::SinkItem>
    where
        Self: Sized,
    {
        Compat01As03Sink::new(self)
    }
}
#[cfg(feature = "sink")]
impl<Si: Sink01> Sink01CompatExt for Si {}

fn poll_01_to_03<T, E>(x: Result<Async01<T>, E>) -> task03::Poll<Result<T, E>> {
    match x? {
        Async01::Ready(t) => task03::Poll::Ready(Ok(t)),
        Async01::NotReady => task03::Poll::Pending,
    }
}

impl<Fut: Future01> Future03 for Compat01As03<Fut> {
    type Output = Result<Fut::Item, Fut::Error>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> task03::Poll<Self::Output> {
        poll_01_to_03(self.in_notify(cx, Future01::poll))
    }
}

impl<St: Stream01> Stream03 for Compat01As03<St> {
    type Item = Result<St::Item, St::Error>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> task03::Poll<Option<Self::Item>> {
        match self.in_notify(cx, Stream01::poll)? {
            Async01::Ready(Some(t)) => task03::Poll::Ready(Some(Ok(t))),
            Async01::Ready(None) => task03::Poll::Ready(None),
            Async01::NotReady => task03::Poll::Pending,
        }
    }
}

/// Converts a futures 0.1 Sink object to a futures 0.3-compatible version
#[cfg(feature = "sink")]
#[cfg_attr(docsrs, doc(cfg(feature = "sink")))]
#[derive(Debug)]
#[must_use = "sinks do nothing unless polled"]
pub struct Compat01As03Sink<S, SinkItem> {
    pub(crate) inner: Spawn01<S>,
    pub(crate) buffer: Option<SinkItem>,
    pub(crate) close_started: bool,
}

#[cfg(feature = "sink")]
impl<S, SinkItem> Unpin for Compat01As03Sink<S, SinkItem> {}

#[cfg(feature = "sink")]
impl<S, SinkItem> Compat01As03Sink<S, SinkItem> {
    /// Wraps a futures 0.1 Sink object in a futures 0.3-compatible wrapper.
    pub fn new(inner: S) -> Self {
        Self { inner: spawn01(inner), buffer: None, close_started: false }
    }

    fn in_notify<R>(&mut self, cx: &mut Context<'_>, f: impl FnOnce(&mut S) -> R) -> R {
        let notify = &WakerToHandle(cx.waker());
        self.inner.poll_fn_notify(notify, 0, f)
    }

    /// Get a reference to 0.1 Sink object contained within.
    pub fn get_ref(&self) -> &S {
        self.inner.get_ref()
    }

    /// Get a mutable reference to 0.1 Sink contained within.
    pub fn get_mut(&mut self) -> &mut S {
        self.inner.get_mut()
    }

    /// Consume this wrapper to return the underlying 0.1 Sink.
    pub fn into_inner(self) -> S {
        self.inner.into_inner()
    }
}

#[cfg(feature = "sink")]
impl<S, SinkItem> Stream03 for Compat01As03Sink<S, SinkItem>
where
    S: Stream01,
{
    type Item = Result<S::Item, S::Error>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> task03::Poll<Option<Self::Item>> {
        match self.in_notify(cx, Stream01::poll)? {
            Async01::Ready(Some(t)) => task03::Poll::Ready(Some(Ok(t))),
            Async01::Ready(None) => task03::Poll::Ready(None),
            Async01::NotReady => task03::Poll::Pending,
        }
    }
}

#[cfg(feature = "sink")]
impl<S, SinkItem> Sink03<SinkItem> for Compat01As03Sink<S, SinkItem>
where
    S: Sink01<SinkItem = SinkItem>,
{
    type Error = S::SinkError;

    fn start_send(mut self: Pin<&mut Self>, item: SinkItem) -> Result<(), Self::Error> {
        debug_assert!(self.buffer.is_none());
        self.buffer = Some(item);
        Ok(())
    }

    fn poll_ready(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> task03::Poll<Result<(), Self::Error>> {
        match self.buffer.take() {
            Some(item) => match self.in_notify(cx, |f| f.start_send(item))? {
                AsyncSink01::Ready => task03::Poll::Ready(Ok(())),
                AsyncSink01::NotReady(i) => {
                    self.buffer = Some(i);
                    task03::Poll::Pending
                }
            },
            None => task03::Poll::Ready(Ok(())),
        }
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> task03::Poll<Result<(), Self::Error>> {
        let item = self.buffer.take();
        match self.in_notify(cx, |f| match item {
            Some(i) => match f.start_send(i)? {
                AsyncSink01::Ready => f.poll_complete().map(|i| (i, None)),
                AsyncSink01::NotReady(t) => Ok((Async01::NotReady, Some(t))),
            },
            None => f.poll_complete().map(|i| (i, None)),
        })? {
            (Async01::Ready(_), _) => task03::Poll::Ready(Ok(())),
            (Async01::NotReady, item) => {
                self.buffer = item;
                task03::Poll::Pending
            }
        }
    }

    fn poll_close(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> task03::Poll<Result<(), Self::Error>> {
        let item = self.buffer.take();
        let close_started = self.close_started;

        let result = self.in_notify(cx, |f| {
            if !close_started {
                if let Some(item) = item {
                    if let AsyncSink01::NotReady(item) = f.start_send(item)? {
                        return Ok((Async01::NotReady, Some(item), false));
                    }
                }

                if let Async01::NotReady = f.poll_complete()? {
                    return Ok((Async01::NotReady, None, false));
                }
            }

            Ok((<S as Sink01>::close(f)?, None, true))
        });

        match result? {
            (Async01::Ready(_), _, _) => task03::Poll::Ready(Ok(())),
            (Async01::NotReady, item, close_started) => {
                self.buffer = item;
                self.close_started = close_started;
                task03::Poll::Pending
            }
        }
    }
}

struct NotifyWaker(task03::Waker);

#[allow(missing_debug_implementations)] // false positive: this is private type
#[derive(Clone)]
struct WakerToHandle<'a>(&'a task03::Waker);

impl From<WakerToHandle<'_>> for NotifyHandle01 {
    fn from(handle: WakerToHandle<'_>) -> Self {
        let ptr = Box::new(NotifyWaker(handle.0.clone()));

        unsafe { Self::new(Box::into_raw(ptr)) }
    }
}

impl Notify01 for NotifyWaker {
    fn notify(&self, _: usize) {
        self.0.wake_by_ref();
    }
}

unsafe impl UnsafeNotify01 for NotifyWaker {
    unsafe fn clone_raw(&self) -> NotifyHandle01 {
        WakerToHandle(&self.0).into()
    }

    unsafe fn drop_raw(&self) {
        let ptr: *const dyn UnsafeNotify01 = self;
        drop(unsafe { Box::from_raw(ptr as *mut dyn UnsafeNotify01) });
    }
}

#[cfg(feature = "io-compat")]
#[cfg_attr(docsrs, doc(cfg(feature = "io-compat")))]
mod io {
    use super::*;
    use futures_io::{AsyncRead as AsyncRead03, AsyncWrite as AsyncWrite03};
    use std::io::Error;
    use tokio_io::{AsyncRead as AsyncRead01, AsyncWrite as AsyncWrite01};

    /// Extension trait for tokio-io [`AsyncRead`](tokio_io::AsyncRead)
    #[cfg_attr(docsrs, doc(cfg(feature = "io-compat")))]
    pub trait AsyncRead01CompatExt: AsyncRead01 {
        /// Converts a tokio-io [`AsyncRead`](tokio_io::AsyncRead) into a futures-io 0.3
        /// [`AsyncRead`](futures_io::AsyncRead).
        ///
        /// ```
        /// # if cfg!(miri) { return; } // https://github.com/rust-lang/futures-rs/issues/2514
        /// # futures::executor::block_on(async {
        /// use futures::io::AsyncReadExt;
        /// use futures_util::compat::AsyncRead01CompatExt;
        ///
        /// let input = b"Hello World!";
        /// let reader /* : impl tokio_io::AsyncRead */ = std::io::Cursor::new(input);
        /// let mut reader /* : impl futures::io::AsyncRead + Unpin */ = reader.compat();
        ///
        /// let mut output = Vec::with_capacity(12);
        /// reader.read_to_end(&mut output).await.unwrap();
        /// assert_eq!(output, input);
        /// # });
        /// ```
        fn compat(self) -> Compat01As03<Self>
        where
            Self: Sized,
        {
            Compat01As03::new(self)
        }
    }
    impl<R: AsyncRead01> AsyncRead01CompatExt for R {}

    /// Extension trait for tokio-io [`AsyncWrite`](tokio_io::AsyncWrite)
    #[cfg_attr(docsrs, doc(cfg(feature = "io-compat")))]
    pub trait AsyncWrite01CompatExt: AsyncWrite01 {
        /// Converts a tokio-io [`AsyncWrite`](tokio_io::AsyncWrite) into a futures-io 0.3
        /// [`AsyncWrite`](futures_io::AsyncWrite).
        ///
        /// ```
        /// # if cfg!(miri) { return; } // https://github.com/rust-lang/futures-rs/issues/2514
        /// # futures::executor::block_on(async {
        /// use futures::io::AsyncWriteExt;
        /// use futures_util::compat::AsyncWrite01CompatExt;
        ///
        /// let input = b"Hello World!";
        /// let mut cursor = std::io::Cursor::new(Vec::with_capacity(12));
        ///
        /// let mut writer = (&mut cursor).compat();
        /// writer.write_all(input).await.unwrap();
        ///
        /// assert_eq!(cursor.into_inner(), input);
        /// # });
        /// ```
        fn compat(self) -> Compat01As03<Self>
        where
            Self: Sized,
        {
            Compat01As03::new(self)
        }
    }
    impl<W: AsyncWrite01> AsyncWrite01CompatExt for W {}

    impl<R: AsyncRead01> AsyncRead03 for Compat01As03<R> {
        fn poll_read(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &mut [u8],
        ) -> task03::Poll<Result<usize, Error>> {
            poll_01_to_03(self.in_notify(cx, |x| x.poll_read(buf)))
        }
    }

    impl<W: AsyncWrite01> AsyncWrite03 for Compat01As03<W> {
        fn poll_write(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &[u8],
        ) -> task03::Poll<Result<usize, Error>> {
            poll_01_to_03(self.in_notify(cx, |x| x.poll_write(buf)))
        }

        fn poll_flush(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
        ) -> task03::Poll<Result<(), Error>> {
            poll_01_to_03(self.in_notify(cx, AsyncWrite01::poll_flush))
        }

        fn poll_close(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
        ) -> task03::Poll<Result<(), Error>> {
            poll_01_to_03(self.in_notify(cx, AsyncWrite01::shutdown))
        }
    }
}
