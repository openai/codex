use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use futures_channel::mpsc;
use futures_util::task::{Context, Poll};
use futures_util::Future;
use futures_util::TryFutureExt;
use hyper::Uri;
use tokio::io::{self, AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::TcpStream;

use hyper::rt::ReadBufCursor;

use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::client::legacy::connect::{Connected, Connection};
use hyper_util::rt::TokioIo;

#[derive(Clone)]
pub struct DebugConnector {
    pub http: HttpConnector,
    pub closes: mpsc::Sender<()>,
    pub connects: Arc<AtomicUsize>,
    pub is_proxy: bool,
    pub alpn_h2: bool,
}

impl DebugConnector {
    pub fn new() -> DebugConnector {
        let http = HttpConnector::new();
        let (tx, _) = mpsc::channel(10);
        DebugConnector::with_http_and_closes(http, tx)
    }

    pub fn with_http_and_closes(http: HttpConnector, closes: mpsc::Sender<()>) -> DebugConnector {
        DebugConnector {
            http,
            closes,
            connects: Arc::new(AtomicUsize::new(0)),
            is_proxy: false,
            alpn_h2: false,
        }
    }

    pub fn proxy(mut self) -> Self {
        self.is_proxy = true;
        self
    }
}

impl tower_service::Service<Uri> for DebugConnector {
    type Response = DebugStream;
    type Error = <HttpConnector as tower_service::Service<Uri>>::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        // don't forget to check inner service is ready :)
        tower_service::Service::<Uri>::poll_ready(&mut self.http, cx)
    }

    fn call(&mut self, dst: Uri) -> Self::Future {
        self.connects.fetch_add(1, Ordering::SeqCst);
        let closes = self.closes.clone();
        let is_proxy = self.is_proxy;
        let is_alpn_h2 = self.alpn_h2;
        Box::pin(self.http.call(dst).map_ok(move |tcp| DebugStream {
            tcp,
            on_drop: closes,
            is_alpn_h2,
            is_proxy,
        }))
    }
}

pub struct DebugStream {
    tcp: TokioIo<TcpStream>,
    on_drop: mpsc::Sender<()>,
    is_alpn_h2: bool,
    is_proxy: bool,
}

impl Drop for DebugStream {
    fn drop(&mut self) {
        let _ = self.on_drop.try_send(());
    }
}

impl Connection for DebugStream {
    fn connected(&self) -> Connected {
        let connected = self.tcp.connected().proxy(self.is_proxy);

        if self.is_alpn_h2 {
            connected.negotiated_h2()
        } else {
            connected
        }
    }
}

impl hyper::rt::Read for DebugStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: ReadBufCursor<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        hyper::rt::Read::poll_read(Pin::new(&mut self.tcp), cx, buf)
    }
}

impl hyper::rt::Write for DebugStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        hyper::rt::Write::poll_write(Pin::new(&mut self.tcp), cx, buf)
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        hyper::rt::Write::poll_flush(Pin::new(&mut self.tcp), cx)
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        hyper::rt::Write::poll_shutdown(Pin::new(&mut self.tcp), cx)
    }

    fn is_write_vectored(&self) -> bool {
        hyper::rt::Write::is_write_vectored(&self.tcp)
    }

    fn poll_write_vectored(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[std::io::IoSlice<'_>],
    ) -> Poll<Result<usize, std::io::Error>> {
        hyper::rt::Write::poll_write_vectored(Pin::new(&mut self.tcp), cx, bufs)
    }
}

impl AsyncWrite for DebugStream {
    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), io::Error>> {
        Pin::new(self.tcp.inner_mut()).poll_shutdown(cx)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        Pin::new(self.tcp.inner_mut()).poll_flush(cx)
    }

    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        Pin::new(self.tcp.inner_mut()).poll_write(cx, buf)
    }
}

impl AsyncRead for DebugStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(self.tcp.inner_mut()).poll_read(cx, buf)
    }
}
