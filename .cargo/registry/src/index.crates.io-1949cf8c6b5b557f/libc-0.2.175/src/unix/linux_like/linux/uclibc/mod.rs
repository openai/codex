// FIXME(ulibc): this module has definitions that are redundant with the parent
#![allow(dead_code)]

use crate::off64_t;
use crate::prelude::*;

pub type shmatt_t = c_ulong;
pub type msgqnum_t = c_ulong;
pub type msglen_t = c_ulong;
pub type regoff_t = c_int;
pub type rlim_t = c_ulong;
pub type __rlimit_resource_t = c_ulong;
pub type __priority_which_t = c_uint;

cfg_if! {
    if #[cfg(doc)] {
        // Used in `linux::arch` to define ioctl constants.
        pub(crate) type Ioctl = c_ulong;
    } else {
        #[doc(hidden)]
        pub type Ioctl = c_ulong;
    }
}

s! {
    pub struct statvfs {
        // Different than GNU!
        pub f_bsize: c_ulong,
        pub f_frsize: c_ulong,
        pub f_blocks: crate::fsblkcnt_t,
        pub f_bfree: crate::fsblkcnt_t,
        pub f_bavail: crate::fsblkcnt_t,
        pub f_files: crate::fsfilcnt_t,
        pub f_ffree: crate::fsfilcnt_t,
        pub f_favail: crate::fsfilcnt_t,
        #[cfg(target_endian = "little")]
        pub f_fsid: c_ulong,
        #[cfg(target_pointer_width = "32")]
        __f_unused: c_int,
        #[cfg(target_endian = "big")]
        pub f_fsid: c_ulong,
        pub f_flag: c_ulong,
        pub f_namemax: c_ulong,
        __f_spare: [c_int; 6],
    }

    pub struct regex_t {
        __buffer: *mut c_void,
        __allocated: size_t,
        __used: size_t,
        __syntax: c_ulong,
        __fastmap: *mut c_char,
        __translate: *mut c_char,
        __re_nsub: size_t,
        __bitfield: u8,
    }

    pub struct rtentry {
        pub rt_pad1: c_ulong,
        pub rt_dst: crate::sockaddr,
        pub rt_gateway: crate::sockaddr,
        pub rt_genmask: crate::sockaddr,
        pub rt_flags: c_ushort,
        pub rt_pad2: c_short,
        pub rt_pad3: c_ulong,
        pub rt_tos: c_uchar,
        pub rt_class: c_uchar,
        #[cfg(target_pointer_width = "64")]
        pub rt_pad4: [c_short; 3usize],
        #[cfg(not(target_pointer_width = "64"))]
        pub rt_pad4: c_short,
        pub rt_metric: c_short,
        pub rt_dev: *mut c_char,
        pub rt_mtu: c_ulong,
        pub rt_window: c_ulong,
        pub rt_irtt: c_ushort,
    }

    pub struct __exit_status {
        pub e_termination: c_short,
        pub e_exit: c_short,
    }

    pub struct ptrace_peeksiginfo_args {
        pub off: crate::__u64,
        pub flags: crate::__u32,
        pub nr: crate::__s32,
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
        size: [u8; crate::__SIZEOF_PTHREAD_MUTEXATTR_T],
    }

    #[repr(align(4))]
    pub struct pthread_condattr_t {
        size: [u8; crate::__SIZEOF_PTHREAD_CONDATTR_T],
    }

    pub struct tcp_info {
        pub tcpi_state: u8,
        pub tcpi_ca_state: u8,
        pub tcpi_retransmits: u8,
        pub tcpi_probes: u8,
        pub tcpi_backoff: u8,
        pub tcpi_options: u8,
        /// This contains the bitfields `tcpi_snd_wscale` and `tcpi_rcv_wscale`.
        /// Each is 4 bits.
        pub tcpi_snd_rcv_wscale: u8,
        pub tcpi_rto: u32,
        pub tcpi_ato: u32,
        pub tcpi_snd_mss: u32,
        pub tcpi_rcv_mss: u32,
        pub tcpi_unacked: u32,
        pub tcpi_sacked: u32,
        pub tcpi_lost: u32,
        pub tcpi_retrans: u32,
        pub tcpi_fackets: u32,
        pub tcpi_last_data_sent: u32,
        pub tcpi_last_ack_sent: u32,
        pub tcpi_last_data_recv: u32,
        pub tcpi_last_ack_recv: u32,
        pub tcpi_pmtu: u32,
        pub tcpi_rcv_ssthresh: u32,
        pub tcpi_rtt: u32,
        pub tcpi_rttvar: u32,
        pub tcpi_snd_ssthresh: u32,
        pub tcpi_snd_cwnd: u32,
        pub tcpi_advmss: u32,
        pub tcpi_reordering: u32,
        pub tcpi_rcv_rtt: u32,
        pub tcpi_rcv_space: u32,
        pub tcpi_total_retrans: u32,
    }
}

impl siginfo_t {
    pub unsafe fn si_addr(&self) -> *mut c_void {
        #[repr(C)]
        struct siginfo_sigfault {
            _si_signo: c_int,
            _si_errno: c_int,
            _si_code: c_int,
            si_addr: *mut c_void,
        }
        (*(self as *const siginfo_t as *const siginfo_sigfault)).si_addr
    }

    pub unsafe fn si_value(&self) -> crate::sigval {
        #[repr(C)]
        struct siginfo_si_value {
            _si_signo: c_int,
            _si_errno: c_int,
            _si_code: c_int,
            _si_timerid: c_int,
            _si_overrun: c_int,
            si_value: crate::sigval,
        }
        (*(self as *const siginfo_t as *const siginfo_si_value)).si_value
    }
}

// Internal, for casts to access union fields
#[repr(C)]
struct sifields_sigchld {
    si_pid: crate::pid_t,
    si_uid: crate::uid_t,
    si_status: c_int,
    si_utime: c_long,
    si_stime: c_long,
}
impl Copy for sifields_sigchld {}
impl Clone for sifields_sigchld {
    fn clone(&self) -> sifields_sigchld {
        *self
    }
}

// Internal, for casts to access union fields
#[repr(C)]
union sifields {
    _align_pointer: *mut c_void,
    sigchld: sifields_sigchld,
}

// Internal, for casts to access union fields. Note that some variants
// of sifields start with a pointer, which makes the alignment of
// sifields vary on 32-bit and 64-bit architectures.
#[repr(C)]
struct siginfo_f {
    _siginfo_base: [c_int; 3],
    sifields: sifields,
}

impl siginfo_t {
    unsafe fn sifields(&self) -> &sifields {
        &(*(self as *const siginfo_t as *const siginfo_f)).sifields
    }

    pub unsafe fn si_pid(&self) -> crate::pid_t {
        self.sifields().sigchld.si_pid
    }

    pub unsafe fn si_uid(&self) -> crate::uid_t {
        self.sifields().sigchld.si_uid
    }

    pub unsafe fn si_status(&self) -> c_int {
        self.sifields().sigchld.si_status
    }

    pub unsafe fn si_utime(&self) -> c_long {
        self.sifields().sigchld.si_utime
    }

    pub unsafe fn si_stime(&self) -> c_long {
        self.sifields().sigchld.si_stime
    }
}

pub const MCL_CURRENT: c_int = 0x0001;
pub const MCL_FUTURE: c_int = 0x0002;
pub const MCL_ONFAULT: c_int = 0x0004;

pub const SIGEV_THREAD_ID: c_int = 4;

pub const AF_VSOCK: c_int = 40;

// Most `*_SUPER_MAGIC` constants are defined at the `linux_like` level; the
// following are only available on newer Linux versions than the versions
// currently used in CI in some configurations, so we define them here.
pub const BINDERFS_SUPER_MAGIC: c_long = 0x6c6f6f70;
pub const XFS_SUPER_MAGIC: c_long = 0x58465342;

pub const PTRACE_TRACEME: c_int = 0;
pub const PTRACE_PEEKTEXT: c_int = 1;
pub const PTRACE_PEEKDATA: c_int = 2;
pub const PTRACE_PEEKUSER: c_int = 3;
pub const PTRACE_POKETEXT: c_int = 4;
pub const PTRACE_POKEDATA: c_int = 5;
pub const PTRACE_POKEUSER: c_int = 6;
pub const PTRACE_CONT: c_int = 7;
pub const PTRACE_KILL: c_int = 8;
pub const PTRACE_SINGLESTEP: c_int = 9;
pub const PTRACE_GETREGS: c_int = 12;
pub const PTRACE_SETREGS: c_int = 13;
pub const PTRACE_GETFPREGS: c_int = 14;
pub const PTRACE_SETFPREGS: c_int = 15;
pub const PTRACE_ATTACH: c_int = 16;
pub const PTRACE_DETACH: c_int = 17;
pub const PTRACE_GETFPXREGS: c_int = 18;
pub const PTRACE_SETFPXREGS: c_int = 19;
pub const PTRACE_SYSCALL: c_int = 24;
pub const PTRACE_SETOPTIONS: c_int = 0x4200;
pub const PTRACE_GETEVENTMSG: c_int = 0x4201;
pub const PTRACE_GETSIGINFO: c_int = 0x4202;
pub const PTRACE_SETSIGINFO: c_int = 0x4203;
pub const PTRACE_GETREGSET: c_int = 0x4204;
pub const PTRACE_SETREGSET: c_int = 0x4205;
pub const PTRACE_SEIZE: c_int = 0x4206;
pub const PTRACE_INTERRUPT: c_int = 0x4207;
pub const PTRACE_LISTEN: c_int = 0x4208;

pub const POSIX_FADV_DONTNEED: c_int = 4;
pub const POSIX_FADV_NOREUSE: c_int = 5;

// These are different than GNU!
pub const LC_CTYPE: c_int = 0;
pub const LC_NUMERIC: c_int = 1;
pub const LC_TIME: c_int = 3;
pub const LC_COLLATE: c_int = 4;
pub const LC_MONETARY: c_int = 2;
pub const LC_MESSAGES: c_int = 5;
pub const LC_ALL: c_int = 6;
// end different section

// MS_ flags for mount(2)
pub const MS_RMT_MASK: c_ulong =
    crate::MS_RDONLY | crate::MS_SYNCHRONOUS | crate::MS_MANDLOCK | crate::MS_I_VERSION;

pub const ENOTSUP: c_int = EOPNOTSUPP;

pub const IPV6_JOIN_GROUP: c_int = 20;
pub const IPV6_LEAVE_GROUP: c_int = 21;

// These are different from GNU
pub const ABDAY_1: crate::nl_item = 0x300;
pub const ABDAY_2: crate::nl_item = 0x301;
pub const ABDAY_3: crate::nl_item = 0x302;
pub const ABDAY_4: crate::nl_item = 0x303;
pub const ABDAY_5: crate::nl_item = 0x304;
pub const ABDAY_6: crate::nl_item = 0x305;
pub const ABDAY_7: crate::nl_item = 0x306;
pub const DAY_1: crate::nl_item = 0x307;
pub const DAY_2: crate::nl_item = 0x308;
pub const DAY_3: crate::nl_item = 0x309;
pub const DAY_4: crate::nl_item = 0x30A;
pub const DAY_5: crate::nl_item = 0x30B;
pub const DAY_6: crate::nl_item = 0x30C;
pub const DAY_7: crate::nl_item = 0x30D;
pub const ABMON_1: crate::nl_item = 0x30E;
pub const ABMON_2: crate::nl_item = 0x30F;
pub const ABMON_3: crate::nl_item = 0x310;
pub const ABMON_4: crate::nl_item = 0x311;
pub const ABMON_5: crate::nl_item = 0x312;
pub const ABMON_6: crate::nl_item = 0x313;
pub const ABMON_7: crate::nl_item = 0x314;
pub const ABMON_8: crate::nl_item = 0x315;
pub const ABMON_9: crate::nl_item = 0x316;
pub const ABMON_10: crate::nl_item = 0x317;
pub const ABMON_11: crate::nl_item = 0x318;
pub const ABMON_12: crate::nl_item = 0x319;
pub const MON_1: crate::nl_item = 0x31A;
pub const MON_2: crate::nl_item = 0x31B;
pub const MON_3: crate::nl_item = 0x31C;
pub const MON_4: crate::nl_item = 0x31D;
pub const MON_5: crate::nl_item = 0x31E;
pub const MON_6: crate::nl_item = 0x31F;
pub const MON_7: crate::nl_item = 0x320;
pub const MON_8: crate::nl_item = 0x321;
pub const MON_9: crate::nl_item = 0x322;
pub const MON_10: crate::nl_item = 0x323;
pub const MON_11: crate::nl_item = 0x324;
pub const MON_12: crate::nl_item = 0x325;
pub const AM_STR: crate::nl_item = 0x326;
pub const PM_STR: crate::nl_item = 0x327;
pub const D_T_FMT: crate::nl_item = 0x328;
pub const D_FMT: crate::nl_item = 0x329;
pub const T_FMT: crate::nl_item = 0x32A;
pub const T_FMT_AMPM: crate::nl_item = 0x32B;
pub const ERA: crate::nl_item = 0x32C;
pub const ERA_D_FMT: crate::nl_item = 0x32E;
pub const ALT_DIGITS: crate::nl_item = 0x32F;
pub const ERA_D_T_FMT: crate::nl_item = 0x330;
pub const ERA_T_FMT: crate::nl_item = 0x331;
pub const CODESET: crate::nl_item = 10;
pub const CRNCYSTR: crate::nl_item = 0x215;
pub const RADIXCHAR: crate::nl_item = 0x100;
pub const THOUSEP: crate::nl_item = 0x101;
pub const NOEXPR: crate::nl_item = 0x501;
pub const YESSTR: crate::nl_item = 0x502;
pub const NOSTR: crate::nl_item = 0x503;

// Different than Gnu.
pub const FILENAME_MAX: c_uint = 4095;

pub const PRIO_PROCESS: c_int = 0;
pub const PRIO_PGRP: c_int = 1;
pub const PRIO_USER: c_int = 2;

pub const SOMAXCONN: c_int = 128;

pub const ST_RELATIME: c_ulong = 4096;

pub const AF_NFC: c_int = PF_NFC;
pub const BUFSIZ: c_int = 4096;
pub const EDEADLOCK: c_int = EDEADLK;
pub const EXTA: c_uint = B19200;
pub const EXTB: c_uint = B38400;
pub const EXTPROC: crate::tcflag_t = 0o200000;
pub const FOPEN_MAX: c_int = 16;
pub const F_GETOWN: c_int = 9;
pub const F_OFD_GETLK: c_int = 36;
pub const F_OFD_SETLK: c_int = 37;
pub const F_OFD_SETLKW: c_int = 38;
pub const F_RDLCK: c_int = 0;
pub const F_SETOWN: c_int = 8;
pub const F_UNLCK: c_int = 2;
pub const F_WRLCK: c_int = 1;
pub const IPV6_MULTICAST_ALL: c_int = 29;
pub const IPV6_ROUTER_ALERT_ISOLATE: c_int = 30;
pub const MAP_HUGE_SHIFT: c_int = 26;
pub const MAP_HUGE_MASK: c_int = 0x3f;
pub const MAP_HUGE_64KB: c_int = 16 << MAP_HUGE_SHIFT;
pub const MAP_HUGE_512KB: c_int = 19 << MAP_HUGE_SHIFT;
pub const MAP_HUGE_1MB: c_int = 20 << MAP_HUGE_SHIFT;
pub const MAP_HUGE_2MB: c_int = 21 << MAP_HUGE_SHIFT;
pub const MAP_HUGE_8MB: c_int = 23 << MAP_HUGE_SHIFT;
pub const MAP_HUGE_16MB: c_int = 24 << MAP_HUGE_SHIFT;
pub const MAP_HUGE_32MB: c_int = 25 << MAP_HUGE_SHIFT;
pub const MAP_HUGE_256MB: c_int = 28 << MAP_HUGE_SHIFT;
pub const MAP_HUGE_512MB: c_int = 29 << MAP_HUGE_SHIFT;
pub const MAP_HUGE_1GB: c_int = 30 << MAP_HUGE_SHIFT;
pub const MAP_HUGE_2GB: c_int = 31 << MAP_HUGE_SHIFT;
pub const MAP_HUGE_16GB: c_int = 34 << MAP_HUGE_SHIFT;
pub const MINSIGSTKSZ: c_int = 2048;
pub const MSG_COPY: c_int = 0o40000;
pub const NI_MAXHOST: crate::socklen_t = 1025;
pub const O_TMPFILE: c_int = 0o20000000 | O_DIRECTORY;
pub const PACKET_MR_UNICAST: c_int = 3;
pub const PF_NFC: c_int = 39;
pub const PF_VSOCK: c_int = 40;
pub const POSIX_MADV_DONTNEED: c_int = 4;
pub const PTRACE_EVENT_STOP: c_int = 128;
pub const PTRACE_GETSIGMASK: c_uint = 0x420a;
pub const PTRACE_PEEKSIGINFO: c_int = 0x4209;
pub const PTRACE_SETSIGMASK: c_uint = 0x420b;
pub const RTLD_NOLOAD: c_int = 0x00004;
pub const RUSAGE_THREAD: c_int = 1;
pub const SHM_EXEC: c_int = 0o100000;
pub const SIGPOLL: c_int = SIGIO;
pub const SOCK_DCCP: c_int = 6;
#[deprecated(since = "0.2.70", note = "AF_PACKET must be used instead")]
pub const SOCK_PACKET: c_int = 10;
pub const TCP_COOKIE_TRANSACTIONS: c_int = 15;
pub const UDP_GRO: c_int = 104;
pub const UDP_SEGMENT: c_int = 103;
pub const YESEXPR: c_int = ((5) << 8) | (0);

extern "C" {
    pub fn gettimeofday(tp: *mut crate::timeval, tz: *mut crate::timezone) -> c_int;

    pub fn pthread_rwlockattr_getkind_np(
        attr: *const crate::pthread_rwlockattr_t,
        val: *mut c_int,
    ) -> c_int;
    pub fn pthread_rwlockattr_setkind_np(
        attr: *mut crate::pthread_rwlockattr_t,
        val: c_int,
    ) -> c_int;

    pub fn ptrace(request: c_uint, ...) -> c_long;

    pub fn sendmmsg(
        sockfd: c_int,
        msgvec: *mut crate::mmsghdr,
        vlen: c_uint,
        flags: c_int,
    ) -> c_int;
    pub fn recvmmsg(
        sockfd: c_int,
        msgvec: *mut crate::mmsghdr,
        vlen: c_uint,
        flags: c_int,
        timeout: *mut crate::timespec,
    ) -> c_int;

    pub fn openpty(
        amaster: *mut c_int,
        aslave: *mut c_int,
        name: *mut c_char,
        termp: *mut termios,
        winp: *mut crate::winsize,
    ) -> c_int;
    pub fn forkpty(
        amaster: *mut c_int,
        name: *mut c_char,
        termp: *mut termios,
        winp: *mut crate::winsize,
    ) -> crate::pid_t;

    pub fn getnameinfo(
        sa: *const crate::sockaddr,
        salen: crate::socklen_t,
        host: *mut c_char,
        hostlen: crate::socklen_t,
        serv: *mut c_char,
        servlen: crate::socklen_t,
        flags: c_int,
    ) -> c_int;

    pub fn pwritev(fd: c_int, iov: *const crate::iovec, iovcnt: c_int, offset: off64_t) -> ssize_t;
    pub fn preadv(fd: c_int, iov: *const crate::iovec, iovcnt: c_int, offset: off64_t) -> ssize_t;

    pub fn sethostid(hostid: c_long) -> c_int;
    pub fn fanotify_mark(
        fd: c_int,
        flags: c_uint,
        mask: u64,
        dirfd: c_int,
        path: *const c_char,
    ) -> c_int;
    pub fn getrlimit64(resource: crate::__rlimit_resource_t, rlim: *mut crate::rlimit64) -> c_int;
    pub fn setrlimit64(resource: crate::__rlimit_resource_t, rlim: *const crate::rlimit64)
        -> c_int;
    pub fn getrlimit(resource: crate::__rlimit_resource_t, rlim: *mut crate::rlimit) -> c_int;
    pub fn setrlimit(resource: crate::__rlimit_resource_t, rlim: *const crate::rlimit) -> c_int;
    pub fn getpriority(which: crate::__priority_which_t, who: crate::id_t) -> c_int;
    pub fn setpriority(which: crate::__priority_which_t, who: crate::id_t, prio: c_int) -> c_int;
    pub fn getauxval(type_: c_ulong) -> c_ulong;
}

cfg_if! {
    if #[cfg(any(target_arch = "mips", target_arch = "mips64"))] {
        mod mips;
        pub use self::mips::*;
    } else if #[cfg(target_arch = "x86_64")] {
        mod x86_64;
        pub use self::x86_64::*;
    } else if #[cfg(target_arch = "arm")] {
        mod arm;
        pub use self::arm::*;
    } else {
        pub use unsupported_target;
    }
}
