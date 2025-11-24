use std::fmt;
use std::marker::PhantomData;
use std::mem::ManuallyDrop;
use std::ops::Deref;
#[cfg(unix)]
use std::os::unix::io::{AsFd, AsRawFd, FromRawFd};
#[cfg(windows)]
use std::os::windows::io::{AsRawSocket, AsSocket, FromRawSocket};

use crate::Socket;

/// A reference to a [`Socket`] that can be used to configure socket types other
/// than the `Socket` type itself.
///
/// This allows for example a [`TcpStream`], found in the standard library, to
/// be configured using all the additional methods found in the [`Socket`] API.
///
/// `SockRef` can be created from any socket type that implements [`AsFd`]
/// (Unix) or [`AsSocket`] (Windows) using the [`From`] implementation.
///
/// [`TcpStream`]: std::net::TcpStream
// Don't use intra-doc links because they won't build on every platform.
/// [`AsFd`]: https://doc.rust-lang.org/stable/std/os/unix/io/trait.AsFd.html
/// [`AsSocket`]: https://doc.rust-lang.org/stable/std/os/windows/io/trait.AsSocket.html
///
/// # Examples
///
/// Below is an example of converting a [`TcpStream`] into a [`SockRef`].
///
/// ```
/// use std::net::{TcpStream, SocketAddr};
///
/// use socket2::SockRef;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// // Create `TcpStream` from the standard library.
/// let address: SocketAddr = "127.0.0.1:1234".parse()?;
/// # let b1 = std::sync::Arc::new(std::sync::Barrier::new(2));
/// # let b2 = b1.clone();
/// # let handle = std::thread::spawn(move || {
/// #    let listener = std::net::TcpListener::bind(address).unwrap();
/// #    b2.wait();
/// #    let (stream, _) = listener.accept().unwrap();
/// #    std::thread::sleep(std::time::Duration::from_millis(10));
/// #    drop(stream);
/// # });
/// # b1.wait();
/// let stream = TcpStream::connect(address)?;
///
/// // Create a `SockRef`erence to the stream.
/// let socket_ref = SockRef::from(&stream);
/// // Use `Socket::set_nodelay` on the stream.
/// socket_ref.set_nodelay(true)?;
/// drop(socket_ref);
///
/// assert_eq!(stream.nodelay()?, true);
/// # handle.join().unwrap();
/// # Ok(())
/// # }
/// ```
pub struct SockRef<'s> {
    /// Because this is a reference we don't own the `Socket`, however `Socket`
    /// closes itself when dropped, so we use `ManuallyDrop` to prevent it from
    /// closing itself.
    socket: ManuallyDrop<Socket>,
    /// Because we don't own the socket we need to ensure the socket remains
    /// open while we have a "reference" to it, the lifetime `'s` ensures this.
    _lifetime: PhantomData<&'s Socket>,
}

impl<'s> Deref for SockRef<'s> {
    type Target = Socket;

    fn deref(&self) -> &Self::Target {
        &self.socket
    }
}

/// On Windows, a corresponding `From<&impl AsSocket>` implementation exists.
#[cfg(unix)]
#[cfg_attr(docsrs, doc(cfg(unix)))]
impl<'s, S> From<&'s S> for SockRef<'s>
where
    S: AsFd,
{
    /// The caller must ensure `S` is actually a socket.
    fn from(socket: &'s S) -> Self {
        let fd = socket.as_fd().as_raw_fd();
        assert!(fd >= 0);
        SockRef {
            socket: ManuallyDrop::new(unsafe { Socket::from_raw_fd(fd) }),
            _lifetime: PhantomData,
        }
    }
}

/// On Unix, a corresponding `From<&impl AsFd>` implementation exists.
#[cfg(windows)]
#[cfg_attr(docsrs, doc(cfg(windows)))]
impl<'s, S> From<&'s S> for SockRef<'s>
where
    S: AsSocket,
{
    /// See the `From<&impl AsFd>` implementation.
    fn from(socket: &'s S) -> Self {
        let socket = socket.as_socket().as_raw_socket();
        assert!(socket != windows_sys::Win32::Networking::WinSock::INVALID_SOCKET as _);
        SockRef {
            socket: ManuallyDrop::new(unsafe { Socket::from_raw_socket(socket) }),
            _lifetime: PhantomData,
        }
    }
}

impl fmt::Debug for SockRef<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SockRef")
            .field("raw", &self.socket.as_raw())
            .field("local_addr", &self.socket.local_addr().ok())
            .field("peer_addr", &self.socket.peer_addr().ok())
            .finish()
    }
}
