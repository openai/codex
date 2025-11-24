//! Networking primitives.
//!
//! The types provided in this module are non-blocking by default and are
//! designed to be portable across all supported Mio platforms. As long as the
//! [portability guidelines] are followed, the behavior should be identical no
//! matter the target platform.
//!
//! [portability guidelines]: ../struct.Poll.html#portability
//!
//! # Notes
//!
//! When using a datagram based socket, i.e. [`UdpSocket`] or [`UnixDatagram`],
//! its only possible to receive a packet once. This means that if you provide a
//! buffer that is too small you won't be able to receive the data anymore. How
//! OSs deal with this situation is different for each OS:
//!  * Unixes, such as Linux, FreeBSD and macOS, will simply fill the buffer and
//!    return the amount of bytes written. This means that if the returned value
//!    is equal to the size of the buffer it may have only written a part of the
//!    packet (or the packet has the same size as the buffer).
//!  * Windows returns an `WSAEMSGSIZE` error.
//!
//! Mio does not change the value (either ok or error) returned by the OS, it's
//! up to the user handle this. How to deal with these difference is still up
//! for debate, specifically in
//! <https://github.com/rust-lang/rust/issues/55794>. The best advice we can
//! give is to always call receive with a large enough buffer.

mod tcp;
pub use self::tcp::{TcpListener, TcpStream};

#[cfg(not(target_os = "wasi"))]
mod udp;
#[cfg(not(target_os = "wasi"))]
pub use self::udp::UdpSocket;

#[cfg(unix)]
mod uds;
#[cfg(unix)]
pub use self::uds::{UnixDatagram, UnixListener, UnixStream};
