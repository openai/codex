//! TCP/UDP/Unix helpers for tokio.

use crate::either::Either;
use std::future::Future;
use std::io::Result;
use std::pin::Pin;
use std::task::{Context, Poll};

#[cfg(unix)]
pub mod unix;

/// A trait for a listener: `TcpListener` and `UnixListener`.
pub trait Listener {
    /// The stream's type of this listener.
    type Io: tokio::io::AsyncRead + tokio::io::AsyncWrite;
    /// The socket address type of this listener.
    type Addr;

    /// Polls to accept a new incoming connection to this listener.
    fn poll_accept(&mut self, cx: &mut Context<'_>) -> Poll<Result<(Self::Io, Self::Addr)>>;

    /// Accepts a new incoming connection from this listener.
    fn accept(&mut self) -> ListenerAcceptFut<'_, Self>
    where
        Self: Sized,
    {
        ListenerAcceptFut { listener: self }
    }

    /// Returns the local address that this listener is bound to.
    fn local_addr(&self) -> Result<Self::Addr>;
}

impl Listener for tokio::net::TcpListener {
    type Io = tokio::net::TcpStream;
    type Addr = std::net::SocketAddr;

    fn poll_accept(&mut self, cx: &mut Context<'_>) -> Poll<Result<(Self::Io, Self::Addr)>> {
        Self::poll_accept(self, cx)
    }

    fn local_addr(&self) -> Result<Self::Addr> {
        self.local_addr()
    }
}

/// Future for accepting a new connection from a listener.
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct ListenerAcceptFut<'a, L> {
    listener: &'a mut L,
}

impl<'a, L> Future for ListenerAcceptFut<'a, L>
where
    L: Listener,
{
    type Output = Result<(L::Io, L::Addr)>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.listener.poll_accept(cx)
    }
}

impl<L, R> Either<L, R>
where
    L: Listener,
    R: Listener,
{
    /// Accepts a new incoming connection from this listener.
    pub async fn accept(&mut self) -> Result<Either<(L::Io, L::Addr), (R::Io, R::Addr)>> {
        match self {
            Either::Left(listener) => {
                let (stream, addr) = listener.accept().await?;
                Ok(Either::Left((stream, addr)))
            }
            Either::Right(listener) => {
                let (stream, addr) = listener.accept().await?;
                Ok(Either::Right((stream, addr)))
            }
        }
    }

    /// Returns the local address that this listener is bound to.
    pub fn local_addr(&self) -> Result<Either<L::Addr, R::Addr>> {
        match self {
            Either::Left(listener) => {
                let addr = listener.local_addr()?;
                Ok(Either::Left(addr))
            }
            Either::Right(listener) => {
                let addr = listener.local_addr()?;
                Ok(Either::Right(addr))
            }
        }
    }
}
