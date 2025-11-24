//! Hermit C type definitions

use crate::prelude::*;

pub type intmax_t = i64;
pub type uintmax_t = u64;
pub type intptr_t = isize;
pub type uintptr_t = usize;

pub type size_t = usize;
pub type ssize_t = isize;
pub type ptrdiff_t = isize;

pub type clockid_t = i32;
pub type in_addr_t = u32;
pub type in_port_t = u16;
pub type mode_t = u32;
pub type nfds_t = usize;
pub type pid_t = i32;
pub type sa_family_t = u8;
pub type socklen_t = u32;
pub type time_t = i64;

s! {
    pub struct addrinfo {
        pub ai_flags: i32,
        pub ai_family: i32,
        pub ai_socktype: i32,
        pub ai_protocol: i32,
        pub ai_addrlen: socklen_t,
        pub ai_canonname: *mut c_char,
        pub ai_addr: *mut sockaddr,
        pub ai_next: *mut addrinfo,
    }

    pub struct dirent64 {
        pub d_ino: u64,
        pub d_off: i64,
        pub d_reclen: u16,
        pub d_type: u8,
        pub d_name: [c_char; 256],
    }

    #[repr(align(4))]
    pub struct in6_addr {
        pub s6_addr: [u8; 16],
    }

    pub struct in_addr {
        pub s_addr: in_addr_t,
    }

    pub struct iovec {
        iov_base: *mut c_void,
        iov_len: usize,
    }

    pub struct pollfd {
        pub fd: i32,
        pub events: i16,
        pub revents: i16,
    }

    pub struct sockaddr {
        pub sa_len: u8,
        pub sa_family: sa_family_t,
        pub sa_data: [c_char; 14],
    }

    pub struct sockaddr_in {
        pub sin_len: u8,
        pub sin_family: sa_family_t,
        pub sin_port: in_port_t,
        pub sin_addr: in_addr,
        pub sin_zero: [c_char; 8],
    }

    pub struct sockaddr_in6 {
        pub sin6_len: u8,
        pub sin6_family: sa_family_t,
        pub sin6_port: in_port_t,
        pub sin6_flowinfo: u32,
        pub sin6_addr: in6_addr,
        pub sin6_scope_id: u32,
    }

    pub struct sockaddr_storage {
        pub ss_len: u8,
        pub ss_family: sa_family_t,
        __ss_pad1: [u8; 6],
        __ss_align: i64,
        __ss_pad2: [u8; 112],
    }

    pub struct stat {
        pub st_dev: u64,
        pub st_ino: u64,
        pub st_nlink: u64,
        pub st_mode: mode_t,
        pub st_uid: u32,
        pub st_gid: u32,
        pub st_rdev: u64,
        pub st_size: i64,
        pub st_blksize: i64,
        pub st_blocks: i64,
        pub st_atim: timespec,
        pub st_mtim: timespec,
        pub st_ctim: timespec,
    }

    pub struct timespec {
        pub tv_sec: time_t,
        pub tv_nsec: i32,
    }
}

pub const AF_UNSPEC: i32 = 0;
pub const AF_INET: i32 = 3;
pub const AF_INET6: i32 = 1;
pub const AF_VSOCK: i32 = 2;

pub const CLOCK_REALTIME: clockid_t = 1;
pub const CLOCK_MONOTONIC: clockid_t = 4;

pub const DT_UNKNOWN: u8 = 0;
pub const DT_FIFO: u8 = 1;
pub const DT_CHR: u8 = 2;
pub const DT_DIR: u8 = 4;
pub const DT_BLK: u8 = 6;
pub const DT_REG: u8 = 8;
pub const DT_LNK: u8 = 10;
pub const DT_SOCK: u8 = 12;
pub const DT_WHT: u8 = 14;

pub const EAI_AGAIN: i32 = 2;
pub const EAI_BADFLAGS: i32 = 3;
pub const EAI_FAIL: i32 = 4;
pub const EAI_FAMILY: i32 = 5;
pub const EAI_MEMORY: i32 = 6;
pub const EAI_NODATA: i32 = 7;
pub const EAI_NONAME: i32 = 8;
pub const EAI_SERVICE: i32 = 9;
pub const EAI_SOCKTYPE: i32 = 10;
pub const EAI_SYSTEM: i32 = 11;
pub const EAI_OVERFLOW: i32 = 14;

pub const EFD_SEMAPHORE: i16 = 0o1;
pub const EFD_NONBLOCK: i16 = 0o4000;
pub const EFD_CLOEXEC: i16 = 0o40000;

pub const F_DUPFD: i32 = 0;
pub const F_GETFD: i32 = 1;
pub const F_SETFD: i32 = 2;
pub const F_GETFL: i32 = 3;
pub const F_SETFL: i32 = 4;

pub const FD_CLOEXEC: i32 = 1;

pub const FIONBIO: i32 = 0x8008667e;

pub const FUTEX_RELATIVE_TIMEOUT: u32 = 1;

pub const IP_TOS: i32 = 1;
pub const IP_TTL: i32 = 2;
pub const IP_ADD_MEMBERSHIP: i32 = 3;
pub const IP_DROP_MEMBERSHIP: i32 = 4;
pub const IP_MULTICAST_TTL: i32 = 5;
pub const IP_MULTICAST_LOOP: i32 = 7;

pub const IPPROTO_IP: i32 = 0;
pub const IPPROTO_TCP: i32 = 6;
pub const IPPROTO_UDP: i32 = 17;
pub const IPPROTO_IPV6: i32 = 41;

pub const IPV6_ADD_MEMBERSHIP: i32 = 12;
pub const IPV6_DROP_MEMBERSHIP: i32 = 13;
pub const IPV6_MULTICAST_LOOP: i32 = 19;
pub const IPV6_V6ONLY: i32 = 27;

pub const MSG_PEEK: i32 = 1;

pub const O_RDONLY: i32 = 0o0;
pub const O_WRONLY: i32 = 0o1;
pub const O_RDWR: i32 = 0o2;
pub const O_CREAT: i32 = 0o100;
pub const O_EXCL: i32 = 0o200;
pub const O_TRUNC: i32 = 0o1000;
pub const O_APPEND: i32 = 0o2000;
pub const O_NONBLOCK: i32 = 0o4000;
pub const O_DIRECTORY: i32 = 0o200000;

pub const POLLIN: i16 = 0x1;
pub const POLLPRI: i16 = 0x2;
pub const POLLOUT: i16 = 0x4;
pub const POLLERR: i16 = 0x8;
pub const POLLHUP: i16 = 0x10;
pub const POLLNVAL: i16 = 0x20;
pub const POLLRDNORM: i16 = 0x040;
pub const POLLRDBAND: i16 = 0x080;
pub const POLLWRNORM: i16 = 0x0100;
pub const POLLWRBAND: i16 = 0x0200;
pub const POLLRDHUP: i16 = 0x2000;

pub const S_IRWXU: mode_t = 0o0700;
pub const S_IRUSR: mode_t = 0o0400;
pub const S_IWUSR: mode_t = 0o0200;
pub const S_IXUSR: mode_t = 0o0100;
pub const S_IRWXG: mode_t = 0o0070;
pub const S_IRGRP: mode_t = 0o0040;
pub const S_IWGRP: mode_t = 0o0020;
pub const S_IXGRP: mode_t = 0o0010;
pub const S_IRWXO: mode_t = 0o0007;
pub const S_IROTH: mode_t = 0o0004;
pub const S_IWOTH: mode_t = 0o0002;
pub const S_IXOTH: mode_t = 0o0001;

pub const S_IFMT: mode_t = 0o17_0000;
pub const S_IFSOCK: mode_t = 0o14_0000;
pub const S_IFLNK: mode_t = 0o12_0000;
pub const S_IFREG: mode_t = 0o10_0000;
pub const S_IFBLK: mode_t = 0o6_0000;
pub const S_IFDIR: mode_t = 0o4_0000;
pub const S_IFCHR: mode_t = 0o2_0000;
pub const S_IFIFO: mode_t = 0o1_0000;

pub const SHUT_RD: i32 = 0;
pub const SHUT_WR: i32 = 1;
pub const SHUT_RDWR: i32 = 2;

pub const SO_REUSEADDR: i32 = 0x0004;
pub const SO_KEEPALIVE: i32 = 0x0008;
pub const SO_BROADCAST: i32 = 0x0020;
pub const SO_LINGER: i32 = 0x0080;
pub const SO_SNDBUF: i32 = 0x1001;
pub const SO_RCVBUF: i32 = 0x1002;
pub const SO_SNDTIMEO: i32 = 0x1005;
pub const SO_RCVTIMEO: i32 = 0x1006;
pub const SO_ERROR: i32 = 0x1007;

pub const SOCK_STREAM: i32 = 1;
pub const SOCK_DGRAM: i32 = 2;
pub const SOCK_NONBLOCK: i32 = 0o4000;
pub const SOCK_CLOEXEC: i32 = 0o40000;

pub const SOL_SOCKET: i32 = 4095;

pub const STDIN_FILENO: c_int = 0;
pub const STDOUT_FILENO: c_int = 1;
pub const STDERR_FILENO: c_int = 2;

pub const TCP_NODELAY: i32 = 1;

pub const EPERM: i32 = 1;
pub const ENOENT: i32 = 2;
pub const ESRCH: i32 = 3;
pub const EINTR: i32 = 4;
pub const EIO: i32 = 5;
pub const ENXIO: i32 = 6;
pub const E2BIG: i32 = 7;
pub const ENOEXEC: i32 = 8;
pub const EBADF: i32 = 9;
pub const ECHILD: i32 = 10;
pub const EAGAIN: i32 = 11;
pub const ENOMEM: i32 = 12;
pub const EACCES: i32 = 13;
pub const EFAULT: i32 = 14;
pub const ENOTBLK: i32 = 15;
pub const EBUSY: i32 = 16;
pub const EEXIST: i32 = 17;
pub const EXDEV: i32 = 18;
pub const ENODEV: i32 = 19;
pub const ENOTDIR: i32 = 20;
pub const EISDIR: i32 = 21;
pub const EINVAL: i32 = 22;
pub const ENFILE: i32 = 23;
pub const EMFILE: i32 = 24;
pub const ENOTTY: i32 = 25;
pub const ETXTBSY: i32 = 26;
pub const EFBIG: i32 = 27;
pub const ENOSPC: i32 = 28;
pub const ESPIPE: i32 = 29;
pub const EROFS: i32 = 30;
pub const EMLINK: i32 = 31;
pub const EPIPE: i32 = 32;
pub const EDOM: i32 = 33;
pub const ERANGE: i32 = 34;
pub const EDEADLK: i32 = 35;
pub const ENAMETOOLONG: i32 = 36;
pub const ENOLCK: i32 = 37;
pub const ENOSYS: i32 = 38;
pub const ENOTEMPTY: i32 = 39;
pub const ELOOP: i32 = 40;
pub const EWOULDBLOCK: i32 = EAGAIN;
pub const ENOMSG: i32 = 42;
pub const EIDRM: i32 = 43;
pub const ECHRNG: i32 = 44;
pub const EL2NSYNC: i32 = 45;
pub const EL3HLT: i32 = 46;
pub const EL3RST: i32 = 47;
pub const ELNRNG: i32 = 48;
pub const EUNATCH: i32 = 49;
pub const ENOCSI: i32 = 50;
pub const EL2HLT: i32 = 51;
pub const EBADE: i32 = 52;
pub const EBADR: i32 = 53;
pub const EXFULL: i32 = 54;
pub const ENOANO: i32 = 55;
pub const EBADRQC: i32 = 56;
pub const EBADSLT: i32 = 57;
pub const EDEADLOCK: i32 = EDEADLK;
pub const EBFONT: i32 = 59;
pub const ENOSTR: i32 = 60;
pub const ENODATA: i32 = 61;
pub const ETIME: i32 = 62;
pub const ENOSR: i32 = 63;
pub const ENONET: i32 = 64;
pub const ENOPKG: i32 = 65;
pub const EREMOTE: i32 = 66;
pub const ENOLINK: i32 = 67;
pub const EADV: i32 = 68;
pub const ESRMNT: i32 = 69;
pub const ECOMM: i32 = 70;
pub const EPROTO: i32 = 71;
pub const EMULTIHOP: i32 = 72;
pub const EDOTDOT: i32 = 73;
pub const EBADMSG: i32 = 74;
pub const EOVERFLOW: i32 = 75;
pub const ENOTUNIQ: i32 = 76;
pub const EBADFD: i32 = 77;
pub const EREMCHG: i32 = 78;
pub const ELIBACC: i32 = 79;
pub const ELIBBAD: i32 = 80;
pub const ELIBSCN: i32 = 81;
pub const ELIBMAX: i32 = 82;
pub const ELIBEXEC: i32 = 83;
pub const EILSEQ: i32 = 84;
pub const ERESTART: i32 = 85;
pub const ESTRPIPE: i32 = 86;
pub const EUSERS: i32 = 87;
pub const ENOTSOCK: i32 = 88;
pub const EDESTADDRREQ: i32 = 89;
pub const EMSGSIZE: i32 = 90;
pub const EPROTOTYPE: i32 = 91;
pub const ENOPROTOOPT: i32 = 92;
pub const EPROTONOSUPPORT: i32 = 93;
pub const ESOCKTNOSUPPORT: i32 = 94;
pub const EOPNOTSUPP: i32 = 95;
pub const EPFNOSUPPORT: i32 = 96;
pub const EAFNOSUPPORT: i32 = 97;
pub const EADDRINUSE: i32 = 98;
pub const EADDRNOTAVAIL: i32 = 99;
pub const ENETDOWN: i32 = 100;
pub const ENETUNREACH: i32 = 101;
pub const ENETRESET: i32 = 102;
pub const ECONNABORTED: i32 = 103;
pub const ECONNRESET: i32 = 104;
pub const ENOBUFS: i32 = 105;
pub const EISCONN: i32 = 106;
pub const ENOTCONN: i32 = 107;
pub const ESHUTDOWN: i32 = 108;
pub const ETOOMANYREFS: i32 = 109;
pub const ETIMEDOUT: i32 = 110;
pub const ECONNREFUSED: i32 = 111;
pub const EHOSTDOWN: i32 = 112;
pub const EHOSTUNREACH: i32 = 113;
pub const EALREADY: i32 = 114;
pub const EINPROGRESS: i32 = 115;
pub const ESTALE: i32 = 116;
pub const EUCLEAN: i32 = 117;
pub const ENOTNAM: i32 = 118;
pub const ENAVAIL: i32 = 119;
pub const EISNAM: i32 = 120;
pub const EREMOTEIO: i32 = 121;
pub const EDQUOT: i32 = 122;
pub const ENOMEDIUM: i32 = 123;
pub const EMEDIUMTYPE: i32 = 124;
pub const ECANCELED: i32 = 125;
pub const ENOKEY: i32 = 126;
pub const EKEYEXPIRED: i32 = 127;
pub const EKEYREVOKED: i32 = 128;
pub const EKEYREJECTED: i32 = 129;
pub const EOWNERDEAD: i32 = 130;
pub const ENOTRECOVERABLE: i32 = 131;
pub const ERFKILL: i32 = 132;
pub const EHWPOISON: i32 = 133;

extern "C" {
    #[link_name = "sys_alloc"]
    pub fn alloc(size: usize, align: usize) -> *mut u8;

    #[link_name = "sys_alloc_zeroed"]
    pub fn alloc_zeroed(size: usize, align: usize) -> *mut u8;

    #[link_name = "sys_realloc"]
    pub fn realloc(ptr: *mut u8, size: usize, align: usize, new_size: usize) -> *mut u8;

    #[link_name = "sys_dealloc"]
    pub fn dealloc(ptr: *mut u8, size: usize, align: usize);

    #[link_name = "sys_exit"]
    pub fn exit(status: i32) -> !;

    #[link_name = "sys_abort"]
    pub fn abort() -> !;

    #[link_name = "sys_errno"]
    pub fn errno() -> i32;

    #[link_name = "sys_clock_gettime"]
    pub fn clock_gettime(clockid: clockid_t, tp: *mut timespec) -> i32;

    #[link_name = "sys_nanosleep"]
    pub fn nanosleep(req: *const timespec) -> i32;

    #[link_name = "sys_available_parallelism"]
    pub fn available_parallelism() -> usize;

    #[link_name = "sys_futex_wait"]
    pub fn futex_wait(
        address: *mut u32,
        expected: u32,
        timeout: *const timespec,
        flags: u32,
    ) -> i32;

    #[link_name = "sys_futex_wake"]
    pub fn futex_wake(address: *mut u32, count: i32) -> i32;

    #[link_name = "sys_stat"]
    pub fn stat(path: *const c_char, stat: *mut stat) -> i32;

    #[link_name = "sys_fstat"]
    pub fn fstat(fd: i32, stat: *mut stat) -> i32;

    #[link_name = "sys_lstat"]
    pub fn lstat(path: *const c_char, stat: *mut stat) -> i32;

    #[link_name = "sys_open"]
    pub fn open(path: *const c_char, flags: i32, mode: mode_t) -> i32;

    #[link_name = "sys_unlink"]
    pub fn unlink(path: *const c_char) -> i32;

    #[link_name = "sys_mkdir"]
    pub fn mkdir(path: *const c_char, mode: mode_t) -> i32;

    #[link_name = "sys_rmdir"]
    pub fn rmdir(path: *const c_char) -> i32;

    #[link_name = "sys_read"]
    pub fn read(fd: i32, buf: *mut u8, len: usize) -> isize;

    #[link_name = "sys_write"]
    pub fn write(fd: i32, buf: *const u8, len: usize) -> isize;

    #[link_name = "sys_readv"]
    pub fn readv(fd: i32, iov: *const iovec, iovcnt: usize) -> isize;

    #[link_name = "sys_writev"]
    pub fn writev(fd: i32, iov: *const iovec, iovcnt: usize) -> isize;

    #[link_name = "sys_close"]
    pub fn close(fd: i32) -> i32;

    #[link_name = "sys_dup"]
    pub fn dup(fd: i32) -> i32;

    #[link_name = "sys_fcntl"]
    pub fn fcntl(fd: i32, cmd: i32, arg: i32) -> i32;

    #[link_name = "sys_getdents64"]
    pub fn getdents64(fd: i32, dirp: *mut dirent64, count: usize) -> isize;

    #[link_name = "sys_getaddrinfo"]
    pub fn getaddrinfo(
        nodename: *const c_char,
        servname: *const c_char,
        hints: *const addrinfo,
        res: *mut *mut addrinfo,
    ) -> i32;

    #[link_name = "sys_freeaddrinfo"]
    pub fn freeaddrinfo(ai: *mut addrinfo);

    #[link_name = "sys_socket"]
    pub fn socket(domain: i32, ty: i32, protocol: i32) -> i32;

    #[link_name = "sys_bind"]
    pub fn bind(sockfd: i32, addr: *const sockaddr, addrlen: socklen_t) -> i32;

    #[link_name = "sys_listen"]
    pub fn listen(sockfd: i32, backlog: i32) -> i32;

    #[link_name = "sys_accept"]
    pub fn accept(sockfd: i32, addr: *mut sockaddr, addrlen: *mut socklen_t) -> i32;

    #[link_name = "sys_connect"]
    pub fn connect(sockfd: i32, addr: *const sockaddr, addrlen: socklen_t) -> i32;

    #[link_name = "sys_recv"]
    pub fn recv(sockfd: i32, buf: *mut u8, len: usize, flags: i32) -> isize;

    #[link_name = "sys_recvfrom"]
    pub fn recvfrom(
        sockfd: i32,
        buf: *mut c_void,
        len: usize,
        flags: i32,
        addr: *mut sockaddr,
        addrlen: *mut socklen_t,
    ) -> isize;

    #[link_name = "sys_send"]
    pub fn send(sockfd: i32, buf: *const c_void, len: usize, flags: i32) -> isize;

    #[link_name = "sys_sendto"]
    pub fn sendto(
        sockfd: i32,
        buf: *const c_void,
        len: usize,
        flags: i32,
        to: *const sockaddr,
        tolen: socklen_t,
    ) -> isize;

    #[link_name = "sys_getpeername"]
    pub fn getpeername(sockfd: i32, addr: *mut sockaddr, addrlen: *mut socklen_t) -> i32;

    #[link_name = "sys_getsockname"]
    pub fn getsockname(sockfd: i32, addr: *mut sockaddr, addrlen: *mut socklen_t) -> i32;

    #[link_name = "sys_getsockopt"]
    pub fn getsockopt(
        sockfd: i32,
        level: i32,
        optname: i32,
        optval: *mut c_void,
        optlen: *mut socklen_t,
    ) -> i32;

    #[link_name = "sys_setsockopt"]
    pub fn setsockopt(
        sockfd: i32,
        level: i32,
        optname: i32,
        optval: *const c_void,
        optlen: socklen_t,
    ) -> i32;

    #[link_name = "sys_ioctl"]
    pub fn ioctl(sockfd: i32, cmd: i32, argp: *mut c_void) -> i32;

    #[link_name = "sys_shutdown"]
    pub fn shutdown(sockfd: i32, how: i32) -> i32;

    #[link_name = "sys_eventfd"]
    pub fn eventfd(initval: u64, flags: i16) -> i32;

    #[link_name = "sys_poll"]
    pub fn poll(fds: *mut pollfd, nfds: nfds_t, timeout: i32) -> i32;
}
