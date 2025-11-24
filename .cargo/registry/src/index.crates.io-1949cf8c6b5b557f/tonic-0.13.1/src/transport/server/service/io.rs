use crate::transport::server::Connected;
use std::io;
use std::io::IoSlice;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
#[cfg(feature = "_tls-any")]
use tokio_rustls::server::TlsStream;
use tower_layer::Layer;
use tower_service::Service;

#[derive(Debug, Clone)]
pub(crate) struct ConnectInfoLayer<T> {
    connect_info: T,
}

impl<T> ConnectInfoLayer<T> {
    pub(crate) fn new(connect_info: T) -> Self {
        Self { connect_info }
    }
}

impl<S, T> Layer<S> for ConnectInfoLayer<T>
where
    T: Clone,
{
    type Service = ConnectInfo<S, T>;

    fn layer(&self, inner: S) -> Self::Service {
        ConnectInfo::new(inner, self.connect_info.clone())
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ConnectInfo<S, T> {
    inner: S,
    connect_info: T,
}

impl<S, T> ConnectInfo<S, T> {
    fn new(inner: S, connect_info: T) -> Self {
        Self {
            inner,
            connect_info,
        }
    }
}

impl<S, IO, ReqBody> Service<http::Request<ReqBody>> for ConnectInfo<S, ServerIoConnectInfo<IO>>
where
    S: Service<http::Request<ReqBody>>,
    IO: Connected,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: http::Request<ReqBody>) -> Self::Future {
        match self.connect_info.clone() {
            ServerIoConnectInfo::Io(inner) => {
                req.extensions_mut().insert(inner);
            }
            #[cfg(feature = "_tls-any")]
            ServerIoConnectInfo::TlsIo(inner) => {
                req.extensions_mut().insert(inner.get_ref().clone());
                req.extensions_mut().insert(inner);
            }
        }
        self.inner.call(req)
    }
}

pub(crate) enum ServerIo<IO> {
    Io(IO),
    #[cfg(feature = "_tls-any")]
    TlsIo(Box<TlsStream<IO>>),
}

pub(crate) enum ServerIoConnectInfo<IO: Connected> {
    Io(<IO as Connected>::ConnectInfo),
    #[cfg(feature = "_tls-any")]
    TlsIo(<TlsStream<IO> as Connected>::ConnectInfo),
}

impl<IO: Connected> Clone for ServerIoConnectInfo<IO> {
    fn clone(&self) -> Self {
        match self {
            Self::Io(io) => Self::Io(io.clone()),
            #[cfg(feature = "_tls-any")]
            Self::TlsIo(io) => Self::TlsIo(io.clone()),
        }
    }
}

impl<IO> ServerIo<IO> {
    pub(in crate::transport) fn new_io(io: IO) -> Self {
        Self::Io(io)
    }

    #[cfg(feature = "_tls-any")]
    pub(in crate::transport) fn new_tls_io(io: TlsStream<IO>) -> Self {
        Self::TlsIo(Box::new(io))
    }

    pub(in crate::transport) fn connect_info(&self) -> ServerIoConnectInfo<IO>
    where
        IO: Connected,
    {
        match self {
            Self::Io(io) => ServerIoConnectInfo::Io(io.connect_info()),
            #[cfg(feature = "_tls-any")]
            Self::TlsIo(io) => ServerIoConnectInfo::TlsIo(io.connect_info()),
        }
    }
}

impl<IO> AsyncRead for ServerIo<IO>
where
    IO: AsyncWrite + AsyncRead + Unpin,
{
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        match &mut *self {
            Self::Io(io) => Pin::new(io).poll_read(cx, buf),
            #[cfg(feature = "_tls-any")]
            Self::TlsIo(io) => Pin::new(io).poll_read(cx, buf),
        }
    }
}

impl<IO> AsyncWrite for ServerIo<IO>
where
    IO: AsyncWrite + AsyncRead + Unpin,
{
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        match &mut *self {
            Self::Io(io) => Pin::new(io).poll_write(cx, buf),
            #[cfg(feature = "_tls-any")]
            Self::TlsIo(io) => Pin::new(io).poll_write(cx, buf),
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match &mut *self {
            Self::Io(io) => Pin::new(io).poll_flush(cx),
            #[cfg(feature = "_tls-any")]
            Self::TlsIo(io) => Pin::new(io).poll_flush(cx),
        }
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match &mut *self {
            Self::Io(io) => Pin::new(io).poll_shutdown(cx),
            #[cfg(feature = "_tls-any")]
            Self::TlsIo(io) => Pin::new(io).poll_shutdown(cx),
        }
    }

    fn poll_write_vectored(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[IoSlice<'_>],
    ) -> Poll<Result<usize, io::Error>> {
        match &mut *self {
            Self::Io(io) => Pin::new(io).poll_write_vectored(cx, bufs),
            #[cfg(feature = "_tls-any")]
            Self::TlsIo(io) => Pin::new(io).poll_write_vectored(cx, bufs),
        }
    }

    fn is_write_vectored(&self) -> bool {
        match self {
            Self::Io(io) => io.is_write_vectored(),
            #[cfg(feature = "_tls-any")]
            Self::TlsIo(io) => io.is_write_vectored(),
        }
    }
}
