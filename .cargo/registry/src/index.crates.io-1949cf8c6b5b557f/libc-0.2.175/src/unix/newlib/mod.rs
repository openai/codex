use crate::prelude::*;

pub type blkcnt_t = i32;
pub type blksize_t = i32;

pub type clockid_t = c_ulong;

cfg_if! {
    if #[cfg(any(target_os = "espidf"))] {
        pub type dev_t = c_short;
        pub type ino_t = c_ushort;
        pub type off_t = c_long;
    } else if #[cfg(any(target_os = "vita"))] {
        pub type dev_t = c_short;
        pub type ino_t = c_ushort;
        pub type off_t = c_int;
    } else {
        pub type dev_t = u32;
        pub type ino_t = u32;
        pub type off_t = i64;
    }
}

pub type fsblkcnt_t = u64;
pub type fsfilcnt_t = u32;
pub type id_t = u32;
pub type key_t = c_int;
pub type loff_t = c_longlong;
pub type mode_t = c_uint;
pub type nfds_t = u32;
pub type nlink_t = c_ushort;
pub type pthread_t = c_ulong;
pub type pthread_key_t = c_uint;
pub type rlim_t = u32;

cfg_if! {
    if #[cfg(target_os = "horizon")] {
        pub type sa_family_t = u16;
    } else {
        pub type sa_family_t = u8;
    }
}

pub type socklen_t = u32;
pub type speed_t = u32;
pub type suseconds_t = i32;
cfg_if! {
    if #[cfg(target_os = "espidf")] {
        pub type tcflag_t = u16;
    } else {
        pub type tcflag_t = c_uint;
    }
}
pub type useconds_t = u32;

cfg_if! {
    if #[cfg(any(
        target_os = "horizon",
        all(target_os = "espidf", not(espidf_time32))
    ))] {
        pub type time_t = c_longlong;
    } else {
        pub type time_t = i32;
    }
}

cfg_if! {
    if #[cfg(not(target_os = "horizon"))] {
        s! {
            pub struct hostent {
                pub h_name: *mut c_char,
                pub h_aliases: *mut *mut c_char,
                pub h_addrtype: c_int,
                pub h_length: c_int,
                pub h_addr_list: *mut *mut c_char,
                pub h_addr: *mut c_char,
            }
        }
    }
}

s! {
    // The order of the `ai_addr` field in this struct is crucial
    // for converting between the Rust and C types.
    pub struct addrinfo {
        pub ai_flags: c_int,
        pub ai_family: c_int,
        pub ai_socktype: c_int,
        pub ai_protocol: c_int,
        pub ai_addrlen: socklen_t,

        #[cfg(target_os = "espidf")]
        pub ai_addr: *mut sockaddr,

        pub ai_canonname: *mut c_char,

        #[cfg(not(any(
            target_os = "espidf",
            all(target_arch = "powerpc", target_vendor = "nintendo")
        )))]
        pub ai_addr: *mut sockaddr,

        pub ai_next: *mut addrinfo,
    }

    pub struct ip_mreq {
        pub imr_multiaddr: in_addr,
        pub imr_interface: in_addr,
    }

    pub struct linger {
        pub l_onoff: c_int,
        pub l_linger: c_int,
    }

    pub struct in_addr {
        pub s_addr: crate::in_addr_t,
    }

    pub struct pollfd {
        pub fd: c_int,
        pub events: c_int,
        pub revents: c_int,
    }

    pub struct lconv {
        pub decimal_point: *mut c_char,
        pub thousands_sep: *mut c_char,
        pub grouping: *mut c_char,
        pub int_curr_symbol: *mut c_char,
        pub currency_symbol: *mut c_char,
        pub mon_decimal_point: *mut c_char,
        pub mon_thousands_sep: *mut c_char,
        pub mon_grouping: *mut c_char,
        pub positive_sign: *mut c_char,
        pub negative_sign: *mut c_char,
        pub int_frac_digits: c_char,
        pub frac_digits: c_char,
        pub p_cs_precedes: c_char,
        pub p_sep_by_space: c_char,
        pub n_cs_precedes: c_char,
        pub n_sep_by_space: c_char,
        pub p_sign_posn: c_char,
        pub n_sign_posn: c_char,
        pub int_n_cs_precedes: c_char,
        pub int_n_sep_by_space: c_char,
        pub int_n_sign_posn: c_char,
        pub int_p_cs_precedes: c_char,
        pub int_p_sep_by_space: c_char,
        pub int_p_sign_posn: c_char,
    }

    pub struct tm {
        pub tm_sec: c_int,
        pub tm_min: c_int,
        pub tm_hour: c_int,
        pub tm_mday: c_int,
        pub tm_mon: c_int,
        pub tm_year: c_int,
        pub tm_wday: c_int,
        pub tm_yday: c_int,
        pub tm_isdst: c_int,
    }

    pub struct statvfs {
        pub f_bsize: c_ulong,
        pub f_frsize: c_ulong,
        pub f_blocks: fsblkcnt_t,
        pub f_bfree: fsblkcnt_t,
        pub f_bavail: fsblkcnt_t,
        pub f_files: fsfilcnt_t,
        pub f_ffree: fsfilcnt_t,
        pub f_favail: fsfilcnt_t,
        pub f_fsid: c_ulong,
        pub f_flag: c_ulong,
        pub f_namemax: c_ulong,
    }

    pub struct sigaction {
        pub sa_handler: extern "C" fn(arg1: c_int),
        pub sa_mask: sigset_t,
        pub sa_flags: c_int,
    }

    pub struct stack_t {
        pub ss_sp: *mut c_void,
        pub ss_flags: c_int,
        pub ss_size: usize,
    }

    pub struct fd_set {
        // Unverified
        fds_bits: [c_ulong; FD_SETSIZE as usize / ULONG_SIZE],
    }

    pub struct passwd {
        // Unverified
        pub pw_name: *mut c_char,
        pub pw_passwd: *mut c_char,
        pub pw_uid: crate::uid_t,
        pub pw_gid: crate::gid_t,
        pub pw_gecos: *mut c_char,
        pub pw_dir: *mut c_char,
        pub pw_shell: *mut c_char,
    }

    pub struct termios {
        // Unverified
        pub c_iflag: crate::tcflag_t,
        pub c_oflag: crate::tcflag_t,
        pub c_cflag: crate::tcflag_t,
        pub c_lflag: crate::tcflag_t,
        pub c_line: crate::cc_t,
        pub c_cc: [crate::cc_t; crate::NCCS],
        #[cfg(target_os = "espidf")]
        pub c_ispeed: u32,
        #[cfg(target_os = "espidf")]
        pub c_ospeed: u32,
    }

    pub struct sem_t {
        // Unverified
        __size: [c_char; 16],
    }

    pub struct Dl_info {
        // Unverified
        pub dli_fname: *const c_char,
        pub dli_fbase: *mut c_void,
        pub dli_sname: *const c_char,
        pub dli_saddr: *mut c_void,
    }

    pub struct utsname {
        // Unverified
        pub sysname: [c_char; 65],
        pub nodename: [c_char; 65],
        pub release: [c_char; 65],
        pub version: [c_char; 65],
        pub machine: [c_char; 65],
        pub domainname: [c_char; 65],
    }

    pub struct cpu_set_t {
        // Unverified
        bits: [u32; 32],
    }

    pub struct pthread_attr_t {
        // Unverified
        #[cfg(not(target_os = "espidf"))]
        __size: [u8; __SIZEOF_PTHREAD_ATTR_T],
        #[cfg(target_os = "espidf")]
        pub is_initialized: i32,
        #[cfg(target_os = "espidf")]
        pub stackaddr: *mut c_void,
        #[cfg(target_os = "espidf")]
        pub stacksize: i32,
        #[cfg(target_os = "espidf")]
        pub contentionscope: i32,
        #[cfg(target_os = "espidf")]
        pub inheritsched: i32,
        #[cfg(target_os = "espidf")]
        pub schedpolicy: i32,
        #[cfg(target_os = "espidf")]
        pub schedparam: i32,
        #[cfg(target_os = "espidf")]
        pub detachstate: i32,
    }

    pub struct pthread_rwlockattr_t {
        // Unverified
        __size: [u8; __SIZEOF_PTHREAD_RWLOCKATTR_T],
    }

    #[cfg_attr(
        all(
            target_pointer_width = "32",
            any(target_arch = "mips", target_arch = "arm", target_arch = "powerpc")
        ),
        repr(align(4))
    )]
    #[cfg_attr(
        any(
            target_pointer_width = "64",
            not(any(target_arch = "mips", target_arch = "arm", target_arch = "powerpc"))
        ),
        repr(align(8))
    )]
    pub struct pthread_mutex_t {
        // Unverified
        size: [u8; crate::__SIZEOF_PTHREAD_MUTEX_T],
    }

    #[cfg_attr(
        all(
            target_pointer_width = "32",
            any(target_arch = "mips", target_arch = "arm", target_arch = "powerpc")
        ),
        repr(align(4))
    )]
    #[cfg_attr(
        any(
            target_pointer_width = "64",
            not(any(target_arch = "mips", target_arch = "arm", target_arch = "powerpc"))
        ),
        repr(align(8))
    )]
    pub struct pthread_rwlock_t {
        // Unverified
        size: [u8; crate::__SIZEOF_PTHREAD_RWLOCK_T],
    }

    #[cfg_attr(
        any(
            target_pointer_width = "32",
            target_arch = "x86_64",
            target_arch = "powerpc64",
            target_arch = "mips64",
            target_arch = "s390x",
            target_arch = "sparc64"
        ),
        repr(align(4))
    )]
    #[cfg_attr(
        not(any(
            target_pointer_width = "32",
            target_arch = "x86_64",
            target_arch = "powerpc64",
            target_arch = "mips64",
            target_arch = "s390x",
            target_arch = "sparc64"
        )),
        repr(align(8))
    )]
    pub struct pthread_mutexattr_t {
        // Unverified
        size: [u8; crate::__SIZEOF_PTHREAD_MUTEXATTR_T],
    }

    #[repr(align(8))]
    pub struct pthread_cond_t {
        // Unverified
        size: [u8; crate::__SIZEOF_PTHREAD_COND_T],
    }

    #[repr(align(4))]
    pub struct pthread_condattr_t {
        // Unverified
        size: [u8; crate::__SIZEOF_PTHREAD_CONDATTR_T],
    }
}

// unverified constants
pub const PTHREAD_MUTEX_INITIALIZER: pthread_mutex_t = pthread_mutex_t {
    size: [__PTHREAD_INITIALIZER_BYTE; __SIZEOF_PTHREAD_MUTEX_T],
};
pub const PTHREAD_COND_INITIALIZER: pthread_cond_t = pthread_cond_t {
    size: [__PTHREAD_INITIALIZER_BYTE; __SIZEOF_PTHREAD_COND_T],
};
pub const PTHREAD_RWLOCK_INITIALIZER: pthread_rwlock_t = pthread_rwlock_t {
    size: [__PTHREAD_INITIALIZER_BYTE; __SIZEOF_PTHREAD_RWLOCK_T],
};

cfg_if! {
    if #[cfg(target_os = "espidf")] {
        pub const NCCS: usize = 11;
    } else {
        pub const NCCS: usize = 32;
    }
}

cfg_if! {
    if #[cfg(target_os = "espidf")] {
        const __PTHREAD_INITIALIZER_BYTE: u8 = 0xff;
        pub const __SIZEOF_PTHREAD_ATTR_T: usize = 32;
        pub const __SIZEOF_PTHREAD_MUTEX_T: usize = 4;
        pub const __SIZEOF_PTHREAD_MUTEXATTR_T: usize = 12;
        pub const __SIZEOF_PTHREAD_COND_T: usize = 4;
        pub const __SIZEOF_PTHREAD_CONDATTR_T: usize = 8;
        pub const __SIZEOF_PTHREAD_RWLOCK_T: usize = 4;
        pub const __SIZEOF_PTHREAD_RWLOCKATTR_T: usize = 12;
        pub const __SIZEOF_PTHREAD_BARRIER_T: usize = 32;
    } else if #[cfg(target_os = "vita")] {
        const __PTHREAD_INITIALIZER_BYTE: u8 = 0xff;
        pub const __SIZEOF_PTHREAD_ATTR_T: usize = 4;
        pub const __SIZEOF_PTHREAD_MUTEX_T: usize = 4;
        pub const __SIZEOF_PTHREAD_MUTEXATTR_T: usize = 4;
        pub const __SIZEOF_PTHREAD_COND_T: usize = 4;
        pub const __SIZEOF_PTHREAD_CONDATTR_T: usize = 4;
        pub const __SIZEOF_PTHREAD_RWLOCK_T: usize = 4;
        pub const __SIZEOF_PTHREAD_RWLOCKATTR_T: usize = 4;
        pub const __SIZEOF_PTHREAD_BARRIER_T: usize = 4;
    } else if #[cfg(target_os = "rtems")] {
        const __PTHREAD_INITIALIZER_BYTE: u8 = 0x00;
        pub const __SIZEOF_PTHREAD_ATTR_T: usize = 96;
        pub const __SIZEOF_PTHREAD_MUTEX_T: usize = 64;
        pub const __SIZEOF_PTHREAD_MUTEXATTR_T: usize = 24;
        pub const __SIZEOF_PTHREAD_COND_T: usize = 28;
        pub const __SIZEOF_PTHREAD_CONDATTR_T: usize = 24;
        pub const __SIZEOF_PTHREAD_RWLOCK_T: usize = 32;
        pub const __SIZEOF_PTHREAD_RWLOCKATTR_T: usize = 8;
        pub const __SIZEOF_PTHREAD_BARRIER_T: usize = 32;
    } else {
        const __PTHREAD_INITIALIZER_BYTE: u8 = 0;
        pub const __SIZEOF_PTHREAD_ATTR_T: usize = 56;
        pub const __SIZEOF_PTHREAD_MUTEX_T: usize = 40;
        pub const __SIZEOF_PTHREAD_MUTEXATTR_T: usize = 4;
        pub const __SIZEOF_PTHREAD_COND_T: usize = 48;
        pub const __SIZEOF_PTHREAD_CONDATTR_T: usize = 4;
        pub const __SIZEOF_PTHREAD_RWLOCK_T: usize = 56;
        pub const __SIZEOF_PTHREAD_RWLOCKATTR_T: usize = 8;
        pub const __SIZEOF_PTHREAD_BARRIER_T: usize = 32;
    }
}

pub const __SIZEOF_PTHREAD_BARRIERATTR_T: usize = 4;
pub const __PTHREAD_MUTEX_HAVE_PREV: usize = 1;
pub const __PTHREAD_RWLOCK_INT_FLAGS_SHARED: usize = 1;
pub const PTHREAD_MUTEX_NORMAL: c_int = 0;
pub const PTHREAD_MUTEX_RECURSIVE: c_int = 1;
pub const PTHREAD_MUTEX_ERRORCHECK: c_int = 2;

cfg_if! {
    if #[cfg(any(target_os = "horizon", target_os = "espidf"))] {
        pub const FD_SETSIZE: usize = 64;
    } else if #[cfg(target_os = "vita")] {
        pub const FD_SETSIZE: usize = 256;
    } else {
        pub const FD_SETSIZE: usize = 1024;
    }
}
// intentionally not public, only used for fd_set
const ULONG_SIZE: usize = 32;

// Other constants
pub const EPERM: c_int = 1;
pub const ENOENT: c_int = 2;
pub const ESRCH: c_int = 3;
pub const EINTR: c_int = 4;
pub const EIO: c_int = 5;
pub const ENXIO: c_int = 6;
pub const E2BIG: c_int = 7;
pub const ENOEXEC: c_int = 8;
pub const EBADF: c_int = 9;
pub const ECHILD: c_int = 10;
pub const EAGAIN: c_int = 11;
pub const ENOMEM: c_int = 12;
pub const EACCES: c_int = 13;
pub const EFAULT: c_int = 14;
pub const EBUSY: c_int = 16;
pub const EEXIST: c_int = 17;
pub const EXDEV: c_int = 18;
pub const ENODEV: c_int = 19;
pub const ENOTDIR: c_int = 20;
pub const EISDIR: c_int = 21;
pub const EINVAL: c_int = 22;
pub const ENFILE: c_int = 23;
pub const EMFILE: c_int = 24;
pub const ENOTTY: c_int = 25;
pub const ETXTBSY: c_int = 26;
pub const EFBIG: c_int = 27;
pub const ENOSPC: c_int = 28;
pub const ESPIPE: c_int = 29;
pub const EROFS: c_int = 30;
pub const EMLINK: c_int = 31;
pub const EPIPE: c_int = 32;
pub const EDOM: c_int = 33;
pub const ERANGE: c_int = 34;
pub const ENOMSG: c_int = 35;
pub const EIDRM: c_int = 36;
pub const EDEADLK: c_int = 45;
pub const ENOLCK: c_int = 46;
pub const ENOSTR: c_int = 60;
pub const ENODATA: c_int = 61;
pub const ETIME: c_int = 62;
pub const ENOSR: c_int = 63;
pub const ENOLINK: c_int = 67;
pub const EPROTO: c_int = 71;
pub const EMULTIHOP: c_int = 74;
pub const EBADMSG: c_int = 77;
pub const EFTYPE: c_int = 79;
pub const ENOSYS: c_int = 88;
pub const ENOTEMPTY: c_int = 90;
pub const ENAMETOOLONG: c_int = 91;
pub const ELOOP: c_int = 92;
pub const EOPNOTSUPP: c_int = 95;
pub const EPFNOSUPPORT: c_int = 96;
pub const ECONNRESET: c_int = 104;
pub const ENOBUFS: c_int = 105;
pub const EAFNOSUPPORT: c_int = 106;
pub const EPROTOTYPE: c_int = 107;
pub const ENOTSOCK: c_int = 108;
pub const ENOPROTOOPT: c_int = 109;
pub const ECONNREFUSED: c_int = 111;
pub const EADDRINUSE: c_int = 112;
pub const ECONNABORTED: c_int = 113;
pub const ENETUNREACH: c_int = 114;
pub const ENETDOWN: c_int = 115;
pub const ETIMEDOUT: c_int = 116;
pub const EHOSTDOWN: c_int = 117;
pub const EHOSTUNREACH: c_int = 118;
pub const EINPROGRESS: c_int = 119;
pub const EALREADY: c_int = 120;
pub const EDESTADDRREQ: c_int = 121;
pub const EMSGSIZE: c_int = 122;
pub const EPROTONOSUPPORT: c_int = 123;
pub const EADDRNOTAVAIL: c_int = 125;
pub const ENETRESET: c_int = 126;
pub const EISCONN: c_int = 127;
pub const ENOTCONN: c_int = 128;
pub const ETOOMANYREFS: c_int = 129;
pub const EDQUOT: c_int = 132;
pub const ESTALE: c_int = 133;
pub const ENOTSUP: c_int = 134;
pub const EILSEQ: c_int = 138;
pub const EOVERFLOW: c_int = 139;
pub const ECANCELED: c_int = 140;
pub const ENOTRECOVERABLE: c_int = 141;
pub const EOWNERDEAD: c_int = 142;
pub const EWOULDBLOCK: c_int = 11;

pub const F_DUPFD: c_int = 0;
pub const F_GETFD: c_int = 1;
pub const F_SETFD: c_int = 2;
pub const F_GETFL: c_int = 3;
pub const F_SETFL: c_int = 4;
pub const F_GETOWN: c_int = 5;
pub const F_SETOWN: c_int = 6;
pub const F_GETLK: c_int = 7;
pub const F_SETLK: c_int = 8;
pub const F_SETLKW: c_int = 9;
pub const F_RGETLK: c_int = 10;
pub const F_RSETLK: c_int = 11;
pub const F_CNVT: c_int = 12;
pub const F_RSETLKW: c_int = 13;
pub const F_DUPFD_CLOEXEC: c_int = 14;

pub const O_RDONLY: c_int = 0;
pub const O_WRONLY: c_int = 1;
pub const O_RDWR: c_int = 2;
pub const O_APPEND: c_int = 8;
pub const O_CREAT: c_int = 512;
pub const O_TRUNC: c_int = 1024;
pub const O_EXCL: c_int = 2048;
pub const O_SYNC: c_int = 8192;
pub const O_NONBLOCK: c_int = 16384;

pub const O_ACCMODE: c_int = 3;
cfg_if! {
    if #[cfg(target_os = "espidf")] {
        pub const O_CLOEXEC: c_int = 0x40000;
    } else {
        pub const O_CLOEXEC: c_int = 0x80000;
    }
}

pub const RTLD_LAZY: c_int = 0x1;

pub const STDIN_FILENO: c_int = 0;
pub const STDOUT_FILENO: c_int = 1;
pub const STDERR_FILENO: c_int = 2;

pub const SEEK_SET: c_int = 0;
pub const SEEK_CUR: c_int = 1;
pub const SEEK_END: c_int = 2;

pub const FIOCLEX: c_ulong = 0x20006601;
pub const FIONCLEX: c_ulong = 0x20006602;

pub const S_BLKSIZE: mode_t = 1024;
pub const S_IREAD: mode_t = 0o0400;
pub const S_IWRITE: mode_t = 0o0200;
pub const S_IEXEC: mode_t = 0o0100;
pub const S_ENFMT: mode_t = 0o2000;
pub const S_IFMT: mode_t = 0o17_0000;
pub const S_IFDIR: mode_t = 0o4_0000;
pub const S_IFCHR: mode_t = 0o2_0000;
pub const S_IFBLK: mode_t = 0o6_0000;
pub const S_IFREG: mode_t = 0o10_0000;
pub const S_IFLNK: mode_t = 0o12_0000;
pub const S_IFSOCK: mode_t = 0o14_0000;
pub const S_IFIFO: mode_t = 0o1_0000;
pub const S_IRUSR: mode_t = 0o0400;
pub const S_IWUSR: mode_t = 0o0200;
pub const S_IXUSR: mode_t = 0o0100;
pub const S_IRGRP: mode_t = 0o0040;
pub const S_IWGRP: mode_t = 0o0020;
pub const S_IXGRP: mode_t = 0o0010;
pub const S_IROTH: mode_t = 0o0004;
pub const S_IWOTH: mode_t = 0o0002;
pub const S_IXOTH: mode_t = 0o0001;

pub const SOL_TCP: c_int = 6;

pub const PF_UNSPEC: c_int = 0;
pub const PF_INET: c_int = 2;
cfg_if! {
    if #[cfg(target_os = "espidf")] {
        pub const PF_INET6: c_int = 10;
    } else {
        pub const PF_INET6: c_int = 23;
    }
}

pub const AF_UNSPEC: c_int = 0;
pub const AF_INET: c_int = 2;

pub const CLOCK_REALTIME: crate::clockid_t = 1;
pub const CLOCK_MONOTONIC: crate::clockid_t = 4;
pub const CLOCK_BOOTTIME: crate::clockid_t = 4;

pub const SOCK_STREAM: c_int = 1;
pub const SOCK_DGRAM: c_int = 2;

pub const SHUT_RD: c_int = 0;
pub const SHUT_WR: c_int = 1;
pub const SHUT_RDWR: c_int = 2;

pub const SO_BINTIME: c_int = 0x2000;
pub const SO_NO_OFFLOAD: c_int = 0x4000;
pub const SO_NO_DDP: c_int = 0x8000;
pub const SO_REUSEPORT_LB: c_int = 0x10000;
pub const SO_LABEL: c_int = 0x1009;
pub const SO_PEERLABEL: c_int = 0x1010;
pub const SO_LISTENQLIMIT: c_int = 0x1011;
pub const SO_LISTENQLEN: c_int = 0x1012;
pub const SO_LISTENINCQLEN: c_int = 0x1013;
pub const SO_SETFIB: c_int = 0x1014;
pub const SO_USER_COOKIE: c_int = 0x1015;
pub const SO_PROTOCOL: c_int = 0x1016;
pub const SO_PROTOTYPE: c_int = SO_PROTOCOL;
pub const SO_VENDOR: c_int = 0x80000000;
pub const SO_DEBUG: c_int = 0x01;
pub const SO_ACCEPTCONN: c_int = 0x0002;
pub const SO_REUSEADDR: c_int = 0x0004;
pub const SO_KEEPALIVE: c_int = 0x0008;
pub const SO_DONTROUTE: c_int = 0x0010;
pub const SO_BROADCAST: c_int = 0x0020;
pub const SO_USELOOPBACK: c_int = 0x0040;
pub const SO_LINGER: c_int = 0x0080;
pub const SO_OOBINLINE: c_int = 0x0100;
pub const SO_REUSEPORT: c_int = 0x0200;
pub const SO_TIMESTAMP: c_int = 0x0400;
pub const SO_NOSIGPIPE: c_int = 0x0800;
pub const SO_ACCEPTFILTER: c_int = 0x1000;
pub const SO_SNDBUF: c_int = 0x1001;
pub const SO_RCVBUF: c_int = 0x1002;
pub const SO_SNDLOWAT: c_int = 0x1003;
pub const SO_RCVLOWAT: c_int = 0x1004;
pub const SO_SNDTIMEO: c_int = 0x1005;
pub const SO_RCVTIMEO: c_int = 0x1006;
cfg_if! {
    if #[cfg(target_os = "horizon")] {
        pub const SO_ERROR: c_int = 0x1009;
    } else {
        pub const SO_ERROR: c_int = 0x1007;
    }
}
pub const SO_TYPE: c_int = 0x1008;

pub const SOCK_CLOEXEC: c_int = O_CLOEXEC;

pub const INET_ADDRSTRLEN: c_int = 16;

// https://github.com/bminor/newlib/blob/HEAD/newlib/libc/sys/linux/include/net/if.h#L121
pub const IFF_UP: c_int = 0x1; // interface is up
pub const IFF_BROADCAST: c_int = 0x2; // broadcast address valid
pub const IFF_DEBUG: c_int = 0x4; // turn on debugging
pub const IFF_LOOPBACK: c_int = 0x8; // is a loopback net
pub const IFF_POINTOPOINT: c_int = 0x10; // interface is point-to-point link
pub const IFF_NOTRAILERS: c_int = 0x20; // avoid use of trailers
pub const IFF_RUNNING: c_int = 0x40; // resources allocated
pub const IFF_NOARP: c_int = 0x80; // no address resolution protocol
pub const IFF_PROMISC: c_int = 0x100; // receive all packets
pub const IFF_ALLMULTI: c_int = 0x200; // receive all multicast packets
pub const IFF_OACTIVE: c_int = 0x400; // transmission in progress
pub const IFF_SIMPLEX: c_int = 0x800; // can't hear own transmissions
pub const IFF_LINK0: c_int = 0x1000; // per link layer defined bit
pub const IFF_LINK1: c_int = 0x2000; // per link layer defined bit
pub const IFF_LINK2: c_int = 0x4000; // per link layer defined bit
pub const IFF_ALTPHYS: c_int = IFF_LINK2; // use alternate physical connection
pub const IFF_MULTICAST: c_int = 0x8000; // supports multicast

cfg_if! {
    if #[cfg(target_os = "vita")] {
        pub const TCP_NODELAY: c_int = 1;
        pub const TCP_MAXSEG: c_int = 2;
    } else if #[cfg(target_os = "espidf")] {
        pub const TCP_NODELAY: c_int = 1;
        pub const TCP_MAXSEG: c_int = 8194;
    } else {
        pub const TCP_NODELAY: c_int = 8193;
        pub const TCP_MAXSEG: c_int = 8194;
    }
}

pub const TCP_NOPUSH: c_int = 4;
pub const TCP_NOOPT: c_int = 8;
cfg_if! {
    if #[cfg(target_os = "espidf")] {
        pub const TCP_KEEPIDLE: c_int = 3;
        pub const TCP_KEEPINTVL: c_int = 4;
        pub const TCP_KEEPCNT: c_int = 5;
    } else {
        pub const TCP_KEEPIDLE: c_int = 256;
        pub const TCP_KEEPINTVL: c_int = 512;
        pub const TCP_KEEPCNT: c_int = 1024;
    }
}

cfg_if! {
    if #[cfg(target_os = "horizon")] {
        pub const IP_TOS: c_int = 7;
    } else if #[cfg(target_os = "espidf")] {
        pub const IP_TOS: c_int = 1;
    } else {
        pub const IP_TOS: c_int = 3;
    }
}
cfg_if! {
    if #[cfg(target_os = "vita")] {
        pub const IP_TTL: c_int = 4;
    } else if #[cfg(target_os = "espidf")] {
        pub const IP_TTL: c_int = 2;
    } else {
        pub const IP_TTL: c_int = 8;
    }
}

cfg_if! {
    if #[cfg(target_os = "espidf")] {
        pub const IP_MULTICAST_IF: c_int = 6;
        pub const IP_MULTICAST_TTL: c_int = 5;
        pub const IP_MULTICAST_LOOP: c_int = 7;
    } else {
        pub const IP_MULTICAST_IF: c_int = 9;
        pub const IP_MULTICAST_TTL: c_int = 10;
        pub const IP_MULTICAST_LOOP: c_int = 11;
    }
}

cfg_if! {
    if #[cfg(target_os = "vita")] {
        pub const IP_ADD_MEMBERSHIP: c_int = 12;
        pub const IP_DROP_MEMBERSHIP: c_int = 13;
    } else if #[cfg(target_os = "espidf")] {
        pub const IP_ADD_MEMBERSHIP: c_int = 3;
        pub const IP_DROP_MEMBERSHIP: c_int = 4;
    } else {
        pub const IP_ADD_MEMBERSHIP: c_int = 11;
        pub const IP_DROP_MEMBERSHIP: c_int = 12;
    }
}
pub const IPV6_UNICAST_HOPS: c_int = 4;
cfg_if! {
    if #[cfg(target_os = "espidf")] {
        pub const IPV6_MULTICAST_IF: c_int = 768;
        pub const IPV6_MULTICAST_HOPS: c_int = 769;
        pub const IPV6_MULTICAST_LOOP: c_int = 770;
    } else {
        pub const IPV6_MULTICAST_IF: c_int = 9;
        pub const IPV6_MULTICAST_HOPS: c_int = 10;
        pub const IPV6_MULTICAST_LOOP: c_int = 11;
    }
}
pub const IPV6_V6ONLY: c_int = 27;
pub const IPV6_JOIN_GROUP: c_int = 12;
pub const IPV6_LEAVE_GROUP: c_int = 13;
pub const IPV6_ADD_MEMBERSHIP: c_int = 12;
pub const IPV6_DROP_MEMBERSHIP: c_int = 13;

cfg_if! {
    if #[cfg(target_os = "espidf")] {
        pub const HOST_NOT_FOUND: c_int = 210;
        pub const NO_DATA: c_int = 211;
        pub const NO_RECOVERY: c_int = 212;
        pub const TRY_AGAIN: c_int = 213;
    } else {
        pub const HOST_NOT_FOUND: c_int = 1;
        pub const NO_DATA: c_int = 2;
        pub const NO_RECOVERY: c_int = 3;
        pub const TRY_AGAIN: c_int = 4;
    }
}
pub const NO_ADDRESS: c_int = 2;

pub const AI_PASSIVE: c_int = 1;
pub const AI_CANONNAME: c_int = 2;
pub const AI_NUMERICHOST: c_int = 4;
cfg_if! {
    if #[cfg(target_os = "espidf")] {
        pub const AI_NUMERICSERV: c_int = 8;
        pub const AI_ADDRCONFIG: c_int = 64;
    } else {
        pub const AI_NUMERICSERV: c_int = 0;
        pub const AI_ADDRCONFIG: c_int = 0;
    }
}

pub const NI_MAXHOST: c_int = 1025;
pub const NI_MAXSERV: c_int = 32;
pub const NI_NOFQDN: c_int = 1;
pub const NI_NUMERICHOST: c_int = 2;
pub const NI_NAMEREQD: c_int = 4;
cfg_if! {
    if #[cfg(target_os = "espidf")] {
        pub const NI_NUMERICSERV: c_int = 8;
        pub const NI_DGRAM: c_int = 16;
    } else {
        pub const NI_NUMERICSERV: c_int = 0;
        pub const NI_DGRAM: c_int = 0;
    }
}

cfg_if! {
    // Defined in vita/mod.rs for "vita"
    if #[cfg(target_os = "espidf")] {
        pub const EAI_FAMILY: c_int = 204;
        pub const EAI_MEMORY: c_int = 203;
        pub const EAI_NONAME: c_int = 200;
        pub const EAI_SOCKTYPE: c_int = 10;
    } else if #[cfg(not(target_os = "vita"))] {
        pub const EAI_FAMILY: c_int = -303;
        pub const EAI_MEMORY: c_int = -304;
        pub const EAI_NONAME: c_int = -305;
        pub const EAI_SOCKTYPE: c_int = -307;
    }
}

pub const EXIT_SUCCESS: c_int = 0;
pub const EXIT_FAILURE: c_int = 1;

pub const PRIO_PROCESS: c_int = 0;
pub const PRIO_PGRP: c_int = 1;
pub const PRIO_USER: c_int = 2;

f! {
    pub fn FD_CLR(fd: c_int, set: *mut fd_set) -> () {
        let bits = size_of_val(&(*set).fds_bits[0]) * 8;
        let fd = fd as usize;
        (*set).fds_bits[fd / bits] &= !(1 << (fd % bits));
        return;
    }

    pub fn FD_ISSET(fd: c_int, set: *const fd_set) -> bool {
        let bits = size_of_val(&(*set).fds_bits[0]) * 8;
        let fd = fd as usize;
        return ((*set).fds_bits[fd / bits] & (1 << (fd % bits))) != 0;
    }

    pub fn FD_SET(fd: c_int, set: *mut fd_set) -> () {
        let bits = size_of_val(&(*set).fds_bits[0]) * 8;
        let fd = fd as usize;
        (*set).fds_bits[fd / bits] |= 1 << (fd % bits);
        return;
    }

    pub fn FD_ZERO(set: *mut fd_set) -> () {
        for slot in (*set).fds_bits.iter_mut() {
            *slot = 0;
        }
    }
}

extern "C" {
    pub fn getrlimit(resource: c_int, rlim: *mut crate::rlimit) -> c_int;
    pub fn setrlimit(resource: c_int, rlim: *const crate::rlimit) -> c_int;

    #[cfg_attr(target_os = "linux", link_name = "__xpg_strerror_r")]
    pub fn strerror_r(errnum: c_int, buf: *mut c_char, buflen: size_t) -> c_int;

    pub fn sem_destroy(sem: *mut sem_t) -> c_int;
    pub fn sem_init(sem: *mut sem_t, pshared: c_int, value: c_uint) -> c_int;

    pub fn abs(i: c_int) -> c_int;
    pub fn labs(i: c_long) -> c_long;
    pub fn rand() -> c_int;
    pub fn srand(seed: c_uint);

    #[cfg(not(all(target_arch = "powerpc", target_vendor = "nintendo")))]
    #[cfg_attr(target_os = "espidf", link_name = "lwip_bind")]
    pub fn bind(fd: c_int, addr: *const sockaddr, len: socklen_t) -> c_int;
    pub fn clock_settime(clock_id: crate::clockid_t, tp: *const crate::timespec) -> c_int;
    pub fn clock_gettime(clock_id: crate::clockid_t, tp: *mut crate::timespec) -> c_int;
    pub fn clock_getres(clock_id: crate::clockid_t, res: *mut crate::timespec) -> c_int;
    #[cfg_attr(target_os = "espidf", link_name = "lwip_close")]
    pub fn closesocket(sockfd: c_int) -> c_int;
    pub fn ioctl(fd: c_int, request: c_ulong, ...) -> c_int;
    #[cfg(not(all(target_arch = "powerpc", target_vendor = "nintendo")))]
    #[cfg_attr(target_os = "espidf", link_name = "lwip_recvfrom")]
    pub fn recvfrom(
        fd: c_int,
        buf: *mut c_void,
        n: usize,
        flags: c_int,
        addr: *mut sockaddr,
        addr_len: *mut socklen_t,
    ) -> isize;
    #[cfg(not(all(target_arch = "powerpc", target_vendor = "nintendo")))]
    pub fn getnameinfo(
        sa: *const sockaddr,
        salen: socklen_t,
        host: *mut c_char,
        hostlen: socklen_t,
        serv: *mut c_char,
        servlen: socklen_t,
        flags: c_int,
    ) -> c_int;
    pub fn memalign(align: size_t, size: size_t) -> *mut c_void;

    // DIFF(main): changed to `*const *mut` in e77f551de9
    pub fn fexecve(fd: c_int, argv: *const *const c_char, envp: *const *const c_char) -> c_int;

    pub fn gettimeofday(tp: *mut crate::timeval, tz: *mut c_void) -> c_int;
    pub fn getgrgid_r(
        gid: crate::gid_t,
        grp: *mut crate::group,
        buf: *mut c_char,
        buflen: size_t,
        result: *mut *mut crate::group,
    ) -> c_int;
    pub fn sigaltstack(ss: *const stack_t, oss: *mut stack_t) -> c_int;
    pub fn sem_close(sem: *mut sem_t) -> c_int;
    pub fn getdtablesize() -> c_int;
    pub fn getgrnam_r(
        name: *const c_char,
        grp: *mut crate::group,
        buf: *mut c_char,
        buflen: size_t,
        result: *mut *mut crate::group,
    ) -> c_int;
    pub fn pthread_sigmask(how: c_int, set: *const sigset_t, oldset: *mut sigset_t) -> c_int;
    pub fn sem_open(name: *const c_char, oflag: c_int, ...) -> *mut sem_t;
    pub fn getgrnam(name: *const c_char) -> *mut crate::group;
    pub fn pthread_kill(thread: crate::pthread_t, sig: c_int) -> c_int;
    pub fn sem_unlink(name: *const c_char) -> c_int;
    pub fn daemon(nochdir: c_int, noclose: c_int) -> c_int;
    pub fn getpwnam_r(
        name: *const c_char,
        pwd: *mut passwd,
        buf: *mut c_char,
        buflen: size_t,
        result: *mut *mut passwd,
    ) -> c_int;
    pub fn getpwuid_r(
        uid: crate::uid_t,
        pwd: *mut passwd,
        buf: *mut c_char,
        buflen: size_t,
        result: *mut *mut passwd,
    ) -> c_int;
    pub fn sigwait(set: *const sigset_t, sig: *mut c_int) -> c_int;
    pub fn pthread_atfork(
        prepare: Option<unsafe extern "C" fn()>,
        parent: Option<unsafe extern "C" fn()>,
        child: Option<unsafe extern "C" fn()>,
    ) -> c_int;
    pub fn getgrgid(gid: crate::gid_t) -> *mut crate::group;
    pub fn popen(command: *const c_char, mode: *const c_char) -> *mut crate::FILE;
    pub fn uname(buf: *mut crate::utsname) -> c_int;
}

mod generic;

cfg_if! {
    if #[cfg(target_os = "espidf")] {
        mod espidf;
        pub use self::espidf::*;
    } else if #[cfg(target_os = "horizon")] {
        mod horizon;
        pub use self::horizon::*;
    } else if #[cfg(target_os = "vita")] {
        mod vita;
        pub use self::vita::*;
    } else if #[cfg(target_arch = "arm")] {
        mod arm;
        pub use self::arm::*;
    } else if #[cfg(target_arch = "aarch64")] {
        mod aarch64;
        pub use self::aarch64::*;
    } else if #[cfg(target_arch = "powerpc")] {
        mod powerpc;
        pub use self::powerpc::*;
    } else {
        // Only tested on ARM so far. Other platforms might have different
        // definitions for types and constants.
        pub use target_arch_not_implemented;
    }
}

cfg_if! {
    if #[cfg(target_os = "rtems")] {
        mod rtems;
        pub use self::rtems::*;
    }
}
