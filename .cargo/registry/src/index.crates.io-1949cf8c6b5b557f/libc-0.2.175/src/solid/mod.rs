//! Interface to the [SOLID] C library
//!
//! [SOLID]: https://solid.kmckk.com/

use crate::prelude::*;

pub type intmax_t = i64;
pub type uintmax_t = u64;

pub type uintptr_t = usize;
pub type intptr_t = isize;
pub type ptrdiff_t = isize;
pub type size_t = crate::uintptr_t;
pub type ssize_t = intptr_t;

pub type clock_t = c_uint;
pub type time_t = i64;
pub type clockid_t = c_int;
pub type timer_t = c_int;
pub type suseconds_t = c_int;
pub type useconds_t = c_uint;

pub type sighandler_t = size_t;

// sys/ansi.h
pub type __caddr_t = *mut c_char;
pub type __gid_t = u32;
pub type __in_addr_t = u32;
pub type __in_port_t = u16;
pub type __mode_t = u32;
pub type __off_t = i64;
pub type __pid_t = i32;
pub type __sa_family_t = u8;
pub type __socklen_t = c_uint;
pub type __uid_t = u32;
pub type __fsblkcnt_t = u64;
pub type __fsfilcnt_t = u64;

// locale.h
pub type locale_t = usize;

// nl_types.h
pub type nl_item = c_long;

// sys/types.h
pub type __va_list = *mut c_char;
pub type u_int8_t = u8;
pub type u_int16_t = u16;
pub type u_int32_t = u32;
pub type u_int64_t = u64;
pub type u_char = c_uchar;
pub type u_short = c_ushort;
pub type u_int = c_uint;
pub type u_long = c_ulong;
pub type unchar = c_uchar;
pub type ushort = c_ushort;
pub type uint = c_uint;
pub type ulong = c_ulong;
pub type u_quad_t = u64;
pub type quad_t = i64;
pub type qaddr_t = *mut quad_t;
pub type longlong_t = i64;
pub type u_longlong_t = u64;
pub type blkcnt_t = i64;
pub type blksize_t = i32;
pub type fsblkcnt_t = __fsblkcnt_t;
pub type fsfilcnt_t = __fsfilcnt_t;
pub type caddr_t = __caddr_t;
pub type daddr_t = i64;
pub type dev_t = u64;
pub type fixpt_t = u32;
pub type gid_t = __gid_t;
pub type idtype_t = c_int;
pub type id_t = u32;
pub type ino_t = u64;
pub type key_t = c_long;
pub type mode_t = __mode_t;
pub type nlink_t = u32;
pub type off_t = __off_t;
pub type pid_t = __pid_t;
pub type lwpid_t = i32;
pub type rlim_t = u64;
pub type segsz_t = i32;
pub type swblk_t = i32;
pub type mqd_t = c_int;
pub type cpuid_t = c_ulong;
pub type psetid_t = c_int;

s! {
    // stat.h
    pub struct stat {
        pub st_dev: dev_t,
        pub st_ino: ino_t,
        pub st_mode: c_short,
        pub st_nlink: c_short,
        pub st_uid: c_short,
        pub st_gid: c_short,
        pub st_rdev: dev_t,
        pub st_size: off_t,
        pub st_atime: time_t,
        pub st_mtime: time_t,
        pub st_ctime: time_t,
        pub st_blksize: blksize_t,
    }

    // time.h
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

    // stdlib.h
    pub struct qdiv_t {
        pub quot: quad_t,
        pub rem: quad_t,
    }
    pub struct lldiv_t {
        pub quot: c_longlong,
        pub rem: c_longlong,
    }
    pub struct div_t {
        pub quot: c_int,
        pub rem: c_int,
    }
    pub struct ldiv_t {
        pub quot: c_long,
        pub rem: c_long,
    }

    // locale.h
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
        pub int_n_cs_precedes: c_char,
        pub int_p_sep_by_space: c_char,
        pub int_n_sep_by_space: c_char,
        pub int_p_sign_posn: c_char,
        pub int_n_sign_posn: c_char,
    }

    pub struct iovec {
        pub iov_base: *mut c_void,
        pub iov_len: size_t,
    }

    pub struct timeval {
        pub tv_sec: c_long,
        pub tv_usec: c_long,
    }
}

pub const INT_MIN: c_int = -2147483648;
pub const INT_MAX: c_int = 2147483647;

pub const EXIT_FAILURE: c_int = 1;
pub const EXIT_SUCCESS: c_int = 0;
pub const RAND_MAX: c_int = 0x7fffffff;
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

pub const O_RDONLY: c_int = 1;
pub const O_WRONLY: c_int = 2;
pub const O_RDWR: c_int = 4;
pub const O_APPEND: c_int = 8;
pub const O_CREAT: c_int = 0x10;
pub const O_EXCL: c_int = 0x400;
pub const O_TEXT: c_int = 0x100;
pub const O_BINARY: c_int = 0x200;
pub const O_TRUNC: c_int = 0x20;
pub const S_IEXEC: c_short = 0o0100;
pub const S_IWRITE: c_short = 0o0200;
pub const S_IREAD: c_short = 0o0400;
pub const S_IFCHR: c_short = 0o2_0000;
pub const S_IFDIR: c_short = 0o4_0000;
pub const S_IFMT: c_short = 0o16_0000;
pub const S_IFIFO: c_short = 0o1_0000;
pub const S_IFBLK: c_short = 0o6_0000;
pub const S_IFREG: c_short = 0o10_0000;

pub const LC_ALL: c_int = 0;
pub const LC_COLLATE: c_int = 1;
pub const LC_CTYPE: c_int = 2;
pub const LC_MONETARY: c_int = 3;
pub const LC_NUMERIC: c_int = 4;
pub const LC_TIME: c_int = 5;
pub const LC_MESSAGES: c_int = 6;
pub const _LC_LAST: c_int = 7;

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

pub const EDEADLK: c_int = 35;
pub const ENAMETOOLONG: c_int = 36;
pub const ENOLCK: c_int = 37;
pub const ENOSYS: c_int = 38;
pub const ENOTEMPTY: c_int = 39;
pub const ELOOP: c_int = 40;
pub const EWOULDBLOCK: c_int = EAGAIN;
pub const ENOMSG: c_int = 42;
pub const EIDRM: c_int = 43;
pub const ECHRNG: c_int = 44;
pub const EL2NSYNC: c_int = 45;
pub const EL3HLT: c_int = 46;
pub const EL3RST: c_int = 47;
pub const ELNRNG: c_int = 48;
pub const EUNATCH: c_int = 49;
pub const ENOCSI: c_int = 50;
pub const EL2HLT: c_int = 51;
pub const EBADE: c_int = 52;
pub const EBADR: c_int = 53;
pub const EXFULL: c_int = 54;
pub const ENOANO: c_int = 55;
pub const EBADRQC: c_int = 56;
pub const EBADSLT: c_int = 57;

pub const EDEADLOCK: c_int = EDEADLK;

pub const EBFONT: c_int = 59;
pub const ENOSTR: c_int = 60;
pub const ENODATA: c_int = 61;
pub const ETIME: c_int = 62;
pub const ENOSR: c_int = 63;
pub const ENONET: c_int = 64;
pub const ENOPKG: c_int = 65;
pub const EREMOTE: c_int = 66;
pub const ENOLINK: c_int = 67;
pub const EADV: c_int = 68;
pub const ESRMNT: c_int = 69;
pub const ECOMM: c_int = 70;
pub const EPROTO: c_int = 71;
pub const EMULTIHOP: c_int = 72;
pub const EDOTDOT: c_int = 73;
pub const EBADMSG: c_int = 74;
pub const EOVERFLOW: c_int = 75;
pub const ENOTUNIQ: c_int = 76;
pub const EBADFD: c_int = 77;
pub const EREMCHG: c_int = 78;
pub const ELIBACC: c_int = 79;
pub const ELIBBAD: c_int = 80;
pub const ELIBSCN: c_int = 81;
pub const ELIBMAX: c_int = 82;
pub const ELIBEXEC: c_int = 83;
pub const EILSEQ: c_int = 84;
pub const ERESTART: c_int = 85;
pub const ESTRPIPE: c_int = 86;
pub const EUSERS: c_int = 87;
pub const ENOTSOCK: c_int = 88;
pub const EDESTADDRREQ: c_int = 89;
pub const EMSGSIZE: c_int = 90;
pub const EPROTOTYPE: c_int = 91;
pub const ENOPROTOOPT: c_int = 92;
pub const EPROTONOSUPPORT: c_int = 93;
pub const ESOCKTNOSUPPORT: c_int = 94;
pub const EOPNOTSUPP: c_int = 95;
pub const EPFNOSUPPORT: c_int = 96;
pub const EAFNOSUPPORT: c_int = 97;
pub const EADDRINUSE: c_int = 98;
pub const EADDRNOTAVAIL: c_int = 99;
pub const ENETDOWN: c_int = 100;
pub const ENETUNREACH: c_int = 101;
pub const ENETRESET: c_int = 102;
pub const ECONNABORTED: c_int = 103;
pub const ECONNRESET: c_int = 104;
pub const ENOBUFS: c_int = 105;
pub const EISCONN: c_int = 106;
pub const ENOTCONN: c_int = 107;
pub const ESHUTDOWN: c_int = 108;
pub const ETOOMANYREFS: c_int = 109;
pub const ETIMEDOUT: c_int = 110;
pub const ECONNREFUSED: c_int = 111;
pub const EHOSTDOWN: c_int = 112;
pub const EHOSTUNREACH: c_int = 113;
pub const EALREADY: c_int = 114;
pub const EINPROGRESS: c_int = 115;
pub const ESTALE: c_int = 116;
pub const EUCLEAN: c_int = 117;
pub const ENOTNAM: c_int = 118;
pub const ENAVAIL: c_int = 119;
pub const EISNAM: c_int = 120;
pub const EREMOTEIO: c_int = 121;
pub const EDQUOT: c_int = 122;

pub const ENOMEDIUM: c_int = 123;
pub const EMEDIUMTYPE: c_int = 124;
pub const ECANCELED: c_int = 125;
pub const ENOKEY: c_int = 126;
pub const EKEYEXPIRED: c_int = 127;
pub const EKEYREVOKED: c_int = 128;
pub const EKEYREJECTED: c_int = 129;

pub const EOWNERDEAD: c_int = 130;
pub const ENOTRECOVERABLE: c_int = 131;

pub const ENOTSUP: c_int = 132;
pub const EFTYPE: c_int = 133;

// signal codes
pub const SIGHUP: c_int = 1;
pub const SIGINT: c_int = 2;
pub const SIGQUIT: c_int = 3;
pub const SIGILL: c_int = 4;
pub const SIGTRAP: c_int = 5;
pub const SIGABRT: c_int = 6;
pub const SIGIOT: c_int = SIGABRT;
pub const SIGEMT: c_int = 7;
pub const SIGFPE: c_int = 8;
pub const SIGKILL: c_int = 9;
pub const SIGBUS: c_int = 10;
pub const SIGSEGV: c_int = 11;
pub const SIGSYS: c_int = 12;
pub const SIGPIPE: c_int = 13;
pub const SIGALRM: c_int = 14;
pub const SIGTERM: c_int = 15;
pub const SIGURG: c_int = 16;
pub const SIGSTOP: c_int = 17;
pub const SIGTSTP: c_int = 18;
pub const SIGCONT: c_int = 19;
pub const SIGCHLD: c_int = 20;
pub const SIGTTIN: c_int = 21;
pub const SIGTTOU: c_int = 22;
pub const SIGIO: c_int = 23;
pub const SIGXCPU: c_int = 24;
pub const SIGXFSZ: c_int = 25;
pub const SIGVTALRM: c_int = 26;
pub const SIGPROF: c_int = 27;
pub const SIGWINCH: c_int = 28;
pub const SIGINFO: c_int = 29;
pub const SIGUSR1: c_int = 30;
pub const SIGUSR2: c_int = 31;
pub const SIGPWR: c_int = 32;

#[cfg_attr(feature = "extra_traits", derive(Debug))]
pub enum FILE {}
impl Copy for FILE {}
impl Clone for FILE {
    fn clone(&self) -> FILE {
        *self
    }
}
#[cfg_attr(feature = "extra_traits", derive(Debug))]
pub enum fpos_t {}
impl Copy for fpos_t {}
impl Clone for fpos_t {
    fn clone(&self) -> fpos_t {
        *self
    }
}

extern "C" {
    // ctype.h
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

    // stdio.h
    pub fn __get_stdio_file(fileno: c_int) -> *mut FILE;
    pub fn clearerr(arg1: *mut FILE);
    pub fn fclose(arg1: *mut FILE) -> c_int;
    pub fn feof(arg1: *mut FILE) -> c_int;
    pub fn ferror(arg1: *mut FILE) -> c_int;
    pub fn fflush(arg1: *mut FILE) -> c_int;
    pub fn fgetc(arg1: *mut FILE) -> c_int;
    pub fn fgets(arg1: *mut c_char, arg2: c_int, arg3: *mut FILE) -> *mut c_char;
    pub fn fopen(arg1: *const c_char, arg2: *const c_char) -> *mut FILE;
    pub fn fprintf(arg1: *mut FILE, arg2: *const c_char, ...) -> c_int;
    pub fn fputc(arg1: c_int, arg2: *mut FILE) -> c_int;
    pub fn fputs(arg1: *const c_char, arg2: *mut FILE) -> c_int;
    pub fn fread(arg1: *mut c_void, arg2: size_t, arg3: size_t, arg4: *mut FILE) -> size_t;
    pub fn freopen(arg1: *const c_char, arg2: *const c_char, arg3: *mut FILE) -> *mut FILE;
    pub fn fscanf(arg1: *mut FILE, arg2: *const c_char, ...) -> c_int;
    pub fn fseek(arg1: *mut FILE, arg2: c_long, arg3: c_int) -> c_int;
    pub fn ftell(arg1: *mut FILE) -> c_long;
    pub fn fwrite(arg1: *const c_void, arg2: size_t, arg3: size_t, arg4: *mut FILE) -> size_t;
    pub fn getc(arg1: *mut FILE) -> c_int;
    pub fn getchar() -> c_int;
    pub fn perror(arg1: *const c_char);
    pub fn printf(arg1: *const c_char, ...) -> c_int;
    pub fn putc(arg1: c_int, arg2: *mut FILE) -> c_int;
    pub fn putchar(arg1: c_int) -> c_int;
    pub fn puts(arg1: *const c_char) -> c_int;
    pub fn remove(arg1: *const c_char) -> c_int;
    pub fn rewind(arg1: *mut FILE);
    pub fn scanf(arg1: *const c_char, ...) -> c_int;
    pub fn setbuf(arg1: *mut FILE, arg2: *mut c_char);
    pub fn setvbuf(arg1: *mut FILE, arg2: *mut c_char, arg3: c_int, arg4: size_t) -> c_int;
    pub fn sscanf(arg1: *const c_char, arg2: *const c_char, ...) -> c_int;
    pub fn tmpfile() -> *mut FILE;
    pub fn ungetc(arg1: c_int, arg2: *mut FILE) -> c_int;
    pub fn vfprintf(arg1: *mut FILE, arg2: *const c_char, arg3: __va_list) -> c_int;
    pub fn vprintf(arg1: *const c_char, arg2: __va_list) -> c_int;
    pub fn gets(arg1: *mut c_char) -> *mut c_char;
    pub fn sprintf(arg1: *mut c_char, arg2: *const c_char, ...) -> c_int;
    pub fn tmpnam(arg1: *const c_char) -> *mut c_char;
    pub fn vsprintf(arg1: *mut c_char, arg2: *const c_char, arg3: __va_list) -> c_int;
    pub fn rename(arg1: *const c_char, arg2: *const c_char) -> c_int;
    pub fn asiprintf(arg1: *mut *mut c_char, arg2: *const c_char, ...) -> c_int;
    pub fn fiprintf(arg1: *mut FILE, arg2: *const c_char, ...) -> c_int;
    pub fn fiscanf(arg1: *mut FILE, arg2: *const c_char, ...) -> c_int;
    pub fn iprintf(arg1: *const c_char, ...) -> c_int;
    pub fn iscanf(arg1: *const c_char, ...) -> c_int;
    pub fn siprintf(arg1: *mut c_char, arg2: *const c_char, ...) -> c_int;
    pub fn siscanf(arg1: *mut c_char, arg2: *const c_char, ...) -> c_int;
    pub fn sniprintf(arg1: *mut c_char, arg2: size_t, arg3: *const c_char, ...) -> c_int;
    pub fn vasiprintf(arg1: *mut *mut c_char, arg2: *const c_char, arg3: __va_list) -> c_int;
    pub fn vfiprintf(arg1: *mut FILE, arg2: *const c_char, arg3: __va_list) -> c_int;
    pub fn vfiscanf(arg1: *mut FILE, arg2: *const c_char, arg3: __va_list) -> c_int;
    pub fn viprintf(arg1: *const c_char, arg2: __va_list) -> c_int;
    pub fn viscanf(arg1: *const c_char, arg2: __va_list) -> c_int;
    pub fn vsiprintf(arg1: *mut c_char, arg2: *const c_char, arg3: __va_list) -> c_int;
    pub fn vsiscanf(arg1: *const c_char, arg2: *const c_char, arg3: __va_list) -> c_int;
    pub fn vsniprintf(
        arg1: *mut c_char,
        arg2: size_t,
        arg3: *const c_char,
        arg4: __va_list,
    ) -> c_int;
    pub fn vdiprintf(arg1: c_int, arg2: *const c_char, arg3: __va_list) -> c_int;
    pub fn diprintf(arg1: c_int, arg2: *const c_char, ...) -> c_int;
    pub fn fgetpos(arg1: *mut FILE, arg2: *mut fpos_t) -> c_int;
    pub fn fsetpos(arg1: *mut FILE, arg2: *const fpos_t) -> c_int;
    pub fn fdopen(arg1: c_int, arg2: *const c_char) -> *mut FILE;
    pub fn fileno(arg1: *mut FILE) -> c_int;
    pub fn flockfile(arg1: *mut FILE);
    pub fn ftrylockfile(arg1: *mut FILE) -> c_int;
    pub fn funlockfile(arg1: *mut FILE);
    pub fn getc_unlocked(arg1: *mut FILE) -> c_int;
    pub fn getchar_unlocked() -> c_int;
    pub fn putc_unlocked(arg1: c_int, arg2: *mut FILE) -> c_int;
    pub fn putchar_unlocked(arg1: c_int) -> c_int;
    pub fn snprintf(arg1: *mut c_char, arg2: size_t, arg3: *const c_char, ...) -> c_int;
    pub fn vsnprintf(
        arg1: *mut c_char,
        arg2: size_t,
        arg3: *const c_char,
        arg4: __va_list,
    ) -> c_int;
    pub fn getw(arg1: *mut FILE) -> c_int;
    pub fn putw(arg1: c_int, arg2: *mut FILE) -> c_int;
    pub fn tempnam(arg1: *const c_char, arg2: *const c_char) -> *mut c_char;
    pub fn fseeko(stream: *mut FILE, offset: off_t, whence: c_int) -> c_int;
    pub fn ftello(stream: *mut FILE) -> off_t;

    // stdlib.h
    pub fn atof(arg1: *const c_char) -> f64;
    pub fn strtod(arg1: *const c_char, arg2: *mut *mut c_char) -> f64;
    pub fn drand48() -> f64;
    pub fn erand48(arg1: *mut c_ushort) -> f64;
    pub fn strtof(arg1: *const c_char, arg2: *mut *mut c_char) -> f32;
    pub fn strtold(arg1: *const c_char, arg2: *mut *mut c_char) -> f64;
    pub fn strtod_l(arg1: *const c_char, arg2: *mut *mut c_char, arg3: locale_t) -> f64;
    pub fn strtof_l(arg1: *const c_char, arg2: *mut *mut c_char, arg3: locale_t) -> f32;
    pub fn strtold_l(arg1: *const c_char, arg2: *mut *mut c_char, arg3: locale_t) -> f64;
    pub fn _Exit(arg1: c_int) -> !;
    pub fn abort() -> !;
    pub fn abs(arg1: c_int) -> c_int;
    pub fn atexit(arg1: Option<unsafe extern "C" fn()>) -> c_int;
    pub fn atoi(arg1: *const c_char) -> c_int;
    pub fn atol(arg1: *const c_char) -> c_long;
    pub fn itoa(arg1: c_int, arg2: *mut c_char, arg3: c_int) -> *mut c_char;
    pub fn ltoa(arg1: c_long, arg2: *mut c_char, arg3: c_int) -> *mut c_char;
    pub fn ultoa(arg1: c_ulong, arg2: *mut c_char, arg3: c_int) -> *mut c_char;
    pub fn bsearch(
        arg1: *const c_void,
        arg2: *const c_void,
        arg3: size_t,
        arg4: size_t,
        arg5: Option<unsafe extern "C" fn(arg1: *const c_void, arg2: *const c_void) -> c_int>,
    ) -> *mut c_void;
    pub fn calloc(arg1: size_t, arg2: size_t) -> *mut c_void;
    pub fn div(arg1: c_int, arg2: c_int) -> div_t;
    pub fn exit(arg1: c_int) -> !;
    pub fn free(arg1: *mut c_void);
    pub fn getenv(arg1: *const c_char) -> *mut c_char;
    pub fn labs(arg1: c_long) -> c_long;
    pub fn ldiv(arg1: c_long, arg2: c_long) -> ldiv_t;
    pub fn malloc(arg1: size_t) -> *mut c_void;
    pub fn qsort(
        arg1: *mut c_void,
        arg2: size_t,
        arg3: size_t,
        arg4: Option<unsafe extern "C" fn(arg1: *const c_void, arg2: *const c_void) -> c_int>,
    );
    pub fn rand() -> c_int;
    pub fn realloc(arg1: *mut c_void, arg2: size_t) -> *mut c_void;
    pub fn srand(arg1: c_uint);
    pub fn strtol(arg1: *const c_char, arg2: *mut *mut c_char, arg3: c_int) -> c_long;
    pub fn strtoul(arg1: *const c_char, arg2: *mut *mut c_char, arg3: c_int) -> c_ulong;
    pub fn mblen(arg1: *const c_char, arg2: size_t) -> c_int;
    pub fn mbstowcs(arg1: *mut wchar_t, arg2: *const c_char, arg3: size_t) -> size_t;
    pub fn wctomb(arg1: *mut c_char, arg2: wchar_t) -> c_int;
    pub fn mbtowc(arg1: *mut wchar_t, arg2: *const c_char, arg3: size_t) -> c_int;
    pub fn wcstombs(arg1: *mut c_char, arg2: *const wchar_t, arg3: size_t) -> size_t;
    pub fn rand_r(arg1: *mut c_uint) -> c_int;
    pub fn jrand48(arg1: *mut c_ushort) -> c_long;
    pub fn lcong48(arg1: *mut c_ushort);
    pub fn lrand48() -> c_long;
    pub fn mrand48() -> c_long;
    pub fn nrand48(arg1: *mut c_ushort) -> c_long;
    pub fn seed48(arg1: *mut c_ushort) -> *mut c_ushort;
    pub fn srand48(arg1: c_long);
    pub fn putenv(arg1: *mut c_char) -> c_int;
    pub fn a64l(arg1: *const c_char) -> c_long;
    pub fn l64a(arg1: c_long) -> *mut c_char;
    pub fn random() -> c_long;
    pub fn setstate(arg1: *mut c_char) -> *mut c_char;
    pub fn initstate(arg1: c_uint, arg2: *mut c_char, arg3: size_t) -> *mut c_char;
    pub fn srandom(arg1: c_uint);
    pub fn mkostemp(arg1: *mut c_char, arg2: c_int) -> c_int;
    pub fn mkostemps(arg1: *mut c_char, arg2: c_int, arg3: c_int) -> c_int;
    pub fn mkdtemp(arg1: *mut c_char) -> *mut c_char;
    pub fn mkstemp(arg1: *mut c_char) -> c_int;
    pub fn mktemp(arg1: *mut c_char) -> *mut c_char;
    pub fn atoll(arg1: *const c_char) -> c_longlong;
    pub fn llabs(arg1: c_longlong) -> c_longlong;
    pub fn lldiv(arg1: c_longlong, arg2: c_longlong) -> lldiv_t;
    pub fn strtoll(arg1: *const c_char, arg2: *mut *mut c_char, arg3: c_int) -> c_longlong;
    pub fn strtoull(arg1: *const c_char, arg2: *mut *mut c_char, arg3: c_int) -> c_ulonglong;
    pub fn aligned_alloc(arg1: size_t, arg2: size_t) -> *mut c_void;
    pub fn at_quick_exit(arg1: Option<unsafe extern "C" fn()>) -> c_int;
    pub fn quick_exit(arg1: c_int);
    pub fn setenv(arg1: *const c_char, arg2: *const c_char, arg3: c_int) -> c_int;
    pub fn unsetenv(arg1: *const c_char) -> c_int;
    pub fn humanize_number(
        arg1: *mut c_char,
        arg2: size_t,
        arg3: i64,
        arg4: *const c_char,
        arg5: c_int,
        arg6: c_int,
    ) -> c_int;
    pub fn dehumanize_number(arg1: *const c_char, arg2: *mut i64) -> c_int;
    pub fn getenv_r(arg1: *const c_char, arg2: *mut c_char, arg3: size_t) -> c_int;
    pub fn heapsort(
        arg1: *mut c_void,
        arg2: size_t,
        arg3: size_t,
        arg4: Option<unsafe extern "C" fn(arg1: *const c_void, arg2: *const c_void) -> c_int>,
    ) -> c_int;
    pub fn mergesort(
        arg1: *mut c_void,
        arg2: size_t,
        arg3: size_t,
        arg4: Option<unsafe extern "C" fn(arg1: *const c_void, arg2: *const c_void) -> c_int>,
    ) -> c_int;
    pub fn radixsort(
        arg1: *mut *const c_uchar,
        arg2: c_int,
        arg3: *const c_uchar,
        arg4: c_uint,
    ) -> c_int;
    pub fn sradixsort(
        arg1: *mut *const c_uchar,
        arg2: c_int,
        arg3: *const c_uchar,
        arg4: c_uint,
    ) -> c_int;
    pub fn getprogname() -> *const c_char;
    pub fn setprogname(arg1: *const c_char);
    pub fn qabs(arg1: quad_t) -> quad_t;
    pub fn strtoq(arg1: *const c_char, arg2: *mut *mut c_char, arg3: c_int) -> quad_t;
    pub fn strtouq(arg1: *const c_char, arg2: *mut *mut c_char, arg3: c_int) -> u_quad_t;
    pub fn strsuftoll(
        arg1: *const c_char,
        arg2: *const c_char,
        arg3: c_longlong,
        arg4: c_longlong,
    ) -> c_longlong;
    pub fn strsuftollx(
        arg1: *const c_char,
        arg2: *const c_char,
        arg3: c_longlong,
        arg4: c_longlong,
        arg5: *mut c_char,
        arg6: size_t,
    ) -> c_longlong;
    pub fn l64a_r(arg1: c_long, arg2: *mut c_char, arg3: c_int) -> c_int;
    pub fn qdiv(arg1: quad_t, arg2: quad_t) -> qdiv_t;
    pub fn strtol_l(
        arg1: *const c_char,
        arg2: *mut *mut c_char,
        arg3: c_int,
        arg4: locale_t,
    ) -> c_long;
    pub fn strtoul_l(
        arg1: *const c_char,
        arg2: *mut *mut c_char,
        arg3: c_int,
        arg4: locale_t,
    ) -> c_ulong;
    pub fn strtoll_l(
        arg1: *const c_char,
        arg2: *mut *mut c_char,
        arg3: c_int,
        arg4: locale_t,
    ) -> c_longlong;
    pub fn strtoull_l(
        arg1: *const c_char,
        arg2: *mut *mut c_char,
        arg3: c_int,
        arg4: locale_t,
    ) -> c_ulonglong;
    pub fn strtoq_l(
        arg1: *const c_char,
        arg2: *mut *mut c_char,
        arg3: c_int,
        arg4: locale_t,
    ) -> quad_t;
    pub fn strtouq_l(
        arg1: *const c_char,
        arg2: *mut *mut c_char,
        arg3: c_int,
        arg4: locale_t,
    ) -> u_quad_t;
    pub fn _mb_cur_max_l(arg1: locale_t) -> size_t;
    pub fn mblen_l(arg1: *const c_char, arg2: size_t, arg3: locale_t) -> c_int;
    pub fn mbstowcs_l(
        arg1: *mut wchar_t,
        arg2: *const c_char,
        arg3: size_t,
        arg4: locale_t,
    ) -> size_t;
    pub fn wctomb_l(arg1: *mut c_char, arg2: wchar_t, arg3: locale_t) -> c_int;
    pub fn mbtowc_l(arg1: *mut wchar_t, arg2: *const c_char, arg3: size_t, arg4: locale_t)
        -> c_int;
    pub fn wcstombs_l(
        arg1: *mut c_char,
        arg2: *const wchar_t,
        arg3: size_t,
        arg4: locale_t,
    ) -> size_t;

    // string.h
    pub fn memchr(arg1: *const c_void, arg2: c_int, arg3: size_t) -> *mut c_void;
    pub fn memcmp(arg1: *const c_void, arg2: *const c_void, arg3: size_t) -> c_int;
    pub fn memcpy(arg1: *mut c_void, arg2: *const c_void, arg3: size_t) -> *mut c_void;
    pub fn memmove(arg1: *mut c_void, arg2: *const c_void, arg3: size_t) -> *mut c_void;
    pub fn memset(arg1: *mut c_void, arg2: c_int, arg3: size_t) -> *mut c_void;
    pub fn strcat(arg1: *mut c_char, arg2: *const c_char) -> *mut c_char;
    pub fn strchr(arg1: *const c_char, arg2: c_int) -> *mut c_char;
    pub fn strcmp(arg1: *const c_char, arg2: *const c_char) -> c_int;
    pub fn strcoll(arg1: *const c_char, arg2: *const c_char) -> c_int;
    pub fn strcpy(arg1: *mut c_char, arg2: *const c_char) -> *mut c_char;
    pub fn strcspn(arg1: *const c_char, arg2: *const c_char) -> size_t;
    pub fn strerror(arg1: c_int) -> *mut c_char;
    pub fn strlen(arg1: *const c_char) -> size_t;
    pub fn strncat(arg1: *mut c_char, arg2: *const c_char, arg3: size_t) -> *mut c_char;
    pub fn strncmp(arg1: *const c_char, arg2: *const c_char, arg3: size_t) -> c_int;
    pub fn strncpy(arg1: *mut c_char, arg2: *const c_char, arg3: size_t) -> *mut c_char;
    pub fn strpbrk(arg1: *const c_char, arg2: *const c_char) -> *mut c_char;
    pub fn strrchr(arg1: *const c_char, arg2: c_int) -> *mut c_char;
    pub fn strspn(arg1: *const c_char, arg2: *const c_char) -> size_t;
    pub fn strstr(arg1: *const c_char, arg2: *const c_char) -> *mut c_char;
    pub fn strtok(arg1: *mut c_char, arg2: *const c_char) -> *mut c_char;
    pub fn strtok_r(arg1: *mut c_char, arg2: *const c_char, arg3: *mut *mut c_char) -> *mut c_char;
    pub fn strerror_r(arg1: c_int, arg2: *mut c_char, arg3: size_t) -> c_int;
    pub fn strxfrm(arg1: *mut c_char, arg2: *const c_char, arg3: size_t) -> size_t;
    pub fn memccpy(
        arg1: *mut c_void,
        arg2: *const c_void,
        arg3: c_int,
        arg4: size_t,
    ) -> *mut c_void;
    pub fn strdup(arg1: *const c_char) -> *mut c_char;
    pub fn stpcpy(arg1: *mut c_char, arg2: *const c_char) -> *mut c_char;
    pub fn stpncpy(arg1: *mut c_char, arg2: *const c_char, arg3: size_t) -> *mut c_char;
    pub fn strnlen(arg1: *const c_char, arg2: size_t) -> size_t;
    pub fn memmem(
        arg1: *const c_void,
        arg2: size_t,
        arg3: *const c_void,
        arg4: size_t,
    ) -> *mut c_void;
    pub fn strcasestr(arg1: *const c_char, arg2: *const c_char) -> *mut c_char;
    pub fn strlcat(arg1: *mut c_char, arg2: *const c_char, arg3: size_t) -> size_t;
    pub fn strlcpy(arg1: *mut c_char, arg2: *const c_char, arg3: size_t) -> size_t;
    pub fn strsep(arg1: *mut *mut c_char, arg2: *const c_char) -> *mut c_char;
    pub fn stresep(arg1: *mut *mut c_char, arg2: *const c_char, arg3: c_int) -> *mut c_char;
    pub fn strndup(arg1: *const c_char, arg2: size_t) -> *mut c_char;
    pub fn memrchr(arg1: *const c_void, arg2: c_int, arg3: size_t) -> *mut c_void;
    pub fn explicit_memset(arg1: *mut c_void, arg2: c_int, arg3: size_t) -> *mut c_void;
    pub fn consttime_memequal(arg1: *const c_void, arg2: *const c_void, arg3: size_t) -> c_int;
    pub fn strcoll_l(arg1: *const c_char, arg2: *const c_char, arg3: locale_t) -> c_int;
    pub fn strxfrm_l(
        arg1: *mut c_char,
        arg2: *const c_char,
        arg3: size_t,
        arg4: locale_t,
    ) -> size_t;
    pub fn strerror_l(arg1: c_int, arg2: locale_t) -> *mut c_char;

    // strings.h
    pub fn bcmp(arg1: *const c_void, arg2: *const c_void, arg3: size_t) -> c_int;
    pub fn bcopy(arg1: *const c_void, arg2: *mut c_void, arg3: size_t);
    pub fn bzero(arg1: *mut c_void, arg2: size_t);
    pub fn ffs(arg1: c_int) -> c_int;
    pub fn popcount(arg1: c_uint) -> c_uint;
    pub fn popcountl(arg1: c_ulong) -> c_uint;
    pub fn popcountll(arg1: c_ulonglong) -> c_uint;
    pub fn popcount32(arg1: u32) -> c_uint;
    pub fn popcount64(arg1: u64) -> c_uint;
    pub fn rindex(arg1: *const c_char, arg2: c_int) -> *mut c_char;
    pub fn strcasecmp(arg1: *const c_char, arg2: *const c_char) -> c_int;
    pub fn strncasecmp(arg1: *const c_char, arg2: *const c_char, arg3: size_t) -> c_int;

    // signal.h
    pub fn signal(arg1: c_int, arg2: sighandler_t) -> sighandler_t;
    pub fn raise(arg1: c_int) -> c_int;

    // time.h
    pub fn asctime(arg1: *const tm) -> *mut c_char;
    pub fn clock() -> clock_t;
    pub fn ctime(arg1: *const time_t) -> *mut c_char;
    pub fn difftime(arg1: time_t, arg2: time_t) -> f64;
    pub fn gmtime(arg1: *const time_t) -> *mut tm;
    pub fn localtime(arg1: *const time_t) -> *mut tm;
    pub fn time(arg1: *mut time_t) -> time_t;
    pub fn mktime(arg1: *mut tm) -> time_t;
    pub fn strftime(
        arg1: *mut c_char,
        arg2: size_t,
        arg3: *const c_char,
        arg4: *const tm,
    ) -> size_t;
    pub fn utime(arg1: *const c_char, arg2: *mut time_t) -> c_int;
    pub fn asctime_r(arg1: *const tm, arg2: *mut c_char) -> *mut c_char;
    pub fn ctime_r(arg1: *const time_t, arg2: *mut c_char) -> *mut c_char;
    pub fn gmtime_r(arg1: *const time_t, arg2: *mut tm) -> *mut tm;
    pub fn localtime_r(arg1: *const time_t, arg2: *mut tm) -> *mut tm;

    // sys/stat.h
    pub fn stat(arg1: *const c_char, arg2: *mut stat) -> c_int;
    pub fn lstat(arg1: *const c_char, arg2: *mut stat) -> c_int;
    pub fn fstat(arg1: c_int, arg2: *mut stat) -> c_int;
    pub fn chmod(arg1: *const c_char, arg2: __mode_t) -> c_int;
    pub fn mkdir(arg1: *const c_char, arg2: __mode_t) -> c_int;

    // fcntl.h
    pub fn open(arg1: *const c_char, arg2: c_int, ...) -> c_int;
    pub fn creat(arg1: *const c_char, arg2: c_int) -> c_int;
    pub fn close(arg1: c_int) -> c_int;
    pub fn read(arg1: c_int, arg2: *mut c_void, arg3: c_int) -> c_int;
    pub fn write(arg1: c_int, arg2: *const c_void, arg3: c_int) -> c_int;
    pub fn unlink(arg1: *const c_char) -> c_int;
    pub fn tell(arg1: c_int) -> c_long;
    pub fn dup(arg1: c_int) -> c_int;
    pub fn dup2(arg1: c_int, arg2: c_int) -> c_int;
    pub fn access(arg1: *const c_char, arg2: c_int) -> c_int;
    pub fn rmdir(arg1: *const c_char) -> c_int;
    pub fn chdir(arg1: *const c_char) -> c_int;
    pub fn _exit(arg1: c_int);
    pub fn getwd(arg1: *mut c_char) -> *mut c_char;
    pub fn getcwd(arg1: *mut c_char, arg2: size_t) -> *mut c_char;
    pub static mut optarg: *mut c_char;
    pub static mut opterr: c_int;
    pub static mut optind: c_int;
    pub static mut optopt: c_int;
    pub static mut optreset: c_int;
    pub fn getopt(arg1: c_int, arg2: *mut *mut c_char, arg3: *const c_char) -> c_int;
    pub static mut suboptarg: *mut c_char;
    pub fn getsubopt(
        arg1: *mut *mut c_char,
        arg2: *const *mut c_char,
        arg3: *mut *mut c_char,
    ) -> c_int;
    pub fn fcntl(arg1: c_int, arg2: c_int, ...) -> c_int;
    pub fn getpid() -> pid_t;
    pub fn sleep(arg1: c_uint) -> c_uint;
    pub fn usleep(arg1: useconds_t) -> c_int;

    // locale.h
    pub fn localeconv() -> *mut lconv;
    pub fn setlocale(arg1: c_int, arg2: *const c_char) -> *mut c_char;
    pub fn duplocale(arg1: locale_t) -> locale_t;
    pub fn freelocale(arg1: locale_t);
    pub fn localeconv_l(arg1: locale_t) -> *mut lconv;
    pub fn newlocale(arg1: c_int, arg2: *const c_char, arg3: locale_t) -> locale_t;

    // langinfo.h
    pub fn nl_langinfo(item: crate::nl_item) -> *mut c_char;
    pub fn nl_langinfo_l(item: crate::nl_item, locale: locale_t) -> *mut c_char;

    // malloc.h
    pub fn memalign(align: size_t, size: size_t) -> *mut c_void;

    // sys/types.h
    pub fn lseek(arg1: c_int, arg2: __off_t, arg3: c_int) -> __off_t;
}

cfg_if! {
    if #[cfg(target_arch = "aarch64")] {
        mod aarch64;
        pub use self::aarch64::*;
    } else if #[cfg(any(target_arch = "arm"))] {
        mod arm;
        pub use self::arm::*;
    } else {
        // Unknown target_arch
    }
}
