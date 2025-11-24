use crate::prelude::*;

pub type off_t = i64;
pub type useconds_t = u32;
pub type blkcnt_t = i64;
pub type socklen_t = u32;
pub type sa_family_t = u8;
pub type pthread_t = crate::uintptr_t;
pub type nfds_t = c_uint;
pub type regoff_t = off_t;

s! {
    pub struct sockaddr {
        pub sa_len: u8,
        pub sa_family: sa_family_t,
        pub sa_data: [c_char; 14],
    }

    pub struct sockaddr_in6 {
        pub sin6_len: u8,
        pub sin6_family: sa_family_t,
        pub sin6_port: crate::in_port_t,
        pub sin6_flowinfo: u32,
        pub sin6_addr: crate::in6_addr,
        pub sin6_scope_id: u32,
    }

    pub struct passwd {
        pub pw_name: *mut c_char,
        pub pw_passwd: *mut c_char,
        pub pw_uid: crate::uid_t,
        pub pw_gid: crate::gid_t,
        pub pw_change: crate::time_t,
        pub pw_class: *mut c_char,
        pub pw_gecos: *mut c_char,
        pub pw_dir: *mut c_char,
        pub pw_shell: *mut c_char,
        pub pw_expire: crate::time_t,

        #[cfg(not(any(
            target_os = "macos",
            target_os = "ios",
            target_os = "tvos",
            target_os = "watchos",
            target_os = "visionos",
            target_os = "netbsd",
            target_os = "openbsd"
        )))]
        pub pw_fields: c_int,
    }

    pub struct ifaddrs {
        pub ifa_next: *mut ifaddrs,
        pub ifa_name: *mut c_char,
        pub ifa_flags: c_uint,
        pub ifa_addr: *mut crate::sockaddr,
        pub ifa_netmask: *mut crate::sockaddr,
        pub ifa_dstaddr: *mut crate::sockaddr,
        pub ifa_data: *mut c_void,
        #[cfg(target_os = "netbsd")]
        pub ifa_addrflags: c_uint,
    }

    pub struct fd_set {
        #[cfg(all(
            target_pointer_width = "64",
            any(target_os = "freebsd", target_os = "dragonfly")
        ))]
        fds_bits: [i64; FD_SETSIZE as usize / 64],
        #[cfg(not(all(
            target_pointer_width = "64",
            any(target_os = "freebsd", target_os = "dragonfly")
        )))]
        fds_bits: [i32; FD_SETSIZE as usize / 32],
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
        pub tm_zone: *mut c_char,
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

    pub struct cmsghdr {
        pub cmsg_len: crate::socklen_t,
        pub cmsg_level: c_int,
        pub cmsg_type: c_int,
    }

    pub struct fsid_t {
        __fsid_val: [i32; 2],
    }

    pub struct if_nameindex {
        pub if_index: c_uint,
        pub if_name: *mut c_char,
    }

    pub struct regex_t {
        __re_magic: c_int,
        __re_nsub: size_t,
        __re_endp: *const c_char,
        __re_g: *mut c_void,
    }

    pub struct regmatch_t {
        pub rm_so: regoff_t,
        pub rm_eo: regoff_t,
    }

    pub struct option {
        pub name: *const c_char,
        pub has_arg: c_int,
        pub flag: *mut c_int,
        pub val: c_int,
    }
}

s_no_extra_traits! {
    pub struct sockaddr_un {
        pub sun_len: u8,
        pub sun_family: sa_family_t,
        pub sun_path: [c_char; 104],
    }

    pub struct utsname {
        #[cfg(not(target_os = "dragonfly"))]
        pub sysname: [c_char; 256],
        #[cfg(target_os = "dragonfly")]
        pub sysname: [c_char; 32],
        #[cfg(not(target_os = "dragonfly"))]
        pub nodename: [c_char; 256],
        #[cfg(target_os = "dragonfly")]
        pub nodename: [c_char; 32],
        #[cfg(not(target_os = "dragonfly"))]
        pub release: [c_char; 256],
        #[cfg(target_os = "dragonfly")]
        pub release: [c_char; 32],
        #[cfg(not(target_os = "dragonfly"))]
        pub version: [c_char; 256],
        #[cfg(target_os = "dragonfly")]
        pub version: [c_char; 32],
        #[cfg(not(target_os = "dragonfly"))]
        pub machine: [c_char; 256],
        #[cfg(target_os = "dragonfly")]
        pub machine: [c_char; 32],
    }
}

cfg_if! {
    if #[cfg(feature = "extra_traits")] {
        impl PartialEq for sockaddr_un {
            fn eq(&self, other: &sockaddr_un) -> bool {
                self.sun_len == other.sun_len
                    && self.sun_family == other.sun_family
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
                self.sun_len.hash(state);
                self.sun_family.hash(state);
                self.sun_path.hash(state);
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
            }
        }
    }
}

pub const LC_ALL: c_int = 0;
pub const LC_COLLATE: c_int = 1;
pub const LC_CTYPE: c_int = 2;
pub const LC_MONETARY: c_int = 3;
pub const LC_NUMERIC: c_int = 4;
pub const LC_TIME: c_int = 5;
pub const LC_MESSAGES: c_int = 6;

pub const FIOCLEX: c_ulong = 0x20006601;
pub const FIONCLEX: c_ulong = 0x20006602;
pub const FIONREAD: c_ulong = 0x4004667f;
pub const FIONBIO: c_ulong = 0x8004667e;
pub const FIOASYNC: c_ulong = 0x8004667d;
pub const FIOSETOWN: c_ulong = 0x8004667c;
pub const FIOGETOWN: c_ulong = 0x4004667b;

pub const PATH_MAX: c_int = 1024;
pub const MAXPATHLEN: c_int = PATH_MAX;

pub const IOV_MAX: c_int = 1024;

pub const SA_ONSTACK: c_int = 0x0001;
pub const SA_SIGINFO: c_int = 0x0040;
pub const SA_RESTART: c_int = 0x0002;
pub const SA_RESETHAND: c_int = 0x0004;
pub const SA_NOCLDSTOP: c_int = 0x0008;
pub const SA_NODEFER: c_int = 0x0010;
pub const SA_NOCLDWAIT: c_int = 0x0020;

pub const SS_ONSTACK: c_int = 1;
pub const SS_DISABLE: c_int = 4;

pub const SIGCHLD: c_int = 20;
pub const SIGBUS: c_int = 10;
pub const SIGUSR1: c_int = 30;
pub const SIGUSR2: c_int = 31;
pub const SIGCONT: c_int = 19;
pub const SIGSTOP: c_int = 17;
pub const SIGTSTP: c_int = 18;
pub const SIGURG: c_int = 16;
pub const SIGIO: c_int = 23;
pub const SIGSYS: c_int = 12;
pub const SIGTTIN: c_int = 21;
pub const SIGTTOU: c_int = 22;
pub const SIGXCPU: c_int = 24;
pub const SIGXFSZ: c_int = 25;
pub const SIGVTALRM: c_int = 26;
pub const SIGPROF: c_int = 27;
pub const SIGWINCH: c_int = 28;
pub const SIGINFO: c_int = 29;

pub const SIG_SETMASK: c_int = 3;
pub const SIG_BLOCK: c_int = 0x1;
pub const SIG_UNBLOCK: c_int = 0x2;

pub const IP_TOS: c_int = 3;
pub const IP_MULTICAST_IF: c_int = 9;
pub const IP_MULTICAST_TTL: c_int = 10;
pub const IP_MULTICAST_LOOP: c_int = 11;

pub const IPV6_UNICAST_HOPS: c_int = 4;
pub const IPV6_MULTICAST_IF: c_int = 9;
pub const IPV6_MULTICAST_HOPS: c_int = 10;
pub const IPV6_MULTICAST_LOOP: c_int = 11;
pub const IPV6_V6ONLY: c_int = 27;
pub const IPV6_DONTFRAG: c_int = 62;

pub const IPTOS_ECN_NOTECT: u8 = 0x00;
pub const IPTOS_ECN_MASK: u8 = 0x03;
pub const IPTOS_ECN_ECT1: u8 = 0x01;
pub const IPTOS_ECN_ECT0: u8 = 0x02;
pub const IPTOS_ECN_CE: u8 = 0x03;

pub const ST_RDONLY: c_ulong = 1;

pub const SCM_RIGHTS: c_int = 0x01;

pub const NCCS: usize = 20;

pub const O_ACCMODE: c_int = 0x3;
pub const O_RDONLY: c_int = 0;
pub const O_WRONLY: c_int = 1;
pub const O_RDWR: c_int = 2;
pub const O_APPEND: c_int = 8;
pub const O_CREAT: c_int = 512;
pub const O_TRUNC: c_int = 1024;
pub const O_EXCL: c_int = 2048;
pub const O_ASYNC: c_int = 0x40;
pub const O_SYNC: c_int = 0x80;
pub const O_NONBLOCK: c_int = 0x4;
pub const O_NOFOLLOW: c_int = 0x100;
pub const O_SHLOCK: c_int = 0x10;
pub const O_EXLOCK: c_int = 0x20;
pub const O_FSYNC: c_int = O_SYNC;
pub const O_NDELAY: c_int = O_NONBLOCK;

pub const F_GETOWN: c_int = 5;
pub const F_SETOWN: c_int = 6;

pub const F_RDLCK: c_short = 1;
pub const F_UNLCK: c_short = 2;
pub const F_WRLCK: c_short = 3;

pub const MNT_RDONLY: c_int = 0x00000001;
pub const MNT_SYNCHRONOUS: c_int = 0x00000002;
pub const MNT_NOEXEC: c_int = 0x00000004;
pub const MNT_NOSUID: c_int = 0x00000008;
pub const MNT_ASYNC: c_int = 0x00000040;
pub const MNT_EXPORTED: c_int = 0x00000100;
pub const MNT_UPDATE: c_int = 0x00010000;
pub const MNT_RELOAD: c_int = 0x00040000;
pub const MNT_FORCE: c_int = 0x00080000;

pub const Q_SYNC: c_int = 0x600;
pub const Q_QUOTAON: c_int = 0x100;
pub const Q_QUOTAOFF: c_int = 0x200;

pub const TCIOFF: c_int = 3;
pub const TCION: c_int = 4;
pub const TCOOFF: c_int = 1;
pub const TCOON: c_int = 2;
pub const TCIFLUSH: c_int = 1;
pub const TCOFLUSH: c_int = 2;
pub const TCIOFLUSH: c_int = 3;
pub const TCSANOW: c_int = 0;
pub const TCSADRAIN: c_int = 1;
pub const TCSAFLUSH: c_int = 2;
pub const VEOF: usize = 0;
pub const VEOL: usize = 1;
pub const VEOL2: usize = 2;
pub const VERASE: usize = 3;
pub const VWERASE: usize = 4;
pub const VKILL: usize = 5;
pub const VREPRINT: usize = 6;
pub const VINTR: usize = 8;
pub const VQUIT: usize = 9;
pub const VSUSP: usize = 10;
pub const VDSUSP: usize = 11;
pub const VSTART: usize = 12;
pub const VSTOP: usize = 13;
pub const VLNEXT: usize = 14;
pub const VDISCARD: usize = 15;
pub const VMIN: usize = 16;
pub const VTIME: usize = 17;
pub const VSTATUS: usize = 18;
pub const _POSIX_VDISABLE: crate::cc_t = 0xff;
pub const IGNBRK: crate::tcflag_t = 0x00000001;
pub const BRKINT: crate::tcflag_t = 0x00000002;
pub const IGNPAR: crate::tcflag_t = 0x00000004;
pub const PARMRK: crate::tcflag_t = 0x00000008;
pub const INPCK: crate::tcflag_t = 0x00000010;
pub const ISTRIP: crate::tcflag_t = 0x00000020;
pub const INLCR: crate::tcflag_t = 0x00000040;
pub const IGNCR: crate::tcflag_t = 0x00000080;
pub const ICRNL: crate::tcflag_t = 0x00000100;
pub const IXON: crate::tcflag_t = 0x00000200;
pub const IXOFF: crate::tcflag_t = 0x00000400;
pub const IXANY: crate::tcflag_t = 0x00000800;
pub const IMAXBEL: crate::tcflag_t = 0x00002000;
pub const OPOST: crate::tcflag_t = 0x1;
pub const ONLCR: crate::tcflag_t = 0x2;
pub const OXTABS: crate::tcflag_t = 0x4;
pub const ONOEOT: crate::tcflag_t = 0x8;
pub const CIGNORE: crate::tcflag_t = 0x00000001;
pub const CSIZE: crate::tcflag_t = 0x00000300;
pub const CS5: crate::tcflag_t = 0x00000000;
pub const CS6: crate::tcflag_t = 0x00000100;
pub const CS7: crate::tcflag_t = 0x00000200;
pub const CS8: crate::tcflag_t = 0x00000300;
pub const CSTOPB: crate::tcflag_t = 0x00000400;
pub const CREAD: crate::tcflag_t = 0x00000800;
pub const PARENB: crate::tcflag_t = 0x00001000;
pub const PARODD: crate::tcflag_t = 0x00002000;
pub const HUPCL: crate::tcflag_t = 0x00004000;
pub const CLOCAL: crate::tcflag_t = 0x00008000;
pub const ECHOKE: crate::tcflag_t = 0x00000001;
pub const ECHOE: crate::tcflag_t = 0x00000002;
pub const ECHOK: crate::tcflag_t = 0x00000004;
pub const ECHO: crate::tcflag_t = 0x00000008;
pub const ECHONL: crate::tcflag_t = 0x00000010;
pub const ECHOPRT: crate::tcflag_t = 0x00000020;
pub const ECHOCTL: crate::tcflag_t = 0x00000040;
pub const ISIG: crate::tcflag_t = 0x00000080;
pub const ICANON: crate::tcflag_t = 0x00000100;
pub const ALTWERASE: crate::tcflag_t = 0x00000200;
pub const IEXTEN: crate::tcflag_t = 0x00000400;
pub const EXTPROC: crate::tcflag_t = 0x00000800;
pub const TOSTOP: crate::tcflag_t = 0x00400000;
pub const FLUSHO: crate::tcflag_t = 0x00800000;
pub const NOKERNINFO: crate::tcflag_t = 0x02000000;
pub const PENDIN: crate::tcflag_t = 0x20000000;
pub const NOFLSH: crate::tcflag_t = 0x80000000;
pub const MDMBUF: crate::tcflag_t = 0x00100000;

pub const WNOHANG: c_int = 0x00000001;
pub const WUNTRACED: c_int = 0x00000002;

pub const RTLD_LAZY: c_int = 0x1;
pub const RTLD_NOW: c_int = 0x2;
pub const RTLD_NEXT: *mut c_void = -1isize as *mut c_void;
pub const RTLD_DEFAULT: *mut c_void = -2isize as *mut c_void;
pub const RTLD_SELF: *mut c_void = -3isize as *mut c_void;

pub const LOG_CRON: c_int = 9 << 3;
pub const LOG_AUTHPRIV: c_int = 10 << 3;
pub const LOG_FTP: c_int = 11 << 3;
pub const LOG_PERROR: c_int = 0x20;

pub const TCP_NODELAY: c_int = 1;
pub const TCP_MAXSEG: c_int = 2;

pub const PIPE_BUF: usize = 512;

// si_code values for SIGBUS signal
pub const BUS_ADRALN: c_int = 1;
pub const BUS_ADRERR: c_int = 2;
pub const BUS_OBJERR: c_int = 3;

// si_code values for SIGCHLD signal
pub const CLD_EXITED: c_int = 1;
pub const CLD_KILLED: c_int = 2;
pub const CLD_DUMPED: c_int = 3;
pub const CLD_TRAPPED: c_int = 4;
pub const CLD_STOPPED: c_int = 5;
pub const CLD_CONTINUED: c_int = 6;

pub const POLLIN: c_short = 0x1;
pub const POLLPRI: c_short = 0x2;
pub const POLLOUT: c_short = 0x4;
pub const POLLERR: c_short = 0x8;
pub const POLLHUP: c_short = 0x10;
pub const POLLNVAL: c_short = 0x20;
pub const POLLRDNORM: c_short = 0x040;
pub const POLLWRNORM: c_short = 0x004;
pub const POLLRDBAND: c_short = 0x080;
pub const POLLWRBAND: c_short = 0x100;

pub const BIOCGBLEN: c_ulong = 0x40044266;
pub const BIOCSBLEN: c_ulong = 0xc0044266;
pub const BIOCFLUSH: c_uint = 0x20004268;
pub const BIOCPROMISC: c_uint = 0x20004269;
pub const BIOCGDLT: c_ulong = 0x4004426a;
pub const BIOCGETIF: c_ulong = 0x4020426b;
pub const BIOCSETIF: c_ulong = 0x8020426c;
pub const BIOCGSTATS: c_ulong = 0x4008426f;
pub const BIOCIMMEDIATE: c_ulong = 0x80044270;
pub const BIOCVERSION: c_ulong = 0x40044271;
pub const BIOCGHDRCMPLT: c_ulong = 0x40044274;
pub const BIOCSHDRCMPLT: c_ulong = 0x80044275;
pub const SIOCGIFADDR: c_ulong = 0xc0206921;

pub const REG_BASIC: c_int = 0o0000;
pub const REG_EXTENDED: c_int = 0o0001;
pub const REG_ICASE: c_int = 0o0002;
pub const REG_NOSUB: c_int = 0o0004;
pub const REG_NEWLINE: c_int = 0o0010;
pub const REG_NOSPEC: c_int = 0o0020;
pub const REG_PEND: c_int = 0o0040;
pub const REG_DUMP: c_int = 0o0200;

pub const REG_NOMATCH: c_int = 1;
pub const REG_BADPAT: c_int = 2;
pub const REG_ECOLLATE: c_int = 3;
pub const REG_ECTYPE: c_int = 4;
pub const REG_EESCAPE: c_int = 5;
pub const REG_ESUBREG: c_int = 6;
pub const REG_EBRACK: c_int = 7;
pub const REG_EPAREN: c_int = 8;
pub const REG_EBRACE: c_int = 9;
pub const REG_BADBR: c_int = 10;
pub const REG_ERANGE: c_int = 11;
pub const REG_ESPACE: c_int = 12;
pub const REG_BADRPT: c_int = 13;
pub const REG_EMPTY: c_int = 14;
pub const REG_ASSERT: c_int = 15;
pub const REG_INVARG: c_int = 16;
pub const REG_ATOI: c_int = 255;
pub const REG_ITOA: c_int = 0o0400;

pub const REG_NOTBOL: c_int = 0o00001;
pub const REG_NOTEOL: c_int = 0o00002;
pub const REG_STARTEND: c_int = 0o00004;
pub const REG_TRACE: c_int = 0o00400;
pub const REG_LARGE: c_int = 0o01000;
pub const REG_BACKR: c_int = 0o02000;

pub const TIOCCBRK: c_uint = 0x2000747a;
pub const TIOCSBRK: c_uint = 0x2000747b;

pub const PRIO_PROCESS: c_int = 0;
pub const PRIO_PGRP: c_int = 1;
pub const PRIO_USER: c_int = 2;

pub const ITIMER_REAL: c_int = 0;
pub const ITIMER_VIRTUAL: c_int = 1;
pub const ITIMER_PROF: c_int = 2;

// net/route.h

pub const RTF_UP: c_int = 0x1;
pub const RTF_GATEWAY: c_int = 0x2;
pub const RTF_HOST: c_int = 0x4;
pub const RTF_REJECT: c_int = 0x8;
pub const RTF_DYNAMIC: c_int = 0x10;
pub const RTF_MODIFIED: c_int = 0x20;
pub const RTF_DONE: c_int = 0x40;
pub const RTF_STATIC: c_int = 0x800;
pub const RTF_BLACKHOLE: c_int = 0x1000;
pub const RTF_PROTO2: c_int = 0x4000;
pub const RTF_PROTO1: c_int = 0x8000;

// Message types
pub const RTM_ADD: c_int = 0x1;
pub const RTM_DELETE: c_int = 0x2;
pub const RTM_CHANGE: c_int = 0x3;
pub const RTM_GET: c_int = 0x4;
pub const RTM_LOSING: c_int = 0x5;
pub const RTM_REDIRECT: c_int = 0x6;
pub const RTM_MISS: c_int = 0x7;

// Bitmask values for rtm_addrs.
pub const RTA_DST: c_int = 0x1;
pub const RTA_GATEWAY: c_int = 0x2;
pub const RTA_NETMASK: c_int = 0x4;
pub const RTA_GENMASK: c_int = 0x8;
pub const RTA_IFP: c_int = 0x10;
pub const RTA_IFA: c_int = 0x20;
pub const RTA_AUTHOR: c_int = 0x40;
pub const RTA_BRD: c_int = 0x80;

// Index offsets for sockaddr array for alternate internal encoding.
pub const RTAX_DST: c_int = 0;
pub const RTAX_GATEWAY: c_int = 1;
pub const RTAX_NETMASK: c_int = 2;
pub const RTAX_GENMASK: c_int = 3;
pub const RTAX_IFP: c_int = 4;
pub const RTAX_IFA: c_int = 5;
pub const RTAX_AUTHOR: c_int = 6;
pub const RTAX_BRD: c_int = 7;

f! {
    pub fn CMSG_FIRSTHDR(mhdr: *const crate::msghdr) -> *mut cmsghdr {
        if (*mhdr).msg_controllen as usize >= size_of::<cmsghdr>() {
            (*mhdr).msg_control.cast::<cmsghdr>()
        } else {
            core::ptr::null_mut()
        }
    }

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
        for slot in &mut (*set).fds_bits {
            *slot = 0;
        }
    }
}

safe_f! {
    pub {const} fn WTERMSIG(status: c_int) -> c_int {
        status & 0o177
    }

    pub {const} fn WIFEXITED(status: c_int) -> bool {
        (status & 0o177) == 0
    }

    pub {const} fn WEXITSTATUS(status: c_int) -> c_int {
        (status >> 8) & 0x00ff
    }

    pub {const} fn WCOREDUMP(status: c_int) -> bool {
        (status & 0o200) != 0
    }

    pub {const} fn QCMD(cmd: c_int, type_: c_int) -> c_int {
        (cmd << 8) | (type_ & 0x00ff)
    }
}

extern "C" {
    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "getrlimit$UNIX2003"
    )]
    pub fn getrlimit(resource: c_int, rlim: *mut crate::rlimit) -> c_int;
    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "setrlimit$UNIX2003"
    )]
    pub fn setrlimit(resource: c_int, rlim: *const crate::rlimit) -> c_int;

    pub fn strerror_r(errnum: c_int, buf: *mut c_char, buflen: size_t) -> c_int;
    pub fn abs(i: c_int) -> c_int;
    pub fn labs(i: c_long) -> c_long;
    #[cfg_attr(
        all(target_os = "freebsd", any(freebsd12, freebsd11, freebsd10)),
        link_name = "rand@FBSD_1.0"
    )]
    pub fn rand() -> c_int;
    #[cfg_attr(
        all(target_os = "freebsd", any(freebsd12, freebsd11, freebsd10)),
        link_name = "srand@FBSD_1.0"
    )]
    pub fn srand(seed: c_uint);

    pub fn getifaddrs(ifap: *mut *mut crate::ifaddrs) -> c_int;
    pub fn freeifaddrs(ifa: *mut crate::ifaddrs);
    pub fn setgroups(ngroups: c_int, ptr: *const crate::gid_t) -> c_int;
    pub fn setlogin(name: *const c_char) -> c_int;
    pub fn ioctl(fd: c_int, request: c_ulong, ...) -> c_int;
    pub fn kqueue() -> c_int;
    pub fn unmount(target: *const c_char, arg: c_int) -> c_int;
    pub fn syscall(num: c_int, ...) -> c_int;
    #[cfg_attr(target_os = "netbsd", link_name = "__getpwent50")]
    pub fn getpwent() -> *mut passwd;
    pub fn setpwent();
    pub fn endpwent();
    pub fn endgrent();
    pub fn getgrent() -> *mut crate::group;

    pub fn getprogname() -> *const c_char;
    pub fn setprogname(name: *const c_char);
    pub fn getloadavg(loadavg: *mut c_double, nelem: c_int) -> c_int;
    pub fn if_nameindex() -> *mut if_nameindex;
    pub fn if_freenameindex(ptr: *mut if_nameindex);

    pub fn getpeereid(socket: c_int, euid: *mut crate::uid_t, egid: *mut crate::gid_t) -> c_int;

    #[cfg_attr(
        all(target_os = "macos", not(target_arch = "aarch64")),
        link_name = "glob$INODE64"
    )]
    #[cfg_attr(target_os = "netbsd", link_name = "__glob30")]
    #[cfg_attr(
        all(target_os = "freebsd", any(freebsd11, freebsd10)),
        link_name = "glob@FBSD_1.0"
    )]
    pub fn glob(
        pattern: *const c_char,
        flags: c_int,
        errfunc: Option<extern "C" fn(epath: *const c_char, errno: c_int) -> c_int>,
        pglob: *mut crate::glob_t,
    ) -> c_int;
    #[cfg_attr(target_os = "netbsd", link_name = "__globfree30")]
    #[cfg_attr(
        all(target_os = "freebsd", any(freebsd11, freebsd10)),
        link_name = "globfree@FBSD_1.0"
    )]
    pub fn globfree(pglob: *mut crate::glob_t);

    pub fn posix_madvise(addr: *mut c_void, len: size_t, advice: c_int) -> c_int;

    pub fn shm_unlink(name: *const c_char) -> c_int;

    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86_64"),
        link_name = "seekdir$INODE64"
    )]
    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "seekdir$INODE64$UNIX2003"
    )]
    pub fn seekdir(dirp: *mut crate::DIR, loc: c_long);

    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86_64"),
        link_name = "telldir$INODE64"
    )]
    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "telldir$INODE64$UNIX2003"
    )]
    pub fn telldir(dirp: *mut crate::DIR) -> c_long;
    pub fn madvise(addr: *mut c_void, len: size_t, advice: c_int) -> c_int;

    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "msync$UNIX2003"
    )]
    #[cfg_attr(target_os = "netbsd", link_name = "__msync13")]
    pub fn msync(addr: *mut c_void, len: size_t, flags: c_int) -> c_int;

    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "recvfrom$UNIX2003"
    )]
    pub fn recvfrom(
        socket: c_int,
        buf: *mut c_void,
        len: size_t,
        flags: c_int,
        addr: *mut crate::sockaddr,
        addrlen: *mut crate::socklen_t,
    ) -> ssize_t;
    pub fn mkstemps(template: *mut c_char, suffixlen: c_int) -> c_int;
    #[cfg_attr(target_os = "netbsd", link_name = "__futimes50")]
    pub fn futimes(fd: c_int, times: *const crate::timeval) -> c_int;
    pub fn nl_langinfo(item: crate::nl_item) -> *mut c_char;

    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "bind$UNIX2003"
    )]
    pub fn bind(
        socket: c_int,
        address: *const crate::sockaddr,
        address_len: crate::socklen_t,
    ) -> c_int;

    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "writev$UNIX2003"
    )]
    pub fn writev(fd: c_int, iov: *const crate::iovec, iovcnt: c_int) -> ssize_t;
    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "readv$UNIX2003"
    )]
    pub fn readv(fd: c_int, iov: *const crate::iovec, iovcnt: c_int) -> ssize_t;

    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "sendmsg$UNIX2003"
    )]
    pub fn sendmsg(fd: c_int, msg: *const crate::msghdr, flags: c_int) -> ssize_t;
    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "recvmsg$UNIX2003"
    )]
    pub fn recvmsg(fd: c_int, msg: *mut crate::msghdr, flags: c_int) -> ssize_t;

    pub fn sync();
    pub fn getgrgid_r(
        gid: crate::gid_t,
        grp: *mut crate::group,
        buf: *mut c_char,
        buflen: size_t,
        result: *mut *mut crate::group,
    ) -> c_int;
    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "sigaltstack$UNIX2003"
    )]
    #[cfg_attr(target_os = "netbsd", link_name = "__sigaltstack14")]
    pub fn sigaltstack(ss: *const stack_t, oss: *mut stack_t) -> c_int;
    pub fn sigsuspend(mask: *const crate::sigset_t) -> c_int;
    pub fn sem_close(sem: *mut sem_t) -> c_int;
    pub fn getdtablesize() -> c_int;
    pub fn getgrnam_r(
        name: *const c_char,
        grp: *mut crate::group,
        buf: *mut c_char,
        buflen: size_t,
        result: *mut *mut crate::group,
    ) -> c_int;
    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "pthread_sigmask$UNIX2003"
    )]
    pub fn pthread_sigmask(how: c_int, set: *const sigset_t, oldset: *mut sigset_t) -> c_int;
    pub fn sem_open(name: *const c_char, oflag: c_int, ...) -> *mut sem_t;
    pub fn getgrnam(name: *const c_char) -> *mut crate::group;
    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "pthread_cancel$UNIX2003"
    )]
    pub fn pthread_cancel(thread: crate::pthread_t) -> c_int;
    pub fn pthread_kill(thread: crate::pthread_t, sig: c_int) -> c_int;
    pub fn sched_get_priority_min(policy: c_int) -> c_int;
    pub fn sched_get_priority_max(policy: c_int) -> c_int;
    pub fn sem_unlink(name: *const c_char) -> c_int;
    #[cfg_attr(target_os = "netbsd", link_name = "__getpwnam_r50")]
    pub fn getpwnam_r(
        name: *const c_char,
        pwd: *mut passwd,
        buf: *mut c_char,
        buflen: size_t,
        result: *mut *mut passwd,
    ) -> c_int;
    #[cfg_attr(target_os = "netbsd", link_name = "__getpwuid_r50")]
    pub fn getpwuid_r(
        uid: crate::uid_t,
        pwd: *mut passwd,
        buf: *mut c_char,
        buflen: size_t,
        result: *mut *mut passwd,
    ) -> c_int;
    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "sigwait$UNIX2003"
    )]
    pub fn sigwait(set: *const sigset_t, sig: *mut c_int) -> c_int;
    pub fn pthread_atfork(
        prepare: Option<unsafe extern "C" fn()>,
        parent: Option<unsafe extern "C" fn()>,
        child: Option<unsafe extern "C" fn()>,
    ) -> c_int;
    pub fn getgrgid(gid: crate::gid_t) -> *mut crate::group;
    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "popen$UNIX2003"
    )]
    pub fn popen(command: *const c_char, mode: *const c_char) -> *mut crate::FILE;
    pub fn faccessat(dirfd: c_int, pathname: *const c_char, mode: c_int, flags: c_int) -> c_int;
    pub fn pthread_create(
        native: *mut crate::pthread_t,
        attr: *const crate::pthread_attr_t,
        f: extern "C" fn(*mut c_void) -> *mut c_void,
        value: *mut c_void,
    ) -> c_int;
    pub fn acct(filename: *const c_char) -> c_int;
    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "wait4$UNIX2003"
    )]
    #[cfg_attr(
        all(target_os = "freebsd", any(freebsd12, freebsd11, freebsd10)),
        link_name = "wait4@FBSD_1.0"
    )]
    pub fn wait4(
        pid: crate::pid_t,
        status: *mut c_int,
        options: c_int,
        rusage: *mut crate::rusage,
    ) -> crate::pid_t;
    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "getitimer$UNIX2003"
    )]
    pub fn getitimer(which: c_int, curr_value: *mut crate::itimerval) -> c_int;
    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "setitimer$UNIX2003"
    )]
    pub fn setitimer(
        which: c_int,
        new_value: *const crate::itimerval,
        old_value: *mut crate::itimerval,
    ) -> c_int;

    pub fn regcomp(preg: *mut regex_t, pattern: *const c_char, cflags: c_int) -> c_int;

    pub fn regexec(
        preg: *const regex_t,
        input: *const c_char,
        nmatch: size_t,
        pmatch: *mut regmatch_t,
        eflags: c_int,
    ) -> c_int;

    pub fn regerror(
        errcode: c_int,
        preg: *const regex_t,
        errbuf: *mut c_char,
        errbuf_size: size_t,
    ) -> size_t;

    pub fn regfree(preg: *mut regex_t);

    pub fn arc4random() -> u32;
    pub fn arc4random_buf(buf: *mut c_void, size: size_t);
    pub fn arc4random_uniform(l: u32) -> u32;

    pub fn drand48() -> c_double;
    pub fn erand48(xseed: *mut c_ushort) -> c_double;
    pub fn lrand48() -> c_long;
    pub fn nrand48(xseed: *mut c_ushort) -> c_long;
    pub fn mrand48() -> c_long;
    pub fn jrand48(xseed: *mut c_ushort) -> c_long;
    pub fn srand48(seed: c_long);
    pub fn seed48(xseed: *mut c_ushort) -> *mut c_ushort;
    pub fn lcong48(p: *mut c_ushort);
    pub fn getopt_long(
        argc: c_int,
        argv: *const *mut c_char,
        optstring: *const c_char,
        longopts: *const option,
        longindex: *mut c_int,
    ) -> c_int;

    pub fn strftime(
        buf: *mut c_char,
        maxsize: size_t,
        format: *const c_char,
        timeptr: *const crate::tm,
    ) -> size_t;
    pub fn strftime_l(
        buf: *mut c_char,
        maxsize: size_t,
        format: *const c_char,
        timeptr: *const crate::tm,
        locale: crate::locale_t,
    ) -> size_t;

    pub fn devname(dev: crate::dev_t, mode_t: crate::mode_t) -> *mut c_char;
}

cfg_if! {
    if #[cfg(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "tvos",
        target_os = "watchos",
        target_os = "visionos"
    ))] {
        mod apple;
        pub use self::apple::*;
    } else if #[cfg(any(target_os = "openbsd", target_os = "netbsd"))] {
        mod netbsdlike;
        pub use self::netbsdlike::*;
    } else if #[cfg(any(target_os = "freebsd", target_os = "dragonfly"))] {
        mod freebsdlike;
        pub use self::freebsdlike::*;
    } else {
        // Unknown target_os
    }
}
