//! The following is derived from Rust's
//! library/std/src/os/windows/io/raw.rs
//! at revision
//! 4f9b394c8a24803e57ba892fa00e539742ebafc0.
//!
//! All code in this file is licensed MIT or Apache 2.0 at your option.

use super::super::raw;

/// Raw SOCKETs.
#[cfg_attr(staged_api, stable(feature = "rust1", since = "1.0.0"))]
pub type RawSocket = raw::SOCKET;

/// Extracts raw sockets.
#[cfg_attr(staged_api, stable(feature = "rust1", since = "1.0.0"))]
pub trait AsRawSocket {
    /// Extracts the raw socket.
    ///
    /// This function is typically used to **borrow** an owned socket.
    /// When used in this way, this method does **not** pass ownership of the
    /// raw socket to the caller, and the socket is only guaranteed
    /// to be valid while the original object has not yet been destroyed.
    ///
    /// However, borrowing is not strictly required. See [`AsSocket::as_socket`]
    /// for an API which strictly borrows a socket.
    #[cfg_attr(staged_api, stable(feature = "rust1", since = "1.0.0"))]
    fn as_raw_socket(&self) -> RawSocket;
}

/// Creates I/O objects from raw sockets.
#[cfg_attr(staged_api, stable(feature = "from_raw_os", since = "1.1.0"))]
pub trait FromRawSocket {
    /// Constructs a new I/O object from the specified raw socket.
    ///
    /// This function is typically used to **consume ownership** of the socket
    /// given, passing responsibility for closing the socket to the returned
    /// object. When used in this way, the returned object
    /// will take responsibility for closing it when the object goes out of
    /// scope.
    ///
    /// However, consuming ownership is not strictly required. Use a
    /// `From<OwnedSocket>::from` implementation for an API which strictly
    /// consumes ownership.
    ///
    /// # Safety
    ///
    /// The `socket` passed in must:
    ///   - be a valid an open socket,
    ///   - be a socket that may be freed via [`closesocket`].
    ///
    /// [`closesocket`]: https://docs.microsoft.com/en-us/windows/win32/api/winsock2/nf-winsock2-closesocket
    #[cfg_attr(staged_api, stable(feature = "from_raw_os", since = "1.1.0"))]
    unsafe fn from_raw_socket(sock: RawSocket) -> Self;
}

/// A trait to express the ability to consume an object and acquire ownership of
/// its raw `SOCKET`.
#[cfg_attr(staged_api, stable(feature = "into_raw_os", since = "1.4.0"))]
pub trait IntoRawSocket {
    /// Consumes this object, returning the raw underlying socket.
    ///
    /// This function is typically used to **transfer ownership** of the underlying
    /// socket to the caller. When used in this way, callers are then the unique
    /// owners of the socket and must close it once it's no longer needed.
    ///
    /// However, transferring ownership is not strictly required. Use a
    /// `Into<OwnedSocket>::into` implementation for an API which strictly
    /// transfers ownership.
    #[cfg_attr(staged_api, stable(feature = "into_raw_os", since = "1.4.0"))]
    fn into_raw_socket(self) -> RawSocket;
}
