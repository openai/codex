//! ARMv6K Nintendo 3DS C Newlib definitions

use crate::off_t;
use crate::prelude::*;

pub type wchar_t = c_uint;

pub type u_register_t = c_uint;
pub type u_char = c_uchar;
pub type u_short = c_ushort;
pub type u_int = c_uint;
pub type u_long = c_ulong;
pub type ushort = c_ushort;
pub type uint = c_uint;
pub type ulong = c_ulong;
pub type clock_t = c_ulong;
pub type daddr_t = c_long;
pub type caddr_t = *mut c_char;
pub type sbintime_t = c_longlong;
pub type sigset_t = c_ulong;

s! {
    pub struct hostent {
        pub h_name: *mut c_char,
        pub h_aliases: *mut *mut c_char,
        pub h_addrtype: u16,
        pub h_length: u16,
        pub h_addr_list: *mut *mut c_char,
    }

    pub struct sockaddr {
        pub sa_family: crate::sa_family_t,
        pub sa_data: [c_char; 26usize],
    }

    pub struct sockaddr_storage {
        pub ss_family: crate::sa_family_t,
        pub __ss_padding: [c_char; 26usize],
    }

    pub struct sockaddr_in {
        pub sin_family: crate::sa_family_t,
        pub sin_port: crate::in_port_t,
        pub sin_addr: crate::in_addr,
        pub sin_zero: [c_char; 8],
    }

    pub struct sockaddr_in6 {
        pub sin6_family: crate::sa_family_t,
        pub sin6_port: crate::in_port_t,
        pub sin6_flowinfo: u32,
        pub sin6_addr: crate::in6_addr,
        pub sin6_scope_id: u32,
    }

    pub struct sockaddr_un {
        pub sun_len: c_uchar,
        pub sun_family: crate::sa_family_t,
        pub sun_path: [c_char; 104usize],
    }

    pub struct sched_param {
        pub sched_priority: c_int,
    }

    pub struct stat {
        pub st_dev: crate::dev_t,
        pub st_ino: crate::ino_t,
        pub st_mode: crate::mode_t,
        pub st_nlink: crate::nlink_t,
        pub st_uid: crate::uid_t,
        pub st_gid: crate::gid_t,
        pub st_rdev: crate::dev_t,
        pub st_size: off_t,
        pub st_atim: crate::timespec,
        pub st_mtim: crate::timespec,
        pub st_ctim: crate::timespec,
        pub st_blksize: crate::blksize_t,
        pub st_blocks: crate::blkcnt_t,
        pub st_spare4: [c_long; 2usize],
    }
}

pub const SIGEV_NONE: c_int = 1;
pub const SIGEV_SIGNAL: c_int = 2;
pub const SIGEV_THREAD: c_int = 3;
pub const SA_NOCLDSTOP: c_int = 1;
pub const MINSIGSTKSZ: c_int = 2048;
pub const SIGSTKSZ: c_int = 8192;
pub const SS_ONSTACK: c_int = 1;
pub const SS_DISABLE: c_int = 2;
pub const SIG_SETMASK: c_int = 0;
pub const SIG_BLOCK: c_int = 1;
pub const SIG_UNBLOCK: c_int = 2;
pub const SIGHUP: c_int = 1;
pub const SIGINT: c_int = 2;
pub const SIGQUIT: c_int = 3;
pub const SIGILL: c_int = 4;
pub const SIGTRAP: c_int = 5;
pub const SIGABRT: c_int = 6;
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
pub const SIGCLD: c_int = 20;
pub const SIGTTIN: c_int = 21;
pub const SIGTTOU: c_int = 22;
pub const SIGIO: c_int = 23;
pub const SIGPOLL: c_int = 23;
pub const SIGXCPU: c_int = 24;
pub const SIGXFSZ: c_int = 25;
pub const SIGVTALRM: c_int = 26;
pub const SIGPROF: c_int = 27;
pub const SIGWINCH: c_int = 28;
pub const SIGLOST: c_int = 29;
pub const SIGUSR1: c_int = 30;
pub const SIGUSR2: c_int = 31;
pub const NSIG: c_int = 32;
pub const CLOCK_ENABLED: c_uint = 1;
pub const CLOCK_DISABLED: c_uint = 0;
pub const CLOCK_ALLOWED: c_uint = 1;
pub const CLOCK_DISALLOWED: c_uint = 0;
pub const TIMER_ABSTIME: c_uint = 4;
pub const SOL_SOCKET: c_int = 65535;
pub const MSG_OOB: c_int = 1;
pub const MSG_PEEK: c_int = 2;
pub const MSG_DONTWAIT: c_int = 4;
pub const MSG_DONTROUTE: c_int = 0;
pub const MSG_WAITALL: c_int = 0;
pub const MSG_MORE: c_int = 0;
pub const MSG_NOSIGNAL: c_int = 0;
pub const SOL_CONFIG: c_uint = 65534;

pub const _SC_PAGESIZE: c_int = 8;
pub const _SC_GETPW_R_SIZE_MAX: c_int = 51;

pub const PTHREAD_STACK_MIN: size_t = 4096;
pub const WNOHANG: c_int = 1;

pub const POLLIN: c_short = 0x0001;
pub const POLLPRI: c_short = 0x0002;
pub const POLLOUT: c_short = 0x0004;
pub const POLLRDNORM: c_short = 0x0040;
pub const POLLWRNORM: c_short = POLLOUT;
pub const POLLRDBAND: c_short = 0x0080;
pub const POLLWRBAND: c_short = 0x0100;
pub const POLLERR: c_short = 0x0008;
pub const POLLHUP: c_short = 0x0010;
pub const POLLNVAL: c_short = 0x0020;

pub const EAI_AGAIN: c_int = 2;
pub const EAI_BADFLAGS: c_int = 3;
pub const EAI_FAIL: c_int = 4;
pub const EAI_SERVICE: c_int = 9;
pub const EAI_SYSTEM: c_int = 11;
pub const EAI_BADHINTS: c_int = 12;
pub const EAI_PROTOCOL: c_int = 13;
pub const EAI_OVERFLOW: c_int = 14;
pub const EAI_MAX: c_int = 15;

pub const AF_UNIX: c_int = 1;
pub const AF_INET6: c_int = 23;

pub const FIONBIO: c_ulong = 1;

pub const RTLD_DEFAULT: *mut c_void = 0 as *mut c_void;

// For pthread get/setschedparam
pub const SCHED_FIFO: c_int = 1;
pub const SCHED_RR: c_int = 2;

// For getrandom()
pub const GRND_NONBLOCK: c_uint = 0x1;
pub const GRND_RANDOM: c_uint = 0x2;

// Horizon OS works doesn't or can't hold any of this information
safe_f! {
    pub {const} fn WIFSTOPPED(_status: c_int) -> bool {
        false
    }

    pub {const} fn WSTOPSIG(_status: c_int) -> c_int {
        0
    }

    pub {const} fn WIFCONTINUED(_status: c_int) -> bool {
        true
    }

    pub {const} fn WIFSIGNALED(_status: c_int) -> bool {
        false
    }

    pub {const} fn WTERMSIG(_status: c_int) -> c_int {
        0
    }

    pub {const} fn WIFEXITED(_status: c_int) -> bool {
        true
    }

    pub {const} fn WEXITSTATUS(_status: c_int) -> c_int {
        0
    }

    pub {const} fn WCOREDUMP(_status: c_int) -> bool {
        false
    }
}

extern "C" {
    pub fn pthread_create(
        native: *mut crate::pthread_t,
        attr: *const crate::pthread_attr_t,
        f: extern "C" fn(_: *mut c_void) -> *mut c_void,
        value: *mut c_void,
    ) -> c_int;

    pub fn pthread_attr_getschedparam(
        attr: *const crate::pthread_attr_t,
        param: *mut sched_param,
    ) -> c_int;

    pub fn pthread_attr_setschedparam(
        attr: *mut crate::pthread_attr_t,
        param: *const sched_param,
    ) -> c_int;

    pub fn pthread_attr_getprocessorid_np(
        attr: *const crate::pthread_attr_t,
        processor_id: *mut c_int,
    ) -> c_int;

    pub fn pthread_attr_setprocessorid_np(
        attr: *mut crate::pthread_attr_t,
        processor_id: c_int,
    ) -> c_int;

    pub fn pthread_getschedparam(
        native: crate::pthread_t,
        policy: *mut c_int,
        param: *mut crate::sched_param,
    ) -> c_int;

    pub fn pthread_setschedparam(
        native: crate::pthread_t,
        policy: c_int,
        param: *const crate::sched_param,
    ) -> c_int;

    pub fn pthread_condattr_getclock(
        attr: *const crate::pthread_condattr_t,
        clock_id: *mut crate::clockid_t,
    ) -> c_int;

    pub fn pthread_condattr_setclock(
        attr: *mut crate::pthread_condattr_t,
        clock_id: crate::clockid_t,
    ) -> c_int;

    pub fn pthread_getprocessorid_np() -> c_int;

    pub fn getrandom(buf: *mut c_void, buflen: size_t, flags: c_uint) -> ssize_t;

    pub fn gethostid() -> c_long;
}

pub use crate::unix::newlib::generic::dirent;
