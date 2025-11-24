use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use http::Uri;
use hyper_util::rt::TokioIo;

use tower::Service;

use crate::status::ConnectError;

#[cfg(not(target_os = "windows"))]
use tokio::net::UnixStream;

#[cfg(not(target_os = "windows"))]
async fn connect_uds(uds_path: String) -> Result<UnixStream, ConnectError> {
    UnixStream::connect(uds_path)
        .await
        .map_err(|err| ConnectError(From::from(err)))
}

// Dummy type that will allow us to compile and match trait bounds
// but is never used.
#[cfg(target_os = "windows")]
#[allow(dead_code)]
type UnixStream = tokio::io::DuplexStream;

#[cfg(target_os = "windows")]
async fn connect_uds(_uds_path: String) -> Result<UnixStream, ConnectError> {
    Err(ConnectError(
        "uds connections are not allowed on windows".into(),
    ))
}

pub(crate) struct UdsConnector {
    uds_filepath: String,
}

impl UdsConnector {
    pub(crate) fn new(uds_filepath: &str) -> Self {
        UdsConnector {
            uds_filepath: uds_filepath.to_string(),
        }
    }
}

impl Service<Uri> for UdsConnector {
    type Response = TokioIo<UnixStream>;
    type Error = ConnectError;
    type Future = UdsConnecting;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, _: Uri) -> Self::Future {
        let uds_path = self.uds_filepath.clone();
        let fut = async move {
            let stream = connect_uds(uds_path).await?;
            Ok(TokioIo::new(stream))
        };
        UdsConnecting {
            inner: Box::pin(fut),
        }
    }
}

type ConnectResult = Result<TokioIo<UnixStream>, ConnectError>;

pub(crate) struct UdsConnecting {
    inner: Pin<Box<dyn Future<Output = ConnectResult> + Send>>,
}

impl Future for UdsConnecting {
    type Output = ConnectResult;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.get_mut().inner.as_mut().poll(cx)
    }
}
