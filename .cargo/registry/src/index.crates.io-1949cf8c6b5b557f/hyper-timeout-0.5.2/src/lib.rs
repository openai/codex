use std::future::Future;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use hyper::rt::{Read, Write};
use tokio::time::timeout;

use hyper::Uri;
use hyper_util::client::legacy::connect::{Connected, Connection};
use tower_service::Service;

mod stream;
use stream::TimeoutStream;

type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// A connector that enforces a connection timeout
#[derive(Debug, Clone)]
pub struct TimeoutConnector<T> {
    /// A connector implementing the `Connect` trait
    connector: T,
    /// Amount of time to wait connecting
    connect_timeout: Option<Duration>,
    /// Amount of time to wait reading response
    read_timeout: Option<Duration>,
    /// Amount of time to wait writing request
    write_timeout: Option<Duration>,
    /// If true, resets the reader timeout whenever a write occures
    reset_reader_on_write: bool,
}

impl<T> TimeoutConnector<T>
where
    T: Service<Uri> + Send,
    T::Response: Read + Write + Send + Unpin,
    T::Future: Send + 'static,
    T::Error: Into<BoxError>,
{
    /// Construct a new TimeoutConnector with a given connector implementing the `Connect` trait
    pub fn new(connector: T) -> Self {
        TimeoutConnector {
            connector,
            connect_timeout: None,
            read_timeout: None,
            write_timeout: None,
            reset_reader_on_write: false,
        }
    }
}

impl<T> Service<Uri> for TimeoutConnector<T>
where
    T: Service<Uri> + Send,
    T::Response: Read + Write + Connection + Send + Unpin,
    T::Future: Send + 'static,
    T::Error: Into<BoxError>,
{
    type Response = Pin<Box<TimeoutStream<T::Response>>>;
    type Error = BoxError;
    #[allow(clippy::type_complexity)]
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.connector.poll_ready(cx).map_err(Into::into)
    }

    fn call(&mut self, dst: Uri) -> Self::Future {
        let connect_timeout = self.connect_timeout;
        let read_timeout = self.read_timeout;
        let write_timeout = self.write_timeout;
        let reset_reader_on_write = self.reset_reader_on_write;
        let connecting = self.connector.call(dst);

        let fut = async move {
            let mut stream = match connect_timeout {
                None => {
                    let io = connecting.await.map_err(Into::into)?;
                    TimeoutStream::new(io)
                }
                Some(connect_timeout) => {
                    let timeout = timeout(connect_timeout, connecting);
                    let connecting = timeout
                        .await
                        .map_err(|e| io::Error::new(io::ErrorKind::TimedOut, e))?;
                    let io = connecting.map_err(Into::into)?;
                    TimeoutStream::new(io)
                }
            };
            stream.set_read_timeout(read_timeout);
            stream.set_write_timeout(write_timeout);
            stream.set_reset_reader_on_write(reset_reader_on_write);
            Ok(Box::pin(stream))
        };

        Box::pin(fut)
    }
}

impl<T> TimeoutConnector<T> {
    /// Set the timeout for connecting to a URL.
    ///
    /// Default is no timeout.
    #[inline]
    pub fn set_connect_timeout(&mut self, val: Option<Duration>) {
        self.connect_timeout = val;
    }

    /// Set the timeout for the response.
    ///
    /// Default is no timeout.
    #[inline]
    pub fn set_read_timeout(&mut self, val: Option<Duration>) {
        self.read_timeout = val;
    }

    /// Set the timeout for the request.
    ///
    /// Default is no timeout.
    #[inline]
    pub fn set_write_timeout(&mut self, val: Option<Duration>) {
        self.write_timeout = val;
    }

    /// Reset on the reader timeout on write
    ///
    /// This will reset the reader timeout when a write is done through the
    /// the TimeoutReader. This is useful when you don't want to trigger
    /// a reader timeout while writes are still be accepted.
    pub fn set_reset_reader_on_write(&mut self, reset: bool) {
        self.reset_reader_on_write = reset;
    }
}

impl<T> Connection for TimeoutConnector<T>
where
    T: Read + Write + Connection + Service<Uri> + Send + Unpin,
    T::Response: Read + Write + Send + Unpin,
    T::Future: Send + 'static,
    T::Error: Into<BoxError>,
{
    fn connected(&self) -> Connected {
        self.connector.connected()
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;
    use std::{error::Error, io};

    use http_body_util::Empty;
    use hyper::body::Bytes;
    use hyper_util::{
        client::legacy::{connect::HttpConnector, Client},
        rt::TokioExecutor,
    };

    use super::TimeoutConnector;

    #[tokio::test]
    async fn test_timeout_connector() {
        // 10.255.255.1 is a not a routable IP address
        let url = "http://10.255.255.1".parse().unwrap();

        let http = HttpConnector::new();
        let mut connector = TimeoutConnector::new(http);
        connector.set_connect_timeout(Some(Duration::from_millis(1)));

        let client = Client::builder(TokioExecutor::new()).build::<_, Empty<Bytes>>(connector);

        let res = client.get(url).await;

        match res {
            Ok(_) => panic!("Expected a timeout"),
            Err(e) => {
                if let Some(io_e) = e.source().unwrap().downcast_ref::<io::Error>() {
                    assert_eq!(io_e.kind(), io::ErrorKind::TimedOut);
                } else {
                    panic!("Expected timeout error");
                }
            }
        }
    }

    #[tokio::test]
    async fn test_read_timeout() {
        let url = "http://example.com".parse().unwrap();

        let http = HttpConnector::new();
        let mut connector = TimeoutConnector::new(http);
        // A 1 ms read timeout should be so short that we trigger a timeout error
        connector.set_read_timeout(Some(Duration::from_millis(1)));

        let client = Client::builder(TokioExecutor::new()).build::<_, Empty<Bytes>>(connector);

        let res = client.get(url).await;

        if let Err(client_e) = res {
            if let Some(hyper_e) = client_e.source() {
                if let Some(io_e) = hyper_e.source().unwrap().downcast_ref::<io::Error>() {
                    return assert_eq!(io_e.kind(), io::ErrorKind::TimedOut);
                }
            }
        }
        panic!("Expected timeout error");
    }
}
