//! Wrappers for applying timeouts to IO operations.
//!
//! This used to depend on [tokio-io-timeout]. After Hyper 1.0 introduced hyper-specific IO traits, this was rewritten to use hyper IO traits instead of tokio IO traits.
//!
//! These timeouts are analogous to the read and write timeouts on traditional blocking sockets. A timeout countdown is
//! initiated when a read/write operation returns [`Poll::Pending`]. If a read/write does not return successfully before
//! the countdown expires, an [`io::Error`] with a kind of [`TimedOut`](io::ErrorKind::TimedOut) is returned.
#![warn(missing_docs)]

use hyper::rt::{Read, ReadBuf, ReadBufCursor, Write};
use hyper_util::client::legacy::connect::{Connected, Connection};
use pin_project_lite::pin_project;
use std::future::Future;
use std::io;
use std::pin::Pin;
use std::task::{ready, Context, Poll};
use std::time::Duration;
use tokio::time::{sleep_until, Instant, Sleep};

pin_project! {
    #[derive(Debug)]
    struct TimeoutState {
        timeout: Option<Duration>,
        #[pin]
        cur: Sleep,
        active: bool,
    }
}

impl TimeoutState {
    #[inline]
    fn new() -> TimeoutState {
        TimeoutState {
            timeout: None,
            cur: sleep_until(Instant::now()),
            active: false,
        }
    }

    #[inline]
    fn timeout(&self) -> Option<Duration> {
        self.timeout
    }

    #[inline]
    fn set_timeout(&mut self, timeout: Option<Duration>) {
        // since this takes &mut self, we can't yet be active
        self.timeout = timeout;
    }

    #[inline]
    fn set_timeout_pinned(mut self: Pin<&mut Self>, timeout: Option<Duration>) {
        *self.as_mut().project().timeout = timeout;
        self.reset();
    }

    #[inline]
    fn reset(self: Pin<&mut Self>) {
        let this = self.project();

        if *this.active {
            *this.active = false;
            this.cur.reset(Instant::now());
        }
    }

    #[inline]
    fn restart(self: Pin<&mut Self>) {
        let this = self.project();

        if *this.active {
            let timeout = match this.timeout {
                Some(timeout) => *timeout,
                None => return,
            };

            this.cur.reset(Instant::now() + timeout);
        }
    }

    #[inline]
    fn poll_check(self: Pin<&mut Self>, cx: &mut Context) -> io::Result<()> {
        let mut this = self.project();

        let timeout = match this.timeout {
            Some(timeout) => *timeout,
            None => return Ok(()),
        };

        if !*this.active {
            this.cur.as_mut().reset(Instant::now() + timeout);
            *this.active = true;
        }

        match this.cur.poll(cx) {
            Poll::Ready(()) => Err(io::Error::from(io::ErrorKind::TimedOut)),
            Poll::Pending => Ok(()),
        }
    }
}

pin_project! {
    /// An `hyper::rt::Read`er which applies a timeout to read operations.
    #[derive(Debug)]
    pub struct TimeoutReader<R> {
        #[pin]
        reader: R,
        #[pin]
        state: TimeoutState,
        reset_on_write: bool,
    }
}

impl<R> TimeoutReader<R>
where
    R: Read,
{
    /// Returns a new `TimeoutReader` wrapping the specified reader.
    ///
    /// There is initially no timeout.
    pub fn new(reader: R) -> TimeoutReader<R> {
        TimeoutReader {
            reader,
            state: TimeoutState::new(),
            reset_on_write: false,
        }
    }

    /// Returns the current read timeout.
    pub fn timeout(&self) -> Option<Duration> {
        self.state.timeout()
    }

    /// Sets the read timeout.
    ///
    /// This can only be used before the reader is pinned; use [`set_timeout_pinned`](Self::set_timeout_pinned)
    /// otherwise.
    pub fn set_timeout(&mut self, timeout: Option<Duration>) {
        self.state.set_timeout(timeout);
    }

    /// Sets the read timeout.
    ///
    /// This will reset any pending timeout. Use [`set_timeout`](Self::set_timeout) instead if the reader is not yet
    /// pinned.
    pub fn set_timeout_pinned(self: Pin<&mut Self>, timeout: Option<Duration>) {
        self.project().state.set_timeout_pinned(timeout);
    }

    /// Returns a shared reference to the inner reader.
    pub fn get_ref(&self) -> &R {
        &self.reader
    }

    /// Returns a mutable reference to the inner reader.
    pub fn get_mut(&mut self) -> &mut R {
        &mut self.reader
    }

    /// Returns a pinned mutable reference to the inner reader.
    pub fn get_pin_mut(self: Pin<&mut Self>) -> Pin<&mut R> {
        self.project().reader
    }

    /// Consumes the `TimeoutReader`, returning the inner reader.
    pub fn into_inner(self) -> R {
        self.reader
    }
}

impl<R> TimeoutReader<R>
where
    R: Read + Write,
{
    /// Reset on the reader timeout on write
    ///
    /// This will reset the reader timeout when a write is done through the
    /// the TimeoutReader. This is useful when you don't want to trigger
    /// a reader timeout while writes are still be accepted.
    pub fn set_reset_on_write(&mut self, reset: bool) {
        self.reset_on_write = reset
    }
}

impl<R> Read for TimeoutReader<R>
where
    R: Read,
{
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context,
        buf: ReadBufCursor,
    ) -> Poll<Result<(), io::Error>> {
        let this = self.project();
        let r = this.reader.poll_read(cx, buf);
        match r {
            Poll::Pending => this.state.poll_check(cx)?,
            _ => this.state.reset(),
        }
        r
    }
}

impl<R> Write for TimeoutReader<R>
where
    R: Write,
{
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        let this = self.project();
        let r = this.reader.poll_write(cx, buf);
        if *this.reset_on_write && r.is_ready() {
            this.state.restart();
        }
        r
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<(), io::Error>> {
        let this = self.project();
        let r = this.reader.poll_flush(cx);
        if *this.reset_on_write && r.is_ready() {
            this.state.restart();
        }
        r
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<(), io::Error>> {
        let this = self.project();
        let r = this.reader.poll_shutdown(cx);
        if *this.reset_on_write && r.is_ready() {
            this.state.restart();
        }
        r
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context,
        bufs: &[io::IoSlice],
    ) -> Poll<io::Result<usize>> {
        let this = self.project();
        let r = this.reader.poll_write_vectored(cx, bufs);
        if *this.reset_on_write && r.is_ready() {
            this.state.restart();
        }
        r
    }

    fn is_write_vectored(&self) -> bool {
        self.reader.is_write_vectored()
    }
}

pin_project! {
    /// An `hyper::rt::Write`er which applies a timeout to write operations.
    #[derive(Debug)]
    pub struct TimeoutWriter<W> {
        #[pin]
        writer: W,
        #[pin]
        state: TimeoutState,
    }
}

impl<W> TimeoutWriter<W>
where
    W: Write,
{
    /// Returns a new `TimeoutReader` wrapping the specified reader.
    ///
    /// There is initially no timeout.
    pub fn new(writer: W) -> TimeoutWriter<W> {
        TimeoutWriter {
            writer,
            state: TimeoutState::new(),
        }
    }

    /// Returns the current write timeout.
    pub fn timeout(&self) -> Option<Duration> {
        self.state.timeout()
    }

    /// Sets the write timeout.
    ///
    /// This can only be used before the writer is pinned; use [`set_timeout_pinned`](Self::set_timeout_pinned)
    /// otherwise.
    pub fn set_timeout(&mut self, timeout: Option<Duration>) {
        self.state.set_timeout(timeout);
    }

    /// Sets the write timeout.
    ///
    /// This will reset any pending timeout. Use [`set_timeout`](Self::set_timeout) instead if the reader is not yet
    /// pinned.
    pub fn set_timeout_pinned(self: Pin<&mut Self>, timeout: Option<Duration>) {
        self.project().state.set_timeout_pinned(timeout);
    }

    /// Returns a shared reference to the inner writer.
    pub fn get_ref(&self) -> &W {
        &self.writer
    }

    /// Returns a mutable reference to the inner writer.
    pub fn get_mut(&mut self) -> &mut W {
        &mut self.writer
    }

    /// Returns a pinned mutable reference to the inner writer.
    pub fn get_pin_mut(self: Pin<&mut Self>) -> Pin<&mut W> {
        self.project().writer
    }

    /// Consumes the `TimeoutWriter`, returning the inner writer.
    pub fn into_inner(self) -> W {
        self.writer
    }
}

impl<W> Write for TimeoutWriter<W>
where
    W: Write,
{
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        let this = self.project();
        let r = this.writer.poll_write(cx, buf);
        match r {
            Poll::Pending => this.state.poll_check(cx)?,
            _ => this.state.reset(),
        }
        r
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<(), io::Error>> {
        let this = self.project();
        let r = this.writer.poll_flush(cx);
        match r {
            Poll::Pending => this.state.poll_check(cx)?,
            _ => this.state.reset(),
        }
        r
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<(), io::Error>> {
        let this = self.project();
        let r = this.writer.poll_shutdown(cx);
        match r {
            Poll::Pending => this.state.poll_check(cx)?,
            _ => this.state.reset(),
        }
        r
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context,
        bufs: &[io::IoSlice],
    ) -> Poll<io::Result<usize>> {
        let this = self.project();
        let r = this.writer.poll_write_vectored(cx, bufs);
        match r {
            Poll::Pending => this.state.poll_check(cx)?,
            _ => this.state.reset(),
        }
        r
    }

    fn is_write_vectored(&self) -> bool {
        self.writer.is_write_vectored()
    }
}

impl<W> Read for TimeoutWriter<W>
where
    W: Read,
{
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context,
        buf: ReadBufCursor,
    ) -> Poll<Result<(), io::Error>> {
        self.project().writer.poll_read(cx, buf)
    }
}

pin_project! {
    /// A stream which applies read and write timeouts to an inner stream.
    #[derive(Debug)]
    pub struct TimeoutStream<S> {
        #[pin]
        stream: TimeoutReader<TimeoutWriter<S>>
    }
}

impl<S> TimeoutStream<S>
where
    S: Read + Write,
{
    /// Returns a new `TimeoutStream` wrapping the specified stream.
    ///
    /// There is initially no read or write timeout.
    pub fn new(stream: S) -> TimeoutStream<S> {
        let writer = TimeoutWriter::new(stream);
        let stream = TimeoutReader::new(writer);
        TimeoutStream { stream }
    }

    /// Returns the current read timeout.
    pub fn read_timeout(&self) -> Option<Duration> {
        self.stream.timeout()
    }

    /// Sets the read timeout.
    ///
    /// This can only be used before the stream is pinned; use
    /// [`set_read_timeout_pinned`](Self::set_read_timeout_pinned) otherwise.
    pub fn set_read_timeout(&mut self, timeout: Option<Duration>) {
        self.stream.set_timeout(timeout)
    }

    /// Sets the read timeout.
    ///
    /// This will reset any pending read timeout. Use [`set_read_timeout`](Self::set_read_timeout) instead if the stream
    /// has not yet been pinned.
    pub fn set_read_timeout_pinned(self: Pin<&mut Self>, timeout: Option<Duration>) {
        self.project().stream.set_timeout_pinned(timeout)
    }

    /// Returns the current write timeout.
    pub fn write_timeout(&self) -> Option<Duration> {
        self.stream.get_ref().timeout()
    }

    /// Sets the write timeout.
    ///
    /// This can only be used before the stream is pinned; use
    /// [`set_write_timeout_pinned`](Self::set_write_timeout_pinned) otherwise.
    pub fn set_write_timeout(&mut self, timeout: Option<Duration>) {
        self.stream.get_mut().set_timeout(timeout)
    }

    /// Sets the write timeout.
    ///
    /// This will reset any pending write timeout. Use [`set_write_timeout`](Self::set_write_timeout) instead if the
    /// stream has not yet been pinned.
    pub fn set_write_timeout_pinned(self: Pin<&mut Self>, timeout: Option<Duration>) {
        self.project()
            .stream
            .get_pin_mut()
            .set_timeout_pinned(timeout)
    }

    /// Reset on the reader timeout on write
    ///
    /// This will reset the reader timeout when a write is done through the
    /// the TimeoutReader. This is useful when you don't want to trigger
    /// a reader timeout while writes are still be accepted.
    pub fn set_reset_reader_on_write(&mut self, reset: bool) {
        self.stream.set_reset_on_write(reset);
    }

    /// Returns a shared reference to the inner stream.
    pub fn get_ref(&self) -> &S {
        self.stream.get_ref().get_ref()
    }

    /// Returns a mutable reference to the inner stream.
    pub fn get_mut(&mut self) -> &mut S {
        self.stream.get_mut().get_mut()
    }

    /// Returns a pinned mutable reference to the inner stream.
    pub fn get_pin_mut(self: Pin<&mut Self>) -> Pin<&mut S> {
        self.project().stream.get_pin_mut().get_pin_mut()
    }

    /// Consumes the stream, returning the inner stream.
    pub fn into_inner(self) -> S {
        self.stream.into_inner().into_inner()
    }
}

impl<S> Read for TimeoutStream<S>
where
    S: Read + Write,
{
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context,
        buf: ReadBufCursor,
    ) -> Poll<Result<(), io::Error>> {
        self.project().stream.poll_read(cx, buf)
    }
}

impl<S> Write for TimeoutStream<S>
where
    S: Read + Write,
{
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        self.project().stream.poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<(), io::Error>> {
        self.project().stream.poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<(), io::Error>> {
        self.project().stream.poll_shutdown(cx)
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context,
        bufs: &[io::IoSlice],
    ) -> Poll<io::Result<usize>> {
        self.project().stream.poll_write_vectored(cx, bufs)
    }

    fn is_write_vectored(&self) -> bool {
        self.stream.is_write_vectored()
    }
}

impl<S> Connection for TimeoutStream<S>
where
    S: Read + Write + Connection + Unpin,
{
    fn connected(&self) -> Connected {
        self.get_ref().connected()
    }
}

impl<S> Connection for Pin<Box<TimeoutStream<S>>>
where
    S: Read + Write + Connection + Unpin,
{
    fn connected(&self) -> Connected {
        self.get_ref().connected()
    }
}

pin_project! {
    /// A future which can be used to easily read available number of bytes to fill
    /// a buffer. Based on the internal [tokio::io::util::read::Read]
    struct ReadFut<'a, R: ?Sized> {
        reader: &'a mut R,
        buf: &'a mut [u8],
    }
}

/// Tries to read some bytes directly into the given `buf` in asynchronous
/// manner, returning a future type.
///
/// The returned future will resolve to both the I/O stream and the buffer
/// as well as the number of bytes read once the read operation is completed.
#[cfg(test)]
fn read<'a, R>(reader: &'a mut R, buf: &'a mut [u8]) -> ReadFut<'a, R>
where
    R: Read + Unpin + ?Sized,
{
    ReadFut { reader, buf }
}

impl<R> Future for ReadFut<'_, R>
where
    R: Read + Unpin + ?Sized,
{
    type Output = io::Result<usize>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<usize>> {
        let me = self.project();
        let mut buf = ReadBuf::new(me.buf);
        ready!(Pin::new(me.reader).poll_read(cx, buf.unfilled()))?;
        Poll::Ready(Ok(buf.filled().len()))
    }
}

#[cfg(test)]
trait ReadExt: Read {
    /// Pulls some bytes from this source into the specified buffer,
    /// returning how many bytes were read.
    fn read<'a>(&'a mut self, buf: &'a mut [u8]) -> ReadFut<'a, Self>
    where
        Self: Unpin,
    {
        read(self, buf)
    }
}

pin_project! {
    /// A future to write some of the buffer to an `AsyncWrite`.-
    struct WriteFut<'a, W: ?Sized> {
        writer: &'a mut W,
        buf: &'a [u8],
    }
}

/// Tries to write some bytes from the given `buf` to the writer in an
/// asynchronous manner, returning a future.
#[cfg(test)]
fn write<'a, W>(writer: &'a mut W, buf: &'a [u8]) -> WriteFut<'a, W>
where
    W: Write + Unpin + ?Sized,
{
    WriteFut { writer, buf }
}

impl<W> Future for WriteFut<'_, W>
where
    W: Write + Unpin + ?Sized,
{
    type Output = io::Result<usize>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<usize>> {
        let me = self.project();
        Pin::new(&mut *me.writer).poll_write(cx, me.buf)
    }
}

#[cfg(test)]
trait WriteExt: Write {
    /// Writes a buffer into this writer, returning how many bytes were
    /// written.
    fn write<'a>(&'a mut self, src: &'a [u8]) -> WriteFut<'a, Self>
    where
        Self: Unpin,
    {
        write(self, src)
    }
}

#[cfg(test)]
impl<R> ReadExt for Pin<&mut TimeoutReader<R>>
where
    R: Read,
{
    fn read<'a>(&'a mut self, buf: &'a mut [u8]) -> ReadFut<'a, Self> {
        read(self, buf)
    }
}

#[cfg(test)]
impl<W> WriteExt for Pin<&mut TimeoutWriter<W>>
where
    W: Write,
{
    fn write<'a>(&'a mut self, src: &'a [u8]) -> WriteFut<'a, Self> {
        write(self, src)
    }
}

#[cfg(test)]
impl<S> ReadExt for Pin<&mut TimeoutStream<S>>
where
    S: Read + Write,
{
    fn read<'a>(&'a mut self, buf: &'a mut [u8]) -> ReadFut<'a, Self> {
        read(self, buf)
    }
}

#[cfg(test)]
impl<S> WriteExt for Pin<&mut TimeoutStream<S>>
where
    S: Read + Write,
{
    fn write<'a>(&'a mut self, src: &'a [u8]) -> WriteFut<'a, Self> {
        write(self, src)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use hyper_util::rt::TokioIo;
    use std::io::Write;
    use std::net::TcpListener;
    use std::thread;
    use tokio::net::TcpStream;
    use tokio::pin;

    pin_project! {
        struct DelayStream {
            #[pin]
            sleep: Sleep,
        }
    }

    impl DelayStream {
        fn new(until: Instant) -> Self {
            DelayStream {
                sleep: sleep_until(until),
            }
        }
    }

    impl Read for DelayStream {
        fn poll_read(
            self: Pin<&mut Self>,
            cx: &mut Context,
            _buf: ReadBufCursor,
        ) -> Poll<Result<(), io::Error>> {
            match self.project().sleep.poll(cx) {
                Poll::Ready(()) => Poll::Ready(Ok(())),
                Poll::Pending => Poll::Pending,
            }
        }
    }

    impl hyper::rt::Write for DelayStream {
        fn poll_write(
            self: Pin<&mut Self>,
            cx: &mut Context,
            buf: &[u8],
        ) -> Poll<Result<usize, io::Error>> {
            match self.project().sleep.poll(cx) {
                Poll::Ready(()) => Poll::Ready(Ok(buf.len())),
                Poll::Pending => Poll::Pending,
            }
        }

        fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context) -> Poll<Result<(), io::Error>> {
            Poll::Ready(Ok(()))
        }

        fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context) -> Poll<Result<(), io::Error>> {
            Poll::Ready(Ok(()))
        }
    }

    #[tokio::test]
    async fn read_timeout() {
        let reader = DelayStream::new(Instant::now() + Duration::from_millis(500));
        let mut reader = TimeoutReader::new(reader);
        reader.set_timeout(Some(Duration::from_millis(100)));
        pin!(reader);

        let r = reader.read(&mut [0, 1, 2]).await;
        assert_eq!(r.err().unwrap().kind(), io::ErrorKind::TimedOut);
    }

    #[tokio::test]
    async fn read_ok() {
        let reader = DelayStream::new(Instant::now() + Duration::from_millis(100));
        let mut reader = TimeoutReader::new(reader);
        reader.set_timeout(Some(Duration::from_millis(500)));
        pin!(reader);

        reader.read(&mut [0]).await.unwrap();
    }

    #[tokio::test]
    async fn write_timeout() {
        let writer = DelayStream::new(Instant::now() + Duration::from_millis(500));
        let mut writer = TimeoutWriter::new(writer);
        writer.set_timeout(Some(Duration::from_millis(100)));
        pin!(writer);

        let r = writer.write(&[0]).await;
        assert_eq!(r.err().unwrap().kind(), io::ErrorKind::TimedOut);
    }

    #[tokio::test]
    async fn write_ok() {
        let writer = DelayStream::new(Instant::now() + Duration::from_millis(100));
        let mut writer = TimeoutWriter::new(writer);
        writer.set_timeout(Some(Duration::from_millis(500)));
        pin!(writer);

        writer.write(&[0]).await.unwrap();
    }

    #[tokio::test]
    async fn tcp_read() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        thread::spawn(move || {
            let mut socket = listener.accept().unwrap().0;
            thread::sleep(Duration::from_millis(10));
            socket.write_all(b"f").unwrap();
            thread::sleep(Duration::from_millis(500));
            let _ = socket.write_all(b"f"); // this may hit an eof
        });

        let s = TcpStream::connect(&addr).await.unwrap();
        let s = TokioIo::new(s);
        let mut s = TimeoutStream::new(s);
        s.set_read_timeout(Some(Duration::from_millis(100)));
        pin!(s);
        s.read(&mut [0]).await.unwrap();
        let r = s.read(&mut [0]).await;

        match r {
            Ok(_) => panic!("unexpected success"),
            Err(ref e) if e.kind() == io::ErrorKind::TimedOut => (),
            Err(e) => panic!("{:?}", e),
        }
    }
}
