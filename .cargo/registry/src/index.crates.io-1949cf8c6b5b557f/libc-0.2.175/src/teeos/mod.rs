//! Libc bindings for teeos
//!
//! Apparently the loader just dynamically links it anyway, but fails
//! when linking is explicitly requested.
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

use crate::prelude::*;

pub type c_bool = i32;

pub type intmax_t = i64;

pub type uintmax_t = u64;

pub type size_t = usize;

pub type ptrdiff_t = isize;

pub type intptr_t = isize;

pub type uintptr_t = usize;

pub type ssize_t = isize;

pub type pid_t = c_int;

pub type wchar_t = u32;

// long double in C means A float point value, which has 128bit length.
// but some bit maybe not used, so the real length of long double could be 80(x86) or 128(power pc/IEEE)
// this is different from f128(not stable and not included default) in Rust, so we use u128 for FFI(Rust to C).
// this is unstable and will cause to memfault/data abort.
pub type c_longdouble = _CLongDouble;

pub type pthread_t = c_ulong;

pub type pthread_key_t = c_uint;

pub type pthread_spinlock_t = c_int;

pub type off_t = i64;

pub type time_t = c_long;

pub type clock_t = c_long;

pub type clockid_t = c_int;

pub type suseconds_t = c_long;

pub type once_fn = extern "C" fn() -> c_void;

pub type pthread_once_t = c_int;

pub type va_list = *mut c_char;

pub type wint_t = c_uint;

pub type wctype_t = c_ulong;

pub type cmpfunc = extern "C" fn(x: *const c_void, y: *const c_void) -> c_int;

#[repr(align(16))]
pub struct _CLongDouble(pub u128);

#[repr(align(8))]
#[repr(C)]
pub struct pthread_cond_t {
    #[doc(hidden)]
    size: [u8; __SIZEOF_PTHREAD_COND_T],
}

#[repr(align(8))]
#[repr(C)]
pub struct pthread_mutex_t {
    #[doc(hidden)]
    size: [u8; __SIZEOF_PTHREAD_MUTEX_T],
}

#[repr(align(4))]
#[repr(C)]
pub struct pthread_mutexattr_t {
    #[doc(hidden)]
    size: [u8; __SIZEOF_PTHREAD_MUTEXATTR_T],
}

#[repr(align(4))]
#[repr(C)]
pub struct pthread_condattr_t {
    #[doc(hidden)]
    size: [u8; __SIZEOF_PTHREAD_CONDATTR_T],
}

#[repr(C)]
pub struct pthread_attr_t {
    __size: [u64; 7],
}

#[repr(C)]
pub struct cpu_set_t {
    bits: [c_ulong; 128 / size_of::<c_ulong>()],
}

#[repr(C)]
pub struct timespec {
    pub tv_sec: time_t,
    pub tv_nsec: c_long,
}

#[repr(C)]
pub struct timeval {
    pub tv_sec: time_t,
    pub tv_usec: suseconds_t,
}

#[repr(C)]
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
    pub __tm_gmtoff: c_long,
    pub __tm_zone: *const c_char,
}

#[repr(C)]
pub struct mbstate_t {
    pub __opaque1: c_uint,
    pub __opaque2: c_uint,
}

#[repr(C)]
pub struct sem_t {
    pub __val: [c_int; 4 * size_of::<c_long>() / size_of::<c_int>()],
}

#[repr(C)]
pub struct div_t {
    pub quot: c_int,
    pub rem: c_int,
}

// fcntl
pub const O_CREAT: u32 = 0o100;

pub const O_EXCL: u32 = 0o200;

pub const O_NOCTTY: u32 = 0o400;

pub const O_TRUNC: u32 = 0o1000;

pub const O_APPEND: u32 = 0o2000;

pub const O_NONBLOCK: u32 = 0o4000;

pub const O_DSYNC: u32 = 0o10000;

pub const O_SYNC: u32 = 0o4010000;

pub const O_RSYNC: u32 = 0o4010000;

pub const O_DIRECTORY: u32 = 0o200000;

pub const O_NOFOLLOW: u32 = 0o400000;

pub const O_CLOEXEC: u32 = 0o2000000;

pub const O_ASYNC: u32 = 0o20000;

pub const O_DIRECT: u32 = 0o40000;

pub const O_LARGEFILE: u32 = 0o100000;

pub const O_NOATIME: u32 = 0o1000000;

pub const O_PATH: u32 = 0o10000000;

pub const O_TMPFILE: u32 = 0o20200000;

pub const O_NDELAY: u32 = O_NONBLOCK;

pub const F_DUPFD: u32 = 0;

pub const F_GETFD: u32 = 1;

pub const F_SETFD: u32 = 2;

pub const F_GETFL: u32 = 3;

pub const F_SETFL: u32 = 4;

pub const F_SETOWN: u32 = 8;

pub const F_GETOWN: u32 = 9;

pub const F_SETSIG: u32 = 10;

pub const F_GETSIG: u32 = 11;

pub const F_GETLK: u32 = 12;

pub const F_SETLK: u32 = 13;

pub const F_SETLKW: u32 = 14;

pub const F_SETOWN_EX: u32 = 15;

pub const F_GETOWN_EX: u32 = 16;

pub const F_GETOWNER_UIDS: u32 = 17;

// mman
pub const MAP_FAILED: u64 = 0xffffffffffffffff;

pub const MAP_FIXED_NOREPLACE: u32 = 0x100000;

pub const MAP_SHARED_VALIDATE: u32 = 0x03;

pub const MAP_SHARED: u32 = 0x01;

pub const MAP_PRIVATE: u32 = 0x02;

pub const MAP_TYPE: u32 = 0x0f;

pub const MAP_FIXED: u32 = 0x10;

pub const MAP_ANON: u32 = 0x20;

pub const MAP_ANONYMOUS: u32 = MAP_ANON;

pub const MAP_NORESERVE: u32 = 0x4000;

pub const MAP_GROWSDOWN: u32 = 0x0100;

pub const MAP_DENYWRITE: u32 = 0x0800;

pub const MAP_EXECUTABLE: u32 = 0x1000;

pub const MAP_LOCKED: u32 = 0x2000;

pub const MAP_POPULATE: u32 = 0x8000;

pub const MAP_NONBLOCK: u32 = 0x10000;

pub const MAP_STACK: u32 = 0x20000;

pub const MAP_HUGETLB: u32 = 0x40000;

pub const MAP_SYNC: u32 = 0x80000;

pub const MAP_FILE: u32 = 0;

pub const MAP_HUGE_SHIFT: u32 = 26;

pub const MAP_HUGE_MASK: u32 = 0x3f;

pub const MAP_HUGE_16KB: u32 = 14 << 26;

pub const MAP_HUGE_64KB: u32 = 16 << 26;

pub const MAP_HUGE_512KB: u32 = 19 << 26;

pub const MAP_HUGE_1MB: u32 = 20 << 26;

pub const MAP_HUGE_2MB: u32 = 21 << 26;

pub const MAP_HUGE_8MB: u32 = 23 << 26;

pub const MAP_HUGE_16MB: u32 = 24 << 26;

pub const MAP_HUGE_32MB: u32 = 25 << 26;

pub const MAP_HUGE_256MB: u32 = 28 << 26;

pub const MAP_HUGE_512MB: u32 = 29 << 26;

pub const MAP_HUGE_1GB: u32 = 30 << 26;

pub const MAP_HUGE_2GB: u32 = 31 << 26;

pub const MAP_HUGE_16GB: u32 = 34u32 << 26;

pub const PROT_NONE: u32 = 0;

pub const PROT_READ: u32 = 1;

pub const PROT_WRITE: u32 = 2;

pub const PROT_EXEC: u32 = 4;

pub const PROT_GROWSDOWN: u32 = 0x01000000;

pub const PROT_GROWSUP: u32 = 0x02000000;

pub const MS_ASYNC: u32 = 1;

pub const MS_INVALIDATE: u32 = 2;

pub const MS_SYNC: u32 = 4;

pub const MCL_CURRENT: u32 = 1;

pub const MCL_FUTURE: u32 = 2;

pub const MCL_ONFAULT: u32 = 4;

pub const POSIX_MADV_NORMAL: u32 = 0;

pub const POSIX_MADV_RANDOM: u32 = 1;

pub const POSIX_MADV_SEQUENTIAL: u32 = 2;

pub const POSIX_MADV_WILLNEED: u32 = 3;

pub const POSIX_MADV_DONTNEED: u32 = 4;

// wctype
pub const WCTYPE_ALNUM: u64 = 1;

pub const WCTYPE_ALPHA: u64 = 2;

pub const WCTYPE_BLANK: u64 = 3;

pub const WCTYPE_CNTRL: u64 = 4;

pub const WCTYPE_DIGIT: u64 = 5;

pub const WCTYPE_GRAPH: u64 = 6;

pub const WCTYPE_LOWER: u64 = 7;

pub const WCTYPE_PRINT: u64 = 8;

pub const WCTYPE_PUNCT: u64 = 9;

pub const WCTYPE_SPACE: u64 = 10;

pub const WCTYPE_UPPER: u64 = 11;

pub const WCTYPE_XDIGIT: u64 = 12;

// locale
pub const LC_CTYPE: i32 = 0;

pub const LC_NUMERIC: i32 = 1;

pub const LC_TIME: i32 = 2;

pub const LC_COLLATE: i32 = 3;

pub const LC_MONETARY: i32 = 4;

pub const LC_MESSAGES: i32 = 5;

pub const LC_ALL: i32 = 6;

// pthread
pub const __SIZEOF_PTHREAD_COND_T: usize = 48;

pub const __SIZEOF_PTHREAD_MUTEX_T: usize = 40;

pub const __SIZEOF_PTHREAD_MUTEXATTR_T: usize = 4;

pub const __SIZEOF_PTHREAD_CONDATTR_T: usize = 4;

// errno.h
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

pub const EPROTONOSUPPOR: c_int = 93;

pub const ESOCKTNOSUPPOR: c_int = 94;

pub const EOPNOTSUPP: c_int = 95;

pub const ENOTSUP: c_int = EOPNOTSUPP;

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

pub const ERFKILL: c_int = 132;

pub const EHWPOISON: c_int = 133;

// pthread_attr.h
pub const TEESMP_THREAD_ATTR_CA_WILDCARD: c_int = 0;

pub const TEESMP_THREAD_ATTR_CA_INHERIT: c_int = -1;

pub const TEESMP_THREAD_ATTR_TASK_ID_INHERIT: c_int = -1;

pub const TEESMP_THREAD_ATTR_HAS_SHADOW: c_int = 0x1;

pub const TEESMP_THREAD_ATTR_NO_SHADOW: c_int = 0x0;

// unistd.h
pub const _SC_ARG_MAX: c_int = 0;

pub const _SC_CHILD_MAX: c_int = 1;

pub const _SC_CLK_TCK: c_int = 2;

pub const _SC_NGROUPS_MAX: c_int = 3;

pub const _SC_OPEN_MAX: c_int = 4;

pub const _SC_STREAM_MAX: c_int = 5;

pub const _SC_TZNAME_MAX: c_int = 6;

pub const _SC_JOB_CONTROL: c_int = 7;

pub const _SC_SAVED_IDS: c_int = 8;

pub const _SC_REALTIME_SIGNALS: c_int = 9;

pub const _SC_PRIORITY_SCHEDULING: c_int = 10;

pub const _SC_TIMERS: c_int = 11;

pub const _SC_ASYNCHRONOUS_IO: c_int = 12;

pub const _SC_PRIORITIZED_IO: c_int = 13;

pub const _SC_SYNCHRONIZED_IO: c_int = 14;

pub const _SC_FSYNC: c_int = 15;

pub const _SC_MAPPED_FILES: c_int = 16;

pub const _SC_MEMLOCK: c_int = 17;

pub const _SC_MEMLOCK_RANGE: c_int = 18;

pub const _SC_MEMORY_PROTECTION: c_int = 19;

pub const _SC_MESSAGE_PASSING: c_int = 20;

pub const _SC_SEMAPHORES: c_int = 21;

pub const _SC_SHARED_MEMORY_OBJECTS: c_int = 22;

pub const _SC_AIO_LISTIO_MAX: c_int = 23;

pub const _SC_AIO_MAX: c_int = 24;

pub const _SC_AIO_PRIO_DELTA_MAX: c_int = 25;

pub const _SC_DELAYTIMER_MAX: c_int = 26;

pub const _SC_MQ_OPEN_MAX: c_int = 27;

pub const _SC_MQ_PRIO_MAX: c_int = 28;

pub const _SC_VERSION: c_int = 29;

pub const _SC_PAGE_SIZE: c_int = 30;

pub const _SC_PAGESIZE: c_int = 30; /* !! */

pub const _SC_RTSIG_MAX: c_int = 31;

pub const _SC_SEM_NSEMS_MAX: c_int = 32;

pub const _SC_SEM_VALUE_MAX: c_int = 33;

pub const _SC_SIGQUEUE_MAX: c_int = 34;

pub const _SC_TIMER_MAX: c_int = 35;

pub const _SC_BC_BASE_MAX: c_int = 36;

pub const _SC_BC_DIM_MAX: c_int = 37;

pub const _SC_BC_SCALE_MAX: c_int = 38;

pub const _SC_BC_STRING_MAX: c_int = 39;

pub const _SC_COLL_WEIGHTS_MAX: c_int = 40;

pub const _SC_EXPR_NEST_MAX: c_int = 42;

pub const _SC_LINE_MAX: c_int = 43;

pub const _SC_RE_DUP_MAX: c_int = 44;

pub const _SC_2_VERSION: c_int = 46;

pub const _SC_2_C_BIND: c_int = 47;

pub const _SC_2_C_DEV: c_int = 48;

pub const _SC_2_FORT_DEV: c_int = 49;

pub const _SC_2_FORT_RUN: c_int = 50;

pub const _SC_2_SW_DEV: c_int = 51;

pub const _SC_2_LOCALEDEF: c_int = 52;

pub const _SC_UIO_MAXIOV: c_int = 60; /* !! */

pub const _SC_IOV_MAX: c_int = 60;

pub const _SC_THREADS: c_int = 67;

pub const _SC_THREAD_SAFE_FUNCTIONS: c_int = 68;

pub const _SC_GETGR_R_SIZE_MAX: c_int = 69;

pub const _SC_GETPW_R_SIZE_MAX: c_int = 70;

pub const _SC_LOGIN_NAME_MAX: c_int = 71;

pub const _SC_TTY_NAME_MAX: c_int = 72;

pub const _SC_THREAD_DESTRUCTOR_ITERATIONS: c_int = 73;

pub const _SC_THREAD_KEYS_MAX: c_int = 74;

pub const _SC_THREAD_STACK_MIN: c_int = 75;

pub const _SC_THREAD_THREADS_MAX: c_int = 76;

pub const _SC_THREAD_ATTR_STACKADDR: c_int = 77;

pub const _SC_THREAD_ATTR_STACKSIZE: c_int = 78;

pub const _SC_THREAD_PRIORITY_SCHEDULING: c_int = 79;

pub const _SC_THREAD_PRIO_INHERIT: c_int = 80;

pub const _SC_THREAD_PRIO_PROTECT: c_int = 81;

pub const _SC_THREAD_PROCESS_SHARED: c_int = 82;

pub const _SC_NPROCESSORS_CONF: c_int = 83;

pub const _SC_NPROCESSORS_ONLN: c_int = 84;

pub const _SC_PHYS_PAGES: c_int = 85;

pub const _SC_AVPHYS_PAGES: c_int = 86;

pub const _SC_ATEXIT_MAX: c_int = 87;

pub const _SC_PASS_MAX: c_int = 88;

pub const _SC_XOPEN_VERSION: c_int = 89;

pub const _SC_XOPEN_XCU_VERSION: c_int = 90;

pub const _SC_XOPEN_UNIX: c_int = 91;

pub const _SC_XOPEN_CRYPT: c_int = 92;

pub const _SC_XOPEN_ENH_I18N: c_int = 93;

pub const _SC_XOPEN_SHM: c_int = 94;

pub const _SC_2_CHAR_TERM: c_int = 95;

pub const _SC_2_UPE: c_int = 97;

pub const _SC_XOPEN_XPG2: c_int = 98;

pub const _SC_XOPEN_XPG3: c_int = 99;

pub const _SC_XOPEN_XPG4: c_int = 100;

pub const _SC_NZERO: c_int = 109;

pub const _SC_XBS5_ILP32_OFF32: c_int = 125;

pub const _SC_XBS5_ILP32_OFFBIG: c_int = 126;

pub const _SC_XBS5_LP64_OFF64: c_int = 127;

pub const _SC_XBS5_LPBIG_OFFBIG: c_int = 128;

pub const _SC_XOPEN_LEGACY: c_int = 129;

pub const _SC_XOPEN_REALTIME: c_int = 130;

pub const _SC_XOPEN_REALTIME_THREADS: c_int = 131;

pub const _SC_ADVISORY_INFO: c_int = 132;

pub const _SC_BARRIERS: c_int = 133;

pub const _SC_CLOCK_SELECTION: c_int = 137;

pub const _SC_CPUTIME: c_int = 138;

pub const _SC_THREAD_CPUTIME: c_int = 139;

pub const _SC_MONOTONIC_CLOCK: c_int = 149;

pub const _SC_READER_WRITER_LOCKS: c_int = 153;

pub const _SC_SPIN_LOCKS: c_int = 154;

pub const _SC_REGEXP: c_int = 155;

pub const _SC_SHELL: c_int = 157;

pub const _SC_SPAWN: c_int = 159;

pub const _SC_SPORADIC_SERVER: c_int = 160;

pub const _SC_THREAD_SPORADIC_SERVER: c_int = 161;

pub const _SC_TIMEOUTS: c_int = 164;

pub const _SC_TYPED_MEMORY_OBJECTS: c_int = 165;

pub const _SC_2_PBS: c_int = 168;

pub const _SC_2_PBS_ACCOUNTING: c_int = 169;

pub const _SC_2_PBS_LOCATE: c_int = 170;

pub const _SC_2_PBS_MESSAGE: c_int = 171;

pub const _SC_2_PBS_TRACK: c_int = 172;

pub const _SC_SYMLOOP_MAX: c_int = 173;

pub const _SC_STREAMS: c_int = 174;

pub const _SC_2_PBS_CHECKPOINT: c_int = 175;

pub const _SC_V6_ILP32_OFF32: c_int = 176;

pub const _SC_V6_ILP32_OFFBIG: c_int = 177;

pub const _SC_V6_LP64_OFF64: c_int = 178;

pub const _SC_V6_LPBIG_OFFBIG: c_int = 179;

pub const _SC_HOST_NAME_MAX: c_int = 180;

pub const _SC_TRACE: c_int = 181;

pub const _SC_TRACE_EVENT_FILTER: c_int = 182;

pub const _SC_TRACE_INHERIT: c_int = 183;

pub const _SC_TRACE_LOG: c_int = 184;

pub const _SC_IPV6: c_int = 235;

pub const _SC_RAW_SOCKETS: c_int = 236;

pub const _SC_V7_ILP32_OFF32: c_int = 237;

pub const _SC_V7_ILP32_OFFBIG: c_int = 238;

pub const _SC_V7_LP64_OFF64: c_int = 239;

pub const _SC_V7_LPBIG_OFFBIG: c_int = 240;

pub const _SC_SS_REPL_MAX: c_int = 241;

pub const _SC_TRACE_EVENT_NAME_MAX: c_int = 242;

pub const _SC_TRACE_NAME_MAX: c_int = 243;

pub const _SC_TRACE_SYS_MAX: c_int = 244;

pub const _SC_TRACE_USER_EVENT_MAX: c_int = 245;

pub const _SC_XOPEN_STREAMS: c_int = 246;

pub const _SC_THREAD_ROBUST_PRIO_INHERIT: c_int = 247;

pub const _SC_THREAD_ROBUST_PRIO_PROTECT: c_int = 248;

// limits.h
pub const PTHREAD_KEYS_MAX: c_int = 128;

pub const PTHREAD_STACK_MIN: c_int = 2048;

pub const PTHREAD_DESTRUCTOR_ITERATIONS: c_int = 4;

pub const SEM_VALUE_MAX: c_int = 0x7fffffff;

pub const SEM_NSEMS_MAX: c_int = 256;

pub const DELAYTIMER_MAX: c_int = 0x7fffffff;

pub const MQ_PRIO_MAX: c_int = 32768;

pub const LOGIN_NAME_MAX: c_int = 256;

// time.h
pub const CLOCK_REALTIME: clockid_t = 0;

pub const CLOCK_MONOTONIC: clockid_t = 1;

pub const PTHREAD_MUTEX_INITIALIZER: pthread_mutex_t = pthread_mutex_t {
    size: [0; __SIZEOF_PTHREAD_MUTEX_T],
};

pub const PTHREAD_COND_INITIALIZER: pthread_cond_t = pthread_cond_t {
    size: [0; __SIZEOF_PTHREAD_COND_T],
};

pub const PTHREAD_MUTEX_NORMAL: c_int = 0;

pub const PTHREAD_MUTEX_RECURSIVE: c_int = 1;

pub const PTHREAD_MUTEX_ERRORCHECK: c_int = 2;

pub const PTHREAD_MUTEX_DEFAULT: c_int = PTHREAD_MUTEX_NORMAL;

pub const PTHREAD_MUTEX_STALLED: c_int = 0;

pub const PTHREAD_MUTEX_ROBUST: c_int = 1;

extern "C" {
    // ---- ALLOC -----------------------------------------------------------------------------
    pub fn calloc(nobj: size_t, size: size_t) -> *mut c_void;

    pub fn malloc(size: size_t) -> *mut c_void;

    pub fn realloc(p: *mut c_void, size: size_t) -> *mut c_void;

    pub fn aligned_alloc(align: size_t, len: size_t) -> *mut c_void;

    pub fn free(p: *mut c_void);

    pub fn posix_memalign(memptr: *mut *mut c_void, align: size_t, size: size_t) -> c_int;

    pub fn memchr(cx: *const c_void, c: c_int, n: size_t) -> *mut c_void;

    pub fn wmemchr(cx: *const wchar_t, c: wchar_t, n: size_t) -> *mut wchar_t;

    pub fn memcmp(cx: *const c_void, ct: *const c_void, n: size_t) -> c_int;

    pub fn memcpy(dest: *mut c_void, src: *const c_void, n: size_t) -> *mut c_void;

    pub fn memmove(dest: *mut c_void, src: *const c_void, n: size_t) -> *mut c_void;

    pub fn memset(dest: *mut c_void, c: c_int, n: size_t) -> *mut c_void;

    // ----- PTHREAD ---------------------------------------------------------------------------
    pub fn pthread_self() -> pthread_t;

    pub fn pthread_join(native: pthread_t, value: *mut *mut c_void) -> c_int;

    // detach or pthread_attr_setdetachstate must not be called!
    //pub fn pthread_detach(thread: pthread_t) -> c_int;

    pub fn pthread_exit(value: *mut c_void) -> !;

    pub fn pthread_attr_init(attr: *mut pthread_attr_t) -> c_int;

    pub fn pthread_attr_destroy(attr: *mut pthread_attr_t) -> c_int;

    pub fn pthread_attr_getstack(
        attr: *const pthread_attr_t,
        stackaddr: *mut *mut c_void,
        stacksize: *mut size_t,
    ) -> c_int;

    pub fn pthread_attr_setstacksize(attr: *mut pthread_attr_t, stack_size: size_t) -> c_int;

    pub fn pthread_attr_getstacksize(attr: *const pthread_attr_t, size: *mut size_t) -> c_int;

    pub fn pthread_attr_settee(
        attr: *mut pthread_attr_t,
        ca: c_int,
        task_id: c_int,
        shadow: c_int,
    ) -> c_int;

    // C-TA API do not include this interface, but TA can use.
    pub fn sched_yield() -> c_int;

    pub fn pthread_key_create(
        key: *mut pthread_key_t,
        dtor: Option<unsafe extern "C" fn(*mut c_void)>,
    ) -> c_int;

    pub fn pthread_key_delete(key: pthread_key_t) -> c_int;

    pub fn pthread_getspecific(key: pthread_key_t) -> *mut c_void;

    pub fn pthread_setspecific(key: pthread_key_t, value: *const c_void) -> c_int;

    pub fn pthread_mutex_destroy(lock: *mut pthread_mutex_t) -> c_int;

    pub fn pthread_mutex_init(
        lock: *mut pthread_mutex_t,
        attr: *const pthread_mutexattr_t,
    ) -> c_int;

    pub fn pthread_mutex_lock(lock: *mut pthread_mutex_t) -> c_int;

    pub fn pthread_mutex_trylock(lock: *mut pthread_mutex_t) -> c_int;

    pub fn pthread_mutex_unlock(lock: *mut pthread_mutex_t) -> c_int;

    pub fn pthread_mutexattr_destroy(attr: *mut pthread_mutexattr_t) -> c_int;

    pub fn pthread_mutexattr_init(attr: *mut pthread_mutexattr_t) -> c_int;

    pub fn pthread_mutexattr_settype(attr: *mut pthread_mutexattr_t, _type: c_int) -> c_int;

    pub fn pthread_mutexattr_setpshared(attr: *mut pthread_mutexattr_t, pshared: c_int) -> c_int;

    pub fn pthread_cond_broadcast(cond: *mut pthread_cond_t) -> c_int;

    pub fn pthread_cond_destroy(cond: *mut pthread_cond_t) -> c_int;

    pub fn pthread_cond_init(cond: *mut pthread_cond_t, attr: *const pthread_condattr_t) -> c_int;

    pub fn pthread_cond_signal(cond: *mut pthread_cond_t) -> c_int;

    pub fn pthread_cond_wait(cond: *mut pthread_cond_t, lock: *mut pthread_mutex_t) -> c_int;

    pub fn pthread_cond_timedwait(
        cond: *mut pthread_cond_t,
        lock: *mut pthread_mutex_t,
        abstime: *const timespec,
    ) -> c_int;

    pub fn pthread_mutexattr_setrobust(attr: *mut pthread_mutexattr_t, robustness: c_int) -> c_int;

    pub fn pthread_create(
        native: *mut pthread_t,
        attr: *const pthread_attr_t,
        f: extern "C" fn(*mut c_void) -> *mut c_void,
        value: *mut c_void,
    ) -> c_int;

    pub fn pthread_spin_init(lock: *mut pthread_spinlock_t, pshared: c_int) -> c_int;

    pub fn pthread_spin_destroy(lock: *mut pthread_spinlock_t) -> c_int;

    pub fn pthread_spin_lock(lock: *mut pthread_spinlock_t) -> c_int;

    pub fn pthread_spin_trylock(lock: *mut pthread_spinlock_t) -> c_int;

    pub fn pthread_spin_unlock(lock: *mut pthread_spinlock_t) -> c_int;

    pub fn pthread_setschedprio(native: pthread_t, priority: c_int) -> c_int;

    pub fn pthread_once(pot: *mut pthread_once_t, f: Option<once_fn>) -> c_int;

    pub fn pthread_equal(p1: pthread_t, p2: pthread_t) -> c_int;

    pub fn pthread_mutexattr_setprotocol(a: *mut pthread_mutexattr_t, protocol: c_int) -> c_int;

    pub fn pthread_attr_setstack(
        attr: *mut pthread_attr_t,
        stack: *mut c_void,
        size: size_t,
    ) -> c_int;

    pub fn pthread_setaffinity_np(td: pthread_t, size: size_t, set: *const cpu_set_t) -> c_int;

    pub fn pthread_getaffinity_np(td: pthread_t, size: size_t, set: *mut cpu_set_t) -> c_int;

    // stdio.h
    pub fn printf(fmt: *const c_char, ...) -> c_int;

    pub fn scanf(fmt: *const c_char, ...) -> c_int;

    pub fn snprintf(s: *mut c_char, n: size_t, fmt: *const c_char, ...) -> c_int;

    pub fn sprintf(s: *mut c_char, fmt: *const c_char, ...) -> c_int;

    pub fn vsnprintf(s: *mut c_char, n: size_t, fmt: *const c_char, ap: va_list) -> c_int;

    pub fn vsprintf(s: *mut c_char, fmt: *const c_char, ap: va_list) -> c_int;

    // Not available.
    //pub fn pthread_setname_np(thread: pthread_t, name: *const c_char) -> c_int;

    pub fn abort() -> !;

    // Not available.
    //pub fn prctl(op: c_int, ...) -> c_int;

    pub fn sched_getaffinity(pid: pid_t, cpusetsize: size_t, cpuset: *mut cpu_set_t) -> c_int;

    pub fn sched_setaffinity(pid: pid_t, cpusetsize: size_t, cpuset: *const cpu_set_t) -> c_int;

    // sysconf is currently only implemented as a stub.
    pub fn sysconf(name: c_int) -> c_long;

    // mman.h
    pub fn mmap(
        addr: *mut c_void,
        len: size_t,
        prot: c_int,
        flags: c_int,
        fd: c_int,
        offset: off_t,
    ) -> *mut c_void;
    pub fn munmap(addr: *mut c_void, len: size_t) -> c_int;

    // errno.h
    pub fn __errno_location() -> *mut c_int;

    pub fn strerror(e: c_int) -> *mut c_char;

    // time.h
    pub fn clock_gettime(clock_id: clockid_t, tp: *mut timespec) -> c_int;

    // unistd
    pub fn getpid() -> pid_t;

    // time
    pub fn gettimeofday(tv: *mut timeval, tz: *mut c_void) -> c_int;

    pub fn strftime(
        restrict: *mut c_char,
        sz: size_t,
        _restrict: *const c_char,
        __restrict: *const tm,
    ) -> size_t;

    pub fn time(t: *mut time_t) -> time_t;

    // sem
    pub fn sem_close(sem: *mut sem_t) -> c_int;

    pub fn sem_destroy(sem: *mut sem_t) -> c_int;

    pub fn sem_getvalue(sem: *mut sem_t, valp: *mut c_int) -> c_int;

    pub fn sem_init(sem: *mut sem_t, pshared: c_int, value: c_uint) -> c_int;

    pub fn sem_open(name: *const c_char, flags: c_int, ...) -> *mut sem_t;

    pub fn sem_post(sem: *mut sem_t) -> c_int;

    pub fn sem_unlink(name: *const c_char) -> c_int;

    pub fn sem_wait(sem: *mut sem_t) -> c_int;

    // locale
    pub fn setlocale(cat: c_int, name: *const c_char) -> *mut c_char;

    pub fn strcoll(l: *const c_char, r: *const c_char) -> c_int;

    pub fn strxfrm(dest: *mut c_char, src: *const c_char, n: size_t) -> size_t;

    pub fn strtod(s: *const c_char, p: *mut *mut c_char) -> c_double;

    // multibyte
    pub fn mbrtowc(wc: *mut wchar_t, src: *const c_char, n: size_t, st: *mut mbstate_t) -> size_t;

    pub fn wcrtomb(s: *mut c_char, wc: wchar_t, st: *mut mbstate_t) -> size_t;

    pub fn wctob(c: wint_t) -> c_int;

    // prng
    pub fn srandom(seed: c_uint);

    pub fn initstate(seed: c_uint, state: *mut c_char, size: size_t) -> *mut c_char;

    pub fn setstate(state: *mut c_char) -> *mut c_char;

    pub fn random() -> c_long;

    // string
    pub fn strchr(s: *const c_char, c: c_int) -> *mut c_char;

    pub fn strlen(cs: *const c_char) -> size_t;

    pub fn strcmp(l: *const c_char, r: *const c_char) -> c_int;

    pub fn strcpy(dest: *mut c_char, src: *const c_char) -> *mut c_char;

    pub fn strncmp(_l: *const c_char, r: *const c_char, n: size_t) -> c_int;

    pub fn strncpy(dest: *mut c_char, src: *const c_char, n: size_t) -> *mut c_char;

    pub fn strnlen(cs: *const c_char, n: size_t) -> size_t;

    pub fn strrchr(s: *const c_char, c: c_int) -> *mut c_char;

    pub fn strstr(h: *const c_char, n: *const c_char) -> *mut c_char;

    pub fn wcschr(s: *const wchar_t, c: wchar_t) -> *mut wchar_t;

    pub fn wcslen(s: *const wchar_t) -> size_t;

    // ctype
    pub fn isalpha(c: c_int) -> c_int;

    pub fn isascii(c: c_int) -> c_int;

    pub fn isdigit(c: c_int) -> c_int;

    pub fn islower(c: c_int) -> c_int;

    pub fn isprint(c: c_int) -> c_int;

    pub fn isspace(c: c_int) -> c_int;

    pub fn iswctype(wc: wint_t, ttype: wctype_t) -> c_int;

    pub fn iswdigit(wc: wint_t) -> c_int;

    pub fn iswlower(wc: wint_t) -> c_int;

    pub fn iswspace(wc: wint_t) -> c_int;

    pub fn iswupper(wc: wint_t) -> c_int;

    pub fn towupper(wc: wint_t) -> wint_t;

    pub fn towlower(wc: wint_t) -> wint_t;

    // cmath
    pub fn atan(x: c_double) -> c_double;

    pub fn ceil(x: c_double) -> c_double;

    pub fn ceilf(x: c_float) -> c_float;

    pub fn exp(x: c_double) -> c_double;

    pub fn fabs(x: c_double) -> c_double;

    pub fn floor(x: c_double) -> c_double;

    pub fn frexp(x: c_double, e: *mut c_int) -> c_double;

    pub fn log(x: c_double) -> c_double;

    pub fn log2(x: c_double) -> c_double;

    pub fn pow(x: c_double, y: c_double) -> c_double;

    pub fn roundf(x: c_float) -> c_float;

    pub fn scalbn(x: c_double, n: c_int) -> c_double;

    pub fn sqrt(x: c_double) -> c_double;

    // stdlib
    pub fn abs(x: c_int) -> c_int;

    pub fn atof(s: *const c_char) -> c_double;

    pub fn atoi(s: *const c_char) -> c_int;

    pub fn atol(s: *const c_char) -> c_long;

    pub fn atoll(s: *const c_char) -> c_longlong;

    pub fn bsearch(
        key: *const c_void,
        base: *const c_void,
        nel: size_t,
        width: size_t,
        cmp: cmpfunc,
    ) -> *mut c_void;

    pub fn div(num: c_int, den: c_int) -> div_t;

    pub fn ecvt(x: c_double, n: c_int, dp: *mut c_int, sign: *mut c_int) -> *mut c_char;

    pub fn imaxabs(a: intmax_t) -> intmax_t;

    pub fn llabs(a: c_longlong) -> c_longlong;

    pub fn qsort(base: *mut c_void, nel: size_t, width: size_t, cmp: cmpfunc);

    pub fn strtoul(s: *const c_char, p: *mut *mut c_char, base: c_int) -> c_ulong;

    pub fn strtol(s: *const c_char, p: *mut *mut c_char, base: c_int) -> c_long;

    pub fn wcstod(s: *const wchar_t, p: *mut *mut wchar_t) -> c_double;
}

pub fn errno() -> c_int {
    unsafe { *__errno_location() }
}

pub fn CPU_COUNT_S(size: usize, cpuset: &cpu_set_t) -> c_int {
    let mut s: u32 = 0;
    let size_of_mask = size_of_val(&cpuset.bits[0]);

    for i in cpuset.bits[..(size / size_of_mask)].iter() {
        s += i.count_ones();
    }
    s as c_int
}

pub fn CPU_COUNT(cpuset: &cpu_set_t) -> c_int {
    CPU_COUNT_S(size_of::<cpu_set_t>(), cpuset)
}
