// Copyright 2015 The Rust Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Utilities for creating and using sockets.
//!
//! The goal of this crate is to create and use a socket using advanced
//! configuration options (those that are not available in the types in the
//! standard library) without using any unsafe code.
//!
//! This crate provides as direct as possible access to the system's
//! functionality for sockets, this means little effort to provide
//! cross-platform utilities. It is up to the user to know how to use sockets
//! when using this crate. *If you don't know how to create a socket using
//! libc/system calls then this crate is not for you*. Most, if not all,
//! functions directly relate to the equivalent system call with no error
//! handling applied, so no handling errors such as [`EINTR`]. As a result using
//! this crate can be a little wordy, but it should give you maximal flexibility
//! over configuration of sockets.
//!
//! [`EINTR`]: std::io::ErrorKind::Interrupted
//!
//! # Examples
//!
//! ```no_run
//! # fn main() -> std::io::Result<()> {
//! use std::net::{SocketAddr, TcpListener};
//! use socket2::{Socket, Domain, Type};
//!
//! // Create a TCP listener bound to two addresses.
//! let socket = Socket::new(Domain::IPV6, Type::STREAM, None)?;
//!
//! socket.set_only_v6(false)?;
//! let address: SocketAddr = "[::1]:12345".parse().unwrap();
//! socket.bind(&address.into())?;
//! socket.listen(128)?;
//!
//! let listener: TcpListener = socket.into();
//! // ...
//! # drop(listener);
//! # Ok(()) }
//! ```
//!
//! ## Features
//!
//! This crate has a single feature `all`, which enables all functions even ones
//! that are not available on all OSs.

#![deny(missing_docs, missing_debug_implementations, rust_2018_idioms)]
// Show required OS/features on docs.rs.
#![cfg_attr(docsrs, feature(doc_cfg))]
// Disallow warnings when running tests.
#![cfg_attr(test, deny(warnings))]
// Disallow warnings in examples.
#![doc(test(attr(deny(warnings))))]

use std::fmt;
#[cfg(not(target_os = "redox"))]
use std::io::IoSlice;
#[cfg(not(target_os = "redox"))]
use std::marker::PhantomData;
#[cfg(not(target_os = "redox"))]
use std::mem;
use std::mem::MaybeUninit;
use std::net::SocketAddr;
use std::ops::{Deref, DerefMut};
use std::time::Duration;

/// Macro to implement `fmt::Debug` for a type, printing the constant names
/// rather than a number.
///
/// Note this is used in the `sys` module and thus must be defined before
/// defining the modules.
macro_rules! impl_debug {
    (
        // Type name for which to implement `fmt::Debug`.
        $type: path,
        $(
            $(#[$target: meta])*
            // The flag(s) to check.
            // Need to specific the libc crate because Windows doesn't use
            // `libc` but `windows_sys`.
            $libc: ident :: $flag: ident
        ),+ $(,)*
    ) => {
        impl std::fmt::Debug for $type {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                let string = match self.0 {
                    $(
                        $(#[$target])*
                        $libc :: $flag => stringify!($flag),
                    )+
                    n => return write!(f, "{n}"),
                };
                f.write_str(string)
            }
        }
    };
}

/// Macro to convert from one network type to another.
macro_rules! from {
    ($from: ty, $for: ty) => {
        impl From<$from> for $for {
            fn from(socket: $from) -> $for {
                #[cfg(unix)]
                unsafe {
                    <$for>::from_raw_fd(socket.into_raw_fd())
                }
                #[cfg(windows)]
                unsafe {
                    <$for>::from_raw_socket(socket.into_raw_socket())
                }
            }
        }
    };
}

/// Link to online documentation for (almost) all supported OSs.
#[rustfmt::skip]
macro_rules! man_links {
    // Links to all OSs.
    ($syscall: tt ( $section: tt ) ) => {
        concat!(
            man_links!(__ intro),
            man_links!(__ unix $syscall($section)),
            man_links!(__ windows $syscall($section)),
        )
    };
    // Links to Unix-like OSs.
    (unix: $syscall: tt ( $section: tt ) ) => {
        concat!(
            man_links!(__ intro),
            man_links!(__ unix $syscall($section)),
        )
    };
    // Links to Windows only.
    (windows: $syscall: tt ( $section: tt ) ) => {
        concat!(
            man_links!(__ intro),
            man_links!(__ windows $syscall($section)),
        )
    };
    // Internals.
    (__ intro) => {
        "\n\nAdditional documentation can be found in manual of the OS:\n\n"
    };
    // List for Unix-like OSs.
    (__ unix $syscall: tt ( $section: tt ) ) => {
        concat!(
            " * DragonFly BSD: <https://man.dragonflybsd.org/?command=", stringify!($syscall), "&section=", stringify!($section), ">\n",
            " * FreeBSD: <https://www.freebsd.org/cgi/man.cgi?query=", stringify!($syscall), "&sektion=", stringify!($section), ">\n",
            " * Linux: <https://man7.org/linux/man-pages/man", stringify!($section), "/", stringify!($syscall), ".", stringify!($section), ".html>\n",
            " * macOS: <https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/", stringify!($syscall), ".", stringify!($section), ".html> (archived, actually for iOS)\n",
            " * NetBSD: <https://man.netbsd.org/", stringify!($syscall), ".", stringify!($section), ">\n",
            " * OpenBSD: <https://man.openbsd.org/", stringify!($syscall), ".", stringify!($section), ">\n",
            " * iOS: <https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/", stringify!($syscall), ".", stringify!($section), ".html> (archived)\n",
            " * illumos: <https://illumos.org/man/3SOCKET/", stringify!($syscall), ">\n",
        )
    };
    // List for Window (so just Windows).
    (__ windows $syscall: tt ( $section: tt ) ) => {
        concat!(
            " * Windows: <https://docs.microsoft.com/en-us/windows/win32/api/winsock2/nf-winsock2-", stringify!($syscall), ">\n",
        )
    };
}

mod sockaddr;
mod socket;
mod sockref;

#[cfg_attr(unix, path = "sys/unix.rs")]
#[cfg_attr(windows, path = "sys/windows.rs")]
mod sys;

#[cfg(not(any(windows, unix)))]
compile_error!("Socket2 doesn't support the compile target");

use sys::c_int;

pub use sockaddr::SockAddr;
pub use socket::Socket;
pub use sockref::SockRef;

#[cfg(not(any(
    target_os = "haiku",
    target_os = "illumos",
    target_os = "netbsd",
    target_os = "redox",
    target_os = "solaris",
)))]
pub use socket::InterfaceIndexOrAddress;

/// Specification of the communication domain for a socket.
///
/// This is a newtype wrapper around an integer which provides a nicer API in
/// addition to an injection point for documentation. Convenience constants such
/// as [`Domain::IPV4`], [`Domain::IPV6`], etc, are provided to avoid reaching
/// into libc for various constants.
///
/// This type is freely interconvertible with C's `int` type, however, if a raw
/// value needs to be provided.
#[derive(Copy, Clone, Eq, PartialEq)]
pub struct Domain(c_int);

impl Domain {
    /// Domain for IPv4 communication, corresponding to `AF_INET`.
    pub const IPV4: Domain = Domain(sys::AF_INET);

    /// Domain for IPv6 communication, corresponding to `AF_INET6`.
    pub const IPV6: Domain = Domain(sys::AF_INET6);

    /// Domain for Unix socket communication, corresponding to `AF_UNIX`.
    pub const UNIX: Domain = Domain(sys::AF_UNIX);

    /// Returns the correct domain for `address`.
    pub const fn for_address(address: SocketAddr) -> Domain {
        match address {
            SocketAddr::V4(_) => Domain::IPV4,
            SocketAddr::V6(_) => Domain::IPV6,
        }
    }
}

impl From<c_int> for Domain {
    fn from(d: c_int) -> Domain {
        Domain(d)
    }
}

impl From<Domain> for c_int {
    fn from(d: Domain) -> c_int {
        d.0
    }
}

/// Specification of communication semantics on a socket.
///
/// This is a newtype wrapper around an integer which provides a nicer API in
/// addition to an injection point for documentation. Convenience constants such
/// as [`Type::STREAM`], [`Type::DGRAM`], etc, are provided to avoid reaching
/// into libc for various constants.
///
/// This type is freely interconvertible with C's `int` type, however, if a raw
/// value needs to be provided.
#[derive(Copy, Clone, Eq, PartialEq)]
pub struct Type(c_int);

impl Type {
    /// Type corresponding to `SOCK_STREAM`.
    ///
    /// Used for protocols such as TCP.
    pub const STREAM: Type = Type(sys::SOCK_STREAM);

    /// Type corresponding to `SOCK_DGRAM`.
    ///
    /// Used for protocols such as UDP.
    pub const DGRAM: Type = Type(sys::SOCK_DGRAM);

    /// Type corresponding to `SOCK_DCCP`.
    ///
    /// Used for the DCCP protocol.
    #[cfg(all(feature = "all", target_os = "linux"))]
    #[cfg_attr(docsrs, doc(cfg(all(feature = "all", target_os = "linux"))))]
    pub const DCCP: Type = Type(sys::SOCK_DCCP);

    /// Type corresponding to `SOCK_SEQPACKET`.
    #[cfg(all(feature = "all", not(target_os = "espidf")))]
    #[cfg_attr(docsrs, doc(cfg(all(feature = "all", not(target_os = "espidf")))))]
    pub const SEQPACKET: Type = Type(sys::SOCK_SEQPACKET);

    /// Type corresponding to `SOCK_RAW`.
    #[cfg(all(feature = "all", not(any(target_os = "redox", target_os = "espidf"))))]
    #[cfg_attr(
        docsrs,
        doc(cfg(all(feature = "all", not(any(target_os = "redox", target_os = "espidf")))))
    )]
    pub const RAW: Type = Type(sys::SOCK_RAW);
}

impl From<c_int> for Type {
    fn from(t: c_int) -> Type {
        Type(t)
    }
}

impl From<Type> for c_int {
    fn from(t: Type) -> c_int {
        t.0
    }
}

/// Protocol specification used for creating sockets via `Socket::new`.
///
/// This is a newtype wrapper around an integer which provides a nicer API in
/// addition to an injection point for documentation.
///
/// This type is freely interconvertible with C's `int` type, however, if a raw
/// value needs to be provided.
#[derive(Copy, Clone, Eq, PartialEq)]
pub struct Protocol(c_int);

impl Protocol {
    /// Protocol corresponding to `ICMPv4`.
    pub const ICMPV4: Protocol = Protocol(sys::IPPROTO_ICMP);

    /// Protocol corresponding to `ICMPv6`.
    pub const ICMPV6: Protocol = Protocol(sys::IPPROTO_ICMPV6);

    /// Protocol corresponding to `TCP`.
    pub const TCP: Protocol = Protocol(sys::IPPROTO_TCP);

    /// Protocol corresponding to `UDP`.
    pub const UDP: Protocol = Protocol(sys::IPPROTO_UDP);

    #[cfg(target_os = "linux")]
    /// Protocol corresponding to `MPTCP`.
    pub const MPTCP: Protocol = Protocol(sys::IPPROTO_MPTCP);

    /// Protocol corresponding to `DCCP`.
    #[cfg(all(feature = "all", target_os = "linux"))]
    #[cfg_attr(docsrs, doc(cfg(all(feature = "all", target_os = "linux"))))]
    pub const DCCP: Protocol = Protocol(sys::IPPROTO_DCCP);

    /// Protocol corresponding to `SCTP`.
    #[cfg(all(feature = "all", any(target_os = "freebsd", target_os = "linux")))]
    pub const SCTP: Protocol = Protocol(sys::IPPROTO_SCTP);

    /// Protocol corresponding to `UDPLITE`.
    #[cfg(all(
        feature = "all",
        any(
            target_os = "android",
            target_os = "freebsd",
            target_os = "fuchsia",
            target_os = "linux",
        )
    ))]
    pub const UDPLITE: Protocol = Protocol(sys::IPPROTO_UDPLITE);

    /// Protocol corresponding to `DIVERT`.
    #[cfg(all(feature = "all", any(target_os = "freebsd", target_os = "openbsd")))]
    pub const DIVERT: Protocol = Protocol(sys::IPPROTO_DIVERT);
}

impl From<c_int> for Protocol {
    fn from(p: c_int) -> Protocol {
        Protocol(p)
    }
}

impl From<Protocol> for c_int {
    fn from(p: Protocol) -> c_int {
        p.0
    }
}

/// Flags for incoming messages.
///
/// Flags provide additional information about incoming messages.
#[cfg(not(target_os = "redox"))]
#[cfg_attr(docsrs, doc(cfg(not(target_os = "redox"))))]
#[derive(Copy, Clone, Eq, PartialEq)]
pub struct RecvFlags(c_int);

#[cfg(not(target_os = "redox"))]
impl RecvFlags {
    /// Check if the message contains a truncated datagram.
    ///
    /// This flag is only used for datagram-based sockets,
    /// not for stream sockets.
    ///
    /// On Unix this corresponds to the `MSG_TRUNC` flag.
    /// On Windows this corresponds to the `WSAEMSGSIZE` error code.
    #[cfg(not(target_os = "espidf"))]
    pub const fn is_truncated(self) -> bool {
        self.0 & sys::MSG_TRUNC != 0
    }
}

/// A version of [`IoSliceMut`] that allows the buffer to be uninitialised.
///
/// [`IoSliceMut`]: std::io::IoSliceMut
#[repr(transparent)]
pub struct MaybeUninitSlice<'a>(sys::MaybeUninitSlice<'a>);

impl<'a> fmt::Debug for MaybeUninitSlice<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.0.as_slice(), fmt)
    }
}

impl<'a> MaybeUninitSlice<'a> {
    /// Creates a new `MaybeUninitSlice` wrapping a byte slice.
    ///
    /// # Panics
    ///
    /// Panics on Windows if the slice is larger than 4GB.
    pub fn new(buf: &'a mut [MaybeUninit<u8>]) -> MaybeUninitSlice<'a> {
        MaybeUninitSlice(sys::MaybeUninitSlice::new(buf))
    }
}

impl<'a> Deref for MaybeUninitSlice<'a> {
    type Target = [MaybeUninit<u8>];

    fn deref(&self) -> &[MaybeUninit<u8>] {
        self.0.as_slice()
    }
}

impl<'a> DerefMut for MaybeUninitSlice<'a> {
    fn deref_mut(&mut self) -> &mut [MaybeUninit<u8>] {
        self.0.as_mut_slice()
    }
}

/// Configures a socket's TCP keepalive parameters.
///
/// See [`Socket::set_tcp_keepalive`].
#[derive(Debug, Clone)]
pub struct TcpKeepalive {
    #[cfg_attr(
        any(target_os = "openbsd", target_os = "haiku", target_os = "vita"),
        allow(dead_code)
    )]
    time: Option<Duration>,
    #[cfg(not(any(
        target_os = "openbsd",
        target_os = "redox",
        target_os = "solaris",
        target_os = "nto",
        target_os = "espidf",
        target_os = "vita",
        target_os = "haiku",
    )))]
    interval: Option<Duration>,
    #[cfg(not(any(
        target_os = "openbsd",
        target_os = "redox",
        target_os = "solaris",
        target_os = "windows",
        target_os = "nto",
        target_os = "espidf",
        target_os = "vita",
        target_os = "haiku",
    )))]
    retries: Option<u32>,
}

impl TcpKeepalive {
    /// Returns a new, empty set of TCP keepalive parameters.
    #[allow(clippy::new_without_default)]
    pub const fn new() -> TcpKeepalive {
        TcpKeepalive {
            time: None,
            #[cfg(not(any(
                target_os = "openbsd",
                target_os = "redox",
                target_os = "solaris",
                target_os = "nto",
                target_os = "espidf",
                target_os = "vita",
                target_os = "haiku",
            )))]
            interval: None,
            #[cfg(not(any(
                target_os = "openbsd",
                target_os = "redox",
                target_os = "solaris",
                target_os = "windows",
                target_os = "nto",
                target_os = "espidf",
                target_os = "vita",
                target_os = "haiku",
            )))]
            retries: None,
        }
    }

    /// Set the amount of time after which TCP keepalive probes will be sent on
    /// idle connections.
    ///
    /// This will set `TCP_KEEPALIVE` on macOS and iOS, and
    /// `TCP_KEEPIDLE` on all other Unix operating systems, except
    /// OpenBSD and Haiku which don't support any way to set this
    /// option. On Windows, this sets the value of the `tcp_keepalive`
    /// struct's `keepalivetime` field.
    ///
    /// Some platforms specify this value in seconds, so sub-second
    /// specifications may be omitted.
    pub const fn with_time(self, time: Duration) -> Self {
        Self {
            time: Some(time),
            ..self
        }
    }

    /// Set the value of the `TCP_KEEPINTVL` option. On Windows, this sets the
    /// value of the `tcp_keepalive` struct's `keepaliveinterval` field.
    ///
    /// Sets the time interval between TCP keepalive probes.
    ///
    /// Some platforms specify this value in seconds, so sub-second
    /// specifications may be omitted.
    #[cfg(any(
        target_os = "android",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "fuchsia",
        target_os = "illumos",
        target_os = "ios",
        target_os = "visionos",
        target_os = "linux",
        target_os = "macos",
        target_os = "netbsd",
        target_os = "tvos",
        target_os = "watchos",
        target_os = "windows",
        target_os = "cygwin",
    ))]
    #[cfg_attr(
        docsrs,
        doc(cfg(any(
            target_os = "android",
            target_os = "dragonfly",
            target_os = "freebsd",
            target_os = "fuchsia",
            target_os = "illumos",
            target_os = "ios",
            target_os = "visionos",
            target_os = "linux",
            target_os = "macos",
            target_os = "netbsd",
            target_os = "tvos",
            target_os = "watchos",
            target_os = "windows",
        )))
    )]
    pub const fn with_interval(self, interval: Duration) -> Self {
        Self {
            interval: Some(interval),
            ..self
        }
    }

    /// Set the value of the `TCP_KEEPCNT` option.
    ///
    /// Set the maximum number of TCP keepalive probes that will be sent before
    /// dropping a connection, if TCP keepalive is enabled on this socket.
    #[cfg(all(
        feature = "all",
        any(
            target_os = "android",
            target_os = "dragonfly",
            target_os = "freebsd",
            target_os = "fuchsia",
            target_os = "illumos",
            target_os = "ios",
            target_os = "visionos",
            target_os = "linux",
            target_os = "macos",
            target_os = "netbsd",
            target_os = "tvos",
            target_os = "watchos",
            target_os = "cygwin",
        )
    ))]
    #[cfg_attr(
        docsrs,
        doc(cfg(all(
            feature = "all",
            any(
                target_os = "android",
                target_os = "dragonfly",
                target_os = "freebsd",
                target_os = "fuchsia",
                target_os = "illumos",
                target_os = "ios",
                target_os = "visionos",
                target_os = "linux",
                target_os = "macos",
                target_os = "netbsd",
                target_os = "tvos",
                target_os = "watchos",
            )
        )))
    )]
    pub const fn with_retries(self, retries: u32) -> Self {
        Self {
            retries: Some(retries),
            ..self
        }
    }
}

/// Configuration of a `sendmsg(2)` system call.
///
/// This wraps `msghdr` on Unix and `WSAMSG` on Windows. Also see [`MsgHdrMut`]
/// for the variant used by `recvmsg(2)`.
#[cfg(not(target_os = "redox"))]
pub struct MsgHdr<'addr, 'bufs, 'control> {
    inner: sys::msghdr,
    #[allow(clippy::type_complexity)]
    _lifetimes: PhantomData<(&'addr SockAddr, &'bufs IoSlice<'bufs>, &'control [u8])>,
}

#[cfg(not(target_os = "redox"))]
impl<'addr, 'bufs, 'control> MsgHdr<'addr, 'bufs, 'control> {
    /// Create a new `MsgHdr` with all empty/zero fields.
    #[allow(clippy::new_without_default)]
    pub fn new() -> MsgHdr<'addr, 'bufs, 'control> {
        // SAFETY: all zero is valid for `msghdr` and `WSAMSG`.
        MsgHdr {
            inner: unsafe { mem::zeroed() },
            _lifetimes: PhantomData,
        }
    }

    /// Set the address (name) of the message.
    ///
    /// Corresponds to setting `msg_name` and `msg_namelen` on Unix and `name`
    /// and `namelen` on Windows.
    pub fn with_addr(mut self, addr: &'addr SockAddr) -> Self {
        sys::set_msghdr_name(&mut self.inner, addr);
        self
    }

    /// Set the buffer(s) of the message.
    ///
    /// Corresponds to setting `msg_iov` and `msg_iovlen` on Unix and `lpBuffers`
    /// and `dwBufferCount` on Windows.
    pub fn with_buffers(mut self, bufs: &'bufs [IoSlice<'_>]) -> Self {
        let ptr = bufs.as_ptr() as *mut _;
        sys::set_msghdr_iov(&mut self.inner, ptr, bufs.len());
        self
    }

    /// Set the control buffer of the message.
    ///
    /// Corresponds to setting `msg_control` and `msg_controllen` on Unix and
    /// `Control` on Windows.
    pub fn with_control(mut self, buf: &'control [u8]) -> Self {
        let ptr = buf.as_ptr() as *mut _;
        sys::set_msghdr_control(&mut self.inner, ptr, buf.len());
        self
    }

    /// Set the flags of the message.
    ///
    /// Corresponds to setting `msg_flags` on Unix and `dwFlags` on Windows.
    pub fn with_flags(mut self, flags: sys::c_int) -> Self {
        sys::set_msghdr_flags(&mut self.inner, flags);
        self
    }
}

#[cfg(not(target_os = "redox"))]
impl<'name, 'bufs, 'control> fmt::Debug for MsgHdr<'name, 'bufs, 'control> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        "MsgHdr".fmt(fmt)
    }
}

/// Configuration of a `recvmsg(2)` system call.
///
/// This wraps `msghdr` on Unix and `WSAMSG` on Windows. Also see [`MsgHdr`] for
/// the variant used by `sendmsg(2)`.
#[cfg(not(target_os = "redox"))]
pub struct MsgHdrMut<'addr, 'bufs, 'control> {
    inner: sys::msghdr,
    #[allow(clippy::type_complexity)]
    _lifetimes: PhantomData<(
        &'addr mut SockAddr,
        &'bufs mut MaybeUninitSlice<'bufs>,
        &'control mut [u8],
    )>,
}

#[cfg(not(target_os = "redox"))]
impl<'addr, 'bufs, 'control> MsgHdrMut<'addr, 'bufs, 'control> {
    /// Create a new `MsgHdrMut` with all empty/zero fields.
    #[allow(clippy::new_without_default)]
    pub fn new() -> MsgHdrMut<'addr, 'bufs, 'control> {
        // SAFETY: all zero is valid for `msghdr` and `WSAMSG`.
        MsgHdrMut {
            inner: unsafe { mem::zeroed() },
            _lifetimes: PhantomData,
        }
    }

    /// Set the mutable address (name) of the message.
    ///
    /// Corresponds to setting `msg_name` and `msg_namelen` on Unix and `name`
    /// and `namelen` on Windows.
    #[allow(clippy::needless_pass_by_ref_mut)]
    pub fn with_addr(mut self, addr: &'addr mut SockAddr) -> Self {
        sys::set_msghdr_name(&mut self.inner, addr);
        self
    }

    /// Set the mutable buffer(s) of the message.
    ///
    /// Corresponds to setting `msg_iov` and `msg_iovlen` on Unix and `lpBuffers`
    /// and `dwBufferCount` on Windows.
    pub fn with_buffers(mut self, bufs: &'bufs mut [MaybeUninitSlice<'_>]) -> Self {
        sys::set_msghdr_iov(&mut self.inner, bufs.as_mut_ptr().cast(), bufs.len());
        self
    }

    /// Set the mutable control buffer of the message.
    ///
    /// Corresponds to setting `msg_control` and `msg_controllen` on Unix and
    /// `Control` on Windows.
    pub fn with_control(mut self, buf: &'control mut [MaybeUninit<u8>]) -> Self {
        sys::set_msghdr_control(&mut self.inner, buf.as_mut_ptr().cast(), buf.len());
        self
    }

    /// Returns the flags of the message.
    pub fn flags(&self) -> RecvFlags {
        sys::msghdr_flags(&self.inner)
    }

    /// Gets the length of the control buffer.
    ///
    /// Can be used to determine how much, if any, of the control buffer was filled by `recvmsg`.
    ///
    /// Corresponds to `msg_controllen` on Unix and `Control.len` on Windows.
    pub fn control_len(&self) -> usize {
        sys::msghdr_control_len(&self.inner)
    }
}

#[cfg(not(target_os = "redox"))]
impl<'name, 'bufs, 'control> fmt::Debug for MsgHdrMut<'name, 'bufs, 'control> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        "MsgHdrMut".fmt(fmt)
    }
}
