use std::{ops::Deref, sync::Arc};

use http::Request;
use tokio::sync::watch;

use super::Connected;

/// [`CaptureConnection`] allows callers to capture [`Connected`] information
///
/// To capture a connection for a request, use [`capture_connection`].
#[derive(Debug, Clone)]
pub struct CaptureConnection {
    rx: watch::Receiver<Option<Connected>>,
}

/// Capture the connection for a given request
///
/// When making a request with Hyper, the underlying connection must implement the [`Connection`] trait.
/// [`capture_connection`] allows a caller to capture the returned [`Connected`] structure as soon
/// as the connection is established.
///
/// [`Connection`]: crate::client::legacy::connect::Connection
///
/// *Note*: If establishing a connection fails, [`CaptureConnection::connection_metadata`] will always return none.
///
/// # Examples
///
/// **Synchronous access**:
/// The [`CaptureConnection::connection_metadata`] method allows callers to check if a connection has been
/// established. This is ideal for situations where you are certain the connection has already
/// been established (e.g. after the response future has already completed).
/// ```rust
/// use hyper_util::client::legacy::connect::capture_connection;
/// let mut request = http::Request::builder()
///   .uri("http://foo.com")
///   .body(())
///   .unwrap();
///
/// let captured_connection = capture_connection(&mut request);
/// // some time later after the request has been sent...
/// let connection_info = captured_connection.connection_metadata();
/// println!("we are connected! {:?}", connection_info.as_ref());
/// ```
///
/// **Asynchronous access**:
/// The [`CaptureConnection::wait_for_connection_metadata`] method returns a future resolves as soon as the
/// connection is available.
///
/// ```rust
/// # #[cfg(feature  = "tokio")]
/// # async fn example() {
/// use hyper_util::client::legacy::connect::capture_connection;
/// use hyper_util::client::legacy::Client;
/// use hyper_util::rt::TokioExecutor;
/// use bytes::Bytes;
/// use http_body_util::Empty;
/// let mut request = http::Request::builder()
///   .uri("http://foo.com")
///   .body(Empty::<Bytes>::new())
///   .unwrap();
///
/// let mut captured = capture_connection(&mut request);
/// tokio::task::spawn(async move {
///     let connection_info = captured.wait_for_connection_metadata().await;
///     println!("we are connected! {:?}", connection_info.as_ref());
/// });
///
/// let client = Client::builder(TokioExecutor::new()).build_http();
/// client.request(request).await.expect("request failed");
/// # }
/// ```
pub fn capture_connection<B>(request: &mut Request<B>) -> CaptureConnection {
    let (tx, rx) = CaptureConnection::new();
    request.extensions_mut().insert(tx);
    rx
}

/// TxSide for [`CaptureConnection`]
///
/// This is inserted into `Extensions` to allow Hyper to back channel connection info
#[derive(Clone)]
pub(crate) struct CaptureConnectionExtension {
    tx: Arc<watch::Sender<Option<Connected>>>,
}

impl CaptureConnectionExtension {
    pub(crate) fn set(&self, connected: &Connected) {
        self.tx.send_replace(Some(connected.clone()));
    }
}

impl CaptureConnection {
    /// Internal API to create the tx and rx half of [`CaptureConnection`]
    pub(crate) fn new() -> (CaptureConnectionExtension, Self) {
        let (tx, rx) = watch::channel(None);
        (
            CaptureConnectionExtension { tx: Arc::new(tx) },
            CaptureConnection { rx },
        )
    }

    /// Retrieve the connection metadata, if available
    pub fn connection_metadata(&self) -> impl Deref<Target = Option<Connected>> + '_ {
        self.rx.borrow()
    }

    /// Wait for the connection to be established
    ///
    /// If a connection was established, this will always return `Some(...)`. If the request never
    /// successfully connected (e.g. DNS resolution failure), this method will never return.
    pub async fn wait_for_connection_metadata(
        &mut self,
    ) -> impl Deref<Target = Option<Connected>> + '_ {
        if self.rx.borrow().is_some() {
            return self.rx.borrow();
        }
        let _ = self.rx.changed().await;
        self.rx.borrow()
    }
}

#[cfg(all(test, not(miri)))]
mod test {
    use super::*;

    #[test]
    fn test_sync_capture_connection() {
        let (tx, rx) = CaptureConnection::new();
        assert!(
            rx.connection_metadata().is_none(),
            "connection has not been set"
        );
        tx.set(&Connected::new().proxy(true));
        assert!(rx
            .connection_metadata()
            .as_ref()
            .expect("connected should be set")
            .is_proxied());

        // ensure it can be called multiple times
        assert!(rx
            .connection_metadata()
            .as_ref()
            .expect("connected should be set")
            .is_proxied());
    }

    #[tokio::test]
    async fn async_capture_connection() {
        let (tx, mut rx) = CaptureConnection::new();
        assert!(
            rx.connection_metadata().is_none(),
            "connection has not been set"
        );
        let test_task = tokio::spawn(async move {
            assert!(rx
                .wait_for_connection_metadata()
                .await
                .as_ref()
                .expect("connection should be set")
                .is_proxied());
            // can be awaited multiple times
            assert!(
                rx.wait_for_connection_metadata().await.is_some(),
                "should be awaitable multiple times"
            );

            assert!(rx.connection_metadata().is_some());
        });
        // can't be finished, we haven't set the connection yet
        assert!(!test_task.is_finished());
        tx.set(&Connected::new().proxy(true));

        assert!(test_task.await.is_ok());
    }

    #[tokio::test]
    async fn capture_connection_sender_side_dropped() {
        let (tx, mut rx) = CaptureConnection::new();
        assert!(
            rx.connection_metadata().is_none(),
            "connection has not been set"
        );
        drop(tx);
        assert!(rx.wait_for_connection_metadata().await.is_none());
    }
}
