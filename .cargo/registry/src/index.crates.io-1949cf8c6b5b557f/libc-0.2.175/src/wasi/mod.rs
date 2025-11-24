//! [wasi-libc](https://github.com/WebAssembly/wasi-libc) definitions.
//!
//! `wasi-libc` project provides multiple libraries including emulated features, but we list only
//! basic features with `libc.a` here.

use core::iter::Iterator;

use crate::prelude::*;

pub type intmax_t = i64;
pub type uintmax_t = u64;
pub type size_t = usize;
pub type ssize_t = isize;
pub type ptrdiff_t = isize;
pub type intptr_t = isize;
pub type uintptr_t = usize;
pub type off_t = i64;
pub type pid_t = i32;
pub type clock_t = c_longlong;
pub type time_t = c_longlong;
pub type ino_t = u64;
pub type sigset_t = c_uchar;
pub type suseconds_t = c_longlong;
pub type mode_t = u32;
pub type dev_t = u64;
pub type uid_t = u32;
pub type gid_t = u32;
pub type nlink_t = u64;
pub type blksize_t = c_long;
pub type blkcnt_t = i64;
pub type nfds_t = c_ulong;
pub type wchar_t = i32;
pub type nl_item = c_int;
pub type __wasi_rights_t = u64;
pub type locale_t = *mut __locale_struct;

s_no_extra_traits! {
    #[repr(align(16))]
    pub struct max_align_t {
        priv_: [f64; 4],
    }
}

#[allow(missing_copy_implementations)]
#[cfg_attr(feature = "extra_traits", derive(Debug))]
pub enum FILE {}
#[allow(missing_copy_implementations)]
#[cfg_attr(feature = "extra_traits", derive(Debug))]
pub enum DIR {}
#[allow(missing_copy_implementations)]
#[cfg_attr(feature = "extra_traits", derive(Debug))]
pub enum __locale_struct {}

s_paren! {
    // in wasi-libc clockid_t is const struct __clockid* (where __clockid is an opaque struct),
    // but that's an implementation detail that we don't want to have to deal with
    #[repr(transparent)]
    #[allow(dead_code)]
    pub struct clockid_t(*const u8);
}

unsafe impl Send for clockid_t {}
unsafe impl Sync for clockid_t {}

s! {
    #[repr(align(8))]
    pub struct fpos_t {
        data: [u8; 16],
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
        pub __tm_gmtoff: c_int,
        pub __tm_zone: *const c_char,
        pub __tm_nsec: c_int,
    }

    pub struct timeval {
        pub tv_sec: time_t,
        pub tv_usec: suseconds_t,
    }

    pub struct timespec {
        pub tv_sec: time_t,
        pub tv_nsec: c_long,
    }

    pub struct tms {
        pub tms_utime: clock_t,
        pub tms_stime: clock_t,
        pub tms_cutime: clock_t,
        pub tms_cstime: clock_t,
    }

    pub struct itimerspec {
        pub it_interval: timespec,
        pub it_value: timespec,
    }

    pub struct iovec {
        pub iov_base: *mut c_void,
        pub iov_len: size_t,
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

    pub struct pollfd {
        pub fd: c_int,
        pub events: c_short,
        pub revents: c_short,
    }

    pub struct rusage {
        pub ru_utime: timeval,
        pub ru_stime: timeval,
    }

    pub struct stat {
        pub st_dev: dev_t,
        pub st_ino: ino_t,
        pub st_nlink: nlink_t,
        pub st_mode: mode_t,
        pub st_uid: uid_t,
        pub st_gid: gid_t,
        __pad0: c_uint,
        pub st_rdev: dev_t,
        pub st_size: off_t,
        pub st_blksize: blksize_t,
        pub st_blocks: blkcnt_t,
        pub st_atim: timespec,
        pub st_mtim: timespec,
        pub st_ctim: timespec,
        __reserved: [c_longlong; 3],
    }

    pub struct fd_set {
        __nfds: usize,
        __fds: [c_int; FD_SETSIZE as usize],
    }
}

// Declare dirent outside of s! so that it doesn't implement Copy, Eq, Hash,
// etc., since it contains a flexible array member with a dynamic size.
#[repr(C)]
#[allow(missing_copy_implementations)]
#[cfg_attr(feature = "extra_traits", derive(Debug))]
pub struct dirent {
    pub d_ino: ino_t,
    pub d_type: c_uchar,
    /// d_name is declared in WASI libc as a flexible array member, which
    /// can't be directly expressed in Rust. As an imperfect workaround,
    /// declare it as a zero-length array instead.
    pub d_name: [c_char; 0],
}

pub const EXIT_SUCCESS: c_int = 0;
pub const EXIT_FAILURE: c_int = 1;
pub const STDIN_FILENO: c_int = 0;
pub const STDOUT_FILENO: c_int = 1;
pub const STDERR_FILENO: c_int = 2;
pub const SEEK_SET: c_int = 0;
pub const SEEK_CUR: c_int = 1;
pub const SEEK_END: c_int = 2;
pub const _IOFBF: c_int = 0;
pub const _IONBF: c_int = 2;
pub const _IOLBF: c_int = 1;
pub const F_GETFD: c_int = 1;
pub const F_SETFD: c_int = 2;
pub const F_GETFL: c_int = 3;
pub const F_SETFL: c_int = 4;
pub const FD_CLOEXEC: c_int = 1;
pub const FD_SETSIZE: size_t = 1024;
pub const O_APPEND: c_int = 0x0001;
pub const O_DSYNC: c_int = 0x0002;
pub const O_NONBLOCK: c_int = 0x0004;
pub const O_RSYNC: c_int = 0x0008;
pub const O_SYNC: c_int = 0x0010;
pub const O_CREAT: c_int = 0x0001 << 12;
pub const O_DIRECTORY: c_int = 0x0002 << 12;
pub const O_EXCL: c_int = 0x0004 << 12;
pub const O_TRUNC: c_int = 0x0008 << 12;
pub const O_NOFOLLOW: c_int = 0x01000000;
pub const O_EXEC: c_int = 0x02000000;
pub const O_RDONLY: c_int = 0x04000000;
pub const O_SEARCH: c_int = 0x08000000;
pub const O_WRONLY: c_int = 0x10000000;
pub const O_CLOEXEC: c_int = 0x0;
pub const O_RDWR: c_int = O_WRONLY | O_RDONLY;
pub const O_ACCMODE: c_int = O_EXEC | O_RDWR | O_SEARCH;
pub const O_NOCTTY: c_int = 0x0;
pub const POSIX_FADV_DONTNEED: c_int = 4;
pub const POSIX_FADV_NOREUSE: c_int = 5;
pub const POSIX_FADV_NORMAL: c_int = 0;
pub const POSIX_FADV_RANDOM: c_int = 2;
pub const POSIX_FADV_SEQUENTIAL: c_int = 1;
pub const POSIX_FADV_WILLNEED: c_int = 3;
pub const AT_FDCWD: c_int = -2;
pub const AT_EACCESS: c_int = 0x0;
pub const AT_SYMLINK_NOFOLLOW: c_int = 0x1;
pub const AT_SYMLINK_FOLLOW: c_int = 0x2;
pub const AT_REMOVEDIR: c_int = 0x4;
pub const UTIME_OMIT: c_long = 0xfffffffe;
pub const UTIME_NOW: c_long = 0xffffffff;
pub const S_IFIFO: mode_t = 0o1_0000;
pub const S_IFCHR: mode_t = 0o2_0000;
pub const S_IFBLK: mode_t = 0o6_0000;
pub const S_IFDIR: mode_t = 0o4_0000;
pub const S_IFREG: mode_t = 0o10_0000;
pub const S_IFLNK: mode_t = 0o12_0000;
pub const S_IFSOCK: mode_t = 0o14_0000;
pub const S_IFMT: mode_t = 0o17_0000;
pub const S_IRWXO: mode_t = 0o0007;
pub const S_IXOTH: mode_t = 0o0001;
pub const S_IWOTH: mode_t = 0o0002;
pub const S_IROTH: mode_t = 0o0004;
pub const S_IRWXG: mode_t = 0o0070;
pub const S_IXGRP: mode_t = 0o0010;
pub const S_IWGRP: mode_t = 0o0020;
pub const S_IRGRP: mode_t = 0o0040;
pub const S_IRWXU: mode_t = 0o0700;
pub const S_IXUSR: mode_t = 0o0100;
pub const S_IWUSR: mode_t = 0o0200;
pub const S_IRUSR: mode_t = 0o0400;
pub const S_ISVTX: mode_t = 0o1000;
pub const S_ISGID: mode_t = 0o2000;
pub const S_ISUID: mode_t = 0o4000;
pub const DT_UNKNOWN: u8 = 0;
pub const DT_BLK: u8 = 1;
pub const DT_CHR: u8 = 2;
pub const DT_DIR: u8 = 3;
pub const DT_REG: u8 = 4;
pub const DT_LNK: u8 = 7;
pub const FIONREAD: c_int = 1;
pub const FIONBIO: c_int = 2;
pub const F_OK: c_int = 0;
pub const R_OK: c_int = 4;
pub const W_OK: c_int = 2;
pub const X_OK: c_int = 1;
pub const POLLIN: c_short = 0x1;
pub const POLLOUT: c_short = 0x2;
pub const POLLERR: c_short = 0x1000;
pub const POLLHUP: c_short = 0x2000;
pub const POLLNVAL: c_short = 0x4000;
pub const POLLRDNORM: c_short = 0x1;
pub const POLLWRNORM: c_short = 0x2;

pub const E2BIG: c_int = 1;
pub const EACCES: c_int = 2;
pub const EADDRINUSE: c_int = 3;
pub const EADDRNOTAVAIL: c_int = 4;
pub const EAFNOSUPPORT: c_int = 5;
pub const EAGAIN: c_int = 6;
pub const EALREADY: c_int = 7;
pub const EBADF: c_int = 8;
pub const EBADMSG: c_int = 9;
pub const EBUSY: c_int = 10;
pub const ECANCELED: c_int = 11;
pub const ECHILD: c_int = 12;
pub const ECONNABORTED: c_int = 13;
pub const ECONNREFUSED: c_int = 14;
pub const ECONNRESET: c_int = 15;
pub const EDEADLK: c_int = 16;
pub const EDESTADDRREQ: c_int = 17;
pub const EDOM: c_int = 18;
pub const EDQUOT: c_int = 19;
pub const EEXIST: c_int = 20;
pub const EFAULT: c_int = 21;
pub const EFBIG: c_int = 22;
pub const EHOSTUNREACH: c_int = 23;
pub const EIDRM: c_int = 24;
pub const EILSEQ: c_int = 25;
pub const EINPROGRESS: c_int = 26;
pub const EINTR: c_int = 27;
pub const EINVAL: c_int = 28;
pub const EIO: c_int = 29;
pub const EISCONN: c_int = 30;
pub const EISDIR: c_int = 31;
pub const ELOOP: c_int = 32;
pub const EMFILE: c_int = 33;
pub const EMLINK: c_int = 34;
pub const EMSGSIZE: c_int = 35;
pub const EMULTIHOP: c_int = 36;
pub const ENAMETOOLONG: c_int = 37;
pub const ENETDOWN: c_int = 38;
pub const ENETRESET: c_int = 39;
pub const ENETUNREACH: c_int = 40;
pub const ENFILE: c_int = 41;
pub const ENOBUFS: c_int = 42;
pub const ENODEV: c_int = 43;
pub const ENOENT: c_int = 44;
pub const ENOEXEC: c_int = 45;
pub const ENOLCK: c_int = 46;
pub const ENOLINK: c_int = 47;
pub const ENOMEM: c_int = 48;
pub const ENOMSG: c_int = 49;
pub const ENOPROTOOPT: c_int = 50;
pub const ENOSPC: c_int = 51;
pub const ENOSYS: c_int = 52;
pub const ENOTCONN: c_int = 53;
pub const ENOTDIR: c_int = 54;
pub const ENOTEMPTY: c_int = 55;
pub const ENOTRECOVERABLE: c_int = 56;
pub const ENOTSOCK: c_int = 57;
pub const ENOTSUP: c_int = 58;
pub const ENOTTY: c_int = 59;
pub const ENXIO: c_int = 60;
pub const EOVERFLOW: c_int = 61;
pub const EOWNERDEAD: c_int = 62;
pub const EPERM: c_int = 63;
pub const EPIPE: c_int = 64;
pub const EPROTO: c_int = 65;
pub const EPROTONOSUPPORT: c_int = 66;
pub const EPROTOTYPE: c_int = 67;
pub const ERANGE: c_int = 68;
pub const EROFS: c_int = 69;
pub const ESPIPE: c_int = 70;
pub const ESRCH: c_int = 71;
pub const ESTALE: c_int = 72;
pub const ETIMEDOUT: c_int = 73;
pub const ETXTBSY: c_int = 74;
pub const EXDEV: c_int = 75;
pub const ENOTCAPABLE: c_int = 76;
pub const EOPNOTSUPP: c_int = ENOTSUP;
pub const EWOULDBLOCK: c_int = EAGAIN;

pub const _SC_PAGESIZE: c_int = 30;
pub const _SC_PAGE_SIZE: c_int = _SC_PAGESIZE;
pub const _SC_IOV_MAX: c_int = 60;
pub const _SC_SYMLOOP_MAX: c_int = 173;

cfg_if! {
    if #[cfg(libc_ctest)] {
        // skip these constants when this is active because `ctest` currently
        // panics on parsing the constants below
    } else {
        // `addr_of!(EXTERN_STATIC)` is now safe; remove `unsafe` when MSRV >= 1.82
        #[allow(unused_unsafe)]
        pub static CLOCK_MONOTONIC: clockid_t =
            unsafe { clockid_t(core::ptr::addr_of!(_CLOCK_MONOTONIC)) };
        #[allow(unused_unsafe)]
        pub static CLOCK_PROCESS_CPUTIME_ID: clockid_t =
            unsafe { clockid_t(core::ptr::addr_of!(_CLOCK_PROCESS_CPUTIME_ID)) };
        #[allow(unused_unsafe)]
        pub static CLOCK_REALTIME: clockid_t =
            unsafe { clockid_t(core::ptr::addr_of!(_CLOCK_REALTIME)) };
        #[allow(unused_unsafe)]
        pub static CLOCK_THREAD_CPUTIME_ID: clockid_t =
            unsafe { clockid_t(core::ptr::addr_of!(_CLOCK_THREAD_CPUTIME_ID)) };
    }
}

pub const ABDAY_1: crate::nl_item = 0x20000;
pub const ABDAY_2: crate::nl_item = 0x20001;
pub const ABDAY_3: crate::nl_item = 0x20002;
pub const ABDAY_4: crate::nl_item = 0x20003;
pub const ABDAY_5: crate::nl_item = 0x20004;
pub const ABDAY_6: crate::nl_item = 0x20005;
pub const ABDAY_7: crate::nl_item = 0x20006;

pub const DAY_1: crate::nl_item = 0x20007;
pub const DAY_2: crate::nl_item = 0x20008;
pub const DAY_3: crate::nl_item = 0x20009;
pub const DAY_4: crate::nl_item = 0x2000A;
pub const DAY_5: crate::nl_item = 0x2000B;
pub const DAY_6: crate::nl_item = 0x2000C;
pub const DAY_7: crate::nl_item = 0x2000D;

pub const ABMON_1: crate::nl_item = 0x2000E;
pub const ABMON_2: crate::nl_item = 0x2000F;
pub const ABMON_3: crate::nl_item = 0x20010;
pub const ABMON_4: crate::nl_item = 0x20011;
pub const ABMON_5: crate::nl_item = 0x20012;
pub const ABMON_6: crate::nl_item = 0x20013;
pub const ABMON_7: crate::nl_item = 0x20014;
pub const ABMON_8: crate::nl_item = 0x20015;
pub const ABMON_9: crate::nl_item = 0x20016;
pub const ABMON_10: crate::nl_item = 0x20017;
pub const ABMON_11: crate::nl_item = 0x20018;
pub const ABMON_12: crate::nl_item = 0x20019;

pub const MON_1: crate::nl_item = 0x2001A;
pub const MON_2: crate::nl_item = 0x2001B;
pub const MON_3: crate::nl_item = 0x2001C;
pub const MON_4: crate::nl_item = 0x2001D;
pub const MON_5: crate::nl_item = 0x2001E;
pub const MON_6: crate::nl_item = 0x2001F;
pub const MON_7: crate::nl_item = 0x20020;
pub const MON_8: crate::nl_item = 0x20021;
pub const MON_9: crate::nl_item = 0x20022;
pub const MON_10: crate::nl_item = 0x20023;
pub const MON_11: crate::nl_item = 0x20024;
pub const MON_12: crate::nl_item = 0x20025;

pub const AM_STR: crate::nl_item = 0x20026;
pub const PM_STR: crate::nl_item = 0x20027;

pub const D_T_FMT: crate::nl_item = 0x20028;
pub const D_FMT: crate::nl_item = 0x20029;
pub const T_FMT: crate::nl_item = 0x2002A;
pub const T_FMT_AMPM: crate::nl_item = 0x2002B;

pub const ERA: crate::nl_item = 0x2002C;
pub const ERA_D_FMT: crate::nl_item = 0x2002E;
pub const ALT_DIGITS: crate::nl_item = 0x2002F;
pub const ERA_D_T_FMT: crate::nl_item = 0x20030;
pub const ERA_T_FMT: crate::nl_item = 0x20031;

pub const CODESET: crate::nl_item = 14;
pub const CRNCYSTR: crate::nl_item = 0x4000F;
pub const RADIXCHAR: crate::nl_item = 0x10000;
pub const THOUSEP: crate::nl_item = 0x10001;
pub const YESEXPR: crate::nl_item = 0x50000;
pub const NOEXPR: crate::nl_item = 0x50001;
pub const YESSTR: crate::nl_item = 0x50002;
pub const NOSTR: crate::nl_item = 0x50003;

f! {
    pub fn FD_ISSET(fd: c_int, set: *const fd_set) -> bool {
        let set = &*set;
        let n = set.__nfds;
        return set.__fds[..n].iter().any(|p| *p == fd);
    }

    pub fn FD_SET(fd: c_int, set: *mut fd_set) -> () {
        let set = &mut *set;
        let n = set.__nfds;
        if !set.__fds[..n].iter().any(|p| *p == fd) {
            set.__nfds = n + 1;
            set.__fds[n] = fd;
        }
    }

    pub fn FD_ZERO(set: *mut fd_set) -> () {
        (*set).__nfds = 0;
        return;
    }
}

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
extern "C" {
    pub fn _Exit(code: c_int) -> !;
    pub fn _exit(code: c_int) -> !;
    pub fn abort() -> !;
    pub fn aligned_alloc(a: size_t, b: size_t) -> *mut c_void;
    pub fn calloc(amt: size_t, amt2: size_t) -> *mut c_void;
    pub fn exit(code: c_int) -> !;
    pub fn free(ptr: *mut c_void);
    pub fn getenv(s: *const c_char) -> *mut c_char;
    pub fn malloc(amt: size_t) -> *mut c_void;
    pub fn malloc_usable_size(ptr: *mut c_void) -> size_t;
    pub fn sbrk(increment: intptr_t) -> *mut c_void;
    pub fn rand() -> c_int;
    pub fn read(fd: c_int, ptr: *mut c_void, size: size_t) -> ssize_t;
    pub fn realloc(ptr: *mut c_void, amt: size_t) -> *mut c_void;
    pub fn setenv(k: *const c_char, v: *const c_char, a: c_int) -> c_int;
    pub fn unsetenv(k: *const c_char) -> c_int;
    pub fn clearenv() -> c_int;
    pub fn write(fd: c_int, ptr: *const c_void, size: size_t) -> ssize_t;
    pub static mut environ: *mut *mut c_char;
    pub fn fopen(a: *const c_char, b: *const c_char) -> *mut FILE;
    pub fn freopen(a: *const c_char, b: *const c_char, f: *mut FILE) -> *mut FILE;
    pub fn fclose(f: *mut FILE) -> c_int;
    pub fn remove(a: *const c_char) -> c_int;
    pub fn rename(a: *const c_char, b: *const c_char) -> c_int;
    pub fn feof(f: *mut FILE) -> c_int;
    pub fn ferror(f: *mut FILE) -> c_int;
    pub fn fflush(f: *mut FILE) -> c_int;
    pub fn clearerr(f: *mut FILE);
    pub fn fseek(f: *mut FILE, b: c_long, c: c_int) -> c_int;
    pub fn ftell(f: *mut FILE) -> c_long;
    pub fn rewind(f: *mut FILE);
    pub fn fgetpos(f: *mut FILE, pos: *mut fpos_t) -> c_int;
    pub fn fsetpos(f: *mut FILE, pos: *const fpos_t) -> c_int;
    pub fn fread(buf: *mut c_void, a: size_t, b: size_t, f: *mut FILE) -> size_t;
    pub fn fwrite(buf: *const c_void, a: size_t, b: size_t, f: *mut FILE) -> size_t;
    pub fn fgetc(f: *mut FILE) -> c_int;
    pub fn getc(f: *mut FILE) -> c_int;
    pub fn getchar() -> c_int;
    pub fn ungetc(a: c_int, f: *mut FILE) -> c_int;
    pub fn fputc(a: c_int, f: *mut FILE) -> c_int;
    pub fn putc(a: c_int, f: *mut FILE) -> c_int;
    pub fn putchar(a: c_int) -> c_int;
    pub fn fputs(a: *const c_char, f: *mut FILE) -> c_int;
    pub fn puts(a: *const c_char) -> c_int;
    pub fn perror(a: *const c_char);
    pub fn srand(a: c_uint);
    pub fn atexit(a: extern "C" fn()) -> c_int;
    pub fn at_quick_exit(a: extern "C" fn()) -> c_int;
    pub fn quick_exit(a: c_int) -> !;
    pub fn posix_memalign(a: *mut *mut c_void, b: size_t, c: size_t) -> c_int;
    pub fn rand_r(a: *mut c_uint) -> c_int;
    pub fn random() -> c_long;
    pub fn srandom(a: c_uint);
    pub fn putenv(a: *mut c_char) -> c_int;
    pub fn clock() -> clock_t;
    pub fn time(a: *mut time_t) -> time_t;
    pub fn difftime(a: time_t, b: time_t) -> c_double;
    pub fn mktime(a: *mut tm) -> time_t;
    pub fn strftime(a: *mut c_char, b: size_t, c: *const c_char, d: *const tm) -> size_t;
    pub fn gmtime(a: *const time_t) -> *mut tm;
    pub fn gmtime_r(a: *const time_t, b: *mut tm) -> *mut tm;
    pub fn localtime(a: *const time_t) -> *mut tm;
    pub fn localtime_r(a: *const time_t, b: *mut tm) -> *mut tm;
    pub fn asctime_r(a: *const tm, b: *mut c_char) -> *mut c_char;
    pub fn ctime_r(a: *const time_t, b: *mut c_char) -> *mut c_char;

    static _CLOCK_MONOTONIC: u8;
    static _CLOCK_PROCESS_CPUTIME_ID: u8;
    static _CLOCK_REALTIME: u8;
    static _CLOCK_THREAD_CPUTIME_ID: u8;
    pub fn nanosleep(a: *const timespec, b: *mut timespec) -> c_int;
    pub fn clock_getres(a: clockid_t, b: *mut timespec) -> c_int;
    pub fn clock_gettime(a: clockid_t, b: *mut timespec) -> c_int;
    pub fn clock_nanosleep(a: clockid_t, a2: c_int, b: *const timespec, c: *mut timespec) -> c_int;

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
    pub fn setvbuf(stream: *mut FILE, buffer: *mut c_char, mode: c_int, size: size_t) -> c_int;
    pub fn setbuf(stream: *mut FILE, buf: *mut c_char);
    pub fn fgets(buf: *mut c_char, n: c_int, stream: *mut FILE) -> *mut c_char;
    pub fn atof(s: *const c_char) -> c_double;
    pub fn atoi(s: *const c_char) -> c_int;
    pub fn atol(s: *const c_char) -> c_long;
    pub fn atoll(s: *const c_char) -> c_longlong;
    pub fn strtod(s: *const c_char, endp: *mut *mut c_char) -> c_double;
    pub fn strtof(s: *const c_char, endp: *mut *mut c_char) -> c_float;
    pub fn strtol(s: *const c_char, endp: *mut *mut c_char, base: c_int) -> c_long;
    pub fn strtoll(s: *const c_char, endp: *mut *mut c_char, base: c_int) -> c_longlong;
    pub fn strtoul(s: *const c_char, endp: *mut *mut c_char, base: c_int) -> c_ulong;
    pub fn strtoull(s: *const c_char, endp: *mut *mut c_char, base: c_int) -> c_ulonglong;

    pub fn strcpy(dst: *mut c_char, src: *const c_char) -> *mut c_char;
    pub fn strncpy(dst: *mut c_char, src: *const c_char, n: size_t) -> *mut c_char;
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
    pub fn strerror(n: c_int) -> *mut c_char;
    pub fn strtok(s: *mut c_char, t: *const c_char) -> *mut c_char;
    pub fn strxfrm(s: *mut c_char, ct: *const c_char, n: size_t) -> size_t;

    pub fn memchr(cx: *const c_void, c: c_int, n: size_t) -> *mut c_void;
    pub fn memcmp(cx: *const c_void, ct: *const c_void, n: size_t) -> c_int;
    pub fn memcpy(dest: *mut c_void, src: *const c_void, n: size_t) -> *mut c_void;
    pub fn memmove(dest: *mut c_void, src: *const c_void, n: size_t) -> *mut c_void;
    pub fn memset(dest: *mut c_void, c: c_int, n: size_t) -> *mut c_void;

    pub fn fprintf(stream: *mut crate::FILE, format: *const c_char, ...) -> c_int;
    pub fn printf(format: *const c_char, ...) -> c_int;
    pub fn snprintf(s: *mut c_char, n: size_t, format: *const c_char, ...) -> c_int;
    pub fn sprintf(s: *mut c_char, format: *const c_char, ...) -> c_int;
    pub fn fscanf(stream: *mut crate::FILE, format: *const c_char, ...) -> c_int;
    pub fn scanf(format: *const c_char, ...) -> c_int;
    pub fn sscanf(s: *const c_char, format: *const c_char, ...) -> c_int;
    pub fn getchar_unlocked() -> c_int;
    pub fn putchar_unlocked(c: c_int) -> c_int;

    pub fn shutdown(socket: c_int, how: c_int) -> c_int;
    pub fn fstat(fildes: c_int, buf: *mut stat) -> c_int;
    pub fn mkdir(path: *const c_char, mode: mode_t) -> c_int;
    pub fn stat(path: *const c_char, buf: *mut stat) -> c_int;
    pub fn fdopen(fd: c_int, mode: *const c_char) -> *mut crate::FILE;
    pub fn fileno(stream: *mut crate::FILE) -> c_int;
    pub fn open(path: *const c_char, oflag: c_int, ...) -> c_int;
    pub fn creat(path: *const c_char, mode: mode_t) -> c_int;
    pub fn fcntl(fd: c_int, cmd: c_int, ...) -> c_int;
    pub fn opendir(dirname: *const c_char) -> *mut crate::DIR;
    pub fn fdopendir(fd: c_int) -> *mut crate::DIR;
    pub fn readdir(dirp: *mut crate::DIR) -> *mut crate::dirent;
    pub fn closedir(dirp: *mut crate::DIR) -> c_int;
    pub fn rewinddir(dirp: *mut crate::DIR);
    pub fn dirfd(dirp: *mut crate::DIR) -> c_int;
    pub fn seekdir(dirp: *mut crate::DIR, loc: c_long);
    pub fn telldir(dirp: *mut crate::DIR) -> c_long;

    pub fn openat(dirfd: c_int, pathname: *const c_char, flags: c_int, ...) -> c_int;
    pub fn fstatat(dirfd: c_int, pathname: *const c_char, buf: *mut stat, flags: c_int) -> c_int;
    pub fn linkat(
        olddirfd: c_int,
        oldpath: *const c_char,
        newdirfd: c_int,
        newpath: *const c_char,
        flags: c_int,
    ) -> c_int;
    pub fn mkdirat(dirfd: c_int, pathname: *const c_char, mode: mode_t) -> c_int;
    pub fn readlinkat(
        dirfd: c_int,
        pathname: *const c_char,
        buf: *mut c_char,
        bufsiz: size_t,
    ) -> ssize_t;
    pub fn renameat(
        olddirfd: c_int,
        oldpath: *const c_char,
        newdirfd: c_int,
        newpath: *const c_char,
    ) -> c_int;
    pub fn symlinkat(target: *const c_char, newdirfd: c_int, linkpath: *const c_char) -> c_int;
    pub fn unlinkat(dirfd: c_int, pathname: *const c_char, flags: c_int) -> c_int;

    pub fn access(path: *const c_char, amode: c_int) -> c_int;
    pub fn close(fd: c_int) -> c_int;
    pub fn fpathconf(filedes: c_int, name: c_int) -> c_long;
    pub fn getopt(argc: c_int, argv: *const *mut c_char, optstr: *const c_char) -> c_int;
    pub fn isatty(fd: c_int) -> c_int;
    pub fn link(src: *const c_char, dst: *const c_char) -> c_int;
    pub fn lseek(fd: c_int, offset: off_t, whence: c_int) -> off_t;
    pub fn pathconf(path: *const c_char, name: c_int) -> c_long;
    pub fn rmdir(path: *const c_char) -> c_int;
    pub fn sleep(secs: c_uint) -> c_uint;
    pub fn unlink(c: *const c_char) -> c_int;
    pub fn pread(fd: c_int, buf: *mut c_void, count: size_t, offset: off_t) -> ssize_t;
    pub fn pwrite(fd: c_int, buf: *const c_void, count: size_t, offset: off_t) -> ssize_t;

    pub fn lstat(path: *const c_char, buf: *mut stat) -> c_int;

    pub fn fsync(fd: c_int) -> c_int;
    pub fn fdatasync(fd: c_int) -> c_int;

    pub fn symlink(path1: *const c_char, path2: *const c_char) -> c_int;

    pub fn truncate(path: *const c_char, length: off_t) -> c_int;
    pub fn ftruncate(fd: c_int, length: off_t) -> c_int;

    pub fn getrusage(resource: c_int, usage: *mut rusage) -> c_int;

    pub fn gettimeofday(tp: *mut crate::timeval, tz: *mut c_void) -> c_int;
    pub fn times(buf: *mut crate::tms) -> crate::clock_t;

    pub fn strerror_r(errnum: c_int, buf: *mut c_char, buflen: size_t) -> c_int;

    pub fn usleep(secs: c_uint) -> c_int;
    pub fn send(socket: c_int, buf: *const c_void, len: size_t, flags: c_int) -> ssize_t;
    pub fn recv(socket: c_int, buf: *mut c_void, len: size_t, flags: c_int) -> ssize_t;
    pub fn poll(fds: *mut pollfd, nfds: nfds_t, timeout: c_int) -> c_int;
    pub fn setlocale(category: c_int, locale: *const c_char) -> *mut c_char;
    pub fn localeconv() -> *mut lconv;

    pub fn readlink(path: *const c_char, buf: *mut c_char, bufsz: size_t) -> ssize_t;

    pub fn timegm(tm: *mut crate::tm) -> time_t;

    pub fn sysconf(name: c_int) -> c_long;

    pub fn ioctl(fd: c_int, request: c_int, ...) -> c_int;

    pub fn fseeko(stream: *mut crate::FILE, offset: off_t, whence: c_int) -> c_int;
    pub fn ftello(stream: *mut crate::FILE) -> off_t;
    pub fn posix_fallocate(fd: c_int, offset: off_t, len: off_t) -> c_int;

    pub fn strcasestr(cs: *const c_char, ct: *const c_char) -> *mut c_char;
    pub fn getline(lineptr: *mut *mut c_char, n: *mut size_t, stream: *mut FILE) -> ssize_t;

    pub fn faccessat(dirfd: c_int, pathname: *const c_char, mode: c_int, flags: c_int) -> c_int;
    pub fn writev(fd: c_int, iov: *const crate::iovec, iovcnt: c_int) -> ssize_t;
    pub fn readv(fd: c_int, iov: *const crate::iovec, iovcnt: c_int) -> ssize_t;
    pub fn pwritev(fd: c_int, iov: *const crate::iovec, iovcnt: c_int, offset: off_t) -> ssize_t;
    pub fn preadv(fd: c_int, iov: *const crate::iovec, iovcnt: c_int, offset: off_t) -> ssize_t;
    pub fn posix_fadvise(fd: c_int, offset: off_t, len: off_t, advise: c_int) -> c_int;
    pub fn futimens(fd: c_int, times: *const crate::timespec) -> c_int;
    pub fn utimensat(
        dirfd: c_int,
        path: *const c_char,
        times: *const crate::timespec,
        flag: c_int,
    ) -> c_int;
    pub fn getentropy(buf: *mut c_void, buflen: size_t) -> c_int;
    pub fn memrchr(cx: *const c_void, c: c_int, n: size_t) -> *mut c_void;
    pub fn abs(i: c_int) -> c_int;
    pub fn labs(i: c_long) -> c_long;
    pub fn duplocale(base: crate::locale_t) -> crate::locale_t;
    pub fn freelocale(loc: crate::locale_t);
    pub fn newlocale(mask: c_int, locale: *const c_char, base: crate::locale_t) -> crate::locale_t;
    pub fn uselocale(loc: crate::locale_t) -> crate::locale_t;
    pub fn sched_yield() -> c_int;
    pub fn getcwd(buf: *mut c_char, size: size_t) -> *mut c_char;
    pub fn chdir(dir: *const c_char) -> c_int;

    pub fn nl_langinfo(item: crate::nl_item) -> *mut c_char;
    pub fn nl_langinfo_l(item: crate::nl_item, loc: crate::locale_t) -> *mut c_char;

    pub fn select(
        nfds: c_int,
        readfds: *mut fd_set,
        writefds: *mut fd_set,
        errorfds: *mut fd_set,
        timeout: *const timeval,
    ) -> c_int;

    pub fn __wasilibc_register_preopened_fd(fd: c_int, path: *const c_char) -> c_int;
    pub fn __wasilibc_fd_renumber(fd: c_int, newfd: c_int) -> c_int;
    pub fn __wasilibc_unlinkat(fd: c_int, path: *const c_char) -> c_int;
    pub fn __wasilibc_rmdirat(fd: c_int, path: *const c_char) -> c_int;
    pub fn __wasilibc_find_relpath(
        path: *const c_char,
        abs_prefix: *mut *const c_char,
        relative_path: *mut *mut c_char,
        relative_path_len: usize,
    ) -> c_int;
    pub fn __wasilibc_tell(fd: c_int) -> off_t;
    pub fn __wasilibc_nocwd___wasilibc_unlinkat(dirfd: c_int, path: *const c_char) -> c_int;
    pub fn __wasilibc_nocwd___wasilibc_rmdirat(dirfd: c_int, path: *const c_char) -> c_int;
    pub fn __wasilibc_nocwd_linkat(
        olddirfd: c_int,
        oldpath: *const c_char,
        newdirfd: c_int,
        newpath: *const c_char,
        flags: c_int,
    ) -> c_int;
    pub fn __wasilibc_nocwd_symlinkat(
        target: *const c_char,
        dirfd: c_int,
        path: *const c_char,
    ) -> c_int;
    pub fn __wasilibc_nocwd_readlinkat(
        dirfd: c_int,
        path: *const c_char,
        buf: *mut c_char,
        bufsize: usize,
    ) -> isize;
    pub fn __wasilibc_nocwd_faccessat(
        dirfd: c_int,
        path: *const c_char,
        mode: c_int,
        flags: c_int,
    ) -> c_int;
    pub fn __wasilibc_nocwd_renameat(
        olddirfd: c_int,
        oldpath: *const c_char,
        newdirfd: c_int,
        newpath: *const c_char,
    ) -> c_int;
    pub fn __wasilibc_nocwd_openat_nomode(dirfd: c_int, path: *const c_char, flags: c_int)
        -> c_int;
    pub fn __wasilibc_nocwd_fstatat(
        dirfd: c_int,
        path: *const c_char,
        buf: *mut stat,
        flags: c_int,
    ) -> c_int;
    pub fn __wasilibc_nocwd_mkdirat_nomode(dirfd: c_int, path: *const c_char) -> c_int;
    pub fn __wasilibc_nocwd_utimensat(
        dirfd: c_int,
        path: *const c_char,
        times: *const crate::timespec,
        flags: c_int,
    ) -> c_int;
    pub fn __wasilibc_nocwd_opendirat(dirfd: c_int, path: *const c_char) -> *mut crate::DIR;
    pub fn __wasilibc_access(pathname: *const c_char, mode: c_int, flags: c_int) -> c_int;
    pub fn __wasilibc_stat(pathname: *const c_char, buf: *mut stat, flags: c_int) -> c_int;
    pub fn __wasilibc_utimens(
        pathname: *const c_char,
        times: *const crate::timespec,
        flags: c_int,
    ) -> c_int;
    pub fn __wasilibc_link(oldpath: *const c_char, newpath: *const c_char, flags: c_int) -> c_int;
    pub fn __wasilibc_link_oldat(
        olddirfd: c_int,
        oldpath: *const c_char,
        newpath: *const c_char,
        flags: c_int,
    ) -> c_int;
    pub fn __wasilibc_link_newat(
        oldpath: *const c_char,
        newdirfd: c_int,
        newpath: *const c_char,
        flags: c_int,
    ) -> c_int;
    pub fn __wasilibc_rename_oldat(
        olddirfd: c_int,
        oldpath: *const c_char,
        newpath: *const c_char,
    ) -> c_int;
    pub fn __wasilibc_rename_newat(
        oldpath: *const c_char,
        newdirfd: c_int,
        newpath: *const c_char,
    ) -> c_int;

    pub fn arc4random() -> u32;
    pub fn arc4random_buf(a: *mut c_void, b: size_t);
    pub fn arc4random_uniform(a: u32) -> u32;

    pub fn __errno_location() -> *mut c_int;
}

cfg_if! {
    if #[cfg(target_env = "p2")] {
        mod p2;
        pub use self::p2::*;
    }
}
