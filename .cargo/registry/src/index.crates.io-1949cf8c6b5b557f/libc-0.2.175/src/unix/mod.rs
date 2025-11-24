//! Definitions found commonly among almost all Unix derivatives
//!
//! More functions and definitions can be found in the more specific modules
//! according to the platform in question.

use crate::prelude::*;

pub type intmax_t = i64;
pub type uintmax_t = u64;

pub type size_t = usize;
pub type ptrdiff_t = isize;
pub type intptr_t = isize;
pub type uintptr_t = usize;
pub type ssize_t = isize;

pub type pid_t = i32;
pub type in_addr_t = u32;
pub type in_port_t = u16;
pub type sighandler_t = size_t;
pub type cc_t = c_uchar;

cfg_if! {
    if #[cfg(any(
        target_os = "espidf",
        target_os = "horizon",
        target_os = "vita"
    ))] {
        pub type uid_t = c_ushort;
        pub type gid_t = c_ushort;
    } else if #[cfg(target_os = "nto")] {
        pub type uid_t = i32;
        pub type gid_t = i32;
    } else {
        pub type uid_t = u32;
        pub type gid_t = u32;
    }
}

missing! {
    #[cfg_attr(feature = "extra_traits", derive(Debug))]
    pub enum DIR {}
}
pub type locale_t = *mut c_void;

s! {
    pub struct group {
        pub gr_name: *mut c_char,
        pub gr_passwd: *mut c_char,
        pub gr_gid: crate::gid_t,
        pub gr_mem: *mut *mut c_char,
    }

    pub struct utimbuf {
        pub actime: time_t,
        pub modtime: time_t,
    }

    pub struct timeval {
        pub tv_sec: time_t,
        #[cfg(not(gnu_time_bits64))]
        pub tv_usec: suseconds_t,
        // For 64 bit time on 32 bit linux glibc, suseconds_t is still
        // a 32 bit type.  Use __suseconds64_t instead
        #[cfg(gnu_time_bits64)]
        pub tv_usec: __suseconds64_t,
    }

    // linux x32 compatibility
    // See https://sourceware.org/bugzilla/show_bug.cgi?id=16437
    #[cfg(not(target_env = "gnu"))]
    pub struct timespec {
        pub tv_sec: time_t,
        #[cfg(all(target_arch = "x86_64", target_pointer_width = "32"))]
        pub tv_nsec: i64,
        #[cfg(not(all(target_arch = "x86_64", target_pointer_width = "32")))]
        pub tv_nsec: c_long,
    }

    pub struct rlimit {
        pub rlim_cur: rlim_t,
        pub rlim_max: rlim_t,
    }

    pub struct rusage {
        pub ru_utime: timeval,
        pub ru_stime: timeval,
        pub ru_maxrss: c_long,
        #[cfg(all(target_arch = "x86_64", target_pointer_width = "32"))]
        __pad1: u32,
        pub ru_ixrss: c_long,
        #[cfg(all(target_arch = "x86_64", target_pointer_width = "32"))]
        __pad2: u32,
        pub ru_idrss: c_long,
        #[cfg(all(target_arch = "x86_64", target_pointer_width = "32"))]
        __pad3: u32,
        pub ru_isrss: c_long,
        #[cfg(all(target_arch = "x86_64", target_pointer_width = "32"))]
        __pad4: u32,
        pub ru_minflt: c_long,
        #[cfg(all(target_arch = "x86_64", target_pointer_width = "32"))]
        __pad5: u32,
        pub ru_majflt: c_long,
        #[cfg(all(target_arch = "x86_64", target_pointer_width = "32"))]
        __pad6: u32,
        pub ru_nswap: c_long,
        #[cfg(all(target_arch = "x86_64", target_pointer_width = "32"))]
        __pad7: u32,
        pub ru_inblock: c_long,
        #[cfg(all(target_arch = "x86_64", target_pointer_width = "32"))]
        __pad8: u32,
        pub ru_oublock: c_long,
        #[cfg(all(target_arch = "x86_64", target_pointer_width = "32"))]
        __pad9: u32,
        pub ru_msgsnd: c_long,
        #[cfg(all(target_arch = "x86_64", target_pointer_width = "32"))]
        __pad10: u32,
        pub ru_msgrcv: c_long,
        #[cfg(all(target_arch = "x86_64", target_pointer_width = "32"))]
        __pad11: u32,
        pub ru_nsignals: c_long,
        #[cfg(all(target_arch = "x86_64", target_pointer_width = "32"))]
        __pad12: u32,
        pub ru_nvcsw: c_long,
        #[cfg(all(target_arch = "x86_64", target_pointer_width = "32"))]
        __pad13: u32,
        pub ru_nivcsw: c_long,
        #[cfg(all(target_arch = "x86_64", target_pointer_width = "32"))]
        __pad14: u32,

        #[cfg(any(target_env = "musl", target_env = "ohos", target_os = "emscripten"))]
        __reserved: [c_long; 16],
    }

    pub struct ipv6_mreq {
        pub ipv6mr_multiaddr: in6_addr,
        #[cfg(target_os = "android")]
        pub ipv6mr_interface: c_int,
        #[cfg(not(target_os = "android"))]
        pub ipv6mr_interface: c_uint,
    }

    #[cfg(not(target_os = "cygwin"))]
    pub struct hostent {
        pub h_name: *mut c_char,
        pub h_aliases: *mut *mut c_char,
        pub h_addrtype: c_int,
        pub h_length: c_int,
        pub h_addr_list: *mut *mut c_char,
    }

    pub struct iovec {
        pub iov_base: *mut c_void,
        pub iov_len: size_t,
    }

    pub struct pollfd {
        pub fd: c_int,
        pub events: c_short,
        pub revents: c_short,
    }

    pub struct winsize {
        pub ws_row: c_ushort,
        pub ws_col: c_ushort,
        pub ws_xpixel: c_ushort,
        pub ws_ypixel: c_ushort,
    }

    #[cfg(not(target_os = "cygwin"))]
    pub struct linger {
        pub l_onoff: c_int,
        pub l_linger: c_int,
    }

    pub struct sigval {
        // Actually a union of an int and a void*
        pub sival_ptr: *mut c_void,
    }

    // <sys/time.h>
    pub struct itimerval {
        pub it_interval: crate::timeval,
        pub it_value: crate::timeval,
    }

    // <sys/times.h>
    pub struct tms {
        pub tms_utime: crate::clock_t,
        pub tms_stime: crate::clock_t,
        pub tms_cutime: crate::clock_t,
        pub tms_cstime: crate::clock_t,
    }

    pub struct servent {
        pub s_name: *mut c_char,
        pub s_aliases: *mut *mut c_char,
        #[cfg(target_os = "cygwin")]
        pub s_port: c_short,
        #[cfg(not(target_os = "cygwin"))]
        pub s_port: c_int,
        pub s_proto: *mut c_char,
    }

    pub struct protoent {
        pub p_name: *mut c_char,
        pub p_aliases: *mut *mut c_char,
        #[cfg(not(target_os = "cygwin"))]
        pub p_proto: c_int,
        #[cfg(target_os = "cygwin")]
        pub p_proto: c_short,
    }

    #[repr(align(4))]
    pub struct in6_addr {
        pub s6_addr: [u8; 16],
    }
}

pub const INT_MIN: c_int = -2147483648;
pub const INT_MAX: c_int = 2147483647;

pub const SIG_DFL: sighandler_t = 0 as sighandler_t;
pub const SIG_IGN: sighandler_t = 1 as sighandler_t;
pub const SIG_ERR: sighandler_t = !0 as sighandler_t;

cfg_if! {
    if #[cfg(all(not(target_os = "nto"), not(target_os = "aix")))] {
        pub const DT_UNKNOWN: u8 = 0;
        pub const DT_FIFO: u8 = 1;
        pub const DT_CHR: u8 = 2;
        pub const DT_DIR: u8 = 4;
        pub const DT_BLK: u8 = 6;
        pub const DT_REG: u8 = 8;
        pub const DT_LNK: u8 = 10;
        pub const DT_SOCK: u8 = 12;
    }
}
cfg_if! {
    if #[cfg(not(target_os = "redox"))] {
        pub const FD_CLOEXEC: c_int = 0x1;
    }
}

cfg_if! {
    if #[cfg(not(target_os = "nto"))] {
        pub const USRQUOTA: c_int = 0;
        pub const GRPQUOTA: c_int = 1;
    }
}
pub const SIGIOT: c_int = 6;

pub const S_ISUID: mode_t = 0o4000;
pub const S_ISGID: mode_t = 0o2000;
pub const S_ISVTX: mode_t = 0o1000;

cfg_if! {
    if #[cfg(not(any(
        target_os = "haiku",
        target_os = "illumos",
        target_os = "solaris",
        target_os = "cygwin"
    )))] {
        pub const IF_NAMESIZE: size_t = 16;
        pub const IFNAMSIZ: size_t = IF_NAMESIZE;
    }
}

pub const LOG_EMERG: c_int = 0;
pub const LOG_ALERT: c_int = 1;
pub const LOG_CRIT: c_int = 2;
pub const LOG_ERR: c_int = 3;
pub const LOG_WARNING: c_int = 4;
pub const LOG_NOTICE: c_int = 5;
pub const LOG_INFO: c_int = 6;
pub const LOG_DEBUG: c_int = 7;

pub const LOG_KERN: c_int = 0;
pub const LOG_USER: c_int = 1 << 3;
pub const LOG_MAIL: c_int = 2 << 3;
pub const LOG_DAEMON: c_int = 3 << 3;
pub const LOG_AUTH: c_int = 4 << 3;
pub const LOG_SYSLOG: c_int = 5 << 3;
pub const LOG_LPR: c_int = 6 << 3;
pub const LOG_NEWS: c_int = 7 << 3;
pub const LOG_UUCP: c_int = 8 << 3;
pub const LOG_LOCAL0: c_int = 16 << 3;
pub const LOG_LOCAL1: c_int = 17 << 3;
pub const LOG_LOCAL2: c_int = 18 << 3;
pub const LOG_LOCAL3: c_int = 19 << 3;
pub const LOG_LOCAL4: c_int = 20 << 3;
pub const LOG_LOCAL5: c_int = 21 << 3;
pub const LOG_LOCAL6: c_int = 22 << 3;
pub const LOG_LOCAL7: c_int = 23 << 3;

cfg_if! {
    if #[cfg(not(target_os = "haiku"))] {
        pub const LOG_PID: c_int = 0x01;
        pub const LOG_CONS: c_int = 0x02;
        pub const LOG_ODELAY: c_int = 0x04;
        pub const LOG_NDELAY: c_int = 0x08;
        pub const LOG_NOWAIT: c_int = 0x10;
    }
}
pub const LOG_PRIMASK: c_int = 7;
pub const LOG_FACMASK: c_int = 0x3f8;

cfg_if! {
    if #[cfg(not(target_os = "nto"))] {
        pub const PRIO_MIN: c_int = -20;
        pub const PRIO_MAX: c_int = 20;
    }
}
pub const IPPROTO_ICMP: c_int = 1;
pub const IPPROTO_ICMPV6: c_int = 58;
pub const IPPROTO_TCP: c_int = 6;
pub const IPPROTO_UDP: c_int = 17;
pub const IPPROTO_IP: c_int = 0;
pub const IPPROTO_IPV6: c_int = 41;

pub const INADDR_LOOPBACK: in_addr_t = 2130706433;
pub const INADDR_ANY: in_addr_t = 0;
pub const INADDR_BROADCAST: in_addr_t = 4294967295;
pub const INADDR_NONE: in_addr_t = 4294967295;

pub const IN6ADDR_LOOPBACK_INIT: in6_addr = in6_addr {
    s6_addr: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
};

pub const IN6ADDR_ANY_INIT: in6_addr = in6_addr {
    s6_addr: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
};

pub const ARPOP_REQUEST: u16 = 1;
pub const ARPOP_REPLY: u16 = 2;

pub const ATF_COM: c_int = 0x02;
pub const ATF_PERM: c_int = 0x04;
pub const ATF_PUBL: c_int = 0x08;
pub const ATF_USETRAILERS: c_int = 0x10;

cfg_if! {
    if #[cfg(any(target_os = "nto", target_os = "aix"))] {
        pub const FNM_PERIOD: c_int = 1 << 1;
    } else {
        pub const FNM_PERIOD: c_int = 1 << 2;
    }
}
pub const FNM_NOMATCH: c_int = 1;

cfg_if! {
    if #[cfg(any(target_os = "illumos", target_os = "solaris",))] {
        pub const FNM_CASEFOLD: c_int = 1 << 3;
    } else if #[cfg(not(target_os = "aix"))] {
        pub const FNM_CASEFOLD: c_int = 1 << 4;
    }
}

cfg_if! {
    if #[cfg(any(
        target_os = "macos",
        target_os = "freebsd",
        target_os = "android",
        target_os = "openbsd",
        target_os = "cygwin",
    ))] {
        pub const FNM_PATHNAME: c_int = 1 << 1;
    } else {
        pub const FNM_PATHNAME: c_int = 1 << 0;
    }
}

cfg_if! {
    if #[cfg(any(
        target_os = "macos",
        target_os = "freebsd",
        target_os = "android",
        target_os = "openbsd",
    ))] {
        pub const FNM_NOESCAPE: c_int = 1 << 0;
    } else if #[cfg(target_os = "nto")] {
        pub const FNM_NOESCAPE: c_int = 1 << 2;
    } else if #[cfg(target_os = "aix")] {
        pub const FNM_NOESCAPE: c_int = 1 << 3;
    } else {
        pub const FNM_NOESCAPE: c_int = 1 << 1;
    }
}

extern "C" {
    pub static in6addr_loopback: in6_addr;
    pub static in6addr_any: in6_addr;
}

cfg_if! {
    if #[cfg(any(
        target_os = "l4re",
        target_os = "espidf",
        target_os = "nuttx"
    ))] {
        // required libraries are linked externally for these platforms:
        // * L4Re
        // * ESP-IDF
        // * NuttX
    } else if #[cfg(feature = "std")] {
        // cargo build, don't pull in anything extra as the std dep
        // already pulls in all libs.
    } else if #[cfg(all(
        any(
            all(
                target_os = "linux",
                any(target_env = "gnu", target_env = "uclibc")
            ),
            target_os = "cygwin"
        ),
        feature = "rustc-dep-of-std"
    ))] {
        #[link(
            name = "util",
            kind = "static",
            modifiers = "-bundle",
            cfg(target_feature = "crt-static")
        )]
        #[link(
            name = "rt",
            kind = "static",
            modifiers = "-bundle",
            cfg(target_feature = "crt-static")
        )]
        #[link(
            name = "pthread",
            kind = "static",
            modifiers = "-bundle",
            cfg(target_feature = "crt-static")
        )]
        #[link(
            name = "m",
            kind = "static",
            modifiers = "-bundle",
            cfg(target_feature = "crt-static")
        )]
        #[link(
            name = "dl",
            kind = "static",
            modifiers = "-bundle",
            cfg(target_feature = "crt-static")
        )]
        #[link(
            name = "c",
            kind = "static",
            modifiers = "-bundle",
            cfg(target_feature = "crt-static")
        )]
        #[link(
            name = "gcc_eh",
            kind = "static",
            modifiers = "-bundle",
            cfg(target_feature = "crt-static")
        )]
        #[link(
            name = "gcc",
            kind = "static",
            modifiers = "-bundle",
            cfg(target_feature = "crt-static")
        )]
        #[link(
            name = "c",
            kind = "static",
            modifiers = "-bundle",
            cfg(target_feature = "crt-static")
        )]
        #[link(name = "util", cfg(not(target_feature = "crt-static")))]
        #[link(name = "rt", cfg(not(target_feature = "crt-static")))]
        #[link(name = "pthread", cfg(not(target_feature = "crt-static")))]
        #[link(name = "m", cfg(not(target_feature = "crt-static")))]
        #[link(name = "dl", cfg(not(target_feature = "crt-static")))]
        #[link(name = "c", cfg(not(target_feature = "crt-static")))]
        extern "C" {}
    } else if #[cfg(any(target_env = "musl", target_env = "ohos"))] {
        #[cfg_attr(
            feature = "rustc-dep-of-std",
            link(
                name = "c",
                kind = "static",
                modifiers = "-bundle",
                cfg(target_feature = "crt-static")
            )
        )]
        #[cfg_attr(
            feature = "rustc-dep-of-std",
            link(name = "c", cfg(not(target_feature = "crt-static")))
        )]
        extern "C" {}
    } else if #[cfg(target_os = "emscripten")] {
        // Don't pass -lc to Emscripten, it breaks. See:
        // https://github.com/emscripten-core/emscripten/issues/22758
    } else if #[cfg(all(target_os = "android", feature = "rustc-dep-of-std"))] {
        #[link(
            name = "c",
            kind = "static",
            modifiers = "-bundle",
            cfg(target_feature = "crt-static")
        )]
        #[link(
            name = "m",
            kind = "static",
            modifiers = "-bundle",
            cfg(target_feature = "crt-static")
        )]
        #[link(name = "m", cfg(not(target_feature = "crt-static")))]
        #[link(name = "c", cfg(not(target_feature = "crt-static")))]
        extern "C" {}
    } else if #[cfg(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "tvos",
        target_os = "watchos",
        target_os = "visionos",
        target_os = "android",
        target_os = "openbsd",
        target_os = "nto",
    ))] {
        #[link(name = "c")]
        #[link(name = "m")]
        extern "C" {}
    } else if #[cfg(target_os = "haiku")] {
        #[link(name = "root")]
        #[link(name = "network")]
        extern "C" {}
    } else if #[cfg(target_env = "newlib")] {
        #[link(name = "c")]
        #[link(name = "m")]
        extern "C" {}
    } else if #[cfg(target_env = "illumos")] {
        #[link(name = "c")]
        #[link(name = "m")]
        extern "C" {}
    } else if #[cfg(target_os = "redox")] {
        #[cfg_attr(
            feature = "rustc-dep-of-std",
            link(
                name = "c",
                kind = "static",
                modifiers = "-bundle",
                cfg(target_feature = "crt-static")
            )
        )]
        #[cfg_attr(
            feature = "rustc-dep-of-std",
            link(name = "c", cfg(not(target_feature = "crt-static")))
        )]
        extern "C" {}
    } else if #[cfg(target_os = "aix")] {
        #[link(name = "c")]
        #[link(name = "m")]
        #[link(name = "bsd")]
        #[link(name = "pthread")]
        extern "C" {}
    } else {
        #[link(name = "c")]
        #[link(name = "m")]
        #[link(name = "rt")]
        #[link(name = "pthread")]
        extern "C" {}
    }
}

cfg_if! {
    if #[cfg(not(all(target_os = "linux", target_env = "gnu")))] {
        missing! {
            #[cfg_attr(feature = "extra_traits", derive(Debug))]
            pub enum fpos_t {} // FIXME(unix): fill this out with a struct
        }
    }
}

missing! {
    #[cfg_attr(feature = "extra_traits", derive(Debug))]
    pub enum FILE {}
}

extern "C" {
    pub fn isalnum(c: c_int) -> c_int;
    pub fn isalpha(c: c_int) -> c_int;
    pub fn iscntrl(c: c_int) -> c_int;
    pub fn isdigit(c: c_int) -> c_int;
    pub fn isgraph(c: c_int) -> c_int;
    pub fn islower(c: c_int) -> c_int;
    pub fn isprint(c: c_int) -> c_int;
    pub fn ispunct(c: c_int) -> c_int;
    pub fn isspace(c: c_int) -> c_int;
    pub fn isupper(c: c_int) -> c_int;
    pub fn isxdigit(c: c_int) -> c_int;
    pub fn isblank(c: c_int) -> c_int;
    pub fn tolower(c: c_int) -> c_int;
    pub fn toupper(c: c_int) -> c_int;
    pub fn qsort(
        base: *mut c_void,
        num: size_t,
        size: size_t,
        compar: Option<unsafe extern "C" fn(*const c_void, *const c_void) -> c_int>,
    );
    pub fn bsearch(
        key: *const c_void,
        base: *const c_void,
        num: size_t,
        size: size_t,
        compar: Option<unsafe extern "C" fn(*const c_void, *const c_void) -> c_int>,
    ) -> *mut c_void;
    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "fopen$UNIX2003"
    )]
    #[cfg_attr(gnu_file_offset_bits64, link_name = "fopen64")]
    pub fn fopen(filename: *const c_char, mode: *const c_char) -> *mut FILE;
    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "freopen$UNIX2003"
    )]
    #[cfg_attr(gnu_file_offset_bits64, link_name = "freopen64")]
    pub fn freopen(filename: *const c_char, mode: *const c_char, file: *mut FILE) -> *mut FILE;

    pub fn fflush(file: *mut FILE) -> c_int;
    pub fn fclose(file: *mut FILE) -> c_int;
    pub fn remove(filename: *const c_char) -> c_int;
    pub fn rename(oldname: *const c_char, newname: *const c_char) -> c_int;
    #[cfg_attr(gnu_file_offset_bits64, link_name = "tmpfile64")]
    pub fn tmpfile() -> *mut FILE;
    pub fn setvbuf(stream: *mut FILE, buffer: *mut c_char, mode: c_int, size: size_t) -> c_int;
    pub fn setbuf(stream: *mut FILE, buf: *mut c_char);
    pub fn getchar() -> c_int;
    pub fn putchar(c: c_int) -> c_int;
    pub fn fgetc(stream: *mut FILE) -> c_int;
    pub fn fgets(buf: *mut c_char, n: c_int, stream: *mut FILE) -> *mut c_char;
    pub fn fputc(c: c_int, stream: *mut FILE) -> c_int;
    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "fputs$UNIX2003"
    )]
    pub fn fputs(s: *const c_char, stream: *mut FILE) -> c_int;
    pub fn puts(s: *const c_char) -> c_int;
    pub fn ungetc(c: c_int, stream: *mut FILE) -> c_int;
    pub fn fread(ptr: *mut c_void, size: size_t, nobj: size_t, stream: *mut FILE) -> size_t;
    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "fwrite$UNIX2003"
    )]
    pub fn fwrite(ptr: *const c_void, size: size_t, nobj: size_t, stream: *mut FILE) -> size_t;
    pub fn fseek(stream: *mut FILE, offset: c_long, whence: c_int) -> c_int;
    pub fn ftell(stream: *mut FILE) -> c_long;
    pub fn rewind(stream: *mut FILE);
    #[cfg_attr(target_os = "netbsd", link_name = "__fgetpos50")]
    #[cfg_attr(gnu_file_offset_bits64, link_name = "fgetpos64")]
    pub fn fgetpos(stream: *mut FILE, ptr: *mut fpos_t) -> c_int;
    #[cfg_attr(target_os = "netbsd", link_name = "__fsetpos50")]
    #[cfg_attr(gnu_file_offset_bits64, link_name = "fsetpos64")]
    pub fn fsetpos(stream: *mut FILE, ptr: *const fpos_t) -> c_int;
    pub fn feof(stream: *mut FILE) -> c_int;
    pub fn ferror(stream: *mut FILE) -> c_int;
    pub fn clearerr(stream: *mut FILE);
    pub fn perror(s: *const c_char);
    pub fn atof(s: *const c_char) -> c_double;
    pub fn atoi(s: *const c_char) -> c_int;
    pub fn atol(s: *const c_char) -> c_long;
    pub fn atoll(s: *const c_char) -> c_longlong;
    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "strtod$UNIX2003"
    )]
    pub fn strtod(s: *const c_char, endp: *mut *mut c_char) -> c_double;
    pub fn strtof(s: *const c_char, endp: *mut *mut c_char) -> c_float;
    pub fn strtol(s: *const c_char, endp: *mut *mut c_char, base: c_int) -> c_long;
    pub fn strtoll(s: *const c_char, endp: *mut *mut c_char, base: c_int) -> c_longlong;
    pub fn strtoul(s: *const c_char, endp: *mut *mut c_char, base: c_int) -> c_ulong;
    pub fn strtoull(s: *const c_char, endp: *mut *mut c_char, base: c_int) -> c_ulonglong;
    #[cfg_attr(target_os = "aix", link_name = "vec_calloc")]
    pub fn calloc(nobj: size_t, size: size_t) -> *mut c_void;
    #[cfg_attr(target_os = "aix", link_name = "vec_malloc")]
    pub fn malloc(size: size_t) -> *mut c_void;
    pub fn realloc(p: *mut c_void, size: size_t) -> *mut c_void;
    pub fn free(p: *mut c_void);
    pub fn abort() -> !;
    pub fn exit(status: c_int) -> !;
    pub fn _exit(status: c_int) -> !;
    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "system$UNIX2003"
    )]
    pub fn system(s: *const c_char) -> c_int;
    pub fn getenv(s: *const c_char) -> *mut c_char;

    pub fn strcpy(dst: *mut c_char, src: *const c_char) -> *mut c_char;
    pub fn strncpy(dst: *mut c_char, src: *const c_char, n: size_t) -> *mut c_char;
    pub fn stpcpy(dst: *mut c_char, src: *const c_char) -> *mut c_char;
    pub fn strcat(s: *mut c_char, ct: *const c_char) -> *mut c_char;
    pub fn strncat(s: *mut c_char, ct: *const c_char, n: size_t) -> *mut c_char;
    pub fn strcmp(cs: *const c_char, ct: *const c_char) -> c_int;
    pub fn strncmp(cs: *const c_char, ct: *const c_char, n: size_t) -> c_int;
    pub fn strcoll(cs: *const c_char, ct: *const c_char) -> c_int;
    pub fn strchr(cs: *const c_char, c: c_int) -> *mut c_char;
    pub fn strrchr(cs: *const c_char, c: c_int) -> *mut c_char;
    pub fn strspn(cs: *const c_char, ct: *const c_char) -> size_t;
    pub fn strcspn(cs: *const c_char, ct: *const c_char) -> size_t;
    pub fn strdup(cs: *const c_char) -> *mut c_char;
    pub fn strndup(cs: *const c_char, n: size_t) -> *mut c_char;
    pub fn strpbrk(cs: *const c_char, ct: *const c_char) -> *mut c_char;
    pub fn strstr(cs: *const c_char, ct: *const c_char) -> *mut c_char;
    pub fn strcasecmp(s1: *const c_char, s2: *const c_char) -> c_int;
    pub fn strncasecmp(s1: *const c_char, s2: *const c_char, n: size_t) -> c_int;
    pub fn strlen(cs: *const c_char) -> size_t;
    pub fn strnlen(cs: *const c_char, maxlen: size_t) -> size_t;
    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "strerror$UNIX2003"
    )]
    pub fn strerror(n: c_int) -> *mut c_char;
    pub fn strtok(s: *mut c_char, t: *const c_char) -> *mut c_char;
    pub fn strtok_r(s: *mut c_char, t: *const c_char, p: *mut *mut c_char) -> *mut c_char;
    pub fn strxfrm(s: *mut c_char, ct: *const c_char, n: size_t) -> size_t;
    pub fn strsignal(sig: c_int) -> *mut c_char;
    pub fn wcslen(buf: *const wchar_t) -> size_t;
    pub fn wcstombs(dest: *mut c_char, src: *const wchar_t, n: size_t) -> size_t;

    pub fn memchr(cx: *const c_void, c: c_int, n: size_t) -> *mut c_void;
    pub fn wmemchr(cx: *const wchar_t, c: wchar_t, n: size_t) -> *mut wchar_t;
    pub fn memcmp(cx: *const c_void, ct: *const c_void, n: size_t) -> c_int;
    pub fn memcpy(dest: *mut c_void, src: *const c_void, n: size_t) -> *mut c_void;
    pub fn memmove(dest: *mut c_void, src: *const c_void, n: size_t) -> *mut c_void;
    pub fn memset(dest: *mut c_void, c: c_int, n: size_t) -> *mut c_void;
    pub fn memccpy(dest: *mut c_void, src: *const c_void, c: c_int, n: size_t) -> *mut c_void;
}

extern "C" {
    #[cfg_attr(target_os = "netbsd", link_name = "__getpwnam50")]
    pub fn getpwnam(name: *const c_char) -> *mut passwd;
    #[cfg_attr(target_os = "netbsd", link_name = "__getpwuid50")]
    pub fn getpwuid(uid: crate::uid_t) -> *mut passwd;

    pub fn fprintf(stream: *mut crate::FILE, format: *const c_char, ...) -> c_int;
    pub fn printf(format: *const c_char, ...) -> c_int;
    pub fn snprintf(s: *mut c_char, n: size_t, format: *const c_char, ...) -> c_int;
    pub fn sprintf(s: *mut c_char, format: *const c_char, ...) -> c_int;
    #[cfg_attr(
        all(target_os = "linux", not(target_env = "uclibc")),
        link_name = "__isoc99_fscanf"
    )]
    pub fn fscanf(stream: *mut crate::FILE, format: *const c_char, ...) -> c_int;
    #[cfg_attr(
        all(target_os = "linux", not(target_env = "uclibc")),
        link_name = "__isoc99_scanf"
    )]
    pub fn scanf(format: *const c_char, ...) -> c_int;
    #[cfg_attr(
        all(target_os = "linux", not(target_env = "uclibc")),
        link_name = "__isoc99_sscanf"
    )]
    pub fn sscanf(s: *const c_char, format: *const c_char, ...) -> c_int;
    pub fn getchar_unlocked() -> c_int;
    pub fn putchar_unlocked(c: c_int) -> c_int;

    #[cfg(not(all(target_arch = "powerpc", target_vendor = "nintendo")))]
    #[cfg_attr(target_os = "netbsd", link_name = "__socket30")]
    #[cfg_attr(target_os = "illumos", link_name = "__xnet_socket")]
    #[cfg_attr(target_os = "solaris", link_name = "__xnet7_socket")]
    #[cfg_attr(target_os = "espidf", link_name = "lwip_socket")]
    pub fn socket(domain: c_int, ty: c_int, protocol: c_int) -> c_int;
    #[cfg(not(all(target_arch = "powerpc", target_vendor = "nintendo")))]
    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "connect$UNIX2003"
    )]
    #[cfg_attr(
        any(target_os = "illumos", target_os = "solaris"),
        link_name = "__xnet_connect"
    )]
    #[cfg_attr(target_os = "espidf", link_name = "lwip_connect")]
    pub fn connect(socket: c_int, address: *const sockaddr, len: socklen_t) -> c_int;
    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "listen$UNIX2003"
    )]
    #[cfg_attr(target_os = "espidf", link_name = "lwip_listen")]
    pub fn listen(socket: c_int, backlog: c_int) -> c_int;
    #[cfg(not(all(target_arch = "powerpc", target_vendor = "nintendo")))]
    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "accept$UNIX2003"
    )]
    #[cfg_attr(target_os = "espidf", link_name = "lwip_accept")]
    #[cfg_attr(target_os = "aix", link_name = "naccept")]
    pub fn accept(socket: c_int, address: *mut sockaddr, address_len: *mut socklen_t) -> c_int;
    #[cfg(not(all(target_arch = "powerpc", target_vendor = "nintendo")))]
    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "getpeername$UNIX2003"
    )]
    #[cfg_attr(target_os = "espidf", link_name = "lwip_getpeername")]
    #[cfg_attr(target_os = "aix", link_name = "ngetpeername")]
    pub fn getpeername(socket: c_int, address: *mut sockaddr, address_len: *mut socklen_t)
        -> c_int;
    #[cfg(not(all(target_arch = "powerpc", target_vendor = "nintendo")))]
    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "getsockname$UNIX2003"
    )]
    #[cfg_attr(target_os = "espidf", link_name = "lwip_getsockname")]
    #[cfg_attr(target_os = "aix", link_name = "ngetsockname")]
    pub fn getsockname(socket: c_int, address: *mut sockaddr, address_len: *mut socklen_t)
        -> c_int;
    #[cfg_attr(target_os = "espidf", link_name = "lwip_setsockopt")]
    #[cfg_attr(gnu_time_bits64, link_name = "__setsockopt64")]
    pub fn setsockopt(
        socket: c_int,
        level: c_int,
        name: c_int,
        value: *const c_void,
        option_len: socklen_t,
    ) -> c_int;
    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "socketpair$UNIX2003"
    )]
    #[cfg_attr(
        any(target_os = "illumos", target_os = "solaris"),
        link_name = "__xnet_socketpair"
    )]
    pub fn socketpair(
        domain: c_int,
        type_: c_int,
        protocol: c_int,
        socket_vector: *mut c_int,
    ) -> c_int;
    #[cfg(not(all(target_arch = "powerpc", target_vendor = "nintendo")))]
    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "sendto$UNIX2003"
    )]
    #[cfg_attr(
        any(target_os = "illumos", target_os = "solaris"),
        link_name = "__xnet_sendto"
    )]
    #[cfg_attr(target_os = "espidf", link_name = "lwip_sendto")]
    pub fn sendto(
        socket: c_int,
        buf: *const c_void,
        len: size_t,
        flags: c_int,
        addr: *const sockaddr,
        addrlen: socklen_t,
    ) -> ssize_t;
    #[cfg_attr(target_os = "espidf", link_name = "lwip_shutdown")]
    pub fn shutdown(socket: c_int, how: c_int) -> c_int;

    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "chmod$UNIX2003"
    )]
    pub fn chmod(path: *const c_char, mode: mode_t) -> c_int;
    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "fchmod$UNIX2003"
    )]
    pub fn fchmod(fd: c_int, mode: mode_t) -> c_int;

    #[cfg_attr(
        all(target_os = "macos", not(target_arch = "aarch64")),
        link_name = "fstat$INODE64"
    )]
    #[cfg_attr(target_os = "netbsd", link_name = "__fstat50")]
    #[cfg_attr(
        all(target_os = "freebsd", any(freebsd11, freebsd10)),
        link_name = "fstat@FBSD_1.0"
    )]
    #[cfg_attr(gnu_time_bits64, link_name = "__fstat64_time64")]
    #[cfg_attr(
        all(not(gnu_time_bits64), gnu_file_offset_bits64),
        link_name = "fstat64"
    )]
    pub fn fstat(fildes: c_int, buf: *mut stat) -> c_int;

    pub fn mkdir(path: *const c_char, mode: mode_t) -> c_int;

    #[cfg_attr(
        all(target_os = "macos", not(target_arch = "aarch64")),
        link_name = "stat$INODE64"
    )]
    #[cfg_attr(target_os = "netbsd", link_name = "__stat50")]
    #[cfg_attr(
        all(target_os = "freebsd", any(freebsd11, freebsd10)),
        link_name = "stat@FBSD_1.0"
    )]
    #[cfg_attr(gnu_time_bits64, link_name = "__stat64_time64")]
    #[cfg_attr(
        all(not(gnu_time_bits64), gnu_file_offset_bits64),
        link_name = "stat64"
    )]
    pub fn stat(path: *const c_char, buf: *mut stat) -> c_int;

    pub fn pclose(stream: *mut crate::FILE) -> c_int;
    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "fdopen$UNIX2003"
    )]
    pub fn fdopen(fd: c_int, mode: *const c_char) -> *mut crate::FILE;
    pub fn fileno(stream: *mut crate::FILE) -> c_int;

    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "open$UNIX2003"
    )]
    #[cfg_attr(gnu_file_offset_bits64, link_name = "open64")]
    pub fn open(path: *const c_char, oflag: c_int, ...) -> c_int;
    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "creat$UNIX2003"
    )]
    #[cfg_attr(gnu_file_offset_bits64, link_name = "creat64")]
    pub fn creat(path: *const c_char, mode: mode_t) -> c_int;
    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "fcntl$UNIX2003"
    )]
    #[cfg_attr(gnu_time_bits64, link_name = "__fcntl_time64")]
    #[cfg_attr(
        all(not(gnu_time_bits64), gnu_file_offset_bits64),
        link_name = "__fcntl_time64"
    )]
    pub fn fcntl(fd: c_int, cmd: c_int, ...) -> c_int;

    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86_64"),
        link_name = "opendir$INODE64"
    )]
    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "opendir$INODE64$UNIX2003"
    )]
    #[cfg_attr(target_os = "netbsd", link_name = "__opendir30")]
    pub fn opendir(dirname: *const c_char) -> *mut crate::DIR;

    #[cfg_attr(
        all(target_os = "macos", not(target_arch = "aarch64")),
        link_name = "readdir$INODE64"
    )]
    #[cfg_attr(target_os = "netbsd", link_name = "__readdir30")]
    #[cfg_attr(
        all(target_os = "freebsd", any(freebsd11, freebsd10)),
        link_name = "readdir@FBSD_1.0"
    )]
    #[cfg_attr(gnu_file_offset_bits64, link_name = "readdir64")]
    pub fn readdir(dirp: *mut crate::DIR) -> *mut crate::dirent;
    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "closedir$UNIX2003"
    )]
    pub fn closedir(dirp: *mut crate::DIR) -> c_int;
    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86_64"),
        link_name = "rewinddir$INODE64"
    )]
    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "rewinddir$INODE64$UNIX2003"
    )]
    pub fn rewinddir(dirp: *mut crate::DIR);

    pub fn fchmodat(dirfd: c_int, pathname: *const c_char, mode: mode_t, flags: c_int) -> c_int;
    pub fn fchown(fd: c_int, owner: crate::uid_t, group: crate::gid_t) -> c_int;
    pub fn fchownat(
        dirfd: c_int,
        pathname: *const c_char,
        owner: crate::uid_t,
        group: crate::gid_t,
        flags: c_int,
    ) -> c_int;
    #[cfg_attr(
        all(target_os = "macos", not(target_arch = "aarch64")),
        link_name = "fstatat$INODE64"
    )]
    #[cfg_attr(
        all(target_os = "freebsd", any(freebsd11, freebsd10)),
        link_name = "fstatat@FBSD_1.1"
    )]
    #[cfg_attr(gnu_time_bits64, link_name = "__fstatat64_time64")]
    #[cfg_attr(
        all(not(gnu_time_bits64), gnu_file_offset_bits64),
        link_name = "fstatat64"
    )]
    pub fn fstatat(dirfd: c_int, pathname: *const c_char, buf: *mut stat, flags: c_int) -> c_int;
    pub fn linkat(
        olddirfd: c_int,
        oldpath: *const c_char,
        newdirfd: c_int,
        newpath: *const c_char,
        flags: c_int,
    ) -> c_int;
    pub fn renameat(
        olddirfd: c_int,
        oldpath: *const c_char,
        newdirfd: c_int,
        newpath: *const c_char,
    ) -> c_int;
    pub fn symlinkat(target: *const c_char, newdirfd: c_int, linkpath: *const c_char) -> c_int;
    pub fn unlinkat(dirfd: c_int, pathname: *const c_char, flags: c_int) -> c_int;

    pub fn access(path: *const c_char, amode: c_int) -> c_int;
    pub fn alarm(seconds: c_uint) -> c_uint;
    pub fn chdir(dir: *const c_char) -> c_int;
    pub fn fchdir(dirfd: c_int) -> c_int;
    pub fn chown(path: *const c_char, uid: uid_t, gid: gid_t) -> c_int;
    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "lchown$UNIX2003"
    )]
    pub fn lchown(path: *const c_char, uid: uid_t, gid: gid_t) -> c_int;
    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "close$NOCANCEL$UNIX2003"
    )]
    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86_64"),
        link_name = "close$NOCANCEL"
    )]
    pub fn close(fd: c_int) -> c_int;
    pub fn dup(fd: c_int) -> c_int;
    pub fn dup2(src: c_int, dst: c_int) -> c_int;

    pub fn execl(path: *const c_char, arg0: *const c_char, ...) -> c_int;
    pub fn execle(path: *const c_char, arg0: *const c_char, ...) -> c_int;
    pub fn execlp(file: *const c_char, arg0: *const c_char, ...) -> c_int;

    // DIFF(main): changed to `*const *mut` in e77f551de9
    pub fn execv(prog: *const c_char, argv: *const *const c_char) -> c_int;
    pub fn execve(
        prog: *const c_char,
        argv: *const *const c_char,
        envp: *const *const c_char,
    ) -> c_int;
    pub fn execvp(c: *const c_char, argv: *const *const c_char) -> c_int;

    pub fn fork() -> pid_t;
    pub fn fpathconf(filedes: c_int, name: c_int) -> c_long;
    pub fn getcwd(buf: *mut c_char, size: size_t) -> *mut c_char;
    pub fn getegid() -> gid_t;
    pub fn geteuid() -> uid_t;
    pub fn getgid() -> gid_t;
    pub fn getgroups(ngroups_max: c_int, groups: *mut gid_t) -> c_int;
    #[cfg_attr(target_os = "illumos", link_name = "getloginx")]
    pub fn getlogin() -> *mut c_char;
    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "getopt$UNIX2003"
    )]
    pub fn getopt(argc: c_int, argv: *const *mut c_char, optstr: *const c_char) -> c_int;
    pub fn getpgid(pid: pid_t) -> pid_t;
    pub fn getpgrp() -> pid_t;
    pub fn getpid() -> pid_t;
    pub fn getppid() -> pid_t;
    pub fn getuid() -> uid_t;
    pub fn isatty(fd: c_int) -> c_int;
    #[cfg_attr(target_os = "solaris", link_name = "__link_xpg4")]
    pub fn link(src: *const c_char, dst: *const c_char) -> c_int;
    #[cfg_attr(gnu_file_offset_bits64, link_name = "lseek64")]
    pub fn lseek(fd: c_int, offset: off_t, whence: c_int) -> off_t;
    pub fn pathconf(path: *const c_char, name: c_int) -> c_long;
    pub fn pipe(fds: *mut c_int) -> c_int;
    pub fn posix_memalign(memptr: *mut *mut c_void, align: size_t, size: size_t) -> c_int;
    pub fn aligned_alloc(alignment: size_t, size: size_t) -> *mut c_void;
    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "read$UNIX2003"
    )]
    pub fn read(fd: c_int, buf: *mut c_void, count: size_t) -> ssize_t;
    pub fn rmdir(path: *const c_char) -> c_int;
    pub fn seteuid(uid: uid_t) -> c_int;
    pub fn setegid(gid: gid_t) -> c_int;
    pub fn setgid(gid: gid_t) -> c_int;
    pub fn setpgid(pid: pid_t, pgid: pid_t) -> c_int;
    pub fn setsid() -> pid_t;
    pub fn setuid(uid: uid_t) -> c_int;
    pub fn setreuid(ruid: uid_t, euid: uid_t) -> c_int;
    pub fn setregid(rgid: gid_t, egid: gid_t) -> c_int;
    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "sleep$UNIX2003"
    )]
    pub fn sleep(secs: c_uint) -> c_uint;
    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "nanosleep$UNIX2003"
    )]
    #[cfg_attr(target_os = "netbsd", link_name = "__nanosleep50")]
    #[cfg_attr(gnu_time_bits64, link_name = "__nanosleep64")]
    pub fn nanosleep(rqtp: *const timespec, rmtp: *mut timespec) -> c_int;
    pub fn tcgetpgrp(fd: c_int) -> pid_t;
    pub fn tcsetpgrp(fd: c_int, pgrp: crate::pid_t) -> c_int;
    pub fn ttyname(fd: c_int) -> *mut c_char;
    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "ttyname_r$UNIX2003"
    )]
    #[cfg_attr(
        any(target_os = "illumos", target_os = "solaris"),
        link_name = "__posix_ttyname_r"
    )]
    pub fn ttyname_r(fd: c_int, buf: *mut c_char, buflen: size_t) -> c_int;
    pub fn unlink(c: *const c_char) -> c_int;
    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "wait$UNIX2003"
    )]
    pub fn wait(status: *mut c_int) -> pid_t;
    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "waitpid$UNIX2003"
    )]
    pub fn waitpid(pid: pid_t, status: *mut c_int, options: c_int) -> pid_t;
    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "write$UNIX2003"
    )]
    pub fn write(fd: c_int, buf: *const c_void, count: size_t) -> ssize_t;
    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "pread$UNIX2003"
    )]
    #[cfg_attr(gnu_file_offset_bits64, link_name = "pread64")]
    pub fn pread(fd: c_int, buf: *mut c_void, count: size_t, offset: off_t) -> ssize_t;
    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "pwrite$UNIX2003"
    )]
    #[cfg_attr(gnu_file_offset_bits64, link_name = "pwrite64")]
    pub fn pwrite(fd: c_int, buf: *const c_void, count: size_t, offset: off_t) -> ssize_t;
    pub fn umask(mask: mode_t) -> mode_t;

    #[cfg_attr(target_os = "netbsd", link_name = "__utime50")]
    #[cfg_attr(gnu_time_bits64, link_name = "__utime64")]
    pub fn utime(file: *const c_char, buf: *const utimbuf) -> c_int;

    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "kill$UNIX2003"
    )]
    pub fn kill(pid: pid_t, sig: c_int) -> c_int;
    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "killpg$UNIX2003"
    )]
    pub fn killpg(pgrp: pid_t, sig: c_int) -> c_int;

    pub fn mlock(addr: *const c_void, len: size_t) -> c_int;
    pub fn munlock(addr: *const c_void, len: size_t) -> c_int;
    pub fn mlockall(flags: c_int) -> c_int;
    pub fn munlockall() -> c_int;

    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "mmap$UNIX2003"
    )]
    #[cfg_attr(gnu_file_offset_bits64, link_name = "mmap64")]
    pub fn mmap(
        addr: *mut c_void,
        len: size_t,
        prot: c_int,
        flags: c_int,
        fd: c_int,
        offset: off_t,
    ) -> *mut c_void;
    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "munmap$UNIX2003"
    )]
    pub fn munmap(addr: *mut c_void, len: size_t) -> c_int;

    pub fn if_nametoindex(ifname: *const c_char) -> c_uint;
    pub fn if_indextoname(ifindex: c_uint, ifname: *mut c_char) -> *mut c_char;

    #[cfg_attr(
        all(target_os = "macos", not(target_arch = "aarch64")),
        link_name = "lstat$INODE64"
    )]
    #[cfg_attr(target_os = "netbsd", link_name = "__lstat50")]
    #[cfg_attr(
        all(target_os = "freebsd", any(freebsd11, freebsd10)),
        link_name = "lstat@FBSD_1.0"
    )]
    #[cfg_attr(gnu_time_bits64, link_name = "__lstat64_time64")]
    #[cfg_attr(
        all(not(gnu_time_bits64), gnu_file_offset_bits64),
        link_name = "lstat64"
    )]
    pub fn lstat(path: *const c_char, buf: *mut stat) -> c_int;

    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "fsync$UNIX2003"
    )]
    pub fn fsync(fd: c_int) -> c_int;

    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "setenv$UNIX2003"
    )]
    pub fn setenv(name: *const c_char, val: *const c_char, overwrite: c_int) -> c_int;
    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "unsetenv$UNIX2003"
    )]
    #[cfg_attr(target_os = "netbsd", link_name = "__unsetenv13")]
    pub fn unsetenv(name: *const c_char) -> c_int;

    pub fn symlink(path1: *const c_char, path2: *const c_char) -> c_int;

    #[cfg_attr(gnu_file_offset_bits64, link_name = "truncate64")]
    pub fn truncate(path: *const c_char, length: off_t) -> c_int;
    #[cfg_attr(gnu_file_offset_bits64, link_name = "ftruncate64")]
    pub fn ftruncate(fd: c_int, length: off_t) -> c_int;

    pub fn signal(signum: c_int, handler: sighandler_t) -> sighandler_t;

    #[cfg_attr(target_os = "netbsd", link_name = "__getrusage50")]
    #[cfg_attr(gnu_time_bits64, link_name = "__getrusage64")]
    pub fn getrusage(resource: c_int, usage: *mut rusage) -> c_int;

    #[cfg_attr(
        any(
            target_os = "macos",
            target_os = "ios",
            target_os = "tvos",
            target_os = "watchos",
            target_os = "visionos"
        ),
        link_name = "realpath$DARWIN_EXTSN"
    )]
    pub fn realpath(pathname: *const c_char, resolved: *mut c_char) -> *mut c_char;

    #[cfg_attr(target_os = "netbsd", link_name = "__times13")]
    pub fn times(buf: *mut crate::tms) -> crate::clock_t;

    pub fn pthread_self() -> crate::pthread_t;
    pub fn pthread_equal(t1: crate::pthread_t, t2: crate::pthread_t) -> c_int;
    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "pthread_join$UNIX2003"
    )]
    pub fn pthread_join(native: crate::pthread_t, value: *mut *mut c_void) -> c_int;
    pub fn pthread_exit(value: *mut c_void) -> !;
    pub fn pthread_attr_init(attr: *mut crate::pthread_attr_t) -> c_int;
    pub fn pthread_attr_destroy(attr: *mut crate::pthread_attr_t) -> c_int;
    pub fn pthread_attr_getstacksize(
        attr: *const crate::pthread_attr_t,
        stacksize: *mut size_t,
    ) -> c_int;
    pub fn pthread_attr_setstacksize(attr: *mut crate::pthread_attr_t, stack_size: size_t)
        -> c_int;
    pub fn pthread_attr_setdetachstate(attr: *mut crate::pthread_attr_t, state: c_int) -> c_int;
    pub fn pthread_detach(thread: crate::pthread_t) -> c_int;
    #[cfg_attr(target_os = "netbsd", link_name = "__libc_thr_yield")]
    pub fn sched_yield() -> c_int;
    pub fn pthread_key_create(
        key: *mut pthread_key_t,
        dtor: Option<unsafe extern "C" fn(*mut c_void)>,
    ) -> c_int;
    pub fn pthread_key_delete(key: pthread_key_t) -> c_int;
    pub fn pthread_getspecific(key: pthread_key_t) -> *mut c_void;
    pub fn pthread_setspecific(key: pthread_key_t, value: *const c_void) -> c_int;
    pub fn pthread_mutex_init(
        lock: *mut pthread_mutex_t,
        attr: *const pthread_mutexattr_t,
    ) -> c_int;
    pub fn pthread_mutex_destroy(lock: *mut pthread_mutex_t) -> c_int;
    pub fn pthread_mutex_lock(lock: *mut pthread_mutex_t) -> c_int;
    pub fn pthread_mutex_trylock(lock: *mut pthread_mutex_t) -> c_int;
    pub fn pthread_mutex_unlock(lock: *mut pthread_mutex_t) -> c_int;

    pub fn pthread_mutexattr_init(attr: *mut pthread_mutexattr_t) -> c_int;
    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "pthread_mutexattr_destroy$UNIX2003"
    )]
    pub fn pthread_mutexattr_destroy(attr: *mut pthread_mutexattr_t) -> c_int;
    pub fn pthread_mutexattr_settype(attr: *mut pthread_mutexattr_t, _type: c_int) -> c_int;

    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "pthread_cond_init$UNIX2003"
    )]
    pub fn pthread_cond_init(cond: *mut pthread_cond_t, attr: *const pthread_condattr_t) -> c_int;
    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "pthread_cond_wait$UNIX2003"
    )]
    pub fn pthread_cond_wait(cond: *mut pthread_cond_t, lock: *mut pthread_mutex_t) -> c_int;
    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "pthread_cond_timedwait$UNIX2003"
    )]
    #[cfg_attr(gnu_time_bits64, link_name = "__pthread_cond_timedwait64")]
    pub fn pthread_cond_timedwait(
        cond: *mut pthread_cond_t,
        lock: *mut pthread_mutex_t,
        abstime: *const crate::timespec,
    ) -> c_int;
    pub fn pthread_cond_signal(cond: *mut pthread_cond_t) -> c_int;
    pub fn pthread_cond_broadcast(cond: *mut pthread_cond_t) -> c_int;
    pub fn pthread_cond_destroy(cond: *mut pthread_cond_t) -> c_int;
    pub fn pthread_condattr_init(attr: *mut pthread_condattr_t) -> c_int;
    pub fn pthread_condattr_destroy(attr: *mut pthread_condattr_t) -> c_int;
    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "pthread_rwlock_init$UNIX2003"
    )]
    pub fn pthread_rwlock_init(
        lock: *mut pthread_rwlock_t,
        attr: *const pthread_rwlockattr_t,
    ) -> c_int;
    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "pthread_rwlock_destroy$UNIX2003"
    )]
    pub fn pthread_rwlock_destroy(lock: *mut pthread_rwlock_t) -> c_int;
    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "pthread_rwlock_rdlock$UNIX2003"
    )]
    pub fn pthread_rwlock_rdlock(lock: *mut pthread_rwlock_t) -> c_int;
    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "pthread_rwlock_tryrdlock$UNIX2003"
    )]
    pub fn pthread_rwlock_tryrdlock(lock: *mut pthread_rwlock_t) -> c_int;
    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "pthread_rwlock_wrlock$UNIX2003"
    )]
    pub fn pthread_rwlock_wrlock(lock: *mut pthread_rwlock_t) -> c_int;
    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "pthread_rwlock_trywrlock$UNIX2003"
    )]
    pub fn pthread_rwlock_trywrlock(lock: *mut pthread_rwlock_t) -> c_int;
    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "pthread_rwlock_unlock$UNIX2003"
    )]
    pub fn pthread_rwlock_unlock(lock: *mut pthread_rwlock_t) -> c_int;
    pub fn pthread_rwlockattr_init(attr: *mut pthread_rwlockattr_t) -> c_int;
    pub fn pthread_rwlockattr_destroy(attr: *mut pthread_rwlockattr_t) -> c_int;

    #[cfg_attr(
        any(target_os = "illumos", target_os = "solaris"),
        link_name = "__xnet_getsockopt"
    )]
    #[cfg_attr(target_os = "espidf", link_name = "lwip_getsockopt")]
    #[cfg_attr(gnu_time_bits64, link_name = "__getsockopt64")]
    pub fn getsockopt(
        sockfd: c_int,
        level: c_int,
        optname: c_int,
        optval: *mut c_void,
        optlen: *mut crate::socklen_t,
    ) -> c_int;
    pub fn raise(signum: c_int) -> c_int;

    #[cfg_attr(target_os = "netbsd", link_name = "__utimes50")]
    #[cfg_attr(gnu_time_bits64, link_name = "__utimes64")]
    pub fn utimes(filename: *const c_char, times: *const crate::timeval) -> c_int;
    pub fn dlopen(filename: *const c_char, flag: c_int) -> *mut c_void;
    pub fn dlerror() -> *mut c_char;
    pub fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    pub fn dlclose(handle: *mut c_void) -> c_int;

    #[cfg(not(all(target_arch = "powerpc", target_vendor = "nintendo")))]
    #[cfg_attr(
        any(target_os = "illumos", target_os = "solaris"),
        link_name = "__xnet_getaddrinfo"
    )]
    #[cfg_attr(target_os = "espidf", link_name = "lwip_getaddrinfo")]
    pub fn getaddrinfo(
        node: *const c_char,
        service: *const c_char,
        hints: *const addrinfo,
        res: *mut *mut addrinfo,
    ) -> c_int;
    #[cfg(not(all(target_arch = "powerpc", target_vendor = "nintendo")))]
    #[cfg_attr(target_os = "espidf", link_name = "lwip_freeaddrinfo")]
    pub fn freeaddrinfo(res: *mut addrinfo);
    pub fn hstrerror(errcode: c_int) -> *const c_char;
    pub fn gai_strerror(errcode: c_int) -> *const c_char;
    #[cfg_attr(
        any(
            all(
                target_os = "linux",
                not(any(target_env = "musl", target_env = "ohos"))
            ),
            target_os = "freebsd",
            target_os = "cygwin",
            target_os = "dragonfly",
            target_os = "haiku"
        ),
        link_name = "__res_init"
    )]
    #[cfg_attr(
        any(
            target_os = "macos",
            target_os = "ios",
            target_os = "tvos",
            target_os = "watchos",
            target_os = "visionos"
        ),
        link_name = "res_9_init"
    )]
    #[cfg_attr(target_os = "aix", link_name = "_res_init")]
    pub fn res_init() -> c_int;

    #[cfg_attr(target_os = "netbsd", link_name = "__gmtime_r50")]
    #[cfg_attr(any(target_env = "musl", target_env = "ohos"), allow(deprecated))]
    // FIXME(time): for `time_t`
    #[cfg_attr(gnu_time_bits64, link_name = "__gmtime64_r")]
    pub fn gmtime_r(time_p: *const time_t, result: *mut tm) -> *mut tm;
    #[cfg_attr(target_os = "netbsd", link_name = "__localtime_r50")]
    #[cfg_attr(any(target_env = "musl", target_env = "ohos"), allow(deprecated))]
    // FIXME(time): for `time_t`
    #[cfg_attr(gnu_time_bits64, link_name = "__localtime64_r")]
    pub fn localtime_r(time_p: *const time_t, result: *mut tm) -> *mut tm;
    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "mktime$UNIX2003"
    )]
    #[cfg_attr(target_os = "netbsd", link_name = "__mktime50")]
    #[cfg_attr(any(target_env = "musl", target_env = "ohos"), allow(deprecated))]
    // FIXME: for `time_t`
    #[cfg_attr(gnu_time_bits64, link_name = "__mktime64")]
    pub fn mktime(tm: *mut tm) -> time_t;
    #[cfg_attr(target_os = "netbsd", link_name = "__time50")]
    #[cfg_attr(any(target_env = "musl", target_env = "ohos"), allow(deprecated))]
    // FIXME: for `time_t`
    #[cfg_attr(gnu_time_bits64, link_name = "__time64")]
    pub fn time(time: *mut time_t) -> time_t;
    #[cfg_attr(target_os = "netbsd", link_name = "__gmtime50")]
    #[cfg_attr(any(target_env = "musl", target_env = "ohos"), allow(deprecated))]
    // FIXME(time): for `time_t`
    #[cfg_attr(gnu_time_bits64, link_name = "__gmtime64")]
    pub fn gmtime(time_p: *const time_t) -> *mut tm;
    #[cfg_attr(target_os = "netbsd", link_name = "__locatime50")]
    #[cfg_attr(any(target_env = "musl", target_env = "ohos"), allow(deprecated))]
    // FIXME(time): for `time_t`
    #[cfg_attr(gnu_time_bits64, link_name = "__localtime64")]
    pub fn localtime(time_p: *const time_t) -> *mut tm;
    #[cfg_attr(target_os = "netbsd", link_name = "__difftime50")]
    #[cfg_attr(any(target_env = "musl", target_env = "ohos"), allow(deprecated))]
    // FIXME(time): for `time_t`
    #[cfg_attr(gnu_time_bits64, link_name = "__difftime64")]
    pub fn difftime(time1: time_t, time0: time_t) -> c_double;
    #[cfg(not(target_os = "aix"))]
    #[cfg_attr(target_os = "netbsd", link_name = "__timegm50")]
    #[cfg_attr(any(target_env = "musl", target_env = "ohos"), allow(deprecated))]
    // FIXME(time): for `time_t`
    #[cfg_attr(gnu_time_bits64, link_name = "__timegm64")]
    pub fn timegm(tm: *mut crate::tm) -> time_t;

    #[cfg_attr(target_os = "netbsd", link_name = "__mknod50")]
    #[cfg_attr(
        all(target_os = "freebsd", any(freebsd11, freebsd10)),
        link_name = "mknod@FBSD_1.0"
    )]
    pub fn mknod(pathname: *const c_char, mode: mode_t, dev: crate::dev_t) -> c_int;
    pub fn gethostname(name: *mut c_char, len: size_t) -> c_int;
    pub fn endservent();
    pub fn getservbyname(name: *const c_char, proto: *const c_char) -> *mut servent;
    pub fn getservbyport(port: c_int, proto: *const c_char) -> *mut servent;
    pub fn getservent() -> *mut servent;
    pub fn setservent(stayopen: c_int);
    pub fn getprotobyname(name: *const c_char) -> *mut protoent;
    pub fn getprotobynumber(proto: c_int) -> *mut protoent;
    pub fn chroot(name: *const c_char) -> c_int;
    #[cfg(target_os = "cygwin")]
    pub fn usleep(secs: useconds_t) -> c_int;
    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "usleep$UNIX2003"
    )]
    #[cfg(not(target_os = "cygwin"))]
    pub fn usleep(secs: c_uint) -> c_int;
    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "send$UNIX2003"
    )]
    #[cfg_attr(target_os = "espidf", link_name = "lwip_send")]
    pub fn send(socket: c_int, buf: *const c_void, len: size_t, flags: c_int) -> ssize_t;
    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "recv$UNIX2003"
    )]
    #[cfg_attr(target_os = "espidf", link_name = "lwip_recv")]
    pub fn recv(socket: c_int, buf: *mut c_void, len: size_t, flags: c_int) -> ssize_t;
    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "putenv$UNIX2003"
    )]
    #[cfg_attr(target_os = "netbsd", link_name = "__putenv50")]
    pub fn putenv(string: *mut c_char) -> c_int;
    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "poll$UNIX2003"
    )]
    pub fn poll(fds: *mut pollfd, nfds: nfds_t, timeout: c_int) -> c_int;
    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86_64"),
        link_name = "select$1050"
    )]
    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "select$UNIX2003"
    )]
    #[cfg_attr(target_os = "netbsd", link_name = "__select50")]
    #[cfg_attr(target_os = "aix", link_name = "__fd_select")]
    #[cfg_attr(gnu_time_bits64, link_name = "__select64")]
    pub fn select(
        nfds: c_int,
        readfds: *mut fd_set,
        writefds: *mut fd_set,
        errorfds: *mut fd_set,
        timeout: *mut timeval,
    ) -> c_int;
    #[cfg_attr(target_os = "netbsd", link_name = "__setlocale50")]
    pub fn setlocale(category: c_int, locale: *const c_char) -> *mut c_char;
    pub fn localeconv() -> *mut lconv;

    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "sem_wait$UNIX2003"
    )]
    pub fn sem_wait(sem: *mut sem_t) -> c_int;
    pub fn sem_trywait(sem: *mut sem_t) -> c_int;
    pub fn sem_post(sem: *mut sem_t) -> c_int;
    #[cfg_attr(gnu_file_offset_bits64, link_name = "statvfs64")]
    pub fn statvfs(path: *const c_char, buf: *mut statvfs) -> c_int;
    #[cfg_attr(gnu_file_offset_bits64, link_name = "fstatvfs64")]
    pub fn fstatvfs(fd: c_int, buf: *mut statvfs) -> c_int;

    #[cfg_attr(target_os = "netbsd", link_name = "__sigemptyset14")]
    pub fn sigemptyset(set: *mut sigset_t) -> c_int;
    #[cfg_attr(target_os = "netbsd", link_name = "__sigaddset14")]
    pub fn sigaddset(set: *mut sigset_t, signum: c_int) -> c_int;
    #[cfg_attr(target_os = "netbsd", link_name = "__sigfillset14")]
    pub fn sigfillset(set: *mut sigset_t) -> c_int;
    #[cfg_attr(target_os = "netbsd", link_name = "__sigdelset14")]
    pub fn sigdelset(set: *mut sigset_t, signum: c_int) -> c_int;
    #[cfg_attr(target_os = "netbsd", link_name = "__sigismember14")]
    pub fn sigismember(set: *const sigset_t, signum: c_int) -> c_int;

    #[cfg_attr(target_os = "netbsd", link_name = "__sigprocmask14")]
    pub fn sigprocmask(how: c_int, set: *const sigset_t, oldset: *mut sigset_t) -> c_int;
    #[cfg_attr(target_os = "netbsd", link_name = "__sigpending14")]
    pub fn sigpending(set: *mut sigset_t) -> c_int;

    #[cfg_attr(target_os = "solaris", link_name = "__sysconf_xpg7")]
    pub fn sysconf(name: c_int) -> c_long;

    pub fn mkfifo(path: *const c_char, mode: mode_t) -> c_int;

    #[cfg_attr(gnu_file_offset_bits64, link_name = "fseeko64")]
    pub fn fseeko(stream: *mut crate::FILE, offset: off_t, whence: c_int) -> c_int;
    #[cfg_attr(gnu_file_offset_bits64, link_name = "ftello64")]
    pub fn ftello(stream: *mut crate::FILE) -> off_t;
    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "tcdrain$UNIX2003"
    )]
    pub fn tcdrain(fd: c_int) -> c_int;
    pub fn cfgetispeed(termios: *const crate::termios) -> crate::speed_t;
    pub fn cfgetospeed(termios: *const crate::termios) -> crate::speed_t;
    pub fn cfsetispeed(termios: *mut crate::termios, speed: crate::speed_t) -> c_int;
    pub fn cfsetospeed(termios: *mut crate::termios, speed: crate::speed_t) -> c_int;
    pub fn tcgetattr(fd: c_int, termios: *mut crate::termios) -> c_int;
    pub fn tcsetattr(fd: c_int, optional_actions: c_int, termios: *const crate::termios) -> c_int;
    pub fn tcflow(fd: c_int, action: c_int) -> c_int;
    pub fn tcflush(fd: c_int, action: c_int) -> c_int;
    pub fn tcgetsid(fd: c_int) -> crate::pid_t;
    pub fn tcsendbreak(fd: c_int, duration: c_int) -> c_int;
    #[cfg_attr(gnu_file_offset_bits64, link_name = "mkstemp64")]
    pub fn mkstemp(template: *mut c_char) -> c_int;
    pub fn mkdtemp(template: *mut c_char) -> *mut c_char;

    pub fn tmpnam(ptr: *mut c_char) -> *mut c_char;

    pub fn openlog(ident: *const c_char, logopt: c_int, facility: c_int);
    pub fn closelog();
    pub fn setlogmask(maskpri: c_int) -> c_int;
    #[cfg_attr(target_os = "macos", link_name = "syslog$DARWIN_EXTSN")]
    pub fn syslog(priority: c_int, message: *const c_char, ...);
    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "nice$UNIX2003"
    )]
    pub fn nice(incr: c_int) -> c_int;

    pub fn grantpt(fd: c_int) -> c_int;
    pub fn posix_openpt(flags: c_int) -> c_int;
    pub fn ptsname(fd: c_int) -> *mut c_char;
    pub fn unlockpt(fd: c_int) -> c_int;

    #[cfg(not(target_os = "aix"))]
    pub fn strcasestr(cs: *const c_char, ct: *const c_char) -> *mut c_char;
    pub fn getline(lineptr: *mut *mut c_char, n: *mut size_t, stream: *mut FILE) -> ssize_t;

    #[cfg_attr(gnu_file_offset_bits64, link_name = "lockf64")]
    pub fn lockf(fd: c_int, cmd: c_int, len: off_t) -> c_int;

}

safe_f! {
    // It seems htonl, etc are macros on macOS. So we have to reimplement them. So let's
    // reimplement them for all UNIX platforms
    pub {const} fn htonl(hostlong: u32) -> u32 {
        u32::to_be(hostlong)
    }
    pub {const} fn htons(hostshort: u16) -> u16 {
        u16::to_be(hostshort)
    }
    pub {const} fn ntohl(netlong: u32) -> u32 {
        u32::from_be(netlong)
    }
    pub {const} fn ntohs(netshort: u16) -> u16 {
        u16::from_be(netshort)
    }
}

cfg_if! {
    if #[cfg(not(any(
        target_os = "emscripten",
        target_os = "android",
        target_os = "haiku",
        target_os = "nto",
        target_os = "solaris",
        target_os = "cygwin",
        target_os = "aix",
    )))] {
        extern "C" {
            #[cfg_attr(gnu_time_bits64, link_name = "__adjtime64")]
            pub fn adjtime(delta: *const timeval, olddelta: *mut timeval) -> c_int;
        }
    } else if #[cfg(target_os = "solaris")] {
        extern "C" {
            pub fn adjtime(delta: *mut timeval, olddelta: *mut timeval) -> c_int;
        }
    }
}

cfg_if! {
    if #[cfg(not(any(
        target_os = "emscripten",
        target_os = "android",
        target_os = "nto"
    )))] {
        extern "C" {
            pub fn stpncpy(dst: *mut c_char, src: *const c_char, n: size_t) -> *mut c_char;
        }
    }
}

cfg_if! {
    if #[cfg(not(target_os = "android"))] {
        extern "C" {
            #[cfg_attr(
                all(target_os = "macos", target_arch = "x86"),
                link_name = "confstr$UNIX2003"
            )]
            #[cfg_attr(target_os = "solaris", link_name = "__confstr_xpg7")]
            pub fn confstr(name: c_int, buf: *mut c_char, len: size_t) -> size_t;
        }
    }
}

cfg_if! {
    if #[cfg(not(target_os = "aix"))] {
        extern "C" {
            pub fn dladdr(addr: *const c_void, info: *mut Dl_info) -> c_int;
        }
    }
}

cfg_if! {
    if #[cfg(not(target_os = "solaris"))] {
        extern "C" {
            pub fn flock(fd: c_int, operation: c_int) -> c_int;
        }
    }
}

cfg_if! {
    if #[cfg(not(any(target_env = "uclibc", target_os = "nto")))] {
        extern "C" {
            pub fn open_wmemstream(ptr: *mut *mut wchar_t, sizeloc: *mut size_t) -> *mut FILE;
        }
    }
}

cfg_if! {
    if #[cfg(not(target_os = "redox"))] {
        extern "C" {
            pub fn getsid(pid: pid_t) -> pid_t;
            #[cfg_attr(
                all(target_os = "macos", target_arch = "x86"),
                link_name = "pause$UNIX2003"
            )]
            pub fn pause() -> c_int;

            pub fn mkdirat(dirfd: c_int, pathname: *const c_char, mode: mode_t) -> c_int;
            #[cfg_attr(gnu_file_offset_bits64, link_name = "openat64")]
            pub fn openat(dirfd: c_int, pathname: *const c_char, flags: c_int, ...) -> c_int;

            #[cfg_attr(
                all(target_os = "macos", target_arch = "x86_64"),
                link_name = "fdopendir$INODE64"
            )]
            #[cfg_attr(
                all(target_os = "macos", target_arch = "x86"),
                link_name = "fdopendir$INODE64$UNIX2003"
            )]
            pub fn fdopendir(fd: c_int) -> *mut crate::DIR;

            #[cfg_attr(
                all(target_os = "macos", not(target_arch = "aarch64")),
                link_name = "readdir_r$INODE64"
            )]
            #[cfg_attr(target_os = "netbsd", link_name = "__readdir_r30")]
            #[cfg_attr(
                all(target_os = "freebsd", any(freebsd11, freebsd10)),
                link_name = "readdir_r@FBSD_1.0"
            )]
            #[allow(non_autolinks)] // FIXME(docs): `<>` breaks line length limit.
            /// The 64-bit libc on Solaris and illumos only has readdir_r. If a
            /// 32-bit Solaris or illumos target is ever created, it should use
            /// __posix_readdir_r. See libc(3LIB) on Solaris or illumos:
            /// https://illumos.org/man/3lib/libc
            /// https://docs.oracle.com/cd/E36784_01/html/E36873/libc-3lib.html
            /// https://www.unix.com/man-page/opensolaris/3LIB/libc/
            #[cfg_attr(gnu_file_offset_bits64, link_name = "readdir64_r")]
            pub fn readdir_r(
                dirp: *mut crate::DIR,
                entry: *mut crate::dirent,
                result: *mut *mut crate::dirent,
            ) -> c_int;
        }
    }
}

cfg_if! {
    if #[cfg(target_os = "nto")] {
        extern "C" {
            pub fn readlinkat(
                dirfd: c_int,
                pathname: *const c_char,
                buf: *mut c_char,
                bufsiz: size_t,
            ) -> c_int;
            pub fn readlink(path: *const c_char, buf: *mut c_char, bufsz: size_t) -> c_int;
            pub fn pselect(
                nfds: c_int,
                readfds: *mut fd_set,
                writefds: *mut fd_set,
                errorfds: *mut fd_set,
                timeout: *mut timespec,
                sigmask: *const sigset_t,
            ) -> c_int;
            pub fn sigaction(signum: c_int, act: *const sigaction, oldact: *mut sigaction)
                -> c_int;
        }
    } else {
        extern "C" {
            pub fn readlinkat(
                dirfd: c_int,
                pathname: *const c_char,
                buf: *mut c_char,
                bufsiz: size_t,
            ) -> ssize_t;
            pub fn fmemopen(buf: *mut c_void, size: size_t, mode: *const c_char) -> *mut FILE;
            pub fn open_memstream(ptr: *mut *mut c_char, sizeloc: *mut size_t) -> *mut FILE;
            pub fn atexit(cb: extern "C" fn()) -> c_int;
            #[cfg_attr(target_os = "netbsd", link_name = "__sigaction14")]
            pub fn sigaction(signum: c_int, act: *const sigaction, oldact: *mut sigaction)
                -> c_int;
            pub fn readlink(path: *const c_char, buf: *mut c_char, bufsz: size_t) -> ssize_t;
            #[cfg_attr(
                all(target_os = "macos", target_arch = "x86_64"),
                link_name = "pselect$1050"
            )]
            #[cfg_attr(
                all(target_os = "macos", target_arch = "x86"),
                link_name = "pselect$UNIX2003"
            )]
            #[cfg_attr(target_os = "netbsd", link_name = "__pselect50")]
            #[cfg_attr(gnu_time_bits64, link_name = "__pselect64")]
            pub fn pselect(
                nfds: c_int,
                readfds: *mut fd_set,
                writefds: *mut fd_set,
                errorfds: *mut fd_set,
                timeout: *const timespec,
                sigmask: *const sigset_t,
            ) -> c_int;
        }
    }
}

cfg_if! {
    if #[cfg(target_os = "aix")] {
        extern "C" {
            pub fn cfmakeraw(termios: *mut crate::termios) -> c_int;
            pub fn cfsetspeed(termios: *mut crate::termios, speed: crate::speed_t) -> c_int;
        }
    } else if #[cfg(not(any(
        target_os = "solaris",
        target_os = "illumos",
        target_os = "nto",
    )))] {
        extern "C" {
            pub fn cfmakeraw(termios: *mut crate::termios);
            pub fn cfsetspeed(termios: *mut crate::termios, speed: crate::speed_t) -> c_int;
        }
    }
}

extern "C" {
    pub fn fnmatch(pattern: *const c_char, name: *const c_char, flags: c_int) -> c_int;
}

cfg_if! {
    if #[cfg(target_env = "newlib")] {
        mod newlib;
        pub use self::newlib::*;
    } else if #[cfg(any(
        target_os = "linux",
        target_os = "l4re",
        target_os = "android",
        target_os = "emscripten"
    ))] {
        mod linux_like;
        pub use self::linux_like::*;
    } else if #[cfg(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "tvos",
        target_os = "watchos",
        target_os = "visionos",
        target_os = "freebsd",
        target_os = "dragonfly",
        target_os = "openbsd",
        target_os = "netbsd"
    ))] {
        mod bsd;
        pub use self::bsd::*;
    } else if #[cfg(any(target_os = "solaris", target_os = "illumos"))] {
        mod solarish;
        pub use self::solarish::*;
    } else if #[cfg(target_os = "haiku")] {
        mod haiku;
        pub use self::haiku::*;
    } else if #[cfg(target_os = "redox")] {
        mod redox;
        pub use self::redox::*;
    } else if #[cfg(target_os = "cygwin")] {
        mod cygwin;
        pub use self::cygwin::*;
    } else if #[cfg(target_os = "nto")] {
        mod nto;
        pub use self::nto::*;
    } else if #[cfg(target_os = "aix")] {
        mod aix;
        pub use self::aix::*;
    } else if #[cfg(target_os = "hurd")] {
        mod hurd;
        pub use self::hurd::*;
    } else if #[cfg(target_os = "nuttx")] {
        mod nuttx;
        pub use self::nuttx::*;
    } else {
        // Unknown target_os
    }
}
