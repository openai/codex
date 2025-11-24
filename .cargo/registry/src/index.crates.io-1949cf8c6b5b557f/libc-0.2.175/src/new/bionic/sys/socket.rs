//! Header: `bionic/libc/include/sys/socket.h`

use crate::prelude::*;

s! {
    pub struct msghdr {
        pub msg_name: *mut c_void,
        pub msg_namelen: crate::socklen_t,
        pub msg_iov: *mut crate::iovec,
        pub msg_iovlen: size_t,
        pub msg_control: *mut c_void,
        pub msg_controllen: size_t,
        pub msg_flags: c_int,
    }

    pub struct cmsghdr {
        pub cmsg_len: size_t,
        pub cmsg_level: c_int,
        pub cmsg_type: c_int,
    }

    pub struct ucred {
        pub pid: crate::pid_t,
        pub uid: crate::uid_t,
        pub gid: crate::gid_t,
    }
}

extern "C" {
    pub fn recvmmsg(
        sockfd: c_int,
        msgvec: *mut crate::mmsghdr,
        vlen: c_uint,
        flags: c_int,
        timeout: *const crate::timespec,
    ) -> c_int;
    pub fn sendmmsg(
        sockfd: c_int,
        msgvec: *const crate::mmsghdr,
        vlen: c_uint,
        flags: c_int,
    ) -> c_int;
    pub fn recvfrom(
        socket: c_int,
        buf: *mut c_void,
        len: size_t,
        flags: c_int,
        addr: *mut crate::sockaddr,
        addrlen: *mut crate::socklen_t,
    ) -> ssize_t;
}
