use crate::off_t;
use crate::prelude::*;

pub type wchar_t = i32;
pub type time_t = i64;
pub type mode_t = u32;
pub type nlink_t = u32;
pub type ino_t = u64;
pub type pthread_key_t = c_int;
pub type rlim_t = u64;
pub type speed_t = c_uint;
pub type tcflag_t = c_uint;
pub type nl_item = c_long;
pub type clockid_t = c_int;
pub type id_t = u32;
pub type sem_t = *mut sem;
pub type key_t = c_long;

#[cfg_attr(feature = "extra_traits", derive(Debug))]
pub enum timezone {}
impl Copy for timezone {}
impl Clone for timezone {
    fn clone(&self) -> timezone {
        *self
    }
}
#[cfg_attr(feature = "extra_traits", derive(Debug))]
pub enum sem {}
impl Copy for sem {}
impl Clone for sem {
    fn clone(&self) -> sem {
        *self
    }
}

s! {
    pub struct sched_param {
        pub sched_priority: c_int,
    }

    pub struct sigaction {
        pub sa_sigaction: crate::sighandler_t,
        pub sa_mask: crate::sigset_t,
        pub sa_flags: c_int,
    }

    pub struct stack_t {
        pub ss_sp: *mut c_void,
        pub ss_size: size_t,
        pub ss_flags: c_int,
    }

    pub struct in6_pktinfo {
        pub ipi6_addr: crate::in6_addr,
        pub ipi6_ifindex: c_uint,
    }

    pub struct termios {
        pub c_iflag: crate::tcflag_t,
        pub c_oflag: crate::tcflag_t,
        pub c_cflag: crate::tcflag_t,
        pub c_lflag: crate::tcflag_t,
        pub c_cc: [crate::cc_t; crate::NCCS],
        pub c_ispeed: c_int,
        pub c_ospeed: c_int,
    }

    pub struct flock {
        pub l_start: off_t,
        pub l_len: off_t,
        pub l_pid: crate::pid_t,
        pub l_type: c_short,
        pub l_whence: c_short,
    }

    pub struct ipc_perm {
        pub cuid: crate::uid_t,
        pub cgid: crate::gid_t,
        pub uid: crate::uid_t,
        pub gid: crate::gid_t,
        pub mode: mode_t,
        #[cfg(target_os = "openbsd")]
        pub seq: c_ushort,
        #[cfg(target_os = "netbsd")]
        pub _seq: c_ushort,
        #[cfg(target_os = "openbsd")]
        pub key: crate::key_t,
        #[cfg(target_os = "netbsd")]
        pub _key: crate::key_t,
    }

    pub struct ptrace_io_desc {
        pub piod_op: c_int,
        pub piod_offs: *mut c_void,
        pub piod_addr: *mut c_void,
        pub piod_len: size_t,
    }

    pub struct mmsghdr {
        pub msg_hdr: crate::msghdr,
        pub msg_len: c_uint,
    }
}

pub const D_T_FMT: crate::nl_item = 0;
pub const D_FMT: crate::nl_item = 1;
pub const T_FMT: crate::nl_item = 2;
pub const T_FMT_AMPM: crate::nl_item = 3;
pub const AM_STR: crate::nl_item = 4;
pub const PM_STR: crate::nl_item = 5;

pub const DAY_1: crate::nl_item = 6;
pub const DAY_2: crate::nl_item = 7;
pub const DAY_3: crate::nl_item = 8;
pub const DAY_4: crate::nl_item = 9;
pub const DAY_5: crate::nl_item = 10;
pub const DAY_6: crate::nl_item = 11;
pub const DAY_7: crate::nl_item = 12;

pub const ABDAY_1: crate::nl_item = 13;
pub const ABDAY_2: crate::nl_item = 14;
pub const ABDAY_3: crate::nl_item = 15;
pub const ABDAY_4: crate::nl_item = 16;
pub const ABDAY_5: crate::nl_item = 17;
pub const ABDAY_6: crate::nl_item = 18;
pub const ABDAY_7: crate::nl_item = 19;

pub const MON_1: crate::nl_item = 20;
pub const MON_2: crate::nl_item = 21;
pub const MON_3: crate::nl_item = 22;
pub const MON_4: crate::nl_item = 23;
pub const MON_5: crate::nl_item = 24;
pub const MON_6: crate::nl_item = 25;
pub const MON_7: crate::nl_item = 26;
pub const MON_8: crate::nl_item = 27;
pub const MON_9: crate::nl_item = 28;
pub const MON_10: crate::nl_item = 29;
pub const MON_11: crate::nl_item = 30;
pub const MON_12: crate::nl_item = 31;

pub const ABMON_1: crate::nl_item = 32;
pub const ABMON_2: crate::nl_item = 33;
pub const ABMON_3: crate::nl_item = 34;
pub const ABMON_4: crate::nl_item = 35;
pub const ABMON_5: crate::nl_item = 36;
pub const ABMON_6: crate::nl_item = 37;
pub const ABMON_7: crate::nl_item = 38;
pub const ABMON_8: crate::nl_item = 39;
pub const ABMON_9: crate::nl_item = 40;
pub const ABMON_10: crate::nl_item = 41;
pub const ABMON_11: crate::nl_item = 42;
pub const ABMON_12: crate::nl_item = 43;

pub const RADIXCHAR: crate::nl_item = 44;
pub const THOUSEP: crate::nl_item = 45;
pub const YESSTR: crate::nl_item = 46;
pub const YESEXPR: crate::nl_item = 47;
pub const NOSTR: crate::nl_item = 48;
pub const NOEXPR: crate::nl_item = 49;
pub const CRNCYSTR: crate::nl_item = 50;

pub const CODESET: crate::nl_item = 51;

pub const EXIT_FAILURE: c_int = 1;
pub const EXIT_SUCCESS: c_int = 0;
pub const RAND_MAX: c_int = 2147483647;
pub const EOF: c_int = -1;
pub const SEEK_SET: c_int = 0;
pub const SEEK_CUR: c_int = 1;
pub const SEEK_END: c_int = 2;
pub const _IOFBF: c_int = 0;
pub const _IONBF: c_int = 2;
pub const _IOLBF: c_int = 1;
pub const BUFSIZ: c_uint = 1024;
pub const FOPEN_MAX: c_uint = 20;
pub const FILENAME_MAX: c_uint = 1024;
pub const L_tmpnam: c_uint = 1024;
pub const O_NOCTTY: c_int = 32768;
pub const S_IFIFO: mode_t = 0o1_0000;
pub const S_IFCHR: mode_t = 0o2_0000;
pub const S_IFBLK: mode_t = 0o6_0000;
pub const S_IFDIR: mode_t = 0o4_0000;
pub const S_IFREG: mode_t = 0o10_0000;
pub const S_IFLNK: mode_t = 0o12_0000;
pub const S_IFSOCK: mode_t = 0o14_0000;
pub const S_IFMT: mode_t = 0o17_0000;
pub const S_IEXEC: mode_t = 0o0100;
pub const S_IWRITE: mode_t = 0o0200;
pub const S_IREAD: mode_t = 0o0400;
pub const S_IRWXU: mode_t = 0o0700;
pub const S_IXUSR: mode_t = 0o0100;
pub const S_IWUSR: mode_t = 0o0200;
pub const S_IRUSR: mode_t = 0o0400;
pub const S_IRWXG: mode_t = 0o0070;
pub const S_IXGRP: mode_t = 0o0010;
pub const S_IWGRP: mode_t = 0o0020;
pub const S_IRGRP: mode_t = 0o0040;
pub const S_IRWXO: mode_t = 0o0007;
pub const S_IXOTH: mode_t = 0o0001;
pub const S_IWOTH: mode_t = 0o0002;
pub const S_IROTH: mode_t = 0o0004;
pub const F_OK: c_int = 0;
pub const R_OK: c_int = 4;
pub const W_OK: c_int = 2;
pub const X_OK: c_int = 1;
pub const STDIN_FILENO: c_int = 0;
pub const STDOUT_FILENO: c_int = 1;
pub const STDERR_FILENO: c_int = 2;
pub const F_LOCK: c_int = 1;
pub const F_TEST: c_int = 3;
pub const F_TLOCK: c_int = 2;
pub const F_ULOCK: c_int = 0;
pub const F_GETLK: c_int = 7;
pub const F_SETLK: c_int = 8;
pub const F_SETLKW: c_int = 9;
pub const SIGHUP: c_int = 1;
pub const SIGINT: c_int = 2;
pub const SIGQUIT: c_int = 3;
pub const SIGILL: c_int = 4;
pub const SIGABRT: c_int = 6;
pub const SIGEMT: c_int = 7;
pub const SIGFPE: c_int = 8;
pub const SIGKILL: c_int = 9;
pub const SIGSEGV: c_int = 11;
pub const SIGPIPE: c_int = 13;
pub const SIGALRM: c_int = 14;
pub const SIGTERM: c_int = 15;

pub const PROT_NONE: c_int = 0;
pub const PROT_READ: c_int = 1;
pub const PROT_WRITE: c_int = 2;
pub const PROT_EXEC: c_int = 4;

pub const MAP_FILE: c_int = 0x0000;
pub const MAP_SHARED: c_int = 0x0001;
pub const MAP_PRIVATE: c_int = 0x0002;
pub const MAP_FIXED: c_int = 0x0010;
pub const MAP_ANON: c_int = 0x1000;
pub const MAP_ANONYMOUS: c_int = MAP_ANON;

pub const MAP_FAILED: *mut c_void = !0 as *mut c_void;

pub const IPC_CREAT: c_int = 0o001000;
pub const IPC_EXCL: c_int = 0o002000;
pub const IPC_NOWAIT: c_int = 0o004000;

pub const IPC_PRIVATE: crate::key_t = 0;

pub const IPC_RMID: c_int = 0;
pub const IPC_SET: c_int = 1;
pub const IPC_STAT: c_int = 2;

pub const IPC_R: c_int = 0o000400;
pub const IPC_W: c_int = 0o000200;
pub const IPC_M: c_int = 0o010000;

pub const SHM_R: c_int = IPC_R;
pub const SHM_W: c_int = IPC_W;

pub const MCL_CURRENT: c_int = 0x0001;
pub const MCL_FUTURE: c_int = 0x0002;

pub const MS_ASYNC: c_int = 0x0001;

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
pub const EDEADLK: c_int = 11;
pub const ENOMEM: c_int = 12;
pub const EACCES: c_int = 13;
pub const EFAULT: c_int = 14;
pub const ENOTBLK: c_int = 15;
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
pub const EAGAIN: c_int = 35;
pub const EWOULDBLOCK: c_int = 35;
pub const EINPROGRESS: c_int = 36;
pub const EALREADY: c_int = 37;
pub const ENOTSOCK: c_int = 38;
pub const EDESTADDRREQ: c_int = 39;
pub const EMSGSIZE: c_int = 40;
pub const EPROTOTYPE: c_int = 41;
pub const ENOPROTOOPT: c_int = 42;
pub const EPROTONOSUPPORT: c_int = 43;
pub const ESOCKTNOSUPPORT: c_int = 44;
pub const EOPNOTSUPP: c_int = 45;
pub const EPFNOSUPPORT: c_int = 46;
pub const EAFNOSUPPORT: c_int = 47;
pub const EADDRINUSE: c_int = 48;
pub const EADDRNOTAVAIL: c_int = 49;
pub const ENETDOWN: c_int = 50;
pub const ENETUNREACH: c_int = 51;
pub const ENETRESET: c_int = 52;
pub const ECONNABORTED: c_int = 53;
pub const ECONNRESET: c_int = 54;
pub const ENOBUFS: c_int = 55;
pub const EISCONN: c_int = 56;
pub const ENOTCONN: c_int = 57;
pub const ESHUTDOWN: c_int = 58;
pub const ETOOMANYREFS: c_int = 59;
pub const ETIMEDOUT: c_int = 60;
pub const ECONNREFUSED: c_int = 61;
pub const ELOOP: c_int = 62;
pub const ENAMETOOLONG: c_int = 63;
pub const EHOSTDOWN: c_int = 64;
pub const EHOSTUNREACH: c_int = 65;
pub const ENOTEMPTY: c_int = 66;
pub const EPROCLIM: c_int = 67;
pub const EUSERS: c_int = 68;
pub const EDQUOT: c_int = 69;
pub const ESTALE: c_int = 70;
pub const EREMOTE: c_int = 71;
pub const EBADRPC: c_int = 72;
pub const ERPCMISMATCH: c_int = 73;
pub const EPROGUNAVAIL: c_int = 74;
pub const EPROGMISMATCH: c_int = 75;
pub const EPROCUNAVAIL: c_int = 76;
pub const ENOLCK: c_int = 77;
pub const ENOSYS: c_int = 78;
pub const EFTYPE: c_int = 79;
pub const EAUTH: c_int = 80;
pub const ENEEDAUTH: c_int = 81;

pub const F_DUPFD: c_int = 0;
pub const F_GETFD: c_int = 1;
pub const F_SETFD: c_int = 2;
pub const F_GETFL: c_int = 3;
pub const F_SETFL: c_int = 4;

pub const SIGTRAP: c_int = 5;

pub const GLOB_APPEND: c_int = 0x0001;
pub const GLOB_DOOFFS: c_int = 0x0002;
pub const GLOB_ERR: c_int = 0x0004;
pub const GLOB_MARK: c_int = 0x0008;
pub const GLOB_NOCHECK: c_int = 0x0010;
pub const GLOB_NOSORT: c_int = 0x0020;
pub const GLOB_NOESCAPE: c_int = 0x1000;

pub const GLOB_NOSPACE: c_int = -1;
pub const GLOB_ABORTED: c_int = -2;
pub const GLOB_NOMATCH: c_int = -3;
pub const GLOB_NOSYS: c_int = -4;

pub const POSIX_MADV_NORMAL: c_int = 0;
pub const POSIX_MADV_RANDOM: c_int = 1;
pub const POSIX_MADV_SEQUENTIAL: c_int = 2;
pub const POSIX_MADV_WILLNEED: c_int = 3;
pub const POSIX_MADV_DONTNEED: c_int = 4;

// DIFF(main): changed to `c_short` in f62eb023ab
pub const POSIX_SPAWN_RESETIDS: c_int = 0x01;
pub const POSIX_SPAWN_SETPGROUP: c_int = 0x02;
pub const POSIX_SPAWN_SETSCHEDPARAM: c_int = 0x04;
pub const POSIX_SPAWN_SETSCHEDULER: c_int = 0x08;
pub const POSIX_SPAWN_SETSIGDEF: c_int = 0x10;
pub const POSIX_SPAWN_SETSIGMASK: c_int = 0x20;

pub const PTHREAD_CREATE_JOINABLE: c_int = 0;
pub const PTHREAD_CREATE_DETACHED: c_int = 1;

pub const PIOD_READ_D: c_int = 1;
pub const PIOD_WRITE_D: c_int = 2;
pub const PIOD_READ_I: c_int = 3;
pub const PIOD_WRITE_I: c_int = 4;
pub const PIOD_READ_AUXV: c_int = 5;

pub const PT_TRACE_ME: c_int = 0;
pub const PT_READ_I: c_int = 1;
pub const PT_READ_D: c_int = 2;
pub const PT_WRITE_I: c_int = 4;
pub const PT_WRITE_D: c_int = 5;
pub const PT_CONTINUE: c_int = 7;
pub const PT_KILL: c_int = 8;
pub const PT_ATTACH: c_int = 9;
pub const PT_DETACH: c_int = 10;
pub const PT_IO: c_int = 11;

// http://man.openbsd.org/OpenBSD-current/man2/clock_getres.2
// The man page says clock_gettime(3) can accept various values as clockid_t but
// http://fxr.watson.org/fxr/source/kern/kern_time.c?v=OPENBSD;im=excerpts#L161
// the implementation rejects anything other than the below two
//
// http://netbsd.gw.com/cgi-bin/man-cgi?clock_gettime
// https://github.com/jsonn/src/blob/HEAD/sys/kern/subr_time.c#L222
// Basically the same goes for NetBSD
pub const CLOCK_REALTIME: crate::clockid_t = 0;
pub const CLOCK_MONOTONIC: crate::clockid_t = 3;

pub const RLIMIT_CPU: c_int = 0;
pub const RLIMIT_FSIZE: c_int = 1;
pub const RLIMIT_DATA: c_int = 2;
pub const RLIMIT_STACK: c_int = 3;
pub const RLIMIT_CORE: c_int = 4;
pub const RLIMIT_RSS: c_int = 5;
pub const RLIMIT_MEMLOCK: c_int = 6;
pub const RLIMIT_NPROC: c_int = 7;
pub const RLIMIT_NOFILE: c_int = 8;

pub const RLIM_INFINITY: rlim_t = 0x7fff_ffff_ffff_ffff;
pub const RLIM_SAVED_MAX: rlim_t = RLIM_INFINITY;
pub const RLIM_SAVED_CUR: rlim_t = RLIM_INFINITY;

pub const RUSAGE_SELF: c_int = 0;
pub const RUSAGE_CHILDREN: c_int = -1;

pub const MADV_NORMAL: c_int = 0;
pub const MADV_RANDOM: c_int = 1;
pub const MADV_SEQUENTIAL: c_int = 2;
pub const MADV_WILLNEED: c_int = 3;
pub const MADV_DONTNEED: c_int = 4;
pub const MADV_FREE: c_int = 6;

// sys/fstypes.h in NetBSD, or sys/mount.h in OpenBSD
pub const MNT_NODEV: c_int = 0x00000010;
pub const MNT_LOCAL: c_int = 0x00001000;
pub const MNT_QUOTA: c_int = 0x00002000;

// sys/ioccom.h in NetBSD and OpenBSD
pub const IOCPARM_MASK: u32 = 0x1fff;

pub const IOC_VOID: c_ulong = 0x20000000;
pub const IOC_OUT: c_ulong = 0x40000000;
pub const IOC_IN: c_ulong = 0x80000000;
pub const IOC_INOUT: c_ulong = IOC_IN | IOC_OUT;
pub const IOC_DIRMASK: c_ulong = 0xe0000000;

pub const fn _IO(g: c_ulong, n: c_ulong) -> c_ulong {
    _IOC(IOC_VOID, g, n, 0)
}

/// Build an ioctl number for an read-only ioctl.
pub const fn _IOR<T>(g: c_ulong, n: c_ulong) -> c_ulong {
    _IOC(IOC_OUT, g, n, mem::size_of::<T>() as c_ulong)
}

/// Build an ioctl number for an write-only ioctl.
pub const fn _IOW<T>(g: c_ulong, n: c_ulong) -> c_ulong {
    _IOC(IOC_IN, g, n, mem::size_of::<T>() as c_ulong)
}

/// Build an ioctl number for a read-write ioctl.
pub const fn _IOWR<T>(g: c_ulong, n: c_ulong) -> c_ulong {
    _IOC(IOC_INOUT, g, n, mem::size_of::<T>() as c_ulong)
}

pub const AF_UNSPEC: c_int = 0;
pub const AF_LOCAL: c_int = 1;
pub const AF_UNIX: c_int = AF_LOCAL;
pub const AF_INET: c_int = 2;
pub const AF_IMPLINK: c_int = 3;
pub const AF_PUP: c_int = 4;
pub const AF_CHAOS: c_int = 5;
pub const AF_NS: c_int = 6;
pub const AF_ISO: c_int = 7;
pub const AF_OSI: c_int = AF_ISO;
pub const AF_DATAKIT: c_int = 9;
pub const AF_CCITT: c_int = 10;
pub const AF_SNA: c_int = 11;
pub const AF_DECnet: c_int = 12;
pub const AF_DLI: c_int = 13;
pub const AF_LAT: c_int = 14;
pub const AF_HYLINK: c_int = 15;
pub const AF_APPLETALK: c_int = 16;
pub const AF_LINK: c_int = 18;
pub const pseudo_AF_XTP: c_int = 19;
pub const AF_COIP: c_int = 20;
pub const AF_CNT: c_int = 21;
pub const pseudo_AF_RTIP: c_int = 22;
pub const AF_IPX: c_int = 23;
pub const AF_INET6: c_int = 24;
pub const pseudo_AF_PIP: c_int = 25;
pub const AF_ISDN: c_int = 26;
pub const AF_E164: c_int = AF_ISDN;
pub const AF_NATM: c_int = 27;

pub const PF_UNSPEC: c_int = AF_UNSPEC;
pub const PF_LOCAL: c_int = AF_LOCAL;
pub const PF_UNIX: c_int = PF_LOCAL;
pub const PF_INET: c_int = AF_INET;
pub const PF_IMPLINK: c_int = AF_IMPLINK;
pub const PF_PUP: c_int = AF_PUP;
pub const PF_CHAOS: c_int = AF_CHAOS;
pub const PF_NS: c_int = AF_NS;
pub const PF_ISO: c_int = AF_ISO;
pub const PF_OSI: c_int = AF_ISO;
pub const PF_DATAKIT: c_int = AF_DATAKIT;
pub const PF_CCITT: c_int = AF_CCITT;
pub const PF_SNA: c_int = AF_SNA;
pub const PF_DECnet: c_int = AF_DECnet;
pub const PF_DLI: c_int = AF_DLI;
pub const PF_LAT: c_int = AF_LAT;
pub const PF_HYLINK: c_int = AF_HYLINK;
pub const PF_APPLETALK: c_int = AF_APPLETALK;
pub const PF_LINK: c_int = AF_LINK;
pub const PF_XTP: c_int = pseudo_AF_XTP;
pub const PF_COIP: c_int = AF_COIP;
pub const PF_CNT: c_int = AF_CNT;
pub const PF_IPX: c_int = AF_IPX;
pub const PF_INET6: c_int = AF_INET6;
pub const PF_RTIP: c_int = pseudo_AF_RTIP;
pub const PF_PIP: c_int = pseudo_AF_PIP;
pub const PF_ISDN: c_int = AF_ISDN;
pub const PF_NATM: c_int = AF_NATM;

pub const SOCK_STREAM: c_int = 1;
pub const SOCK_DGRAM: c_int = 2;
pub const SOCK_RAW: c_int = 3;
pub const SOCK_RDM: c_int = 4;
pub const SOCK_SEQPACKET: c_int = 5;
pub const IP_TTL: c_int = 4;
pub const IP_HDRINCL: c_int = 2;
pub const IP_ADD_MEMBERSHIP: c_int = 12;
pub const IP_DROP_MEMBERSHIP: c_int = 13;
pub const IPV6_RECVPKTINFO: c_int = 36;
pub const IPV6_PKTINFO: c_int = 46;
pub const IPV6_RECVTCLASS: c_int = 57;
pub const IPV6_TCLASS: c_int = 61;

pub const SOL_SOCKET: c_int = 0xffff;
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
pub const SO_SNDBUF: c_int = 0x1001;
pub const SO_RCVBUF: c_int = 0x1002;
pub const SO_SNDLOWAT: c_int = 0x1003;
pub const SO_RCVLOWAT: c_int = 0x1004;
pub const SO_ERROR: c_int = 0x1007;
pub const SO_TYPE: c_int = 0x1008;

pub const SOMAXCONN: c_int = 128;

pub const MSG_OOB: c_int = 0x1;
pub const MSG_PEEK: c_int = 0x2;
pub const MSG_DONTROUTE: c_int = 0x4;
pub const MSG_EOR: c_int = 0x8;
pub const MSG_TRUNC: c_int = 0x10;
pub const MSG_CTRUNC: c_int = 0x20;
pub const MSG_WAITALL: c_int = 0x40;
pub const MSG_DONTWAIT: c_int = 0x80;
pub const MSG_BCAST: c_int = 0x100;
pub const MSG_MCAST: c_int = 0x200;
pub const MSG_NOSIGNAL: c_int = 0x400;
pub const MSG_CMSG_CLOEXEC: c_int = 0x800;

pub const SHUT_RD: c_int = 0;
pub const SHUT_WR: c_int = 1;
pub const SHUT_RDWR: c_int = 2;

pub const LOCK_SH: c_int = 1;
pub const LOCK_EX: c_int = 2;
pub const LOCK_NB: c_int = 4;
pub const LOCK_UN: c_int = 8;

pub const IPPROTO_RAW: c_int = 255;

pub const _SC_ARG_MAX: c_int = 1;
pub const _SC_CHILD_MAX: c_int = 2;
pub const _SC_NGROUPS_MAX: c_int = 4;
pub const _SC_OPEN_MAX: c_int = 5;
pub const _SC_JOB_CONTROL: c_int = 6;
pub const _SC_SAVED_IDS: c_int = 7;
pub const _SC_VERSION: c_int = 8;
pub const _SC_BC_BASE_MAX: c_int = 9;
pub const _SC_BC_DIM_MAX: c_int = 10;
pub const _SC_BC_SCALE_MAX: c_int = 11;
pub const _SC_BC_STRING_MAX: c_int = 12;
pub const _SC_COLL_WEIGHTS_MAX: c_int = 13;
pub const _SC_EXPR_NEST_MAX: c_int = 14;
pub const _SC_LINE_MAX: c_int = 15;
pub const _SC_RE_DUP_MAX: c_int = 16;
pub const _SC_2_VERSION: c_int = 17;
pub const _SC_2_C_BIND: c_int = 18;
pub const _SC_2_C_DEV: c_int = 19;
pub const _SC_2_CHAR_TERM: c_int = 20;
pub const _SC_2_FORT_DEV: c_int = 21;
pub const _SC_2_FORT_RUN: c_int = 22;
pub const _SC_2_LOCALEDEF: c_int = 23;
pub const _SC_2_SW_DEV: c_int = 24;
pub const _SC_2_UPE: c_int = 25;
pub const _SC_STREAM_MAX: c_int = 26;
pub const _SC_TZNAME_MAX: c_int = 27;
pub const _SC_PAGESIZE: c_int = 28;
pub const _SC_PAGE_SIZE: c_int = _SC_PAGESIZE;
pub const _SC_FSYNC: c_int = 29;
pub const _SC_XOPEN_SHM: c_int = 30;

pub const Q_GETQUOTA: c_int = 0x300;
pub const Q_SETQUOTA: c_int = 0x400;

pub const RTLD_GLOBAL: c_int = 0x100;

pub const LOG_NFACILITIES: c_int = 24;

pub const HW_NCPU: c_int = 3;

pub const B0: speed_t = 0;
pub const B50: speed_t = 50;
pub const B75: speed_t = 75;
pub const B110: speed_t = 110;
pub const B134: speed_t = 134;
pub const B150: speed_t = 150;
pub const B200: speed_t = 200;
pub const B300: speed_t = 300;
pub const B600: speed_t = 600;
pub const B1200: speed_t = 1200;
pub const B1800: speed_t = 1800;
pub const B2400: speed_t = 2400;
pub const B4800: speed_t = 4800;
pub const B9600: speed_t = 9600;
pub const B19200: speed_t = 19200;
pub const B38400: speed_t = 38400;
pub const B7200: speed_t = 7200;
pub const B14400: speed_t = 14400;
pub const B28800: speed_t = 28800;
pub const B57600: speed_t = 57600;
pub const B76800: speed_t = 76800;
pub const B115200: speed_t = 115200;
pub const B230400: speed_t = 230400;
pub const EXTA: speed_t = 19200;
pub const EXTB: speed_t = 38400;

pub const SEM_FAILED: *mut sem_t = 0 as *mut sem_t;

pub const CRTSCTS: crate::tcflag_t = 0x00010000;
pub const CRTS_IFLOW: crate::tcflag_t = CRTSCTS;
pub const CCTS_OFLOW: crate::tcflag_t = CRTSCTS;
pub const OCRNL: crate::tcflag_t = 0x10;

pub const TIOCEXCL: c_ulong = 0x2000740d;
pub const TIOCNXCL: c_ulong = 0x2000740e;
pub const TIOCFLUSH: c_ulong = 0x80047410;
pub const TIOCGETA: c_ulong = 0x402c7413;
pub const TIOCSETA: c_ulong = 0x802c7414;
pub const TIOCSETAW: c_ulong = 0x802c7415;
pub const TIOCSETAF: c_ulong = 0x802c7416;
pub const TIOCGETD: c_ulong = 0x4004741a;
pub const TIOCSETD: c_ulong = 0x8004741b;
pub const TIOCMGET: c_ulong = 0x4004746a;
pub const TIOCMBIC: c_ulong = 0x8004746b;
pub const TIOCMBIS: c_ulong = 0x8004746c;
pub const TIOCMSET: c_ulong = 0x8004746d;
pub const TIOCSTART: c_ulong = 0x2000746e;
pub const TIOCSTOP: c_ulong = 0x2000746f;
pub const TIOCSCTTY: c_ulong = 0x20007461;
pub const TIOCGWINSZ: c_ulong = 0x40087468;
pub const TIOCSWINSZ: c_ulong = 0x80087467;
pub const TIOCM_LE: c_int = 0o0001;
pub const TIOCM_DTR: c_int = 0o0002;
pub const TIOCM_RTS: c_int = 0o0004;
pub const TIOCM_ST: c_int = 0o0010;
pub const TIOCM_SR: c_int = 0o0020;
pub const TIOCM_CTS: c_int = 0o0040;
pub const TIOCM_CAR: c_int = 0o0100;
pub const TIOCM_RNG: c_int = 0o0200;
pub const TIOCM_DSR: c_int = 0o0400;
pub const TIOCM_CD: c_int = TIOCM_CAR;
pub const TIOCM_RI: c_int = TIOCM_RNG;

pub const TIMER_ABSTIME: c_int = 1;

// sys/reboot.h

pub const RB_AUTOBOOT: c_int = 0;

pub const TCP_INFO: c_int = 9;

#[link(name = "util")]
extern "C" {
    pub fn setgrent();
    pub fn sem_destroy(sem: *mut sem_t) -> c_int;
    pub fn sem_init(sem: *mut sem_t, pshared: c_int, value: c_uint) -> c_int;

    pub fn daemon(nochdir: c_int, noclose: c_int) -> c_int;
    pub fn accept4(
        s: c_int,
        addr: *mut crate::sockaddr,
        addrlen: *mut crate::socklen_t,
        flags: c_int,
    ) -> c_int;
    pub fn mincore(addr: *mut c_void, len: size_t, vec: *mut c_char) -> c_int;
    #[cfg_attr(target_os = "netbsd", link_name = "__clock_getres50")]
    pub fn clock_getres(clk_id: crate::clockid_t, tp: *mut crate::timespec) -> c_int;
    #[cfg_attr(target_os = "netbsd", link_name = "__clock_gettime50")]
    pub fn clock_gettime(clk_id: crate::clockid_t, tp: *mut crate::timespec) -> c_int;
    #[cfg_attr(target_os = "netbsd", link_name = "__clock_settime50")]
    pub fn clock_settime(clk_id: crate::clockid_t, tp: *const crate::timespec) -> c_int;
    pub fn __errno() -> *mut c_int;
    pub fn shm_open(name: *const c_char, oflag: c_int, mode: mode_t) -> c_int;
    pub fn memrchr(cx: *const c_void, c: c_int, n: size_t) -> *mut c_void;
    pub fn mkostemp(template: *mut c_char, flags: c_int) -> c_int;
    pub fn mkostemps(template: *mut c_char, suffixlen: c_int, flags: c_int) -> c_int;
    pub fn pwritev(fd: c_int, iov: *const crate::iovec, iovcnt: c_int, offset: off_t) -> ssize_t;
    pub fn preadv(fd: c_int, iov: *const crate::iovec, iovcnt: c_int, offset: off_t) -> ssize_t;
    pub fn futimens(fd: c_int, times: *const crate::timespec) -> c_int;
    pub fn utimensat(
        dirfd: c_int,
        path: *const c_char,
        times: *const crate::timespec,
        flag: c_int,
    ) -> c_int;
    pub fn fdatasync(fd: c_int) -> c_int;
    pub fn login_tty(fd: c_int) -> c_int;
    pub fn getpriority(which: c_int, who: crate::id_t) -> c_int;
    pub fn setpriority(which: c_int, who: crate::id_t, prio: c_int) -> c_int;

    pub fn mknodat(dirfd: c_int, pathname: *const c_char, mode: mode_t, dev: dev_t) -> c_int;
    pub fn mkfifoat(dirfd: c_int, pathname: *const c_char, mode: mode_t) -> c_int;
    pub fn sem_timedwait(sem: *mut sem_t, abstime: *const crate::timespec) -> c_int;
    pub fn sem_getvalue(sem: *mut sem_t, sval: *mut c_int) -> c_int;
    pub fn pthread_condattr_setclock(
        attr: *mut pthread_condattr_t,
        clock_id: crate::clockid_t,
    ) -> c_int;
    pub fn sethostname(name: *const c_char, len: size_t) -> c_int;
    pub fn pthread_mutex_timedlock(
        lock: *mut pthread_mutex_t,
        abstime: *const crate::timespec,
    ) -> c_int;
    pub fn pthread_spin_init(lock: *mut pthread_spinlock_t, pshared: c_int) -> c_int;
    pub fn pthread_spin_destroy(lock: *mut pthread_spinlock_t) -> c_int;
    pub fn pthread_spin_lock(lock: *mut pthread_spinlock_t) -> c_int;
    pub fn pthread_spin_trylock(lock: *mut pthread_spinlock_t) -> c_int;
    pub fn pthread_spin_unlock(lock: *mut pthread_spinlock_t) -> c_int;
    pub fn pthread_setschedparam(
        native: crate::pthread_t,
        policy: c_int,
        param: *const sched_param,
    ) -> c_int;
    pub fn pthread_getschedparam(
        native: crate::pthread_t,
        policy: *mut c_int,
        param: *mut sched_param,
    ) -> c_int;
    pub fn pipe2(fds: *mut c_int, flags: c_int) -> c_int;

    pub fn getgrouplist(
        name: *const c_char,
        basegid: crate::gid_t,
        groups: *mut crate::gid_t,
        ngroups: *mut c_int,
    ) -> c_int;
    pub fn initgroups(name: *const c_char, basegid: crate::gid_t) -> c_int;
    pub fn getdomainname(name: *mut c_char, len: size_t) -> c_int;
    pub fn setdomainname(name: *const c_char, len: size_t) -> c_int;
    pub fn uname(buf: *mut crate::utsname) -> c_int;

    pub fn shmget(key: crate::key_t, size: size_t, shmflg: c_int) -> c_int;
    pub fn shmat(shmid: c_int, shmaddr: *const c_void, shmflg: c_int) -> *mut c_void;
    pub fn shmdt(shmaddr: *const c_void) -> c_int;
    pub fn shmctl(shmid: c_int, cmd: c_int, buf: *mut crate::shmid_ds) -> c_int;

    // DIFF(main): changed to `*const *mut` in e77f551de9
    pub fn execvpe(
        file: *const c_char,
        argv: *const *const c_char,
        envp: *const *const c_char,
    ) -> c_int;

    pub fn waitid(
        idtype: idtype_t,
        id: crate::id_t,
        infop: *mut crate::siginfo_t,
        options: c_int,
    ) -> c_int;

    pub fn posix_spawn(
        pid: *mut crate::pid_t,
        path: *const c_char,
        file_actions: *const crate::posix_spawn_file_actions_t,
        attrp: *const crate::posix_spawnattr_t,
        argv: *const *mut c_char,
        envp: *const *mut c_char,
    ) -> c_int;
    pub fn posix_spawnp(
        pid: *mut crate::pid_t,
        file: *const c_char,
        file_actions: *const crate::posix_spawn_file_actions_t,
        attrp: *const crate::posix_spawnattr_t,
        argv: *const *mut c_char,
        envp: *const *mut c_char,
    ) -> c_int;
    pub fn posix_spawnattr_init(attr: *mut posix_spawnattr_t) -> c_int;
    pub fn posix_spawnattr_destroy(attr: *mut posix_spawnattr_t) -> c_int;
    pub fn posix_spawnattr_getsigdefault(
        attr: *const posix_spawnattr_t,
        default: *mut crate::sigset_t,
    ) -> c_int;
    pub fn posix_spawnattr_setsigdefault(
        attr: *mut posix_spawnattr_t,
        default: *const crate::sigset_t,
    ) -> c_int;
    pub fn posix_spawnattr_getsigmask(
        attr: *const posix_spawnattr_t,
        default: *mut crate::sigset_t,
    ) -> c_int;
    pub fn posix_spawnattr_setsigmask(
        attr: *mut posix_spawnattr_t,
        default: *const crate::sigset_t,
    ) -> c_int;
    pub fn posix_spawnattr_getflags(attr: *const posix_spawnattr_t, flags: *mut c_short) -> c_int;
    pub fn posix_spawnattr_setflags(attr: *mut posix_spawnattr_t, flags: c_short) -> c_int;
    pub fn posix_spawnattr_getpgroup(
        attr: *const posix_spawnattr_t,
        flags: *mut crate::pid_t,
    ) -> c_int;
    pub fn posix_spawnattr_setpgroup(attr: *mut posix_spawnattr_t, flags: crate::pid_t) -> c_int;
    pub fn posix_spawnattr_getschedpolicy(
        attr: *const posix_spawnattr_t,
        flags: *mut c_int,
    ) -> c_int;
    pub fn posix_spawnattr_setschedpolicy(attr: *mut posix_spawnattr_t, flags: c_int) -> c_int;
    pub fn posix_spawnattr_getschedparam(
        attr: *const posix_spawnattr_t,
        param: *mut crate::sched_param,
    ) -> c_int;
    pub fn posix_spawnattr_setschedparam(
        attr: *mut posix_spawnattr_t,
        param: *const crate::sched_param,
    ) -> c_int;

    pub fn posix_spawn_file_actions_init(actions: *mut posix_spawn_file_actions_t) -> c_int;
    pub fn posix_spawn_file_actions_destroy(actions: *mut posix_spawn_file_actions_t) -> c_int;
    pub fn posix_spawn_file_actions_addopen(
        actions: *mut posix_spawn_file_actions_t,
        fd: c_int,
        path: *const c_char,
        oflag: c_int,
        mode: mode_t,
    ) -> c_int;
    pub fn posix_spawn_file_actions_addclose(
        actions: *mut posix_spawn_file_actions_t,
        fd: c_int,
    ) -> c_int;
    pub fn posix_spawn_file_actions_adddup2(
        actions: *mut posix_spawn_file_actions_t,
        fd: c_int,
        newfd: c_int,
    ) -> c_int;
}

extern "C" {
    pub fn reallocarray(ptr: *mut c_void, nmemb: size_t, size: size_t) -> *mut c_void;
    pub fn gethostid() -> c_long;
    pub fn sethostid(hostid: c_long) -> c_int;
    pub fn ftok(path: *const c_char, id: c_int) -> crate::key_t;

    pub fn dirname(path: *mut c_char) -> *mut c_char;
    pub fn basename(path: *mut c_char) -> *mut c_char;
    pub fn getentropy(buf: *mut c_void, buflen: size_t) -> c_int;

    pub fn sendmmsg(sockfd: c_int, mmsg: *mut crate::mmsghdr, vlen: c_uint, flags: c_int) -> c_int;
    pub fn recvmmsg(
        sockfd: c_int,
        mmsg: *mut crate::mmsghdr,
        vlen: c_uint,
        flags: c_int,
        timeout: *mut crate::timespec,
    ) -> c_int;

    pub fn closefrom(lowfd: c_int) -> c_int;
}

cfg_if! {
    if #[cfg(target_os = "netbsd")] {
        mod netbsd;
        pub use self::netbsd::*;
    } else if #[cfg(target_os = "openbsd")] {
        mod openbsd;
        pub use self::openbsd::*;
    } else {
        // Unknown target_os
    }
}
