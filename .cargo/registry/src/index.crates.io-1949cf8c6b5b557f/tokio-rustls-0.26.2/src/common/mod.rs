use std::io::{self, BufRead as _, IoSlice, Read, Write};
use std::ops::{Deref, DerefMut};
use std::pin::Pin;
use std::task::{Context, Poll};

use rustls::{ConnectionCommon, SideData};
use tokio::io::{AsyncBufRead, AsyncRead, AsyncWrite, ReadBuf};

mod handshake;
pub(crate) use handshake::{IoSession, MidHandshake};

#[derive(Debug)]
pub enum TlsState {
    #[cfg(feature = "early-data")]
    EarlyData(usize, Vec<u8>),
    Stream,
    ReadShutdown,
    WriteShutdown,
    FullyShutdown,
}

impl TlsState {
    #[inline]
    pub fn shutdown_read(&mut self) {
        match *self {
            TlsState::WriteShutdown | TlsState::FullyShutdown => *self = TlsState::FullyShutdown,
            _ => *self = TlsState::ReadShutdown,
        }
    }

    #[inline]
    pub fn shutdown_write(&mut self) {
        match *self {
            TlsState::ReadShutdown | TlsState::FullyShutdown => *self = TlsState::FullyShutdown,
            _ => *self = TlsState::WriteShutdown,
        }
    }

    #[inline]
    pub fn writeable(&self) -> bool {
        !matches!(*self, TlsState::WriteShutdown | TlsState::FullyShutdown)
    }

    #[inline]
    pub fn readable(&self) -> bool {
        !matches!(*self, TlsState::ReadShutdown | TlsState::FullyShutdown)
    }

    #[inline]
    #[cfg(feature = "early-data")]
    pub fn is_early_data(&self) -> bool {
        matches!(self, TlsState::EarlyData(..))
    }

    #[inline]
    #[cfg(not(feature = "early-data"))]
    pub const fn is_early_data(&self) -> bool {
        false
    }
}

pub struct Stream<'a, IO, C> {
    pub io: &'a mut IO,
    pub session: &'a mut C,
    pub eof: bool,
}

impl<'a, IO: AsyncRead + AsyncWrite + Unpin, C, SD> Stream<'a, IO, C>
where
    C: DerefMut + Deref<Target = ConnectionCommon<SD>>,
    SD: SideData,
{
    pub fn new(io: &'a mut IO, session: &'a mut C) -> Self {
        Stream {
            io,
            session,
            // The state so far is only used to detect EOF, so either Stream
            // or EarlyData state should both be all right.
            eof: false,
        }
    }

    pub fn set_eof(mut self, eof: bool) -> Self {
        self.eof = eof;
        self
    }

    pub fn as_mut_pin(&mut self) -> Pin<&mut Self> {
        Pin::new(self)
    }

    pub fn read_io(&mut self, cx: &mut Context) -> Poll<io::Result<usize>> {
        let mut reader = SyncReadAdapter { io: self.io, cx };

        let n = match self.session.read_tls(&mut reader) {
            Ok(n) => n,
            Err(ref err) if err.kind() == io::ErrorKind::WouldBlock => return Poll::Pending,
            Err(err) => return Poll::Ready(Err(err)),
        };

        self.session.process_new_packets().map_err(|err| {
            // In case we have an alert to send describing this error,
            // try a last-gasp write -- but don't predate the primary
            // error.
            let _ = self.write_io(cx);

            io::Error::new(io::ErrorKind::InvalidData, err)
        })?;

        Poll::Ready(Ok(n))
    }

    pub fn write_io(&mut self, cx: &mut Context) -> Poll<io::Result<usize>> {
        let mut writer = SyncWriteAdapter { io: self.io, cx };

        match self.session.write_tls(&mut writer) {
            Err(ref err) if err.kind() == io::ErrorKind::WouldBlock => Poll::Pending,
            result => Poll::Ready(result),
        }
    }

    pub fn handshake(&mut self, cx: &mut Context) -> Poll<io::Result<(usize, usize)>> {
        let mut wrlen = 0;
        let mut rdlen = 0;

        loop {
            let mut write_would_block = false;
            let mut read_would_block = false;
            let mut need_flush = false;

            while self.session.wants_write() {
                match self.write_io(cx) {
                    Poll::Ready(Ok(0)) => return Poll::Ready(Err(io::ErrorKind::WriteZero.into())),
                    Poll::Ready(Ok(n)) => {
                        wrlen += n;
                        need_flush = true;
                    }
                    Poll::Pending => {
                        write_would_block = true;
                        break;
                    }
                    Poll::Ready(Err(err)) => return Poll::Ready(Err(err)),
                }
            }

            if need_flush {
                match Pin::new(&mut self.io).poll_flush(cx) {
                    Poll::Ready(Ok(())) => (),
                    Poll::Ready(Err(err)) => return Poll::Ready(Err(err)),
                    Poll::Pending => write_would_block = true,
                }
            }

            while !self.eof && self.session.wants_read() {
                match self.read_io(cx) {
                    Poll::Ready(Ok(0)) => self.eof = true,
                    Poll::Ready(Ok(n)) => rdlen += n,
                    Poll::Pending => {
                        read_would_block = true;
                        break;
                    }
                    Poll::Ready(Err(err)) => return Poll::Ready(Err(err)),
                }
            }

            return match (self.eof, self.session.is_handshaking()) {
                (true, true) => {
                    let err = io::Error::new(io::ErrorKind::UnexpectedEof, "tls handshake eof");
                    Poll::Ready(Err(err))
                }
                (_, false) => Poll::Ready(Ok((rdlen, wrlen))),
                (_, true) if write_would_block || read_would_block => {
                    if rdlen != 0 || wrlen != 0 {
                        Poll::Ready(Ok((rdlen, wrlen)))
                    } else {
                        Poll::Pending
                    }
                }
                (..) => continue,
            };
        }
    }

    pub(crate) fn poll_fill_buf(mut self, cx: &mut Context<'_>) -> Poll<io::Result<&'a [u8]>>
    where
        SD: 'a,
    {
        let mut io_pending = false;

        // read a packet
        while !self.eof && self.session.wants_read() {
            match self.read_io(cx) {
                Poll::Ready(Ok(0)) => {
                    break;
                }
                Poll::Ready(Ok(_)) => (),
                Poll::Pending => {
                    io_pending = true;
                    break;
                }
                Poll::Ready(Err(err)) => return Poll::Ready(Err(err)),
            }
        }

        match self.session.reader().into_first_chunk() {
            Ok(buf) => {
                // Note that this could be empty (i.e. EOF) if a `CloseNotify` has been
                // received and there is no more buffered data.
                Poll::Ready(Ok(buf))
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                if !io_pending {
                    // If `wants_read()` is satisfied, rustls will not return `WouldBlock`.
                    // but if it does, we can try again.
                    //
                    // If the rustls state is abnormal, it may cause a cyclic wakeup.
                    // but tokio's cooperative budget will prevent infinite wakeup.
                    cx.waker().wake_by_ref();
                }

                Poll::Pending
            }
            Err(e) => Poll::Ready(Err(e)),
        }
    }
}

impl<'a, IO: AsyncRead + AsyncWrite + Unpin, C, SD> AsyncRead for Stream<'a, IO, C>
where
    C: DerefMut + Deref<Target = ConnectionCommon<SD>>,
    SD: SideData + 'a,
{
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let data = ready!(self.as_mut().poll_fill_buf(cx))?;
        let amount = buf.remaining().min(data.len());
        buf.put_slice(&data[..amount]);
        self.session.reader().consume(amount);
        Poll::Ready(Ok(()))
    }
}

impl<'a, IO: AsyncRead + AsyncWrite + Unpin, C, SD> AsyncBufRead for Stream<'a, IO, C>
where
    C: DerefMut + Deref<Target = ConnectionCommon<SD>>,
    SD: SideData + 'a,
{
    fn poll_fill_buf(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<&[u8]>> {
        let this = self.get_mut();
        Stream {
            // reborrow
            io: this.io,
            session: this.session,
            ..*this
        }
        .poll_fill_buf(cx)
    }

    fn consume(mut self: Pin<&mut Self>, amt: usize) {
        self.session.reader().consume(amt);
    }
}

impl<IO: AsyncRead + AsyncWrite + Unpin, C, SD> AsyncWrite for Stream<'_, IO, C>
where
    C: DerefMut + Deref<Target = ConnectionCommon<SD>>,
    SD: SideData,
{
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let mut pos = 0;

        while pos != buf.len() {
            let mut would_block = false;

            match self.session.writer().write(&buf[pos..]) {
                Ok(n) => pos += n,
                Err(err) => return Poll::Ready(Err(err)),
            };

            while self.session.wants_write() {
                match self.write_io(cx) {
                    Poll::Ready(Ok(0)) | Poll::Pending => {
                        would_block = true;
                        break;
                    }
                    Poll::Ready(Ok(_)) => (),
                    Poll::Ready(Err(err)) => return Poll::Ready(Err(err)),
                }
            }

            return match (pos, would_block) {
                (0, true) => Poll::Pending,
                (n, true) => Poll::Ready(Ok(n)),
                (_, false) => continue,
            };
        }

        Poll::Ready(Ok(pos))
    }

    fn poll_write_vectored(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        if bufs.iter().all(|buf| buf.is_empty()) {
            return Poll::Ready(Ok(0));
        }

        loop {
            let mut would_block = false;
            let written = self.session.writer().write_vectored(bufs)?;

            while self.session.wants_write() {
                match self.write_io(cx) {
                    Poll::Ready(Ok(0)) | Poll::Pending => {
                        would_block = true;
                        break;
                    }
                    Poll::Ready(Ok(_)) => (),
                    Poll::Ready(Err(err)) => return Poll::Ready(Err(err)),
                }
            }

            return match (written, would_block) {
                (0, true) => Poll::Pending,
                (0, false) => continue,
                (n, _) => Poll::Ready(Ok(n)),
            };
        }
    }

    #[inline]
    fn is_write_vectored(&self) -> bool {
        true
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<io::Result<()>> {
        self.session.writer().flush()?;
        while self.session.wants_write() {
            if ready!(self.write_io(cx))? == 0 {
                return Poll::Ready(Err(io::ErrorKind::WriteZero.into()));
            }
        }
        Pin::new(&mut self.io).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        while self.session.wants_write() {
            if ready!(self.write_io(cx))? == 0 {
                return Poll::Ready(Err(io::ErrorKind::WriteZero.into()));
            }
        }

        Poll::Ready(match ready!(Pin::new(&mut self.io).poll_shutdown(cx)) {
            Ok(()) => Ok(()),
            // When trying to shutdown, not being connected seems fine
            Err(err) if err.kind() == io::ErrorKind::NotConnected => Ok(()),
            Err(err) => Err(err),
        })
    }
}

/// An adapter that implements a [`Read`] interface for [`AsyncRead`] types and an
/// associated [`Context`].
///
/// Turns `Poll::Pending` into `WouldBlock`.
pub struct SyncReadAdapter<'a, 'b, T> {
    pub io: &'a mut T,
    pub cx: &'a mut Context<'b>,
}

impl<T: AsyncRead + Unpin> Read for SyncReadAdapter<'_, '_, T> {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut buf = ReadBuf::new(buf);
        match Pin::new(&mut self.io).poll_read(self.cx, &mut buf) {
            Poll::Ready(Ok(())) => Ok(buf.filled().len()),
            Poll::Ready(Err(err)) => Err(err),
            Poll::Pending => Err(io::ErrorKind::WouldBlock.into()),
        }
    }
}

/// An adapter that implements a [`Write`] interface for [`AsyncWrite`] types and an
/// associated [`Context`].
///
/// Turns `Poll::Pending` into `WouldBlock`.
pub struct SyncWriteAdapter<'a, 'b, T> {
    pub io: &'a mut T,
    pub cx: &'a mut Context<'b>,
}

impl<T: Unpin> SyncWriteAdapter<'_, '_, T> {
    #[inline]
    fn poll_with<U>(
        &mut self,
        f: impl FnOnce(Pin<&mut T>, &mut Context<'_>) -> Poll<io::Result<U>>,
    ) -> io::Result<U> {
        match f(Pin::new(self.io), self.cx) {
            Poll::Ready(result) => result,
            Poll::Pending => Err(io::ErrorKind::WouldBlock.into()),
        }
    }
}

impl<T: AsyncWrite + Unpin> Write for SyncWriteAdapter<'_, '_, T> {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.poll_with(|io, cx| io.poll_write(cx, buf))
    }

    #[inline]
    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        self.poll_with(|io, cx| io.poll_write_vectored(cx, bufs))
    }

    fn flush(&mut self) -> io::Result<()> {
        self.poll_with(|io, cx| io.poll_flush(cx))
    }
}

#[cfg(test)]
mod test_stream;
