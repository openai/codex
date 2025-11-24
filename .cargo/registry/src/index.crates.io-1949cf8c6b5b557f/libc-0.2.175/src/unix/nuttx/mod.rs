use crate::prelude::*;
use crate::{in6_addr, in_addr_t, timespec, DIR};

pub type nlink_t = u16;
pub type ino_t = u16;
pub type blkcnt_t = u64;
pub type blksize_t = i16;
pub type cc_t = u8;
pub type clock_t = i64;
pub type dev_t = i32;
pub type fsblkcnt_t = u64;
pub type locale_t = *mut i8;
pub type mode_t = u32;
pub type nfds_t = u32;
pub type off_t = i64;
pub type pthread_key_t = i32;
pub type pthread_mutexattr_t = u8;
pub type pthread_rwlockattr_t = i32;
pub type pthread_t = i32;
pub type rlim_t = i64;
pub type sa_family_t = u16;
pub type socklen_t = u32;
pub type speed_t = usize;
pub type suseconds_t = i32;
pub type tcflag_t = u32;
pub type clockid_t = i32;
pub type time_t = i64;
pub type wchar_t = i32;

s! {
    pub struct stat {
        pub st_dev: dev_t,
        pub st_ino: ino_t,
        pub st_mode: mode_t,
        pub st_nlink: nlink_t,
        pub st_uid: u32,
        pub st_gid: u32,
        pub st_rdev: dev_t,
        pub st_size: off_t,
        pub st_atim: timespec,
        pub st_mtim: timespec,
        pub st_ctim: timespec,
        pub st_blksize: blksize_t,
        pub st_blocks: i64,
        __reserved: [usize; __DEFAULT_RESERVED_SIZE__],
    }

    pub struct sockaddr {
        pub sa_family: sa_family_t,
        pub sa_data: [u8; 14],
    }

    pub struct passwd {
        pub pw_name: *const c_char,
        pub pw_passwd: *const c_char,
        pub pw_uid: u32,
        pub pw_gid: u32,
        pub pw_gecos: *const c_char,
        pub pw_dir: *const c_char,
        pub pw_shell: *const c_char,
        __reserved: [usize; __DEFAULT_RESERVED_SIZE__],
    }

    pub struct sem_t {
        __val: [usize; __SEM_SIZE__],
    }

    pub struct pthread_attr_t {
        __val: [usize; __PTHREAD_ATTR_SIZE__],
    }

    pub struct pthread_mutex_t {
        __val: [usize; __PTHREAD_MUTEX_SIZE__],
    }

    pub struct pthread_cond_t {
        __val: [usize; __PTHREAD_COND_SIZE__],
    }

    pub struct pthread_condattr_t {
        __val: [usize; __PTHREAD_CONDATTR_SIZE__],
    }

    pub struct Dl_info {
        pub dli_fname: *const c_char,
        pub dli_fbase: *mut c_void,
        pub dli_sname: *const c_char,
        pub dli_saddr: *mut c_void,
    }

    pub struct lconv {
        pub decimal_point: *const c_char,
        pub thousands_sep: *const c_char,
        pub grouping: *const c_char,
        pub int_curr_symbol: *const c_char,
        pub currency_symbol: *const c_char,
        pub mon_decimal_point: *const c_char,
        pub mon_thousands_sep: *const c_char,
        pub mon_grouping: *const c_char,
        pub positive_sign: *const c_char,
        pub negative_sign: *const c_char,
        pub int_frac_digits: i8,
        pub frac_digits: i8,
        pub p_cs_precedes: i8,
        pub p_sep_by_space: i8,
        pub n_cs_precedes: i8,
        pub n_sep_by_space: i8,
        pub p_sign_posn: i8,
        pub n_sign_posn: i8,
        pub int_n_cs_precedes: i8,
        pub int_n_sep_by_space: i8,
        pub int_n_sign_posn: i8,
        pub int_p_cs_precedes: i8,
        pub int_p_sep_by_space: i8,
        pub int_p_sign_posn: i8,
        __reserved: [usize; __DEFAULT_RESERVED_SIZE__],
    }

    pub struct tm {
        pub tm_sec: i32,
        pub tm_min: i32,
        pub tm_hour: i32,
        pub tm_mday: i32,
        pub tm_mon: i32,
        pub tm_year: i32,
        pub tm_wday: i32,
        pub tm_yday: i32,
        pub tm_isdst: i32,
        pub tm_gmtoff: isize,
        pub tm_zone: *const c_char,
        __reserved: [usize; __DEFAULT_RESERVED_SIZE__],
    }

    pub struct addrinfo {
        pub ai_flags: i32,
        pub ai_family: i32,
        pub ai_socktype: i32,
        pub ai_protocol: i32,
        pub ai_addrlen: socklen_t,
        pub ai_addr: *mut sockaddr,
        pub ai_canonname: *mut c_char,
        pub ai_next: *mut addrinfo,
        __reserved: [usize; __DEFAULT_RESERVED_SIZE__],
    }

    pub struct pthread_rwlock_t {
        __val: [usize; __PTHREAD_RWLOCK_SIZE__],
    }

    pub struct statvfs {
        pub f_bsize: usize,
        pub f_frsize: usize,
        pub f_blocks: fsblkcnt_t,
        pub f_bfree: fsblkcnt_t,
        pub f_bavail: fsblkcnt_t,
        pub f_files: fsblkcnt_t,
        pub f_ffree: fsblkcnt_t,
        pub f_favail: fsblkcnt_t,
        pub f_fsid: usize,
        pub f_flag: usize,
        pub f_namemax: usize,
        __reserved: [usize; __DEFAULT_RESERVED_SIZE__],
    }

    pub struct dirent {
        pub d_type: u8,
        pub d_name: [c_char; __NAME_MAX__ + 1],
    }

    pub struct fd_set {
        __val: [u32; __FDSET_SIZE__],
    }

    pub struct sigset_t {
        __val: [u32; __SIGSET_SIZE__],
    }

    pub struct sigaction {
        pub sa_handler: usize,
        pub sa_mask: sigset_t,
        pub sa_flags: i32,
        pub sa_user: usize,
        __reserved: [usize; __DEFAULT_RESERVED_SIZE__],
    }

    pub struct termios {
        pub c_iflag: tcflag_t,
        pub c_oflag: tcflag_t,
        pub c_cflag: tcflag_t,
        pub c_lflag: tcflag_t,
        pub c_cc: [cc_t; 12],
        pub c_speed: speed_t,
        __reserved: [usize; __DEFAULT_RESERVED_SIZE__],
    }

    pub struct in_addr {
        pub s_addr: in_addr_t,
    }

    pub struct sockaddr_in {
        pub sin_family: sa_family_t,
        pub sin_port: crate::in_port_t,
        pub sin_addr: crate::in_addr,
        pub sin_zero: [u8; 8],
    }

    pub struct sockaddr_in6 {
        pub sin6_family: sa_family_t,
        pub sin6_port: crate::in_port_t,
        pub sin6_flowinfo: u32,
        pub sin6_addr: crate::in6_addr,
        pub sin6_scope_id: u32,
    }

    pub struct sockaddr_un {
        pub sun_family: sa_family_t,
        pub sun_path: [c_char; 108],
    }

    pub struct sockaddr_storage {
        pub ss_family: sa_family_t,
        ss_data: [u32; __SOCKADDR_STORAGE_SIZE__],
    }

    pub struct ip_mreq {
        pub imr_multiaddr: in_addr,
        pub imr_interface: in_addr,
    }

    pub struct ipv6_mreq {
        pub ipv6mr_multiaddr: in6_addr,
        pub ipv6mr_interface: u32,
    }

    pub struct timeval {
        pub tv_sec: time_t,
        pub tv_usec: suseconds_t,
    }
}

// Reserved two pointer size for reserved area for some structures.
// This ensures that the size of these structures is large enough
// if more fields are added in the NuttX side.
//
// These structures are that defined by POSIX but only necessary fields are included,
// for example, struct passwd, https://pubs.opengroup.org/onlinepubs/009695399/basedefs/pwd.h.html,
// POSIX only defines following fields in struct passwd:
// char    *pw_name   User's login name.
// char    *pw_passwd Encrypted password.
// uid_t    pw_uid    Numerical user ID.
// gid_t    pw_gid    Numerical group ID.
// char    *pw_dir    Initial working directory.
// char    *pw_shell  Program to use as shell.
// Other fields can be different depending on the implementation.

const __DEFAULT_RESERVED_SIZE__: usize = 2;

const __SOCKADDR_STORAGE_SIZE__: usize = 36;
const __PTHREAD_ATTR_SIZE__: usize = 5;
const __PTHREAD_MUTEX_SIZE__: usize = 9;
const __PTHREAD_COND_SIZE__: usize = 7;
const __PTHREAD_CONDATTR_SIZE__: usize = 5;
const __PTHREAD_RWLOCK_SIZE__: usize = 17;
const __SEM_SIZE__: usize = 6;
const __NAME_MAX__: usize = 64;
const __FDSET_SIZE__: usize = 10;
const __SIGSET_SIZE__: usize = 8;

pub const PTHREAD_COND_INITIALIZER: pthread_cond_t = pthread_cond_t {
    __val: [0; __PTHREAD_COND_SIZE__],
};
pub const PTHREAD_MUTEX_INITIALIZER: pthread_mutex_t = pthread_mutex_t {
    __val: [0; __PTHREAD_MUTEX_SIZE__],
};

// dlfcn.h
pub const RTLD_DEFAULT: *mut c_void = 0 as *mut c_void;

// stdlib.h
pub const EXIT_SUCCESS: i32 = 0;
pub const EXIT_FAILURE: i32 = 1;

// time.h
pub const CLOCK_REALTIME: i32 = 0;
pub const CLOCK_MONOTONIC: i32 = 1;

// errno.h
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
pub const ELBIN: i32 = 134;
pub const EFTYPE: i32 = 135;
pub const ENMFILE: i32 = 136;
pub const EPROCLIM: i32 = 137;
pub const ENOTSUP: i32 = 138;
pub const ENOSHARE: i32 = 139;
pub const ECASECLASH: i32 = 140;

// fcntl.h
pub const FIOCLEX: i32 = 0x30b;
pub const F_SETFL: i32 = 0x9;
pub const F_DUPFD_CLOEXEC: i32 = 0x12;
pub const F_GETFD: i32 = 0x1;
pub const F_GETFL: i32 = 0x2;
pub const O_RDONLY: i32 = 0x1;
pub const O_WRONLY: i32 = 0x2;
pub const O_RDWR: i32 = 0x3;
pub const O_CREAT: i32 = 0x4;
pub const O_EXCL: i32 = 0x8;
pub const O_NOCTTY: i32 = 0x0;
pub const O_TRUNC: i32 = 0x20;
pub const O_APPEND: i32 = 0x10;
pub const O_NONBLOCK: i32 = 0x40;
pub const O_DSYNC: i32 = 0x80;
pub const O_DIRECT: i32 = 0x200;
pub const O_LARGEFILE: i32 = 0x2000;
pub const O_DIRECTORY: i32 = 0x800;
pub const O_NOFOLLOW: i32 = 0x1000;
pub const O_NOATIME: i32 = 0x40000;
pub const O_CLOEXEC: i32 = 0x400;
pub const O_ACCMODE: i32 = 0x0003;
pub const AT_FDCWD: i32 = -100;
pub const AT_REMOVEDIR: i32 = 0x200;

// sys/types.h
pub const SEEK_SET: i32 = 0;
pub const SEEK_CUR: i32 = 1;
pub const SEEK_END: i32 = 2;

// sys/stat.h
pub const S_IFDIR: u32 = 0x4000;
pub const S_IFLNK: u32 = 0xA000;
pub const S_IFREG: u32 = 0x8000;
pub const S_IFMT: u32 = 0xF000;
pub const S_IFIFO: u32 = 0x1000;
pub const S_IFSOCK: u32 = 0xc000;
pub const S_IFBLK: u32 = 0x6000;
pub const S_IFCHR: u32 = 0x2000;
pub const S_IRUSR: u32 = 0x100;
pub const S_IWUSR: u32 = 0x80;
pub const S_IXUSR: u32 = 0x40;
pub const S_IRGRP: u32 = 0x20;
pub const S_IWGRP: u32 = 0x10;
pub const S_IXGRP: u32 = 0x8;
pub const S_IROTH: u32 = 0x004;
pub const S_IWOTH: u32 = 0x002;
pub const S_IXOTH: u32 = 0x001;

// sys/poll.h
pub const POLLIN: i16 = 0x01;
pub const POLLOUT: i16 = 0x04;
pub const POLLHUP: i16 = 0x10;
pub const POLLERR: i16 = 0x08;
pub const POLLNVAL: i16 = 0x20;

// sys/socket.h
pub const AF_UNIX: i32 = 1;
pub const SOCK_DGRAM: i32 = 2;
pub const SOCK_STREAM: i32 = 1;
pub const AF_INET: i32 = 2;
pub const AF_INET6: i32 = 10;
pub const MSG_PEEK: i32 = 0x02;
pub const SOL_SOCKET: i32 = 1;
pub const SHUT_WR: i32 = 2;
pub const SHUT_RD: i32 = 1;
pub const SHUT_RDWR: i32 = 3;
pub const SO_ERROR: i32 = 4;
pub const SO_REUSEADDR: i32 = 11;
pub const SOMAXCONN: i32 = 8;
pub const SO_LINGER: i32 = 6;
pub const SO_RCVTIMEO: i32 = 0xa;
pub const SO_SNDTIMEO: i32 = 0xe;
pub const SO_BROADCAST: i32 = 1;

// netinet/tcp.h
pub const TCP_NODELAY: i32 = 0x10;

// nuttx/fs/ioctl.h
pub const FIONBIO: i32 = 0x30a;

// unistd.h
pub const STDIN_FILENO: i32 = 0;
pub const STDOUT_FILENO: i32 = 1;
pub const STDERR_FILENO: i32 = 2;
pub const _SC_PAGESIZE: i32 = 0x36;
pub const _SC_THREAD_STACK_MIN: i32 = 0x58;
pub const _SC_GETPW_R_SIZE_MAX: i32 = 0x25;

// signal.h
pub const SIGHUP: c_int = 1;
pub const SIGINT: c_int = 2;
pub const SIGQUIT: c_int = 3;
pub const SIGILL: c_int = 4;
pub const SIGTRAP: c_int = 5;
pub const SIGABRT: c_int = 6;
pub const SIGIOT: c_int = 6;
pub const SIGBUS: c_int = 7;
pub const SIGFPE: c_int = 8;
pub const SIGKILL: c_int = 9;
pub const SIGUSR1: c_int = 10;
pub const SIGSEGV: c_int = 11;
pub const SIGUSR2: c_int = 12;
pub const SIGPIPE: c_int = 13;
pub const SIGALRM: c_int = 14;
pub const SIGTERM: c_int = 15;
pub const SIGSTKFLT: c_int = 16;
pub const SIGCHLD: c_int = 17;
pub const SIGCONT: c_int = 18;
pub const SIGSTOP: c_int = 19;
pub const SIGTSTP: c_int = 20;
pub const SIGTTIN: c_int = 21;
pub const SIGTTOU: c_int = 22;
pub const SIGURG: c_int = 23;
pub const SIGXCPU: c_int = 24;
pub const SIGXFSZ: c_int = 25;
pub const SIGVTALRM: c_int = 26;
pub const SIGPROF: c_int = 27;
pub const SIGWINCH: c_int = 28;
pub const SIGIO: c_int = 29;
pub const SIGPOLL: c_int = SIGIO;
pub const SIGPWR: c_int = 30;
pub const SIGSYS: c_int = 31;

// pthread.h
pub const PTHREAD_MUTEX_NORMAL: i32 = 0;

// netinet/in.h
pub const IP_TTL: i32 = 0x1e;
pub const IPV6_V6ONLY: i32 = 0x17;
pub const IPV6_JOIN_GROUP: i32 = 0x11;
pub const IPV6_LEAVE_GROUP: i32 = 0x12;
pub const IP_MULTICAST_LOOP: i32 = 0x13;
pub const IPV6_MULTICAST_LOOP: i32 = 0x15;
pub const IP_MULTICAST_TTL: i32 = 0x12;
pub const IP_ADD_MEMBERSHIP: i32 = 0x14;
pub const IP_DROP_MEMBERSHIP: i32 = 0x15;

extern "C" {
    pub fn bind(sockfd: i32, addr: *const sockaddr, addrlen: socklen_t) -> i32;
    pub fn ioctl(fd: i32, request: i32, ...) -> i32;
    pub fn dirfd(dirp: *mut DIR) -> i32;
    pub fn recvfrom(
        sockfd: i32,
        buf: *mut c_void,
        len: usize,
        flags: i32,
        src_addr: *mut sockaddr,
        addrlen: *mut socklen_t,
    ) -> i32;

    pub fn pthread_create(
        thread: *mut pthread_t,
        attr: *const pthread_attr_t,
        start_routine: extern "C" fn(*mut c_void) -> *mut c_void,
        arg: *mut c_void,
    ) -> i32;

    pub fn clock_gettime(clockid: clockid_t, tp: *mut timespec) -> i32;
    pub fn futimens(fd: i32, times: *const timespec) -> i32;
    pub fn pthread_condattr_setclock(attr: *mut pthread_condattr_t, clock_id: clockid_t) -> i32;
    pub fn pthread_setname_np(thread: pthread_t, name: *const c_char) -> i32;
    pub fn pthread_getname_np(thread: pthread_t, name: *mut c_char, len: usize) -> i32;
    pub fn getrandom(buf: *mut c_void, buflen: usize, flags: u32) -> isize;
    pub fn arc4random() -> u32;
    pub fn arc4random_buf(bytes: *mut c_void, nbytes: usize);
}
