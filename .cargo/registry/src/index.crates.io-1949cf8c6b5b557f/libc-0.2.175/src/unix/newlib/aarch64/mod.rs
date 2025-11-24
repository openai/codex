use crate::prelude::*;

pub type clock_t = c_long;
pub type wchar_t = u32;

s! {
    pub struct sockaddr {
        pub sa_len: u8,
        pub sa_family: crate::sa_family_t,
        pub sa_data: [c_char; 14],
    }

    pub struct sockaddr_in6 {
        pub sin6_len: u8,
        pub sin6_family: crate::sa_family_t,
        pub sin6_port: crate::in_port_t,
        pub sin6_flowinfo: u32,
        pub sin6_addr: crate::in6_addr,
        pub sin6_scope_id: u32,
    }

    pub struct sockaddr_in {
        pub sin_len: u8,
        pub sin_family: crate::sa_family_t,
        pub sin_port: crate::in_port_t,
        pub sin_addr: crate::in_addr,
        pub sin_zero: [c_char; 8],
    }
}

pub const AF_INET6: c_int = 23;

pub const FIONBIO: c_ulong = 1;

pub const POLLIN: c_short = 0x1;
pub const POLLPRI: c_short = 0x2;
pub const POLLOUT: c_short = 0x4;
pub const POLLERR: c_short = 0x8;
pub const POLLHUP: c_short = 0x10;
pub const POLLNVAL: c_short = 0x20;

pub const SOL_SOCKET: c_int = 65535;

pub const MSG_OOB: c_int = 1;
pub const MSG_PEEK: c_int = 2;
pub const MSG_DONTWAIT: c_int = 4;
pub const MSG_DONTROUTE: c_int = 0;
pub const MSG_WAITALL: c_int = 0;
pub const MSG_MORE: c_int = 0;
pub const MSG_NOSIGNAL: c_int = 0;

pub use crate::unix::newlib::generic::{dirent, sigset_t, stat};
