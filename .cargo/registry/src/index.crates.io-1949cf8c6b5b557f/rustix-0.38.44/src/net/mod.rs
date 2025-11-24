//! Network-related operations.
//!
//! On Windows, one must call [`wsa_startup`] in the process before calling any
//! of these APIs. [`wsa_cleanup`] may be used in the process if these APIs are
//! no longer needed.
//!
//! [`wsa_startup`]: https://docs.rs/rustix/*/x86_64-pc-windows-msvc/rustix/net/fn.wsa_startup.html
//! [`wsa_cleanup`]: https://docs.rs/rustix/*/x86_64-pc-windows-msvc/rustix/net/fn.wsa_cleanup.html

mod send_recv;
mod socket;
mod socket_addr_any;
#[cfg(not(any(windows, target_os = "wasi")))]
mod socketpair;
mod types;
#[cfg(windows)]
mod wsa;

#[cfg(linux_kernel)]
pub mod netdevice;
pub mod sockopt;

pub use crate::maybe_polyfill::net::{
    IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6,
};
pub use send_recv::*;
pub use socket::*;
pub use socket_addr_any::{SocketAddrAny, SocketAddrStorage};
#[cfg(not(any(windows, target_os = "wasi")))]
pub use socketpair::socketpair;
pub use types::*;
#[cfg(windows)]
pub use wsa::{wsa_cleanup, wsa_startup};
