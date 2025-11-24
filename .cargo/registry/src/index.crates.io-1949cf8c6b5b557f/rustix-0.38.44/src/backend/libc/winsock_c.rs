//! Adapt the Winsock API to resemble a POSIX-style libc API.

#![allow(unused_imports)]
#![allow(non_camel_case_types)]
#![allow(dead_code)]

use windows_sys::Win32::Networking::WinSock;

// Define the basic C types. With Rust 1.64, we can use these from `core::ffi`.
pub(crate) type c_schar = i8;
pub(crate) type c_uchar = u8;
pub(crate) type c_short = i16;
pub(crate) type c_ushort = u16;
pub(crate) type c_int = i32;
pub(crate) type c_uint = u32;
pub(crate) type c_longlong = i64;
pub(crate) type c_ulonglong = u64;
pub(crate) type ssize_t = isize;
pub(crate) type c_char = i8;
pub(crate) type c_long = i32;
pub(crate) type c_ulong = u32;
pub(crate) use core::ffi::c_void;

// windows-sys declares these constants as u16. For better compatibility
// with Unix-family APIs, redeclare them as u32.
pub(crate) const AF_INET: i32 = WinSock::AF_INET as _;
pub(crate) const AF_INET6: i32 = WinSock::AF_INET6 as _;
pub(crate) const AF_UNSPEC: i32 = WinSock::AF_UNSPEC as _;

// Include the contents of `WinSock`, renaming as needed to match POSIX.
//
// Use `WSA_E_CANCELLED` for `ECANCELED` instead of `WSAECANCELLED`, because
// `WSAECANCELLED` will be removed in the future.
// <https://docs.microsoft.com/en-us/windows/win32/api/ws2spi/nc-ws2spi-lpnsplookupserviceend#remarks>
pub(crate) use WinSock::{
    closesocket as close, ioctlsocket as ioctl, WSAPoll as poll, ADDRESS_FAMILY as sa_family_t,
    ADDRINFOA as addrinfo, IN6_ADDR as in6_addr, IN_ADDR as in_addr, IPV6_MREQ as ipv6_mreq,
    IP_MREQ as ip_mreq, LINGER as linger, SD_BOTH as SHUT_RDWR, SD_RECEIVE as SHUT_RD,
    SD_SEND as SHUT_WR, SOCKADDR as sockaddr, SOCKADDR_IN as sockaddr_in,
    SOCKADDR_IN6 as sockaddr_in6, SOCKADDR_STORAGE as sockaddr_storage, WSAEACCES as EACCES,
    WSAEADDRINUSE as EADDRINUSE, WSAEADDRNOTAVAIL as EADDRNOTAVAIL,
    WSAEAFNOSUPPORT as EAFNOSUPPORT, WSAEALREADY as EALREADY, WSAEBADF as EBADF,
    WSAECONNABORTED as ECONNABORTED, WSAECONNREFUSED as ECONNREFUSED, WSAECONNRESET as ECONNRESET,
    WSAEDESTADDRREQ as EDESTADDRREQ, WSAEDISCON as EDISCON, WSAEDQUOT as EDQUOT,
    WSAEFAULT as EFAULT, WSAEHOSTDOWN as EHOSTDOWN, WSAEHOSTUNREACH as EHOSTUNREACH,
    WSAEINPROGRESS as EINPROGRESS, WSAEINTR as EINTR, WSAEINVAL as EINVAL,
    WSAEINVALIDPROCTABLE as EINVALIDPROCTABLE, WSAEINVALIDPROVIDER as EINVALIDPROVIDER,
    WSAEISCONN as EISCONN, WSAELOOP as ELOOP, WSAEMFILE as EMFILE, WSAEMSGSIZE as EMSGSIZE,
    WSAENAMETOOLONG as ENAMETOOLONG, WSAENETDOWN as ENETDOWN, WSAENETRESET as ENETRESET,
    WSAENETUNREACH as ENETUNREACH, WSAENOBUFS as ENOBUFS, WSAENOMORE as ENOMORE,
    WSAENOPROTOOPT as ENOPROTOOPT, WSAENOTCONN as ENOTCONN, WSAENOTEMPTY as ENOTEMPTY,
    WSAENOTSOCK as ENOTSOCK, WSAEOPNOTSUPP as EOPNOTSUPP, WSAEPFNOSUPPORT as EPFNOSUPPORT,
    WSAEPROCLIM as EPROCLIM, WSAEPROTONOSUPPORT as EPROTONOSUPPORT, WSAEPROTOTYPE as EPROTOTYPE,
    WSAEPROVIDERFAILEDINIT as EPROVIDERFAILEDINIT, WSAEREFUSED as EREFUSED, WSAEREMOTE as EREMOTE,
    WSAESHUTDOWN as ESHUTDOWN, WSAESOCKTNOSUPPORT as ESOCKTNOSUPPORT, WSAESTALE as ESTALE,
    WSAETIMEDOUT as ETIMEDOUT, WSAETOOMANYREFS as ETOOMANYREFS, WSAEUSERS as EUSERS,
    WSAEWOULDBLOCK as EWOULDBLOCK, WSAEWOULDBLOCK as EAGAIN, WSAPOLLFD as pollfd,
    WSA_E_CANCELLED as ECANCELED, *,
};

// Windows doesn't have `timespec`, just `timeval`. Rustix only uses `timespec`
// in its public API. So define one, and we'll convert it internally.
pub struct timespec {
    pub tv_sec: time_t,
    pub tv_nsec: i64,
}
pub type time_t = i64;
