//! HTTP Upgrades
//!
//! This module deals with managing [HTTP Upgrades][mdn] in hyper. Since
//! several concepts in HTTP allow for first talking HTTP, and then converting
//! to a different protocol, this module conflates them into a single API.
//! Those include:
//!
//! - HTTP/1.1 Upgrades
//! - HTTP `CONNECT`
//!
//! You are responsible for any other pre-requisites to establish an upgrade,
//! such as sending the appropriate headers, methods, and status codes. You can
//! then use [`on`][] to grab a `Future` which will resolve to the upgraded
//! connection object, or an error if the upgrade fails.
//!
//! [mdn]: https://developer.mozilla.org/en-US/docs/Web/HTTP/Protocol_upgrade_mechanism
//!
//! # Client
//!
//! Sending an HTTP upgrade from the [`client`](super::client) involves setting
//! either the appropriate method, if wanting to `CONNECT`, or headers such as
//! `Upgrade` and `Connection`, on the `http::Request`. Once receiving the
//! `http::Response` back, you must check for the specific information that the
//! upgrade is agreed upon by the server (such as a `101` status code), and then
//! get the `Future` from the `Response`.
//!
//! # Server
//!
//! Receiving upgrade requests in a server requires you to check the relevant
//! headers in a `Request`, and if an upgrade should be done, you then send the
//! corresponding headers in a response. To then wait for hyper to finish the
//! upgrade, you call `on()` with the `Request`, and then can spawn a task
//! awaiting it.
//!
//! # Example
//!
//! See [this example][example] showing how upgrades work with both
//! Clients and Servers.
//!
//! [example]: https://github.com/hyperium/hyper/blob/master/examples/upgrades.rs

use std::any::TypeId;
use std::error::Error as StdError;
use std::fmt;
use std::future::Future;
use std::io;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use crate::rt::{Read, ReadBufCursor, Write};
use bytes::Bytes;
use tokio::sync::oneshot;

use crate::common::io::Rewind;

/// An upgraded HTTP connection.
///
/// This type holds a trait object internally of the original IO that
/// was used to speak HTTP before the upgrade. It can be used directly
/// as a [`Read`] or [`Write`] for convenience.
///
/// Alternatively, if the exact type is known, this can be deconstructed
/// into its parts.
pub struct Upgraded {
    io: Rewind<Box<dyn Io + Send>>,
}

/// A future for a possible HTTP upgrade.
///
/// If no upgrade was available, or it doesn't succeed, yields an `Error`.
#[derive(Clone)]
pub struct OnUpgrade {
    rx: Option<Arc<Mutex<oneshot::Receiver<crate::Result<Upgraded>>>>>,
}

/// The deconstructed parts of an [`Upgraded`] type.
///
/// Includes the original IO type, and a read buffer of bytes that the
/// HTTP state machine may have already read before completing an upgrade.
#[derive(Debug)]
#[non_exhaustive]
pub struct Parts<T> {
    /// The original IO object used before the upgrade.
    pub io: T,
    /// A buffer of bytes that have been read but not processed as HTTP.
    ///
    /// For instance, if the `Connection` is used for an HTTP upgrade request,
    /// it is possible the server sent back the first bytes of the new protocol
    /// along with the response upgrade.
    ///
    /// You will want to check for any existing bytes if you plan to continue
    /// communicating on the IO object.
    pub read_buf: Bytes,
}

/// Gets a pending HTTP upgrade from this message.
///
/// This can be called on the following types:
///
/// - `http::Request<B>`
/// - `http::Response<B>`
/// - `&mut http::Request<B>`
/// - `&mut http::Response<B>`
pub fn on<T: sealed::CanUpgrade>(msg: T) -> OnUpgrade {
    msg.on_upgrade()
}

#[cfg(all(
    any(feature = "client", feature = "server"),
    any(feature = "http1", feature = "http2"),
))]
pub(super) struct Pending {
    tx: oneshot::Sender<crate::Result<Upgraded>>,
}

#[cfg(all(
    any(feature = "client", feature = "server"),
    any(feature = "http1", feature = "http2"),
))]
pub(super) fn pending() -> (Pending, OnUpgrade) {
    let (tx, rx) = oneshot::channel();
    (
        Pending { tx },
        OnUpgrade {
            rx: Some(Arc::new(Mutex::new(rx))),
        },
    )
}

// ===== impl Upgraded =====

impl Upgraded {
    #[cfg(all(
        any(feature = "client", feature = "server"),
        any(feature = "http1", feature = "http2")
    ))]
    pub(super) fn new<T>(io: T, read_buf: Bytes) -> Self
    where
        T: Read + Write + Unpin + Send + 'static,
    {
        Upgraded {
            io: Rewind::new_buffered(Box::new(io), read_buf),
        }
    }

    /// Tries to downcast the internal trait object to the type passed.
    ///
    /// On success, returns the downcasted parts. On error, returns the
    /// `Upgraded` back.
    pub fn downcast<T: Read + Write + Unpin + 'static>(self) -> Result<Parts<T>, Self> {
        let (io, buf) = self.io.into_inner();
        match io.__hyper_downcast() {
            Ok(t) => Ok(Parts {
                io: *t,
                read_buf: buf,
            }),
            Err(io) => Err(Upgraded {
                io: Rewind::new_buffered(io, buf),
            }),
        }
    }
}

impl Read for Upgraded {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: ReadBufCursor<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.io).poll_read(cx, buf)
    }
}

impl Write for Upgraded {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.io).poll_write(cx, buf)
    }

    fn poll_write_vectored(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[io::IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.io).poll_write_vectored(cx, bufs)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.io).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.io).poll_shutdown(cx)
    }

    fn is_write_vectored(&self) -> bool {
        self.io.is_write_vectored()
    }
}

impl fmt::Debug for Upgraded {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Upgraded").finish()
    }
}

// ===== impl OnUpgrade =====

impl OnUpgrade {
    pub(super) fn none() -> Self {
        OnUpgrade { rx: None }
    }

    #[cfg(all(any(feature = "client", feature = "server"), feature = "http1"))]
    pub(super) fn is_none(&self) -> bool {
        self.rx.is_none()
    }
}

impl Future for OnUpgrade {
    type Output = Result<Upgraded, crate::Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.rx {
            Some(ref rx) => Pin::new(&mut *rx.lock().unwrap())
                .poll(cx)
                .map(|res| match res {
                    Ok(Ok(upgraded)) => Ok(upgraded),
                    Ok(Err(err)) => Err(err),
                    Err(_oneshot_canceled) => {
                        Err(crate::Error::new_canceled().with(UpgradeExpected))
                    }
                }),
            None => Poll::Ready(Err(crate::Error::new_user_no_upgrade())),
        }
    }
}

impl fmt::Debug for OnUpgrade {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OnUpgrade").finish()
    }
}

// ===== impl Pending =====

#[cfg(all(
    any(feature = "client", feature = "server"),
    any(feature = "http1", feature = "http2")
))]
impl Pending {
    pub(super) fn fulfill(self, upgraded: Upgraded) {
        trace!("pending upgrade fulfill");
        let _ = self.tx.send(Ok(upgraded));
    }

    #[cfg(feature = "http1")]
    /// Don't fulfill the pending Upgrade, but instead signal that
    /// upgrades are handled manually.
    pub(super) fn manual(self) {
        #[cfg(any(feature = "http1", feature = "http2"))]
        trace!("pending upgrade handled manually");
        let _ = self.tx.send(Err(crate::Error::new_user_manual_upgrade()));
    }
}

// ===== impl UpgradeExpected =====

/// Error cause returned when an upgrade was expected but canceled
/// for whatever reason.
///
/// This likely means the actual `Conn` future wasn't polled and upgraded.
#[derive(Debug)]
struct UpgradeExpected;

impl fmt::Display for UpgradeExpected {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("upgrade expected but not completed")
    }
}

impl StdError for UpgradeExpected {}

// ===== impl Io =====

pub(super) trait Io: Read + Write + Unpin + 'static {
    fn __hyper_type_id(&self) -> TypeId {
        TypeId::of::<Self>()
    }
}

impl<T: Read + Write + Unpin + 'static> Io for T {}

impl dyn Io + Send {
    fn __hyper_is<T: Io>(&self) -> bool {
        let t = TypeId::of::<T>();
        self.__hyper_type_id() == t
    }

    fn __hyper_downcast<T: Io>(self: Box<Self>) -> Result<Box<T>, Box<Self>> {
        if self.__hyper_is::<T>() {
            // Taken from `std::error::Error::downcast()`.
            unsafe {
                let raw: *mut dyn Io = Box::into_raw(self);
                Ok(Box::from_raw(raw as *mut T))
            }
        } else {
            Err(self)
        }
    }
}

mod sealed {
    use super::OnUpgrade;

    pub trait CanUpgrade {
        fn on_upgrade(self) -> OnUpgrade;
    }

    impl<B> CanUpgrade for http::Request<B> {
        fn on_upgrade(mut self) -> OnUpgrade {
            self.extensions_mut()
                .remove::<OnUpgrade>()
                .unwrap_or_else(OnUpgrade::none)
        }
    }

    impl<B> CanUpgrade for &'_ mut http::Request<B> {
        fn on_upgrade(self) -> OnUpgrade {
            self.extensions_mut()
                .remove::<OnUpgrade>()
                .unwrap_or_else(OnUpgrade::none)
        }
    }

    impl<B> CanUpgrade for http::Response<B> {
        fn on_upgrade(mut self) -> OnUpgrade {
            self.extensions_mut()
                .remove::<OnUpgrade>()
                .unwrap_or_else(OnUpgrade::none)
        }
    }

    impl<B> CanUpgrade for &'_ mut http::Response<B> {
        fn on_upgrade(self) -> OnUpgrade {
            self.extensions_mut()
                .remove::<OnUpgrade>()
                .unwrap_or_else(OnUpgrade::none)
        }
    }
}

#[cfg(all(
    any(feature = "client", feature = "server"),
    any(feature = "http1", feature = "http2"),
))]
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upgraded_downcast() {
        let upgraded = Upgraded::new(Mock, Bytes::new());

        let upgraded = upgraded
            .downcast::<crate::common::io::Compat<std::io::Cursor<Vec<u8>>>>()
            .unwrap_err();

        upgraded.downcast::<Mock>().unwrap();
    }

    // TODO: replace with tokio_test::io when it can test write_buf
    struct Mock;

    impl Read for Mock {
        fn poll_read(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            _buf: ReadBufCursor<'_>,
        ) -> Poll<io::Result<()>> {
            unreachable!("Mock::poll_read")
        }
    }

    impl Write for Mock {
        fn poll_write(
            self: Pin<&mut Self>,
            _: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<io::Result<usize>> {
            // panic!("poll_write shouldn't be called");
            Poll::Ready(Ok(buf.len()))
        }

        fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            unreachable!("Mock::poll_flush")
        }

        fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            unreachable!("Mock::poll_shutdown")
        }
    }
}
