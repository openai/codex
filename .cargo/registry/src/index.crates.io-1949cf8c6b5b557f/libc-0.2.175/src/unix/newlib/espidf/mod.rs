use crate::prelude::*;

pub type clock_t = c_ulong;
pub type wchar_t = u32;

s! {
    pub struct cmsghdr {
        pub cmsg_len: crate::socklen_t,
        pub cmsg_level: c_int,
        pub cmsg_type: c_int,
    }

    pub struct msghdr {
        pub msg_name: *mut c_void,
        pub msg_namelen: crate::socklen_t,
        pub msg_iov: *mut crate::iovec,
        pub msg_iovlen: c_int,
        pub msg_control: *mut c_void,
        pub msg_controllen: crate::socklen_t,
        pub msg_flags: c_int,
    }

    pub struct sockaddr_un {
        pub sun_family: crate::sa_family_t,
        pub sun_path: [c_char; 108],
    }

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

    pub struct sockaddr_storage {
        pub s2_len: u8,
        pub ss_family: crate::sa_family_t,
        pub s2_data1: [c_char; 2],
        pub s2_data2: [u32; 3],
        pub s2_data3: [u32; 3],
    }
}

pub const AF_UNIX: c_int = 1;
pub const AF_INET6: c_int = 10;

pub const FIONBIO: c_ulong = 2147772030;

pub const POLLIN: c_short = 1 << 0;
pub const POLLRDNORM: c_short = 1 << 1;
pub const POLLRDBAND: c_short = 1 << 2;
pub const POLLPRI: c_short = POLLRDBAND;
pub const POLLOUT: c_short = 1 << 3;
pub const POLLWRNORM: c_short = POLLOUT;
pub const POLLWRBAND: c_short = 1 << 4;
pub const POLLERR: c_short = 1 << 5;
pub const POLLHUP: c_short = 1 << 6;

pub const SOL_SOCKET: c_int = 0xfff;

pub const MSG_OOB: c_int = 0x04;
pub const MSG_PEEK: c_int = 0x01;
pub const MSG_DONTWAIT: c_int = 0x08;
pub const MSG_DONTROUTE: c_int = 0x4;
pub const MSG_WAITALL: c_int = 0x02;
pub const MSG_MORE: c_int = 0x10;
pub const MSG_NOSIGNAL: c_int = 0x20;
pub const MSG_TRUNC: c_int = 0x04;
pub const MSG_CTRUNC: c_int = 0x08;
pub const MSG_EOR: c_int = 0x08;

pub const PTHREAD_STACK_MIN: size_t = 768;

pub const SIGABRT: c_int = 6;
pub const SIGFPE: c_int = 8;
pub const SIGILL: c_int = 4;
pub const SIGINT: c_int = 2;
pub const SIGSEGV: c_int = 11;
pub const SIGTERM: c_int = 15;
pub const SIGHUP: c_int = 1;
pub const SIGQUIT: c_int = 3;
pub const NSIG: size_t = 32;

extern "C" {
    pub fn pthread_create(
        native: *mut crate::pthread_t,
        attr: *const crate::pthread_attr_t,
        f: extern "C" fn(_: *mut c_void) -> *mut c_void,
        value: *mut c_void,
    ) -> c_int;

    pub fn getrandom(buf: *mut c_void, buflen: size_t, flags: c_uint) -> ssize_t;

    pub fn gethostname(name: *mut c_char, namelen: ssize_t);

    #[link_name = "lwip_sendmsg"]
    pub fn sendmsg(s: c_int, msg: *const crate::msghdr, flags: c_int) -> ssize_t;
    #[link_name = "lwip_recvmsg"]
    pub fn recvmsg(s: c_int, msg: *mut crate::msghdr, flags: c_int) -> ssize_t;

    pub fn eventfd(initval: c_uint, flags: c_int) -> c_int;
}

pub use crate::unix::newlib::generic::{dirent, sigset_t, stat};
