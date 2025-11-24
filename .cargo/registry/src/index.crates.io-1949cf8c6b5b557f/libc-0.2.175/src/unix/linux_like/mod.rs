use crate::prelude::*;

pub type sa_family_t = u16;
pub type speed_t = c_uint;
pub type tcflag_t = c_uint;
pub type clockid_t = c_int;
pub type timer_t = *mut c_void;
pub type key_t = c_int;
pub type id_t = c_uint;

missing! {
    #[cfg_attr(feature = "extra_traits", derive(Debug))]
    pub enum timezone {}
}

s! {
    pub struct in_addr {
        pub s_addr: crate::in_addr_t,
    }

    pub struct ip_mreq {
        pub imr_multiaddr: in_addr,
        pub imr_interface: in_addr,
    }

    pub struct ip_mreqn {
        pub imr_multiaddr: in_addr,
        pub imr_address: in_addr,
        pub imr_ifindex: c_int,
    }

    pub struct ip_mreq_source {
        pub imr_multiaddr: in_addr,
        pub imr_interface: in_addr,
        pub imr_sourceaddr: in_addr,
    }

    pub struct sockaddr {
        pub sa_family: sa_family_t,
        pub sa_data: [c_char; 14],
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

    // The order of the `ai_addr` field in this struct is crucial
    // for converting between the Rust and C types.
    pub struct addrinfo {
        pub ai_flags: c_int,
        pub ai_family: c_int,
        pub ai_socktype: c_int,
        pub ai_protocol: c_int,
        pub ai_addrlen: socklen_t,

        #[cfg(any(target_os = "linux", target_os = "emscripten"))]
        pub ai_addr: *mut crate::sockaddr,

        pub ai_canonname: *mut c_char,

        #[cfg(target_os = "android")]
        pub ai_addr: *mut crate::sockaddr,

        pub ai_next: *mut addrinfo,
    }

    pub struct sockaddr_ll {
        pub sll_family: c_ushort,
        pub sll_protocol: c_ushort,
        pub sll_ifindex: c_int,
        pub sll_hatype: c_ushort,
        pub sll_pkttype: c_uchar,
        pub sll_halen: c_uchar,
        pub sll_addr: [c_uchar; 8],
    }

    pub struct fd_set {
        fds_bits: [c_ulong; FD_SETSIZE as usize / ULONG_SIZE],
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
        pub tm_gmtoff: c_long,
        pub tm_zone: *const c_char,
    }

    pub struct sched_param {
        pub sched_priority: c_int,
        #[cfg(any(target_env = "musl", target_os = "emscripten", target_env = "ohos"))]
        pub sched_ss_low_priority: c_int,
        #[cfg(any(target_env = "musl", target_os = "emscripten", target_env = "ohos"))]
        pub sched_ss_repl_period: crate::timespec,
        #[cfg(any(target_env = "musl", target_os = "emscripten", target_env = "ohos"))]
        pub sched_ss_init_budget: crate::timespec,
        #[cfg(any(target_env = "musl", target_os = "emscripten", target_env = "ohos"))]
        pub sched_ss_max_repl: c_int,
    }

    pub struct Dl_info {
        pub dli_fname: *const c_char,
        pub dli_fbase: *mut c_void,
        pub dli_sname: *const c_char,
        pub dli_saddr: *mut c_void,
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
        pub int_p_cs_precedes: c_char,
        pub int_p_sep_by_space: c_char,
        pub int_n_cs_precedes: c_char,
        pub int_n_sep_by_space: c_char,
        pub int_p_sign_posn: c_char,
        pub int_n_sign_posn: c_char,
    }

    pub struct in_pktinfo {
        pub ipi_ifindex: c_int,
        pub ipi_spec_dst: crate::in_addr,
        pub ipi_addr: crate::in_addr,
    }

    pub struct ifaddrs {
        pub ifa_next: *mut ifaddrs,
        pub ifa_name: *mut c_char,
        pub ifa_flags: c_uint,
        pub ifa_addr: *mut crate::sockaddr,
        pub ifa_netmask: *mut crate::sockaddr,
        pub ifa_ifu: *mut crate::sockaddr, // FIXME(union) This should be a union
        pub ifa_data: *mut c_void,
    }

    pub struct in6_rtmsg {
        rtmsg_dst: crate::in6_addr,
        rtmsg_src: crate::in6_addr,
        rtmsg_gateway: crate::in6_addr,
        rtmsg_type: u32,
        rtmsg_dst_len: u16,
        rtmsg_src_len: u16,
        rtmsg_metric: u32,
        rtmsg_info: c_ulong,
        rtmsg_flags: u32,
        rtmsg_ifindex: c_int,
    }

    pub struct arpreq {
        pub arp_pa: crate::sockaddr,
        pub arp_ha: crate::sockaddr,
        pub arp_flags: c_int,
        pub arp_netmask: crate::sockaddr,
        pub arp_dev: [c_char; 16],
    }

    pub struct arpreq_old {
        pub arp_pa: crate::sockaddr,
        pub arp_ha: crate::sockaddr,
        pub arp_flags: c_int,
        pub arp_netmask: crate::sockaddr,
    }

    pub struct arphdr {
        pub ar_hrd: u16,
        pub ar_pro: u16,
        pub ar_hln: u8,
        pub ar_pln: u8,
        pub ar_op: u16,
    }

    pub struct mmsghdr {
        pub msg_hdr: crate::msghdr,
        pub msg_len: c_uint,
    }
}

cfg_if! {
    if #[cfg(not(target_os = "emscripten"))] {
        s! {
            pub struct file_clone_range {
                pub src_fd: crate::__s64,
                pub src_offset: crate::__u64,
                pub src_length: crate::__u64,
                pub dest_offset: crate::__u64,
            }

            // linux/filter.h
            pub struct sock_filter {
                pub code: __u16,
                pub jt: __u8,
                pub jf: __u8,
                pub k: __u32,
            }

            pub struct sock_fprog {
                pub len: c_ushort,
                pub filter: *mut sock_filter,
            }
        }
    }
}

cfg_if! {
    if #[cfg(any(
        target_env = "gnu",
        target_os = "android",
        all(target_env = "musl", musl_v1_2_3)
    ))] {
        s! {
            pub struct statx {
                pub stx_mask: crate::__u32,
                pub stx_blksize: crate::__u32,
                pub stx_attributes: crate::__u64,
                pub stx_nlink: crate::__u32,
                pub stx_uid: crate::__u32,
                pub stx_gid: crate::__u32,
                pub stx_mode: crate::__u16,
                __statx_pad1: [crate::__u16; 1],
                pub stx_ino: crate::__u64,
                pub stx_size: crate::__u64,
                pub stx_blocks: crate::__u64,
                pub stx_attributes_mask: crate::__u64,
                pub stx_atime: statx_timestamp,
                pub stx_btime: statx_timestamp,
                pub stx_ctime: statx_timestamp,
                pub stx_mtime: statx_timestamp,
                pub stx_rdev_major: crate::__u32,
                pub stx_rdev_minor: crate::__u32,
                pub stx_dev_major: crate::__u32,
                pub stx_dev_minor: crate::__u32,
                pub stx_mnt_id: crate::__u64,
                pub stx_dio_mem_align: crate::__u32,
                pub stx_dio_offset_align: crate::__u32,
                __statx_pad3: [crate::__u64; 12],
            }

            pub struct statx_timestamp {
                pub tv_sec: crate::__s64,
                pub tv_nsec: crate::__u32,
                __statx_timestamp_pad1: [crate::__s32; 1],
            }
        }
    }
}

s_no_extra_traits! {
    #[cfg_attr(
        any(
            all(
                target_arch = "x86",
                not(target_env = "musl"),
                not(target_os = "android")
            ),
            target_arch = "x86_64"
        ),
        repr(packed)
    )]
    pub struct epoll_event {
        pub events: u32,
        pub u64: u64,
    }

    pub struct sockaddr_un {
        pub sun_family: sa_family_t,
        pub sun_path: [c_char; 108],
    }

    pub struct sockaddr_storage {
        pub ss_family: sa_family_t,
        #[cfg(target_pointer_width = "32")]
        __ss_pad2: [u8; 128 - 2 - 4],
        #[cfg(target_pointer_width = "64")]
        __ss_pad2: [u8; 128 - 2 - 8],
        __ss_align: size_t,
    }

    pub struct utsname {
        pub sysname: [c_char; 65],
        pub nodename: [c_char; 65],
        pub release: [c_char; 65],
        pub version: [c_char; 65],
        pub machine: [c_char; 65],
        pub domainname: [c_char; 65],
    }

    pub struct sigevent {
        pub sigev_value: crate::sigval,
        pub sigev_signo: c_int,
        pub sigev_notify: c_int,
        // Actually a union.  We only expose sigev_notify_thread_id because it's
        // the most useful member
        pub sigev_notify_thread_id: c_int,
        #[cfg(target_pointer_width = "64")]
        __unused1: [c_int; 11],
        #[cfg(target_pointer_width = "32")]
        __unused1: [c_int; 12],
    }
}

cfg_if! {
    if #[cfg(feature = "extra_traits")] {
        impl PartialEq for epoll_event {
            fn eq(&self, other: &epoll_event) -> bool {
                self.events == other.events && self.u64 == other.u64
            }
        }
        impl Eq for epoll_event {}
        impl hash::Hash for epoll_event {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                let events = self.events;
                let u64 = self.u64;
                events.hash(state);
                u64.hash(state);
            }
        }

        impl PartialEq for sockaddr_un {
            fn eq(&self, other: &sockaddr_un) -> bool {
                self.sun_family == other.sun_family
                    && self
                        .sun_path
                        .iter()
                        .zip(other.sun_path.iter())
                        .all(|(a, b)| a == b)
            }
        }
        impl Eq for sockaddr_un {}
        impl hash::Hash for sockaddr_un {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                self.sun_family.hash(state);
                self.sun_path.hash(state);
            }
        }

        impl PartialEq for sockaddr_storage {
            fn eq(&self, other: &sockaddr_storage) -> bool {
                self.ss_family == other.ss_family
                    && self
                        .__ss_pad2
                        .iter()
                        .zip(other.__ss_pad2.iter())
                        .all(|(a, b)| a == b)
            }
        }

        impl Eq for sockaddr_storage {}

        impl hash::Hash for sockaddr_storage {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                self.ss_family.hash(state);
                self.__ss_pad2.hash(state);
            }
        }

        impl PartialEq for utsname {
            fn eq(&self, other: &utsname) -> bool {
                self.sysname
                    .iter()
                    .zip(other.sysname.iter())
                    .all(|(a, b)| a == b)
                    && self
                        .nodename
                        .iter()
                        .zip(other.nodename.iter())
                        .all(|(a, b)| a == b)
                    && self
                        .release
                        .iter()
                        .zip(other.release.iter())
                        .all(|(a, b)| a == b)
                    && self
                        .version
                        .iter()
                        .zip(other.version.iter())
                        .all(|(a, b)| a == b)
                    && self
                        .machine
                        .iter()
                        .zip(other.machine.iter())
                        .all(|(a, b)| a == b)
                    && self
                        .domainname
                        .iter()
                        .zip(other.domainname.iter())
                        .all(|(a, b)| a == b)
            }
        }

        impl Eq for utsname {}

        impl hash::Hash for utsname {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                self.sysname.hash(state);
                self.nodename.hash(state);
                self.release.hash(state);
                self.version.hash(state);
                self.machine.hash(state);
                self.domainname.hash(state);
            }
        }

        impl PartialEq for sigevent {
            fn eq(&self, other: &sigevent) -> bool {
                self.sigev_value == other.sigev_value
                    && self.sigev_signo == other.sigev_signo
                    && self.sigev_notify == other.sigev_notify
                    && self.sigev_notify_thread_id == other.sigev_notify_thread_id
            }
        }
        impl Eq for sigevent {}
        impl hash::Hash for sigevent {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                self.sigev_value.hash(state);
                self.sigev_signo.hash(state);
                self.sigev_notify.hash(state);
                self.sigev_notify_thread_id.hash(state);
            }
        }
    }
}

// intentionally not public, only used for fd_set
cfg_if! {
    if #[cfg(target_pointer_width = "32")] {
        const ULONG_SIZE: usize = 32;
    } else if #[cfg(target_pointer_width = "64")] {
        const ULONG_SIZE: usize = 64;
    } else {
        // Unknown target_pointer_width
    }
}

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

pub const F_DUPFD: c_int = 0;
pub const F_GETFD: c_int = 1;
pub const F_SETFD: c_int = 2;
pub const F_GETFL: c_int = 3;
pub const F_SETFL: c_int = 4;

// Linux-specific fcntls
pub const F_SETLEASE: c_int = 1024;
pub const F_GETLEASE: c_int = 1025;
pub const F_NOTIFY: c_int = 1026;
pub const F_CANCELLK: c_int = 1029;
pub const F_DUPFD_CLOEXEC: c_int = 1030;
pub const F_SETPIPE_SZ: c_int = 1031;
pub const F_GETPIPE_SZ: c_int = 1032;
pub const F_ADD_SEALS: c_int = 1033;
pub const F_GET_SEALS: c_int = 1034;

pub const F_SEAL_SEAL: c_int = 0x0001;
pub const F_SEAL_SHRINK: c_int = 0x0002;
pub const F_SEAL_GROW: c_int = 0x0004;
pub const F_SEAL_WRITE: c_int = 0x0008;

// FIXME(#235): Include file sealing fcntls once we have a way to verify them.

pub const SIGTRAP: c_int = 5;

pub const PTHREAD_CREATE_JOINABLE: c_int = 0;
pub const PTHREAD_CREATE_DETACHED: c_int = 1;

pub const CLOCK_REALTIME: crate::clockid_t = 0;
pub const CLOCK_MONOTONIC: crate::clockid_t = 1;
pub const CLOCK_PROCESS_CPUTIME_ID: crate::clockid_t = 2;
pub const CLOCK_THREAD_CPUTIME_ID: crate::clockid_t = 3;
pub const CLOCK_MONOTONIC_RAW: crate::clockid_t = 4;
pub const CLOCK_REALTIME_COARSE: crate::clockid_t = 5;
pub const CLOCK_MONOTONIC_COARSE: crate::clockid_t = 6;
pub const CLOCK_BOOTTIME: crate::clockid_t = 7;
pub const CLOCK_REALTIME_ALARM: crate::clockid_t = 8;
pub const CLOCK_BOOTTIME_ALARM: crate::clockid_t = 9;
pub const CLOCK_TAI: crate::clockid_t = 11;
pub const TIMER_ABSTIME: c_int = 1;

pub const RUSAGE_SELF: c_int = 0;

pub const O_RDONLY: c_int = 0;
pub const O_WRONLY: c_int = 1;
pub const O_RDWR: c_int = 2;

pub const SOCK_CLOEXEC: c_int = O_CLOEXEC;

pub const S_IFIFO: mode_t = 0o1_0000;
pub const S_IFCHR: mode_t = 0o2_0000;
pub const S_IFBLK: mode_t = 0o6_0000;
pub const S_IFDIR: mode_t = 0o4_0000;
pub const S_IFREG: mode_t = 0o10_0000;
pub const S_IFLNK: mode_t = 0o12_0000;
pub const S_IFSOCK: mode_t = 0o14_0000;
pub const S_IFMT: mode_t = 0o17_0000;
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
pub const SIGHUP: c_int = 1;
pub const SIGINT: c_int = 2;
pub const SIGQUIT: c_int = 3;
pub const SIGILL: c_int = 4;
pub const SIGABRT: c_int = 6;
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

pub const XATTR_CREATE: c_int = 0x1;
pub const XATTR_REPLACE: c_int = 0x2;

cfg_if! {
    if #[cfg(target_os = "android")] {
        pub const RLIM64_INFINITY: c_ulonglong = !0;
    } else {
        pub const RLIM64_INFINITY: crate::rlim64_t = !0;
    }
}

cfg_if! {
    if #[cfg(target_env = "ohos")] {
        pub const LC_CTYPE: c_int = 0;
        pub const LC_NUMERIC: c_int = 1;
        pub const LC_TIME: c_int = 2;
        pub const LC_COLLATE: c_int = 3;
        pub const LC_MONETARY: c_int = 4;
        pub const LC_MESSAGES: c_int = 5;
        pub const LC_PAPER: c_int = 6;
        pub const LC_NAME: c_int = 7;
        pub const LC_ADDRESS: c_int = 8;
        pub const LC_TELEPHONE: c_int = 9;
        pub const LC_MEASUREMENT: c_int = 10;
        pub const LC_IDENTIFICATION: c_int = 11;
        pub const LC_ALL: c_int = 12;
    } else if #[cfg(not(target_env = "uclibc"))] {
        pub const LC_CTYPE: c_int = 0;
        pub const LC_NUMERIC: c_int = 1;
        pub const LC_TIME: c_int = 2;
        pub const LC_COLLATE: c_int = 3;
        pub const LC_MONETARY: c_int = 4;
        pub const LC_MESSAGES: c_int = 5;
        pub const LC_ALL: c_int = 6;
    }
}

pub const LC_CTYPE_MASK: c_int = 1 << LC_CTYPE;
pub const LC_NUMERIC_MASK: c_int = 1 << LC_NUMERIC;
pub const LC_TIME_MASK: c_int = 1 << LC_TIME;
pub const LC_COLLATE_MASK: c_int = 1 << LC_COLLATE;
pub const LC_MONETARY_MASK: c_int = 1 << LC_MONETARY;
pub const LC_MESSAGES_MASK: c_int = 1 << LC_MESSAGES;
// LC_ALL_MASK defined per platform

pub const MAP_FILE: c_int = 0x0000;
pub const MAP_SHARED: c_int = 0x0001;
pub const MAP_PRIVATE: c_int = 0x0002;
pub const MAP_FIXED: c_int = 0x0010;

pub const MAP_FAILED: *mut c_void = !0 as *mut c_void;

// MS_ flags for msync(2)
pub const MS_ASYNC: c_int = 0x0001;
pub const MS_INVALIDATE: c_int = 0x0002;
pub const MS_SYNC: c_int = 0x0004;

// MS_ flags for mount(2)
pub const MS_RDONLY: c_ulong = 0x01;
pub const MS_NOSUID: c_ulong = 0x02;
pub const MS_NODEV: c_ulong = 0x04;
pub const MS_NOEXEC: c_ulong = 0x08;
pub const MS_SYNCHRONOUS: c_ulong = 0x10;
pub const MS_REMOUNT: c_ulong = 0x20;
pub const MS_MANDLOCK: c_ulong = 0x40;
pub const MS_DIRSYNC: c_ulong = 0x80;
pub const MS_NOATIME: c_ulong = 0x0400;
pub const MS_NODIRATIME: c_ulong = 0x0800;
pub const MS_BIND: c_ulong = 0x1000;
pub const MS_MOVE: c_ulong = 0x2000;
pub const MS_REC: c_ulong = 0x4000;
pub const MS_SILENT: c_ulong = 0x8000;
pub const MS_POSIXACL: c_ulong = 0x010000;
pub const MS_UNBINDABLE: c_ulong = 0x020000;
pub const MS_PRIVATE: c_ulong = 0x040000;
pub const MS_SLAVE: c_ulong = 0x080000;
pub const MS_SHARED: c_ulong = 0x100000;
pub const MS_RELATIME: c_ulong = 0x200000;
pub const MS_KERNMOUNT: c_ulong = 0x400000;
pub const MS_I_VERSION: c_ulong = 0x800000;
pub const MS_STRICTATIME: c_ulong = 0x1000000;
pub const MS_LAZYTIME: c_ulong = 0x2000000;
pub const MS_ACTIVE: c_ulong = 0x40000000;
pub const MS_MGC_VAL: c_ulong = 0xc0ed0000;
pub const MS_MGC_MSK: c_ulong = 0xffff0000;

pub const SCM_RIGHTS: c_int = 0x01;
pub const SCM_CREDENTIALS: c_int = 0x02;

pub const PROT_GROWSDOWN: c_int = 0x1000000;
pub const PROT_GROWSUP: c_int = 0x2000000;

pub const MAP_TYPE: c_int = 0x000f;

pub const MADV_NORMAL: c_int = 0;
pub const MADV_RANDOM: c_int = 1;
pub const MADV_SEQUENTIAL: c_int = 2;
pub const MADV_WILLNEED: c_int = 3;
pub const MADV_DONTNEED: c_int = 4;
pub const MADV_FREE: c_int = 8;
pub const MADV_REMOVE: c_int = 9;
pub const MADV_DONTFORK: c_int = 10;
pub const MADV_DOFORK: c_int = 11;
pub const MADV_MERGEABLE: c_int = 12;
pub const MADV_UNMERGEABLE: c_int = 13;
pub const MADV_HUGEPAGE: c_int = 14;
pub const MADV_NOHUGEPAGE: c_int = 15;
pub const MADV_DONTDUMP: c_int = 16;
pub const MADV_DODUMP: c_int = 17;
pub const MADV_WIPEONFORK: c_int = 18;
pub const MADV_KEEPONFORK: c_int = 19;
pub const MADV_COLD: c_int = 20;
pub const MADV_PAGEOUT: c_int = 21;
pub const MADV_HWPOISON: c_int = 100;
cfg_if! {
    if #[cfg(not(target_os = "emscripten"))] {
        pub const MADV_POPULATE_READ: c_int = 22;
        pub const MADV_POPULATE_WRITE: c_int = 23;
        pub const MADV_DONTNEED_LOCKED: c_int = 24;
    }
}

pub const IFF_UP: c_int = 0x1;
pub const IFF_BROADCAST: c_int = 0x2;
pub const IFF_DEBUG: c_int = 0x4;
pub const IFF_LOOPBACK: c_int = 0x8;
pub const IFF_POINTOPOINT: c_int = 0x10;
pub const IFF_NOTRAILERS: c_int = 0x20;
pub const IFF_RUNNING: c_int = 0x40;
pub const IFF_NOARP: c_int = 0x80;
pub const IFF_PROMISC: c_int = 0x100;
pub const IFF_ALLMULTI: c_int = 0x200;
pub const IFF_MASTER: c_int = 0x400;
pub const IFF_SLAVE: c_int = 0x800;
pub const IFF_MULTICAST: c_int = 0x1000;
pub const IFF_PORTSEL: c_int = 0x2000;
pub const IFF_AUTOMEDIA: c_int = 0x4000;
pub const IFF_DYNAMIC: c_int = 0x8000;

pub const SOL_IP: c_int = 0;
pub const SOL_TCP: c_int = 6;
pub const SOL_UDP: c_int = 17;
pub const SOL_IPV6: c_int = 41;
pub const SOL_ICMPV6: c_int = 58;
pub const SOL_RAW: c_int = 255;
pub const SOL_DECNET: c_int = 261;
pub const SOL_X25: c_int = 262;
pub const SOL_PACKET: c_int = 263;
pub const SOL_ATM: c_int = 264;
pub const SOL_AAL: c_int = 265;
pub const SOL_IRDA: c_int = 266;
pub const SOL_NETBEUI: c_int = 267;
pub const SOL_LLC: c_int = 268;
pub const SOL_DCCP: c_int = 269;
pub const SOL_NETLINK: c_int = 270;
pub const SOL_TIPC: c_int = 271;
pub const SOL_BLUETOOTH: c_int = 274;
pub const SOL_ALG: c_int = 279;

pub const AF_UNSPEC: c_int = 0;
pub const AF_UNIX: c_int = 1;
pub const AF_LOCAL: c_int = 1;
pub const AF_INET: c_int = 2;
pub const AF_AX25: c_int = 3;
pub const AF_IPX: c_int = 4;
pub const AF_APPLETALK: c_int = 5;
pub const AF_NETROM: c_int = 6;
pub const AF_BRIDGE: c_int = 7;
pub const AF_ATMPVC: c_int = 8;
pub const AF_X25: c_int = 9;
pub const AF_INET6: c_int = 10;
pub const AF_ROSE: c_int = 11;
pub const AF_DECnet: c_int = 12;
pub const AF_NETBEUI: c_int = 13;
pub const AF_SECURITY: c_int = 14;
pub const AF_KEY: c_int = 15;
pub const AF_NETLINK: c_int = 16;
pub const AF_ROUTE: c_int = AF_NETLINK;
pub const AF_PACKET: c_int = 17;
pub const AF_ASH: c_int = 18;
pub const AF_ECONET: c_int = 19;
pub const AF_ATMSVC: c_int = 20;
pub const AF_RDS: c_int = 21;
pub const AF_SNA: c_int = 22;
pub const AF_IRDA: c_int = 23;
pub const AF_PPPOX: c_int = 24;
pub const AF_WANPIPE: c_int = 25;
pub const AF_LLC: c_int = 26;
pub const AF_CAN: c_int = 29;
pub const AF_TIPC: c_int = 30;
pub const AF_BLUETOOTH: c_int = 31;
pub const AF_IUCV: c_int = 32;
pub const AF_RXRPC: c_int = 33;
pub const AF_ISDN: c_int = 34;
pub const AF_PHONET: c_int = 35;
pub const AF_IEEE802154: c_int = 36;
pub const AF_CAIF: c_int = 37;
pub const AF_ALG: c_int = 38;

pub const PF_UNSPEC: c_int = AF_UNSPEC;
pub const PF_UNIX: c_int = AF_UNIX;
pub const PF_LOCAL: c_int = AF_LOCAL;
pub const PF_INET: c_int = AF_INET;
pub const PF_AX25: c_int = AF_AX25;
pub const PF_IPX: c_int = AF_IPX;
pub const PF_APPLETALK: c_int = AF_APPLETALK;
pub const PF_NETROM: c_int = AF_NETROM;
pub const PF_BRIDGE: c_int = AF_BRIDGE;
pub const PF_ATMPVC: c_int = AF_ATMPVC;
pub const PF_X25: c_int = AF_X25;
pub const PF_INET6: c_int = AF_INET6;
pub const PF_ROSE: c_int = AF_ROSE;
pub const PF_DECnet: c_int = AF_DECnet;
pub const PF_NETBEUI: c_int = AF_NETBEUI;
pub const PF_SECURITY: c_int = AF_SECURITY;
pub const PF_KEY: c_int = AF_KEY;
pub const PF_NETLINK: c_int = AF_NETLINK;
pub const PF_ROUTE: c_int = AF_ROUTE;
pub const PF_PACKET: c_int = AF_PACKET;
pub const PF_ASH: c_int = AF_ASH;
pub const PF_ECONET: c_int = AF_ECONET;
pub const PF_ATMSVC: c_int = AF_ATMSVC;
pub const PF_RDS: c_int = AF_RDS;
pub const PF_SNA: c_int = AF_SNA;
pub const PF_IRDA: c_int = AF_IRDA;
pub const PF_PPPOX: c_int = AF_PPPOX;
pub const PF_WANPIPE: c_int = AF_WANPIPE;
pub const PF_LLC: c_int = AF_LLC;
pub const PF_CAN: c_int = AF_CAN;
pub const PF_TIPC: c_int = AF_TIPC;
pub const PF_BLUETOOTH: c_int = AF_BLUETOOTH;
pub const PF_IUCV: c_int = AF_IUCV;
pub const PF_RXRPC: c_int = AF_RXRPC;
pub const PF_ISDN: c_int = AF_ISDN;
pub const PF_PHONET: c_int = AF_PHONET;
pub const PF_IEEE802154: c_int = AF_IEEE802154;
pub const PF_CAIF: c_int = AF_CAIF;
pub const PF_ALG: c_int = AF_ALG;

pub const MSG_OOB: c_int = 1;
pub const MSG_PEEK: c_int = 2;
pub const MSG_DONTROUTE: c_int = 4;
pub const MSG_CTRUNC: c_int = 8;
pub const MSG_TRUNC: c_int = 0x20;
pub const MSG_DONTWAIT: c_int = 0x40;
pub const MSG_EOR: c_int = 0x80;
pub const MSG_WAITALL: c_int = 0x100;
pub const MSG_FIN: c_int = 0x200;
pub const MSG_SYN: c_int = 0x400;
pub const MSG_CONFIRM: c_int = 0x800;
pub const MSG_RST: c_int = 0x1000;
pub const MSG_ERRQUEUE: c_int = 0x2000;
pub const MSG_NOSIGNAL: c_int = 0x4000;
pub const MSG_MORE: c_int = 0x8000;
pub const MSG_WAITFORONE: c_int = 0x10000;
pub const MSG_FASTOPEN: c_int = 0x20000000;
pub const MSG_CMSG_CLOEXEC: c_int = 0x40000000;

pub const SCM_TIMESTAMP: c_int = SO_TIMESTAMP;

pub const SOCK_RAW: c_int = 3;
pub const SOCK_RDM: c_int = 4;
pub const IP_TOS: c_int = 1;
pub const IP_TTL: c_int = 2;
pub const IP_HDRINCL: c_int = 3;
pub const IP_OPTIONS: c_int = 4;
pub const IP_ROUTER_ALERT: c_int = 5;
pub const IP_RECVOPTS: c_int = 6;
pub const IP_RETOPTS: c_int = 7;
pub const IP_PKTINFO: c_int = 8;
pub const IP_PKTOPTIONS: c_int = 9;
pub const IP_MTU_DISCOVER: c_int = 10;
pub const IP_RECVERR: c_int = 11;
pub const IP_RECVTTL: c_int = 12;
pub const IP_RECVTOS: c_int = 13;
pub const IP_MTU: c_int = 14;
pub const IP_FREEBIND: c_int = 15;
pub const IP_IPSEC_POLICY: c_int = 16;
pub const IP_XFRM_POLICY: c_int = 17;
pub const IP_PASSSEC: c_int = 18;
pub const IP_TRANSPARENT: c_int = 19;
pub const IP_ORIGDSTADDR: c_int = 20;
pub const IP_RECVORIGDSTADDR: c_int = IP_ORIGDSTADDR;
pub const IP_MINTTL: c_int = 21;
pub const IP_NODEFRAG: c_int = 22;
pub const IP_CHECKSUM: c_int = 23;
pub const IP_BIND_ADDRESS_NO_PORT: c_int = 24;
pub const IP_MULTICAST_IF: c_int = 32;
pub const IP_MULTICAST_TTL: c_int = 33;
pub const IP_MULTICAST_LOOP: c_int = 34;
pub const IP_ADD_MEMBERSHIP: c_int = 35;
pub const IP_DROP_MEMBERSHIP: c_int = 36;
pub const IP_UNBLOCK_SOURCE: c_int = 37;
pub const IP_BLOCK_SOURCE: c_int = 38;
pub const IP_ADD_SOURCE_MEMBERSHIP: c_int = 39;
pub const IP_DROP_SOURCE_MEMBERSHIP: c_int = 40;
pub const IP_MSFILTER: c_int = 41;
pub const IP_MULTICAST_ALL: c_int = 49;
pub const IP_UNICAST_IF: c_int = 50;

pub const IP_DEFAULT_MULTICAST_TTL: c_int = 1;
pub const IP_DEFAULT_MULTICAST_LOOP: c_int = 1;

pub const IP_PMTUDISC_DONT: c_int = 0;
pub const IP_PMTUDISC_WANT: c_int = 1;
pub const IP_PMTUDISC_DO: c_int = 2;
pub const IP_PMTUDISC_PROBE: c_int = 3;
pub const IP_PMTUDISC_INTERFACE: c_int = 4;
pub const IP_PMTUDISC_OMIT: c_int = 5;

// IPPROTO_IP defined in src/unix/mod.rs
/// Hop-by-hop option header
pub const IPPROTO_HOPOPTS: c_int = 0;
// IPPROTO_ICMP defined in src/unix/mod.rs
/// group mgmt protocol
pub const IPPROTO_IGMP: c_int = 2;
/// for compatibility
pub const IPPROTO_IPIP: c_int = 4;
// IPPROTO_TCP defined in src/unix/mod.rs
/// exterior gateway protocol
pub const IPPROTO_EGP: c_int = 8;
/// pup
pub const IPPROTO_PUP: c_int = 12;
// IPPROTO_UDP defined in src/unix/mod.rs
/// xns idp
pub const IPPROTO_IDP: c_int = 22;
/// tp-4 w/ class negotiation
pub const IPPROTO_TP: c_int = 29;
/// DCCP
pub const IPPROTO_DCCP: c_int = 33;
// IPPROTO_IPV6 defined in src/unix/mod.rs
/// IP6 routing header
pub const IPPROTO_ROUTING: c_int = 43;
/// IP6 fragmentation header
pub const IPPROTO_FRAGMENT: c_int = 44;
/// resource reservation
pub const IPPROTO_RSVP: c_int = 46;
/// General Routing Encap.
pub const IPPROTO_GRE: c_int = 47;
/// IP6 Encap Sec. Payload
pub const IPPROTO_ESP: c_int = 50;
/// IP6 Auth Header
pub const IPPROTO_AH: c_int = 51;
// IPPROTO_ICMPV6 defined in src/unix/mod.rs
/// IP6 no next header
pub const IPPROTO_NONE: c_int = 59;
/// IP6 destination option
pub const IPPROTO_DSTOPTS: c_int = 60;
pub const IPPROTO_MTP: c_int = 92;
/// encapsulation header
pub const IPPROTO_ENCAP: c_int = 98;
/// Protocol indep. multicast
pub const IPPROTO_PIM: c_int = 103;
/// IP Payload Comp. Protocol
pub const IPPROTO_COMP: c_int = 108;
/// SCTP
pub const IPPROTO_SCTP: c_int = 132;
pub const IPPROTO_MH: c_int = 135;
pub const IPPROTO_UDPLITE: c_int = 136;
/// raw IP packet
pub const IPPROTO_RAW: c_int = 255;
pub const IPPROTO_BEETPH: c_int = 94;
pub const IPPROTO_MPLS: c_int = 137;
/// Multipath TCP
pub const IPPROTO_MPTCP: c_int = 262;
/// Ethernet-within-IPv6 encapsulation.
pub const IPPROTO_ETHERNET: c_int = 143;

pub const MCAST_EXCLUDE: c_int = 0;
pub const MCAST_INCLUDE: c_int = 1;
pub const MCAST_JOIN_GROUP: c_int = 42;
pub const MCAST_BLOCK_SOURCE: c_int = 43;
pub const MCAST_UNBLOCK_SOURCE: c_int = 44;
pub const MCAST_LEAVE_GROUP: c_int = 45;
pub const MCAST_JOIN_SOURCE_GROUP: c_int = 46;
pub const MCAST_LEAVE_SOURCE_GROUP: c_int = 47;
pub const MCAST_MSFILTER: c_int = 48;

pub const IPV6_ADDRFORM: c_int = 1;
pub const IPV6_2292PKTINFO: c_int = 2;
pub const IPV6_2292HOPOPTS: c_int = 3;
pub const IPV6_2292DSTOPTS: c_int = 4;
pub const IPV6_2292RTHDR: c_int = 5;
pub const IPV6_2292PKTOPTIONS: c_int = 6;
pub const IPV6_CHECKSUM: c_int = 7;
pub const IPV6_2292HOPLIMIT: c_int = 8;
pub const IPV6_NEXTHOP: c_int = 9;
pub const IPV6_AUTHHDR: c_int = 10;
pub const IPV6_UNICAST_HOPS: c_int = 16;
pub const IPV6_MULTICAST_IF: c_int = 17;
pub const IPV6_MULTICAST_HOPS: c_int = 18;
pub const IPV6_MULTICAST_LOOP: c_int = 19;
pub const IPV6_ADD_MEMBERSHIP: c_int = 20;
pub const IPV6_DROP_MEMBERSHIP: c_int = 21;
pub const IPV6_ROUTER_ALERT: c_int = 22;
pub const IPV6_MTU_DISCOVER: c_int = 23;
pub const IPV6_MTU: c_int = 24;
pub const IPV6_RECVERR: c_int = 25;
pub const IPV6_V6ONLY: c_int = 26;
pub const IPV6_JOIN_ANYCAST: c_int = 27;
pub const IPV6_LEAVE_ANYCAST: c_int = 28;
pub const IPV6_IPSEC_POLICY: c_int = 34;
pub const IPV6_XFRM_POLICY: c_int = 35;
pub const IPV6_HDRINCL: c_int = 36;
pub const IPV6_RECVPKTINFO: c_int = 49;
pub const IPV6_PKTINFO: c_int = 50;
pub const IPV6_RECVHOPLIMIT: c_int = 51;
pub const IPV6_HOPLIMIT: c_int = 52;
pub const IPV6_RECVHOPOPTS: c_int = 53;
pub const IPV6_HOPOPTS: c_int = 54;
pub const IPV6_RTHDRDSTOPTS: c_int = 55;
pub const IPV6_RECVRTHDR: c_int = 56;
pub const IPV6_RTHDR: c_int = 57;
pub const IPV6_RECVDSTOPTS: c_int = 58;
pub const IPV6_DSTOPTS: c_int = 59;
pub const IPV6_RECVPATHMTU: c_int = 60;
pub const IPV6_PATHMTU: c_int = 61;
pub const IPV6_DONTFRAG: c_int = 62;
pub const IPV6_RECVTCLASS: c_int = 66;
pub const IPV6_TCLASS: c_int = 67;
pub const IPV6_AUTOFLOWLABEL: c_int = 70;
pub const IPV6_ADDR_PREFERENCES: c_int = 72;
pub const IPV6_MINHOPCOUNT: c_int = 73;
pub const IPV6_ORIGDSTADDR: c_int = 74;
pub const IPV6_RECVORIGDSTADDR: c_int = IPV6_ORIGDSTADDR;
pub const IPV6_TRANSPARENT: c_int = 75;
pub const IPV6_UNICAST_IF: c_int = 76;
pub const IPV6_PREFER_SRC_TMP: c_int = 0x0001;
pub const IPV6_PREFER_SRC_PUBLIC: c_int = 0x0002;
pub const IPV6_PREFER_SRC_PUBTMP_DEFAULT: c_int = 0x0100;
pub const IPV6_PREFER_SRC_COA: c_int = 0x0004;
pub const IPV6_PREFER_SRC_HOME: c_int = 0x0400;
pub const IPV6_PREFER_SRC_CGA: c_int = 0x0008;
pub const IPV6_PREFER_SRC_NONCGA: c_int = 0x0800;

pub const IPV6_PMTUDISC_DONT: c_int = 0;
pub const IPV6_PMTUDISC_WANT: c_int = 1;
pub const IPV6_PMTUDISC_DO: c_int = 2;
pub const IPV6_PMTUDISC_PROBE: c_int = 3;
pub const IPV6_PMTUDISC_INTERFACE: c_int = 4;
pub const IPV6_PMTUDISC_OMIT: c_int = 5;

pub const TCP_NODELAY: c_int = 1;
pub const TCP_MAXSEG: c_int = 2;
pub const TCP_CORK: c_int = 3;
pub const TCP_KEEPIDLE: c_int = 4;
pub const TCP_KEEPINTVL: c_int = 5;
pub const TCP_KEEPCNT: c_int = 6;
pub const TCP_SYNCNT: c_int = 7;
pub const TCP_LINGER2: c_int = 8;
pub const TCP_DEFER_ACCEPT: c_int = 9;
pub const TCP_WINDOW_CLAMP: c_int = 10;
pub const TCP_INFO: c_int = 11;
pub const TCP_QUICKACK: c_int = 12;
pub const TCP_CONGESTION: c_int = 13;
pub const TCP_MD5SIG: c_int = 14;
cfg_if! {
    if #[cfg(all(
        target_os = "linux",
        any(target_env = "gnu", target_env = "musl", target_env = "ohos")
    ))] {
        // WARN: deprecated
        pub const TCP_COOKIE_TRANSACTIONS: c_int = 15;
    }
}
pub const TCP_THIN_LINEAR_TIMEOUTS: c_int = 16;
pub const TCP_THIN_DUPACK: c_int = 17;
pub const TCP_USER_TIMEOUT: c_int = 18;
pub const TCP_REPAIR: c_int = 19;
pub const TCP_REPAIR_QUEUE: c_int = 20;
pub const TCP_QUEUE_SEQ: c_int = 21;
pub const TCP_REPAIR_OPTIONS: c_int = 22;
pub const TCP_FASTOPEN: c_int = 23;
pub const TCP_TIMESTAMP: c_int = 24;
pub const TCP_NOTSENT_LOWAT: c_int = 25;
pub const TCP_CC_INFO: c_int = 26;
pub const TCP_SAVE_SYN: c_int = 27;
pub const TCP_SAVED_SYN: c_int = 28;
cfg_if! {
    if #[cfg(not(target_os = "emscripten"))] {
        // NOTE: emscripten doesn't support these options yet.

        pub const TCP_REPAIR_WINDOW: c_int = 29;
        pub const TCP_FASTOPEN_CONNECT: c_int = 30;
        pub const TCP_ULP: c_int = 31;
        pub const TCP_MD5SIG_EXT: c_int = 32;
        pub const TCP_FASTOPEN_KEY: c_int = 33;
        pub const TCP_FASTOPEN_NO_COOKIE: c_int = 34;
        pub const TCP_ZEROCOPY_RECEIVE: c_int = 35;
        pub const TCP_INQ: c_int = 36;
        pub const TCP_CM_INQ: c_int = TCP_INQ;
        // NOTE: Some CI images doesn't have this option yet.
        // pub const TCP_TX_DELAY: c_int = 37;
        pub const TCP_MD5SIG_MAXKEYLEN: usize = 80;
    }
}

pub const SO_DEBUG: c_int = 1;

pub const SHUT_RD: c_int = 0;
pub const SHUT_WR: c_int = 1;
pub const SHUT_RDWR: c_int = 2;

pub const LOCK_SH: c_int = 1;
pub const LOCK_EX: c_int = 2;
pub const LOCK_NB: c_int = 4;
pub const LOCK_UN: c_int = 8;

pub const SS_ONSTACK: c_int = 1;
pub const SS_DISABLE: c_int = 2;

pub const PATH_MAX: c_int = 4096;

pub const UIO_MAXIOV: c_int = 1024;

pub const FD_SETSIZE: usize = 1024;

pub const EPOLLIN: c_int = 0x1;
pub const EPOLLPRI: c_int = 0x2;
pub const EPOLLOUT: c_int = 0x4;
pub const EPOLLERR: c_int = 0x8;
pub const EPOLLHUP: c_int = 0x10;
pub const EPOLLRDNORM: c_int = 0x40;
pub const EPOLLRDBAND: c_int = 0x80;
pub const EPOLLWRNORM: c_int = 0x100;
pub const EPOLLWRBAND: c_int = 0x200;
pub const EPOLLMSG: c_int = 0x400;
pub const EPOLLRDHUP: c_int = 0x2000;
pub const EPOLLEXCLUSIVE: c_int = 0x10000000;
pub const EPOLLWAKEUP: c_int = 0x20000000;
pub const EPOLLONESHOT: c_int = 0x40000000;
pub const EPOLLET: c_int = 0x80000000;

pub const EPOLL_CTL_ADD: c_int = 1;
pub const EPOLL_CTL_MOD: c_int = 3;
pub const EPOLL_CTL_DEL: c_int = 2;

pub const MNT_FORCE: c_int = 0x1;
pub const MNT_DETACH: c_int = 0x2;
pub const MNT_EXPIRE: c_int = 0x4;
pub const UMOUNT_NOFOLLOW: c_int = 0x8;

pub const Q_GETFMT: c_int = 0x800004;
pub const Q_GETINFO: c_int = 0x800005;
pub const Q_SETINFO: c_int = 0x800006;
pub const QIF_BLIMITS: u32 = 1;
pub const QIF_SPACE: u32 = 2;
pub const QIF_ILIMITS: u32 = 4;
pub const QIF_INODES: u32 = 8;
pub const QIF_BTIME: u32 = 16;
pub const QIF_ITIME: u32 = 32;
pub const QIF_LIMITS: u32 = 5;
pub const QIF_USAGE: u32 = 10;
pub const QIF_TIMES: u32 = 48;
pub const QIF_ALL: u32 = 63;

pub const Q_SYNC: c_int = 0x800001;
pub const Q_QUOTAON: c_int = 0x800002;
pub const Q_QUOTAOFF: c_int = 0x800003;
pub const Q_GETQUOTA: c_int = 0x800007;
pub const Q_SETQUOTA: c_int = 0x800008;

pub const TCIOFF: c_int = 2;
pub const TCION: c_int = 3;
pub const TCOOFF: c_int = 0;
pub const TCOON: c_int = 1;
pub const TCIFLUSH: c_int = 0;
pub const TCOFLUSH: c_int = 1;
pub const TCIOFLUSH: c_int = 2;
pub const NL0: crate::tcflag_t = 0x00000000;
pub const NL1: crate::tcflag_t = 0x00000100;
pub const TAB0: crate::tcflag_t = 0x00000000;
pub const CR0: crate::tcflag_t = 0x00000000;
pub const FF0: crate::tcflag_t = 0x00000000;
pub const BS0: crate::tcflag_t = 0x00000000;
pub const VT0: crate::tcflag_t = 0x00000000;
pub const VERASE: usize = 2;
pub const VKILL: usize = 3;
pub const VINTR: usize = 0;
pub const VQUIT: usize = 1;
pub const VLNEXT: usize = 15;
pub const IGNBRK: crate::tcflag_t = 0x00000001;
pub const BRKINT: crate::tcflag_t = 0x00000002;
pub const IGNPAR: crate::tcflag_t = 0x00000004;
pub const PARMRK: crate::tcflag_t = 0x00000008;
pub const INPCK: crate::tcflag_t = 0x00000010;
pub const ISTRIP: crate::tcflag_t = 0x00000020;
pub const INLCR: crate::tcflag_t = 0x00000040;
pub const IGNCR: crate::tcflag_t = 0x00000080;
pub const ICRNL: crate::tcflag_t = 0x00000100;
pub const IXANY: crate::tcflag_t = 0x00000800;
pub const IMAXBEL: crate::tcflag_t = 0x00002000;
pub const OPOST: crate::tcflag_t = 0x1;
pub const CS5: crate::tcflag_t = 0x00000000;
pub const CRTSCTS: crate::tcflag_t = 0x80000000;
pub const ECHO: crate::tcflag_t = 0x00000008;
pub const OCRNL: crate::tcflag_t = 0o000010;
pub const ONOCR: crate::tcflag_t = 0o000020;
pub const ONLRET: crate::tcflag_t = 0o000040;
pub const OFILL: crate::tcflag_t = 0o000100;
pub const OFDEL: crate::tcflag_t = 0o000200;

pub const CLONE_VM: c_int = 0x100;
pub const CLONE_FS: c_int = 0x200;
pub const CLONE_FILES: c_int = 0x400;
pub const CLONE_SIGHAND: c_int = 0x800;
pub const CLONE_PTRACE: c_int = 0x2000;
pub const CLONE_VFORK: c_int = 0x4000;
pub const CLONE_PARENT: c_int = 0x8000;
pub const CLONE_THREAD: c_int = 0x10000;
pub const CLONE_NEWNS: c_int = 0x20000;
pub const CLONE_SYSVSEM: c_int = 0x40000;
pub const CLONE_SETTLS: c_int = 0x80000;
pub const CLONE_PARENT_SETTID: c_int = 0x100000;
pub const CLONE_CHILD_CLEARTID: c_int = 0x200000;
pub const CLONE_DETACHED: c_int = 0x400000;
pub const CLONE_UNTRACED: c_int = 0x800000;
pub const CLONE_CHILD_SETTID: c_int = 0x01000000;
pub const CLONE_NEWCGROUP: c_int = 0x02000000;
pub const CLONE_NEWUTS: c_int = 0x04000000;
pub const CLONE_NEWIPC: c_int = 0x08000000;
pub const CLONE_NEWUSER: c_int = 0x10000000;
pub const CLONE_NEWPID: c_int = 0x20000000;
pub const CLONE_NEWNET: c_int = 0x40000000;
pub const CLONE_IO: c_int = 0x80000000;

pub const WNOHANG: c_int = 0x00000001;
pub const WUNTRACED: c_int = 0x00000002;
pub const WSTOPPED: c_int = WUNTRACED;
pub const WEXITED: c_int = 0x00000004;
pub const WCONTINUED: c_int = 0x00000008;
pub const WNOWAIT: c_int = 0x01000000;

// Options for personality(2).
pub const ADDR_NO_RANDOMIZE: c_int = 0x0040000;
pub const MMAP_PAGE_ZERO: c_int = 0x0100000;
pub const ADDR_COMPAT_LAYOUT: c_int = 0x0200000;
pub const READ_IMPLIES_EXEC: c_int = 0x0400000;
pub const ADDR_LIMIT_32BIT: c_int = 0x0800000;
pub const SHORT_INODE: c_int = 0x1000000;
pub const WHOLE_SECONDS: c_int = 0x2000000;
pub const STICKY_TIMEOUTS: c_int = 0x4000000;
pub const ADDR_LIMIT_3GB: c_int = 0x8000000;

// Options set using PTRACE_SETOPTIONS.
pub const PTRACE_O_TRACESYSGOOD: c_int = 0x00000001;
pub const PTRACE_O_TRACEFORK: c_int = 0x00000002;
pub const PTRACE_O_TRACEVFORK: c_int = 0x00000004;
pub const PTRACE_O_TRACECLONE: c_int = 0x00000008;
pub const PTRACE_O_TRACEEXEC: c_int = 0x00000010;
pub const PTRACE_O_TRACEVFORKDONE: c_int = 0x00000020;
pub const PTRACE_O_TRACEEXIT: c_int = 0x00000040;
pub const PTRACE_O_TRACESECCOMP: c_int = 0x00000080;
pub const PTRACE_O_SUSPEND_SECCOMP: c_int = 0x00200000;
pub const PTRACE_O_EXITKILL: c_int = 0x00100000;
pub const PTRACE_O_MASK: c_int = 0x003000ff;

// Wait extended result codes for the above trace options.
pub const PTRACE_EVENT_FORK: c_int = 1;
pub const PTRACE_EVENT_VFORK: c_int = 2;
pub const PTRACE_EVENT_CLONE: c_int = 3;
pub const PTRACE_EVENT_EXEC: c_int = 4;
pub const PTRACE_EVENT_VFORK_DONE: c_int = 5;
pub const PTRACE_EVENT_EXIT: c_int = 6;
pub const PTRACE_EVENT_SECCOMP: c_int = 7;

pub const __WNOTHREAD: c_int = 0x20000000;
pub const __WALL: c_int = 0x40000000;
pub const __WCLONE: c_int = 0x80000000;

pub const SPLICE_F_MOVE: c_uint = 0x01;
pub const SPLICE_F_NONBLOCK: c_uint = 0x02;
pub const SPLICE_F_MORE: c_uint = 0x04;
pub const SPLICE_F_GIFT: c_uint = 0x08;

pub const RTLD_LOCAL: c_int = 0;
pub const RTLD_LAZY: c_int = 1;

pub const POSIX_FADV_NORMAL: c_int = 0;
pub const POSIX_FADV_RANDOM: c_int = 1;
pub const POSIX_FADV_SEQUENTIAL: c_int = 2;
pub const POSIX_FADV_WILLNEED: c_int = 3;

pub const AT_FDCWD: c_int = -100;
pub const AT_SYMLINK_NOFOLLOW: c_int = 0x100;
pub const AT_REMOVEDIR: c_int = 0x200;
pub const AT_SYMLINK_FOLLOW: c_int = 0x400;
pub const AT_NO_AUTOMOUNT: c_int = 0x800;
pub const AT_EMPTY_PATH: c_int = 0x1000;
pub const AT_RECURSIVE: c_int = 0x8000;

pub const LOG_CRON: c_int = 9 << 3;
pub const LOG_AUTHPRIV: c_int = 10 << 3;
pub const LOG_FTP: c_int = 11 << 3;
pub const LOG_PERROR: c_int = 0x20;

pub const PIPE_BUF: usize = 4096;

pub const SI_LOAD_SHIFT: c_uint = 16;

// si_code values
pub const SI_USER: c_int = 0;
pub const SI_KERNEL: c_int = 0x80;
pub const SI_QUEUE: c_int = -1;
cfg_if! {
    if #[cfg(not(any(
        target_arch = "mips",
        target_arch = "mips32r6",
        target_arch = "mips64"
    )))] {
        pub const SI_TIMER: c_int = -2;
        pub const SI_MESGQ: c_int = -3;
        pub const SI_ASYNCIO: c_int = -4;
    } else {
        pub const SI_TIMER: c_int = -3;
        pub const SI_MESGQ: c_int = -4;
        pub const SI_ASYNCIO: c_int = -2;
    }
}
pub const SI_SIGIO: c_int = -5;
pub const SI_TKILL: c_int = -6;
pub const SI_ASYNCNL: c_int = -60;

// si_code values for SIGBUS signal
pub const BUS_ADRALN: c_int = 1;
pub const BUS_ADRERR: c_int = 2;
pub const BUS_OBJERR: c_int = 3;
// Linux-specific si_code values for SIGBUS signal
pub const BUS_MCEERR_AR: c_int = 4;
pub const BUS_MCEERR_AO: c_int = 5;

// si_code values for SIGTRAP
pub const TRAP_BRKPT: c_int = 1;
pub const TRAP_TRACE: c_int = 2;
pub const TRAP_BRANCH: c_int = 3;
pub const TRAP_HWBKPT: c_int = 4;
pub const TRAP_UNK: c_int = 5;

// si_code values for SIGCHLD signal
pub const CLD_EXITED: c_int = 1;
pub const CLD_KILLED: c_int = 2;
pub const CLD_DUMPED: c_int = 3;
pub const CLD_TRAPPED: c_int = 4;
pub const CLD_STOPPED: c_int = 5;
pub const CLD_CONTINUED: c_int = 6;

pub const SIGEV_SIGNAL: c_int = 0;
pub const SIGEV_NONE: c_int = 1;
pub const SIGEV_THREAD: c_int = 2;

pub const P_ALL: idtype_t = 0;
pub const P_PID: idtype_t = 1;
pub const P_PGID: idtype_t = 2;
cfg_if! {
    if #[cfg(not(target_os = "emscripten"))] {
        pub const P_PIDFD: idtype_t = 3;
    }
}

pub const UTIME_OMIT: c_long = 1073741822;
pub const UTIME_NOW: c_long = 1073741823;

pub const POLLIN: c_short = 0x1;
pub const POLLPRI: c_short = 0x2;
pub const POLLOUT: c_short = 0x4;
pub const POLLERR: c_short = 0x8;
pub const POLLHUP: c_short = 0x10;
pub const POLLNVAL: c_short = 0x20;
pub const POLLRDNORM: c_short = 0x040;
pub const POLLRDBAND: c_short = 0x080;
#[cfg(not(any(target_arch = "sparc", target_arch = "sparc64")))]
pub const POLLRDHUP: c_short = 0x2000;
#[cfg(any(target_arch = "sparc", target_arch = "sparc64"))]
pub const POLLRDHUP: c_short = 0x800;

pub const IPTOS_LOWDELAY: u8 = 0x10;
pub const IPTOS_THROUGHPUT: u8 = 0x08;
pub const IPTOS_RELIABILITY: u8 = 0x04;
pub const IPTOS_MINCOST: u8 = 0x02;

pub const IPTOS_PREC_NETCONTROL: u8 = 0xe0;
pub const IPTOS_PREC_INTERNETCONTROL: u8 = 0xc0;
pub const IPTOS_PREC_CRITIC_ECP: u8 = 0xa0;
pub const IPTOS_PREC_FLASHOVERRIDE: u8 = 0x80;
pub const IPTOS_PREC_FLASH: u8 = 0x60;
pub const IPTOS_PREC_IMMEDIATE: u8 = 0x40;
pub const IPTOS_PREC_PRIORITY: u8 = 0x20;
pub const IPTOS_PREC_ROUTINE: u8 = 0x00;

pub const IPTOS_ECN_MASK: u8 = 0x03;
pub const IPTOS_ECN_ECT1: u8 = 0x01;
pub const IPTOS_ECN_ECT0: u8 = 0x02;
pub const IPTOS_ECN_CE: u8 = 0x03;

pub const IPOPT_COPY: u8 = 0x80;
pub const IPOPT_CLASS_MASK: u8 = 0x60;
pub const IPOPT_NUMBER_MASK: u8 = 0x1f;

pub const IPOPT_CONTROL: u8 = 0x00;
pub const IPOPT_RESERVED1: u8 = 0x20;
pub const IPOPT_MEASUREMENT: u8 = 0x40;
pub const IPOPT_RESERVED2: u8 = 0x60;
pub const IPOPT_END: u8 = 0 | IPOPT_CONTROL;
pub const IPOPT_NOOP: u8 = 1 | IPOPT_CONTROL;
pub const IPOPT_SEC: u8 = 2 | IPOPT_CONTROL | IPOPT_COPY;
pub const IPOPT_LSRR: u8 = 3 | IPOPT_CONTROL | IPOPT_COPY;
pub const IPOPT_TIMESTAMP: u8 = 4 | IPOPT_MEASUREMENT;
pub const IPOPT_RR: u8 = 7 | IPOPT_CONTROL;
pub const IPOPT_SID: u8 = 8 | IPOPT_CONTROL | IPOPT_COPY;
pub const IPOPT_SSRR: u8 = 9 | IPOPT_CONTROL | IPOPT_COPY;
pub const IPOPT_RA: u8 = 20 | IPOPT_CONTROL | IPOPT_COPY;
pub const IPVERSION: u8 = 4;
pub const MAXTTL: u8 = 255;
pub const IPDEFTTL: u8 = 64;
pub const IPOPT_OPTVAL: u8 = 0;
pub const IPOPT_OLEN: u8 = 1;
pub const IPOPT_OFFSET: u8 = 2;
pub const IPOPT_MINOFF: u8 = 4;
pub const MAX_IPOPTLEN: u8 = 40;
pub const IPOPT_NOP: u8 = IPOPT_NOOP;
pub const IPOPT_EOL: u8 = IPOPT_END;
pub const IPOPT_TS: u8 = IPOPT_TIMESTAMP;
pub const IPOPT_TS_TSONLY: u8 = 0;
pub const IPOPT_TS_TSANDADDR: u8 = 1;
pub const IPOPT_TS_PRESPEC: u8 = 3;

pub const ARPOP_RREQUEST: u16 = 3;
pub const ARPOP_RREPLY: u16 = 4;
pub const ARPOP_InREQUEST: u16 = 8;
pub const ARPOP_InREPLY: u16 = 9;
pub const ARPOP_NAK: u16 = 10;

pub const ATF_NETMASK: c_int = 0x20;
pub const ATF_DONTPUB: c_int = 0x40;

pub const ARPHRD_NETROM: u16 = 0;
pub const ARPHRD_ETHER: u16 = 1;
pub const ARPHRD_EETHER: u16 = 2;
pub const ARPHRD_AX25: u16 = 3;
pub const ARPHRD_PRONET: u16 = 4;
pub const ARPHRD_CHAOS: u16 = 5;
pub const ARPHRD_IEEE802: u16 = 6;
pub const ARPHRD_ARCNET: u16 = 7;
pub const ARPHRD_APPLETLK: u16 = 8;
pub const ARPHRD_DLCI: u16 = 15;
pub const ARPHRD_ATM: u16 = 19;
pub const ARPHRD_METRICOM: u16 = 23;
pub const ARPHRD_IEEE1394: u16 = 24;
pub const ARPHRD_EUI64: u16 = 27;
pub const ARPHRD_INFINIBAND: u16 = 32;

pub const ARPHRD_SLIP: u16 = 256;
pub const ARPHRD_CSLIP: u16 = 257;
pub const ARPHRD_SLIP6: u16 = 258;
pub const ARPHRD_CSLIP6: u16 = 259;
pub const ARPHRD_RSRVD: u16 = 260;
pub const ARPHRD_ADAPT: u16 = 264;
pub const ARPHRD_ROSE: u16 = 270;
pub const ARPHRD_X25: u16 = 271;
pub const ARPHRD_HWX25: u16 = 272;
pub const ARPHRD_CAN: u16 = 280;
pub const ARPHRD_PPP: u16 = 512;
pub const ARPHRD_CISCO: u16 = 513;
pub const ARPHRD_HDLC: u16 = ARPHRD_CISCO;
pub const ARPHRD_LAPB: u16 = 516;
pub const ARPHRD_DDCMP: u16 = 517;
pub const ARPHRD_RAWHDLC: u16 = 518;

pub const ARPHRD_TUNNEL: u16 = 768;
pub const ARPHRD_TUNNEL6: u16 = 769;
pub const ARPHRD_FRAD: u16 = 770;
pub const ARPHRD_SKIP: u16 = 771;
pub const ARPHRD_LOOPBACK: u16 = 772;
pub const ARPHRD_LOCALTLK: u16 = 773;
pub const ARPHRD_FDDI: u16 = 774;
pub const ARPHRD_BIF: u16 = 775;
pub const ARPHRD_SIT: u16 = 776;
pub const ARPHRD_IPDDP: u16 = 777;
pub const ARPHRD_IPGRE: u16 = 778;
pub const ARPHRD_PIMREG: u16 = 779;
pub const ARPHRD_HIPPI: u16 = 780;
pub const ARPHRD_ASH: u16 = 781;
pub const ARPHRD_ECONET: u16 = 782;
pub const ARPHRD_IRDA: u16 = 783;
pub const ARPHRD_FCPP: u16 = 784;
pub const ARPHRD_FCAL: u16 = 785;
pub const ARPHRD_FCPL: u16 = 786;
pub const ARPHRD_FCFABRIC: u16 = 787;
pub const ARPHRD_IEEE802_TR: u16 = 800;
pub const ARPHRD_IEEE80211: u16 = 801;
pub const ARPHRD_IEEE80211_PRISM: u16 = 802;
pub const ARPHRD_IEEE80211_RADIOTAP: u16 = 803;
pub const ARPHRD_IEEE802154: u16 = 804;

pub const ARPHRD_VOID: u16 = 0xFFFF;
pub const ARPHRD_NONE: u16 = 0xFFFE;

cfg_if! {
    if #[cfg(not(target_os = "emscripten"))] {
        // linux/if_tun.h
        /* TUNSETIFF ifr flags */
        pub const IFF_TUN: c_int = 0x0001;
        pub const IFF_TAP: c_int = 0x0002;
        pub const IFF_NAPI: c_int = 0x0010;
        pub const IFF_NAPI_FRAGS: c_int = 0x0020;
        // Used in TUNSETIFF to bring up tun/tap without carrier
        pub const IFF_NO_CARRIER: c_int = 0x0040;
        pub const IFF_NO_PI: c_int = 0x1000;
        // Read queue size
        pub const TUN_READQ_SIZE: c_short = 500;
        // TUN device type flags: deprecated. Use IFF_TUN/IFF_TAP instead.
        pub const TUN_TUN_DEV: c_short = crate::IFF_TUN as c_short;
        pub const TUN_TAP_DEV: c_short = crate::IFF_TAP as c_short;
        pub const TUN_TYPE_MASK: c_short = 0x000f;
        // This flag has no real effect
        pub const IFF_ONE_QUEUE: c_int = 0x2000;
        pub const IFF_VNET_HDR: c_int = 0x4000;
        pub const IFF_TUN_EXCL: c_int = 0x8000;
        pub const IFF_MULTI_QUEUE: c_int = 0x0100;
        pub const IFF_ATTACH_QUEUE: c_int = 0x0200;
        pub const IFF_DETACH_QUEUE: c_int = 0x0400;
        // read-only flag
        pub const IFF_PERSIST: c_int = 0x0800;
        pub const IFF_NOFILTER: c_int = 0x1000;
        // Socket options
        pub const TUN_TX_TIMESTAMP: c_int = 1;
        // Features for GSO (TUNSETOFFLOAD)
        pub const TUN_F_CSUM: c_uint = 0x01;
        pub const TUN_F_TSO4: c_uint = 0x02;
        pub const TUN_F_TSO6: c_uint = 0x04;
        pub const TUN_F_TSO_ECN: c_uint = 0x08;
        pub const TUN_F_UFO: c_uint = 0x10;
        pub const TUN_F_USO4: c_uint = 0x20;
        pub const TUN_F_USO6: c_uint = 0x40;
        // Protocol info prepended to the packets (when IFF_NO_PI is not set)
        pub const TUN_PKT_STRIP: c_int = 0x0001;
        // Accept all multicast packets
        pub const TUN_FLT_ALLMULTI: c_int = 0x0001;
        // Ioctl operation codes
        const T_TYPE: u32 = b'T' as u32;
        pub const TUNSETNOCSUM: Ioctl = _IOW::<c_int>(T_TYPE, 200);
        pub const TUNSETDEBUG: Ioctl = _IOW::<c_int>(T_TYPE, 201);
        pub const TUNSETIFF: Ioctl = _IOW::<c_int>(T_TYPE, 202);
        pub const TUNSETPERSIST: Ioctl = _IOW::<c_int>(T_TYPE, 203);
        pub const TUNSETOWNER: Ioctl = _IOW::<c_int>(T_TYPE, 204);
        pub const TUNSETLINK: Ioctl = _IOW::<c_int>(T_TYPE, 205);
        pub const TUNSETGROUP: Ioctl = _IOW::<c_int>(T_TYPE, 206);
        pub const TUNGETFEATURES: Ioctl = _IOR::<c_int>(T_TYPE, 207);
        pub const TUNSETOFFLOAD: Ioctl = _IOW::<c_int>(T_TYPE, 208);
        pub const TUNSETTXFILTER: Ioctl = _IOW::<c_int>(T_TYPE, 209);
        pub const TUNGETIFF: Ioctl = _IOR::<c_int>(T_TYPE, 210);
        pub const TUNGETSNDBUF: Ioctl = _IOR::<c_int>(T_TYPE, 211);
        pub const TUNSETSNDBUF: Ioctl = _IOW::<c_int>(T_TYPE, 212);
        pub const TUNATTACHFILTER: Ioctl = _IOW::<sock_fprog>(T_TYPE, 213);
        pub const TUNDETACHFILTER: Ioctl = _IOW::<sock_fprog>(T_TYPE, 214);
        pub const TUNGETVNETHDRSZ: Ioctl = _IOR::<c_int>(T_TYPE, 215);
        pub const TUNSETVNETHDRSZ: Ioctl = _IOW::<c_int>(T_TYPE, 216);
        pub const TUNSETQUEUE: Ioctl = _IOW::<c_int>(T_TYPE, 217);
        pub const TUNSETIFINDEX: Ioctl = _IOW::<c_int>(T_TYPE, 218);
        pub const TUNGETFILTER: Ioctl = _IOR::<sock_fprog>(T_TYPE, 219);
        pub const TUNSETVNETLE: Ioctl = _IOW::<c_int>(T_TYPE, 220);
        pub const TUNGETVNETLE: Ioctl = _IOR::<c_int>(T_TYPE, 221);
        pub const TUNSETVNETBE: Ioctl = _IOW::<c_int>(T_TYPE, 222);
        pub const TUNGETVNETBE: Ioctl = _IOR::<c_int>(T_TYPE, 223);
        pub const TUNSETSTEERINGEBPF: Ioctl = _IOR::<c_int>(T_TYPE, 224);
        pub const TUNSETFILTEREBPF: Ioctl = _IOR::<c_int>(T_TYPE, 225);
        pub const TUNSETCARRIER: Ioctl = _IOW::<c_int>(T_TYPE, 226);
        pub const TUNGETDEVNETNS: Ioctl = _IO(T_TYPE, 227);

        // linux/fs.h
        pub const FS_IOC_GETFLAGS: Ioctl = _IOR::<c_long>('f' as u32, 1);
        pub const FS_IOC_SETFLAGS: Ioctl = _IOW::<c_long>('f' as u32, 2);
        pub const FS_IOC_GETVERSION: Ioctl = _IOR::<c_long>('v' as u32, 1);
        pub const FS_IOC_SETVERSION: Ioctl = _IOW::<c_long>('v' as u32, 2);
        pub const FS_IOC32_GETFLAGS: Ioctl = _IOR::<c_int>('f' as u32, 1);
        pub const FS_IOC32_SETFLAGS: Ioctl = _IOW::<c_int>('f' as u32, 2);
        pub const FS_IOC32_GETVERSION: Ioctl = _IOR::<c_int>('v' as u32, 1);
        pub const FS_IOC32_SETVERSION: Ioctl = _IOW::<c_int>('v' as u32, 2);

        pub const FICLONE: Ioctl = _IOW::<c_int>(0x94, 9);
        pub const FICLONERANGE: Ioctl = _IOW::<crate::file_clone_range>(0x94, 13);
    }
}

cfg_if! {
    if #[cfg(target_os = "emscripten")] {
        // Emscripten does not define any `*_SUPER_MAGIC` constants.
    } else if #[cfg(not(target_arch = "s390x"))] {
        pub const ADFS_SUPER_MAGIC: c_long = 0x0000adf5;
        pub const AFFS_SUPER_MAGIC: c_long = 0x0000adff;
        pub const AFS_SUPER_MAGIC: c_long = 0x5346414f;
        pub const AUTOFS_SUPER_MAGIC: c_long = 0x0187;
        pub const BPF_FS_MAGIC: c_long = 0xcafe4a11;
        pub const BTRFS_SUPER_MAGIC: c_long = 0x9123683e;
        pub const CGROUP2_SUPER_MAGIC: c_long = 0x63677270;
        pub const CGROUP_SUPER_MAGIC: c_long = 0x27e0eb;
        pub const CODA_SUPER_MAGIC: c_long = 0x73757245;
        pub const CRAMFS_MAGIC: c_long = 0x28cd3d45;
        pub const DEBUGFS_MAGIC: c_long = 0x64626720;
        pub const DEVPTS_SUPER_MAGIC: c_long = 0x1cd1;
        pub const ECRYPTFS_SUPER_MAGIC: c_long = 0xf15f;
        pub const EFS_SUPER_MAGIC: c_long = 0x00414a53;
        pub const EXT2_SUPER_MAGIC: c_long = 0x0000ef53;
        pub const EXT3_SUPER_MAGIC: c_long = 0x0000ef53;
        pub const EXT4_SUPER_MAGIC: c_long = 0x0000ef53;
        pub const F2FS_SUPER_MAGIC: c_long = 0xf2f52010;
        pub const FUSE_SUPER_MAGIC: c_long = 0x65735546;
        pub const FUTEXFS_SUPER_MAGIC: c_long = 0xbad1dea;
        pub const HOSTFS_SUPER_MAGIC: c_long = 0x00c0ffee;
        pub const HPFS_SUPER_MAGIC: c_long = 0xf995e849;
        pub const HUGETLBFS_MAGIC: c_long = 0x958458f6;
        pub const ISOFS_SUPER_MAGIC: c_long = 0x00009660;
        pub const JFFS2_SUPER_MAGIC: c_long = 0x000072b6;
        pub const MINIX2_SUPER_MAGIC2: c_long = 0x00002478;
        pub const MINIX2_SUPER_MAGIC: c_long = 0x00002468;
        pub const MINIX3_SUPER_MAGIC: c_long = 0x4d5a;
        pub const MINIX_SUPER_MAGIC2: c_long = 0x0000138f;
        pub const MINIX_SUPER_MAGIC: c_long = 0x0000137f;
        pub const MSDOS_SUPER_MAGIC: c_long = 0x00004d44;
        pub const NCP_SUPER_MAGIC: c_long = 0x0000564c;
        pub const NFS_SUPER_MAGIC: c_long = 0x00006969;
        pub const NILFS_SUPER_MAGIC: c_long = 0x3434;
        pub const OCFS2_SUPER_MAGIC: c_long = 0x7461636f;
        pub const OPENPROM_SUPER_MAGIC: c_long = 0x00009fa1;
        pub const OVERLAYFS_SUPER_MAGIC: c_long = 0x794c7630;
        pub const PROC_SUPER_MAGIC: c_long = 0x00009fa0;
        pub const QNX4_SUPER_MAGIC: c_long = 0x0000002f;
        pub const QNX6_SUPER_MAGIC: c_long = 0x68191122;
        pub const RDTGROUP_SUPER_MAGIC: c_long = 0x7655821;
        pub const REISERFS_SUPER_MAGIC: c_long = 0x52654973;
        pub const SECURITYFS_MAGIC: c_long = 0x73636673;
        pub const SELINUX_MAGIC: c_long = 0xf97cff8c;
        pub const SMACK_MAGIC: c_long = 0x43415d53;
        pub const SMB_SUPER_MAGIC: c_long = 0x0000517b;
        pub const SYSFS_MAGIC: c_long = 0x62656572;
        pub const TMPFS_MAGIC: c_long = 0x01021994;
        pub const TRACEFS_MAGIC: c_long = 0x74726163;
        pub const UDF_SUPER_MAGIC: c_long = 0x15013346;
        pub const USBDEVICE_SUPER_MAGIC: c_long = 0x00009fa2;
        pub const XENFS_SUPER_MAGIC: c_long = 0xabba1974;
        pub const NSFS_MAGIC: c_long = 0x6e736673;
    } else if #[cfg(target_arch = "s390x")] {
        pub const ADFS_SUPER_MAGIC: c_uint = 0x0000adf5;
        pub const AFFS_SUPER_MAGIC: c_uint = 0x0000adff;
        pub const AFS_SUPER_MAGIC: c_uint = 0x5346414f;
        pub const AUTOFS_SUPER_MAGIC: c_uint = 0x0187;
        pub const BPF_FS_MAGIC: c_uint = 0xcafe4a11;
        pub const BTRFS_SUPER_MAGIC: c_uint = 0x9123683e;
        pub const CGROUP2_SUPER_MAGIC: c_uint = 0x63677270;
        pub const CGROUP_SUPER_MAGIC: c_uint = 0x27e0eb;
        pub const CODA_SUPER_MAGIC: c_uint = 0x73757245;
        pub const CRAMFS_MAGIC: c_uint = 0x28cd3d45;
        pub const DEBUGFS_MAGIC: c_uint = 0x64626720;
        pub const DEVPTS_SUPER_MAGIC: c_uint = 0x1cd1;
        pub const ECRYPTFS_SUPER_MAGIC: c_uint = 0xf15f;
        pub const EFS_SUPER_MAGIC: c_uint = 0x00414a53;
        pub const EXT2_SUPER_MAGIC: c_uint = 0x0000ef53;
        pub const EXT3_SUPER_MAGIC: c_uint = 0x0000ef53;
        pub const EXT4_SUPER_MAGIC: c_uint = 0x0000ef53;
        pub const F2FS_SUPER_MAGIC: c_uint = 0xf2f52010;
        pub const FUSE_SUPER_MAGIC: c_uint = 0x65735546;
        pub const FUTEXFS_SUPER_MAGIC: c_uint = 0xbad1dea;
        pub const HOSTFS_SUPER_MAGIC: c_uint = 0x00c0ffee;
        pub const HPFS_SUPER_MAGIC: c_uint = 0xf995e849;
        pub const HUGETLBFS_MAGIC: c_uint = 0x958458f6;
        pub const ISOFS_SUPER_MAGIC: c_uint = 0x00009660;
        pub const JFFS2_SUPER_MAGIC: c_uint = 0x000072b6;
        pub const MINIX2_SUPER_MAGIC2: c_uint = 0x00002478;
        pub const MINIX2_SUPER_MAGIC: c_uint = 0x00002468;
        pub const MINIX3_SUPER_MAGIC: c_uint = 0x4d5a;
        pub const MINIX_SUPER_MAGIC2: c_uint = 0x0000138f;
        pub const MINIX_SUPER_MAGIC: c_uint = 0x0000137f;
        pub const MSDOS_SUPER_MAGIC: c_uint = 0x00004d44;
        pub const NCP_SUPER_MAGIC: c_uint = 0x0000564c;
        pub const NFS_SUPER_MAGIC: c_uint = 0x00006969;
        pub const NILFS_SUPER_MAGIC: c_uint = 0x3434;
        pub const OCFS2_SUPER_MAGIC: c_uint = 0x7461636f;
        pub const OPENPROM_SUPER_MAGIC: c_uint = 0x00009fa1;
        pub const OVERLAYFS_SUPER_MAGIC: c_uint = 0x794c7630;
        pub const PROC_SUPER_MAGIC: c_uint = 0x00009fa0;
        pub const QNX4_SUPER_MAGIC: c_uint = 0x0000002f;
        pub const QNX6_SUPER_MAGIC: c_uint = 0x68191122;
        pub const RDTGROUP_SUPER_MAGIC: c_uint = 0x7655821;
        pub const REISERFS_SUPER_MAGIC: c_uint = 0x52654973;
        pub const SECURITYFS_MAGIC: c_uint = 0x73636673;
        pub const SELINUX_MAGIC: c_uint = 0xf97cff8c;
        pub const SMACK_MAGIC: c_uint = 0x43415d53;
        pub const SMB_SUPER_MAGIC: c_uint = 0x0000517b;
        pub const SYSFS_MAGIC: c_uint = 0x62656572;
        pub const TMPFS_MAGIC: c_uint = 0x01021994;
        pub const TRACEFS_MAGIC: c_uint = 0x74726163;
        pub const UDF_SUPER_MAGIC: c_uint = 0x15013346;
        pub const USBDEVICE_SUPER_MAGIC: c_uint = 0x00009fa2;
        pub const XENFS_SUPER_MAGIC: c_uint = 0xabba1974;
        pub const NSFS_MAGIC: c_uint = 0x6e736673;
    }
}

cfg_if! {
    if #[cfg(any(
        target_env = "gnu",
        target_os = "android",
        all(target_env = "musl", musl_v1_2_3)
    ))] {
        pub const AT_STATX_SYNC_TYPE: c_int = 0x6000;
        pub const AT_STATX_SYNC_AS_STAT: c_int = 0x0000;
        pub const AT_STATX_FORCE_SYNC: c_int = 0x2000;
        pub const AT_STATX_DONT_SYNC: c_int = 0x4000;
        pub const STATX_TYPE: c_uint = 0x0001;
        pub const STATX_MODE: c_uint = 0x0002;
        pub const STATX_NLINK: c_uint = 0x0004;
        pub const STATX_UID: c_uint = 0x0008;
        pub const STATX_GID: c_uint = 0x0010;
        pub const STATX_ATIME: c_uint = 0x0020;
        pub const STATX_MTIME: c_uint = 0x0040;
        pub const STATX_CTIME: c_uint = 0x0080;
        pub const STATX_INO: c_uint = 0x0100;
        pub const STATX_SIZE: c_uint = 0x0200;
        pub const STATX_BLOCKS: c_uint = 0x0400;
        pub const STATX_BASIC_STATS: c_uint = 0x07ff;
        pub const STATX_BTIME: c_uint = 0x0800;
        pub const STATX_ALL: c_uint = 0x0fff;
        pub const STATX_MNT_ID: c_uint = 0x1000;
        pub const STATX_DIOALIGN: c_uint = 0x2000;
        pub const STATX__RESERVED: c_int = 0x80000000;
        pub const STATX_ATTR_COMPRESSED: c_int = 0x0004;
        pub const STATX_ATTR_IMMUTABLE: c_int = 0x0010;
        pub const STATX_ATTR_APPEND: c_int = 0x0020;
        pub const STATX_ATTR_NODUMP: c_int = 0x0040;
        pub const STATX_ATTR_ENCRYPTED: c_int = 0x0800;
        pub const STATX_ATTR_AUTOMOUNT: c_int = 0x1000;
        pub const STATX_ATTR_MOUNT_ROOT: c_int = 0x2000;
        pub const STATX_ATTR_VERITY: c_int = 0x100000;
        pub const STATX_ATTR_DAX: c_int = 0x200000;
    }
}

// https://github.com/search?q=repo%3Atorvalds%2Flinux+%22%23define+_IOC_NONE%22&type=code
cfg_if! {
    if #[cfg(not(target_os = "emscripten"))] {
        const _IOC_NRBITS: u32 = 8;
        const _IOC_TYPEBITS: u32 = 8;

        cfg_if! {
            if #[cfg(any(
                any(target_arch = "powerpc", target_arch = "powerpc64"),
                any(target_arch = "sparc", target_arch = "sparc64"),
                any(target_arch = "mips", target_arch = "mips64"),
            ))] {
                // https://github.com/torvalds/linux/blob/b311c1b497e51a628aa89e7cb954481e5f9dced2/arch/powerpc/include/uapi/asm/ioctl.h
                // https://github.com/torvalds/linux/blob/b311c1b497e51a628aa89e7cb954481e5f9dced2/arch/sparc/include/uapi/asm/ioctl.h
                // https://github.com/torvalds/linux/blob/b311c1b497e51a628aa89e7cb954481e5f9dced2/arch/mips/include/uapi/asm/ioctl.h

                const _IOC_SIZEBITS: u32 = 13;
                const _IOC_DIRBITS: u32 = 3;

                const _IOC_NONE: u32 = 1;
                const _IOC_READ: u32 = 2;
                const _IOC_WRITE: u32 = 4;
            } else {
                // https://github.com/torvalds/linux/blob/b311c1b497e51a628aa89e7cb954481e5f9dced2/include/uapi/asm-generic/ioctl.h

                const _IOC_SIZEBITS: u32 = 14;
                const _IOC_DIRBITS: u32 = 2;

                const _IOC_NONE: u32 = 0;
                const _IOC_WRITE: u32 = 1;
                const _IOC_READ: u32 = 2;
            }
        }
        const _IOC_NRMASK: u32 = (1 << _IOC_NRBITS) - 1;
        const _IOC_TYPEMASK: u32 = (1 << _IOC_TYPEBITS) - 1;
        const _IOC_SIZEMASK: u32 = (1 << _IOC_SIZEBITS) - 1;
        const _IOC_DIRMASK: u32 = (1 << _IOC_DIRBITS) - 1;

        const _IOC_NRSHIFT: u32 = 0;
        const _IOC_TYPESHIFT: u32 = _IOC_NRSHIFT + _IOC_NRBITS;
        const _IOC_SIZESHIFT: u32 = _IOC_TYPESHIFT + _IOC_TYPEBITS;
        const _IOC_DIRSHIFT: u32 = _IOC_SIZESHIFT + _IOC_SIZEBITS;

        // adapted from https://github.com/torvalds/linux/blob/8a696a29c6905594e4abf78eaafcb62165ac61f1/rust/kernel/ioctl.rs

        /// Build an ioctl number, analogous to the C macro of the same name.
        const fn _IOC(dir: u32, ty: u32, nr: u32, size: usize) -> Ioctl {
            // FIXME(ctest) the `garando_syntax` crate (used by ctest in the CI test suite)
            // cannot currently parse these `debug_assert!`s
            //
            // debug_assert!(dir <= _IOC_DIRMASK);
            // debug_assert!(ty <= _IOC_TYPEMASK);
            // debug_assert!(nr <= _IOC_NRMASK);
            // debug_assert!(size <= (_IOC_SIZEMASK as usize));

            ((dir << _IOC_DIRSHIFT)
                | (ty << _IOC_TYPESHIFT)
                | (nr << _IOC_NRSHIFT)
                | ((size as u32) << _IOC_SIZESHIFT)) as Ioctl
        }

        /// Build an ioctl number for an argumentless ioctl.
        pub const fn _IO(ty: u32, nr: u32) -> Ioctl {
            _IOC(_IOC_NONE, ty, nr, 0)
        }

        /// Build an ioctl number for an read-only ioctl.
        pub const fn _IOR<T>(ty: u32, nr: u32) -> Ioctl {
            _IOC(_IOC_READ, ty, nr, size_of::<T>())
        }

        /// Build an ioctl number for an write-only ioctl.
        pub const fn _IOW<T>(ty: u32, nr: u32) -> Ioctl {
            _IOC(_IOC_WRITE, ty, nr, size_of::<T>())
        }

        /// Build an ioctl number for a read-write ioctl.
        pub const fn _IOWR<T>(ty: u32, nr: u32) -> Ioctl {
            _IOC(_IOC_READ | _IOC_WRITE, ty, nr, size_of::<T>())
        }

        extern "C" {
            #[cfg_attr(gnu_time_bits64, link_name = "__ioctl_time64")]
            pub fn ioctl(fd: c_int, request: Ioctl, ...) -> c_int;
        }
    }
}

const_fn! {
    {const} fn CMSG_ALIGN(len: usize) -> usize {
        (len + size_of::<usize>() - 1) & !(size_of::<usize>() - 1)
    }
}

f! {
    pub fn CMSG_FIRSTHDR(mhdr: *const crate::msghdr) -> *mut crate::cmsghdr {
        if (*mhdr).msg_controllen as usize >= size_of::<crate::cmsghdr>() {
            (*mhdr).msg_control.cast::<crate::cmsghdr>()
        } else {
            core::ptr::null_mut::<crate::cmsghdr>()
        }
    }

    pub fn CMSG_DATA(cmsg: *const crate::cmsghdr) -> *mut c_uchar {
        cmsg.offset(1) as *mut c_uchar
    }

    pub {const} fn CMSG_SPACE(length: c_uint) -> c_uint {
        (CMSG_ALIGN(length as usize) + CMSG_ALIGN(size_of::<crate::cmsghdr>())) as c_uint
    }

    pub {const} fn CMSG_LEN(length: c_uint) -> c_uint {
        CMSG_ALIGN(size_of::<crate::cmsghdr>()) as c_uint + length
    }

    pub fn FD_CLR(fd: c_int, set: *mut fd_set) -> () {
        let fd = fd as usize;
        let size = size_of_val(&(*set).fds_bits[0]) * 8;
        (*set).fds_bits[fd / size] &= !(1 << (fd % size));
        return;
    }

    pub fn FD_ISSET(fd: c_int, set: *const fd_set) -> bool {
        let fd = fd as usize;
        let size = size_of_val(&(*set).fds_bits[0]) * 8;
        return ((*set).fds_bits[fd / size] & (1 << (fd % size))) != 0;
    }

    pub fn FD_SET(fd: c_int, set: *mut fd_set) -> () {
        let fd = fd as usize;
        let size = size_of_val(&(*set).fds_bits[0]) * 8;
        (*set).fds_bits[fd / size] |= 1 << (fd % size);
        return;
    }

    pub fn FD_ZERO(set: *mut fd_set) -> () {
        for slot in &mut (*set).fds_bits {
            *slot = 0;
        }
    }
}

safe_f! {
    pub fn SIGRTMAX() -> c_int {
        unsafe { __libc_current_sigrtmax() }
    }

    pub fn SIGRTMIN() -> c_int {
        unsafe { __libc_current_sigrtmin() }
    }

    pub {const} fn WIFSTOPPED(status: c_int) -> bool {
        (status & 0xff) == 0x7f
    }

    pub {const} fn WSTOPSIG(status: c_int) -> c_int {
        (status >> 8) & 0xff
    }

    pub {const} fn WIFCONTINUED(status: c_int) -> bool {
        status == 0xffff
    }

    pub {const} fn WIFSIGNALED(status: c_int) -> bool {
        ((status & 0x7f) + 1) as i8 >= 2
    }

    pub {const} fn WTERMSIG(status: c_int) -> c_int {
        status & 0x7f
    }

    pub {const} fn WIFEXITED(status: c_int) -> bool {
        (status & 0x7f) == 0
    }

    pub {const} fn WEXITSTATUS(status: c_int) -> c_int {
        (status >> 8) & 0xff
    }

    pub {const} fn WCOREDUMP(status: c_int) -> bool {
        (status & 0x80) != 0
    }

    pub {const} fn W_EXITCODE(ret: c_int, sig: c_int) -> c_int {
        (ret << 8) | sig
    }

    pub {const} fn W_STOPCODE(sig: c_int) -> c_int {
        (sig << 8) | 0x7f
    }

    pub {const} fn QCMD(cmd: c_int, type_: c_int) -> c_int {
        (cmd << 8) | (type_ & 0x00ff)
    }

    pub {const} fn IPOPT_COPIED(o: u8) -> u8 {
        o & IPOPT_COPY
    }

    pub {const} fn IPOPT_CLASS(o: u8) -> u8 {
        o & IPOPT_CLASS_MASK
    }

    pub {const} fn IPOPT_NUMBER(o: u8) -> u8 {
        o & IPOPT_NUMBER_MASK
    }

    pub {const} fn IPTOS_ECN(x: u8) -> u8 {
        x & crate::IPTOS_ECN_MASK
    }

    #[allow(ellipsis_inclusive_range_patterns)]
    pub {const} fn KERNEL_VERSION(a: u32, b: u32, c: u32) -> u32 {
        ((a << 16) + (b << 8)) + if c > 255 { 255 } else { c }
    }
}

extern "C" {
    #[doc(hidden)]
    pub fn __libc_current_sigrtmax() -> c_int;
    #[doc(hidden)]
    pub fn __libc_current_sigrtmin() -> c_int;

    pub fn sem_destroy(sem: *mut sem_t) -> c_int;
    pub fn sem_init(sem: *mut sem_t, pshared: c_int, value: c_uint) -> c_int;
    pub fn fdatasync(fd: c_int) -> c_int;
    pub fn mincore(addr: *mut c_void, len: size_t, vec: *mut c_uchar) -> c_int;

    #[cfg_attr(gnu_time_bits64, link_name = "__clock_getres64")]
    pub fn clock_getres(clk_id: crate::clockid_t, tp: *mut crate::timespec) -> c_int;
    #[cfg_attr(gnu_time_bits64, link_name = "__clock_gettime64")]
    pub fn clock_gettime(clk_id: crate::clockid_t, tp: *mut crate::timespec) -> c_int;
    #[cfg_attr(gnu_time_bits64, link_name = "__clock_settime64")]
    pub fn clock_settime(clk_id: crate::clockid_t, tp: *const crate::timespec) -> c_int;
    pub fn clock_getcpuclockid(pid: crate::pid_t, clk_id: *mut crate::clockid_t) -> c_int;

    pub fn dirfd(dirp: *mut crate::DIR) -> c_int;

    pub fn pthread_getattr_np(native: crate::pthread_t, attr: *mut crate::pthread_attr_t) -> c_int;
    pub fn pthread_attr_getstack(
        attr: *const crate::pthread_attr_t,
        stackaddr: *mut *mut c_void,
        stacksize: *mut size_t,
    ) -> c_int;
    pub fn pthread_attr_setstack(
        attr: *mut crate::pthread_attr_t,
        stackaddr: *mut c_void,
        stacksize: size_t,
    ) -> c_int;
    pub fn memalign(align: size_t, size: size_t) -> *mut c_void;
    pub fn setgroups(ngroups: size_t, ptr: *const crate::gid_t) -> c_int;
    pub fn pipe2(fds: *mut c_int, flags: c_int) -> c_int;
    #[cfg_attr(gnu_file_offset_bits64, link_name = "statfs64")]
    pub fn statfs(path: *const c_char, buf: *mut statfs) -> c_int;
    #[cfg_attr(gnu_file_offset_bits64, link_name = "fstatfs64")]
    pub fn fstatfs(fd: c_int, buf: *mut statfs) -> c_int;
    pub fn memrchr(cx: *const c_void, c: c_int, n: size_t) -> *mut c_void;
    #[cfg_attr(gnu_file_offset_bits64, link_name = "posix_fadvise64")]
    pub fn posix_fadvise(fd: c_int, offset: off_t, len: off_t, advise: c_int) -> c_int;
    #[cfg_attr(gnu_time_bits64, link_name = "__futimens64")]
    pub fn futimens(fd: c_int, times: *const crate::timespec) -> c_int;
    #[cfg_attr(gnu_time_bits64, link_name = "__utimensat64")]
    pub fn utimensat(
        dirfd: c_int,
        path: *const c_char,
        times: *const crate::timespec,
        flag: c_int,
    ) -> c_int;
    pub fn duplocale(base: crate::locale_t) -> crate::locale_t;
    pub fn freelocale(loc: crate::locale_t);
    pub fn newlocale(mask: c_int, locale: *const c_char, base: crate::locale_t) -> crate::locale_t;
    pub fn uselocale(loc: crate::locale_t) -> crate::locale_t;
    pub fn mknodat(dirfd: c_int, pathname: *const c_char, mode: mode_t, dev: dev_t) -> c_int;
    pub fn pthread_condattr_getclock(
        attr: *const pthread_condattr_t,
        clock_id: *mut clockid_t,
    ) -> c_int;
    pub fn pthread_condattr_setclock(
        attr: *mut pthread_condattr_t,
        clock_id: crate::clockid_t,
    ) -> c_int;
    pub fn pthread_condattr_setpshared(attr: *mut pthread_condattr_t, pshared: c_int) -> c_int;
    pub fn pthread_mutexattr_setpshared(attr: *mut pthread_mutexattr_t, pshared: c_int) -> c_int;
    pub fn pthread_rwlockattr_getpshared(
        attr: *const pthread_rwlockattr_t,
        val: *mut c_int,
    ) -> c_int;
    pub fn pthread_rwlockattr_setpshared(attr: *mut pthread_rwlockattr_t, val: c_int) -> c_int;
    pub fn ptsname_r(fd: c_int, buf: *mut c_char, buflen: size_t) -> c_int;
    pub fn clearenv() -> c_int;
    pub fn waitid(
        idtype: idtype_t,
        id: id_t,
        infop: *mut crate::siginfo_t,
        options: c_int,
    ) -> c_int;
    pub fn getresuid(
        ruid: *mut crate::uid_t,
        euid: *mut crate::uid_t,
        suid: *mut crate::uid_t,
    ) -> c_int;
    pub fn getresgid(
        rgid: *mut crate::gid_t,
        egid: *mut crate::gid_t,
        sgid: *mut crate::gid_t,
    ) -> c_int;
    pub fn acct(filename: *const c_char) -> c_int;
    pub fn brk(addr: *mut c_void) -> c_int;
    pub fn sbrk(increment: intptr_t) -> *mut c_void;
    #[deprecated(
        since = "0.2.66",
        note = "causes memory corruption, see rust-lang/libc#1596"
    )]
    pub fn vfork() -> crate::pid_t;
    pub fn setresgid(rgid: crate::gid_t, egid: crate::gid_t, sgid: crate::gid_t) -> c_int;
    pub fn setresuid(ruid: crate::uid_t, euid: crate::uid_t, suid: crate::uid_t) -> c_int;
    #[cfg_attr(gnu_time_bits64, link_name = "__wait4_time64")]
    pub fn wait4(
        pid: crate::pid_t,
        status: *mut c_int,
        options: c_int,
        rusage: *mut crate::rusage,
    ) -> crate::pid_t;
    pub fn login_tty(fd: c_int) -> c_int;

    // DIFF(main): changed to `*const *mut` in e77f551de9
    pub fn execvpe(
        file: *const c_char,
        argv: *const *const c_char,
        envp: *const *const c_char,
    ) -> c_int;
    pub fn fexecve(fd: c_int, argv: *const *const c_char, envp: *const *const c_char) -> c_int;

    pub fn getifaddrs(ifap: *mut *mut crate::ifaddrs) -> c_int;
    pub fn freeifaddrs(ifa: *mut crate::ifaddrs);
    pub fn bind(
        socket: c_int,
        address: *const crate::sockaddr,
        address_len: crate::socklen_t,
    ) -> c_int;

    pub fn writev(fd: c_int, iov: *const crate::iovec, iovcnt: c_int) -> ssize_t;
    pub fn readv(fd: c_int, iov: *const crate::iovec, iovcnt: c_int) -> ssize_t;

    #[cfg_attr(gnu_time_bits64, link_name = "__sendmsg64")]
    pub fn sendmsg(fd: c_int, msg: *const crate::msghdr, flags: c_int) -> ssize_t;
    #[cfg_attr(gnu_time_bits64, link_name = "__recvmsg64")]
    pub fn recvmsg(fd: c_int, msg: *mut crate::msghdr, flags: c_int) -> ssize_t;
    pub fn uname(buf: *mut crate::utsname) -> c_int;

    pub fn strchrnul(s: *const c_char, c: c_int) -> *mut c_char;

    pub fn strftime(
        s: *mut c_char,
        max: size_t,
        format: *const c_char,
        tm: *const crate::tm,
    ) -> size_t;
    pub fn strftime_l(
        s: *mut c_char,
        max: size_t,
        format: *const c_char,
        tm: *const crate::tm,
        locale: crate::locale_t,
    ) -> size_t;
    pub fn strptime(s: *const c_char, format: *const c_char, tm: *mut crate::tm) -> *mut c_char;

    #[cfg_attr(gnu_file_offset_bits64, link_name = "mkostemp64")]
    pub fn mkostemp(template: *mut c_char, flags: c_int) -> c_int;
    #[cfg_attr(gnu_file_offset_bits64, link_name = "mkostemps64")]
    pub fn mkostemps(template: *mut c_char, suffixlen: c_int, flags: c_int) -> c_int;

    pub fn getdomainname(name: *mut c_char, len: size_t) -> c_int;
    pub fn setdomainname(name: *const c_char, len: size_t) -> c_int;
}

// LFS64 extensions
//
// * musl and Emscripten has 64-bit versions only so aliases the LFS64 symbols to the standard ones
// * ulibc doesn't have preadv64/pwritev64
cfg_if! {
    if #[cfg(not(any(target_env = "musl", target_os = "emscripten")))] {
        extern "C" {
            pub fn fstatfs64(fd: c_int, buf: *mut statfs64) -> c_int;
            pub fn statvfs64(path: *const c_char, buf: *mut statvfs64) -> c_int;
            pub fn fstatvfs64(fd: c_int, buf: *mut statvfs64) -> c_int;
            pub fn statfs64(path: *const c_char, buf: *mut statfs64) -> c_int;
            pub fn creat64(path: *const c_char, mode: mode_t) -> c_int;
            #[cfg_attr(gnu_time_bits64, link_name = "__fstat64_time64")]
            pub fn fstat64(fildes: c_int, buf: *mut stat64) -> c_int;
            #[cfg_attr(gnu_time_bits64, link_name = "__fstatat64_time64")]
            pub fn fstatat64(
                dirfd: c_int,
                pathname: *const c_char,
                buf: *mut stat64,
                flags: c_int,
            ) -> c_int;
            pub fn ftruncate64(fd: c_int, length: off64_t) -> c_int;
            pub fn lseek64(fd: c_int, offset: off64_t, whence: c_int) -> off64_t;
            #[cfg_attr(gnu_time_bits64, link_name = "__lstat64_time64")]
            pub fn lstat64(path: *const c_char, buf: *mut stat64) -> c_int;
            pub fn mmap64(
                addr: *mut c_void,
                len: size_t,
                prot: c_int,
                flags: c_int,
                fd: c_int,
                offset: off64_t,
            ) -> *mut c_void;
            pub fn open64(path: *const c_char, oflag: c_int, ...) -> c_int;
            pub fn openat64(fd: c_int, path: *const c_char, oflag: c_int, ...) -> c_int;
            pub fn posix_fadvise64(
                fd: c_int,
                offset: off64_t,
                len: off64_t,
                advise: c_int,
            ) -> c_int;
            pub fn pread64(fd: c_int, buf: *mut c_void, count: size_t, offset: off64_t) -> ssize_t;
            pub fn pwrite64(
                fd: c_int,
                buf: *const c_void,
                count: size_t,
                offset: off64_t,
            ) -> ssize_t;
            pub fn readdir64(dirp: *mut crate::DIR) -> *mut crate::dirent64;
            pub fn readdir64_r(
                dirp: *mut crate::DIR,
                entry: *mut crate::dirent64,
                result: *mut *mut crate::dirent64,
            ) -> c_int;
            #[cfg_attr(gnu_time_bits64, link_name = "__stat64_time64")]
            pub fn stat64(path: *const c_char, buf: *mut stat64) -> c_int;
            pub fn truncate64(path: *const c_char, length: off64_t) -> c_int;
        }
    }
}

cfg_if! {
    if #[cfg(not(any(
        target_env = "uclibc",
        target_env = "musl",
        target_os = "emscripten"
    )))] {
        extern "C" {
            pub fn preadv64(
                fd: c_int,
                iov: *const crate::iovec,
                iovcnt: c_int,
                offset: off64_t,
            ) -> ssize_t;
            pub fn pwritev64(
                fd: c_int,
                iov: *const crate::iovec,
                iovcnt: c_int,
                offset: off64_t,
            ) -> ssize_t;
        }
    }
}

cfg_if! {
    if #[cfg(not(target_env = "uclibc"))] {
        extern "C" {
            // uclibc has separate non-const version of this function
            pub fn forkpty(
                amaster: *mut c_int,
                name: *mut c_char,
                termp: *const termios,
                winp: *const crate::winsize,
            ) -> crate::pid_t;
            // uclibc has separate non-const version of this function
            pub fn openpty(
                amaster: *mut c_int,
                aslave: *mut c_int,
                name: *mut c_char,
                termp: *const termios,
                winp: *const crate::winsize,
            ) -> c_int;
        }
    }
}

// The statx syscall, available on some libcs.
cfg_if! {
    if #[cfg(any(
        target_env = "gnu",
        target_os = "android",
        all(target_env = "musl", musl_v1_2_3)
    ))] {
        extern "C" {
            pub fn statx(
                dirfd: c_int,
                pathname: *const c_char,
                flags: c_int,
                mask: c_uint,
                statxbuf: *mut statx,
            ) -> c_int;
        }
    }
}

cfg_if! {
    if #[cfg(target_os = "emscripten")] {
        mod emscripten;
        pub use self::emscripten::*;
    } else if #[cfg(target_os = "linux")] {
        mod linux;
        pub use self::linux::*;
    } else if #[cfg(target_os = "l4re")] {
        mod linux;
        pub use self::linux::*;
    } else if #[cfg(target_os = "android")] {
        mod android;
        pub use self::android::*;
    } else {
        // Unknown target_os
    }
}
