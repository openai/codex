// defined in architecture specific module

use crate::prelude::*;

s! {
    pub struct sockaddr_un {
        pub sun_family: crate::sa_family_t,
        pub sun_path: [c_char; 108usize],
    }
}

pub const AF_UNIX: c_int = 1;

pub const RTLD_DEFAULT: *mut c_void = -2isize as *mut c_void;

pub const UTIME_OMIT: c_long = -1;
pub const AT_FDCWD: c_int = -2;

pub const O_DIRECTORY: c_int = 0x200000;
pub const O_NOFOLLOW: c_int = 0x100000;

pub const AT_EACCESS: c_int = 1;
pub const AT_SYMLINK_NOFOLLOW: c_int = 2;
pub const AT_SYMLINK_FOLLOW: c_int = 4;
pub const AT_REMOVEDIR: c_int = 8;

// signal.h
pub const SIG_BLOCK: c_int = 1;
pub const SIG_UNBLOCK: c_int = 2;
pub const SIG_SETMASK: c_int = 0;
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
pub const SIGWINCH: c_int = 24;
pub const SIGUSR1: c_int = 25;
pub const SIGUSR2: c_int = 26;
pub const SIGRTMIN: c_int = 27;
pub const SIGRTMAX: c_int = 31;
pub const SIGXCPU: c_int = 24;
pub const SIGXFSZ: c_int = 25;
pub const SIGVTALRM: c_int = 26;
pub const SIGPROF: c_int = 27;

pub const SA_NOCLDSTOP: c_ulong = 0x00000001;
pub const SA_SIGINFO: c_ulong = 0x00000002;
pub const SA_ONSTACK: c_ulong = 0x00000004;

pub const EAI_AGAIN: c_int = 2;
pub const EAI_BADFLAGS: c_int = 3;
pub const EAI_FAIL: c_int = 4;
pub const EAI_SERVICE: c_int = 9;
pub const EAI_SYSTEM: c_int = 11;
pub const EAI_OVERFLOW: c_int = 14;

pub const _SC_PAGESIZE: c_int = 8;
pub const _SC_GETPW_R_SIZE_MAX: c_int = 51;
pub const PTHREAD_STACK_MIN: size_t = 0;

// sys/wait.h
pub const WNOHANG: c_int = 1;
pub const WUNTRACED: c_int = 2;

// sys/socket.h
pub const SOMAXCONN: c_int = 128;

safe_f! {
    pub {const} fn WIFSTOPPED(status: c_int) -> bool {
        (status & 0xff) == 0x7f
    }

    pub {const} fn WSTOPSIG(status: c_int) -> c_int {
        // (status >> 8) & 0xff
        WEXITSTATUS(status)
    }

    pub {const} fn WIFSIGNALED(status: c_int) -> bool {
        ((status & 0x7f) > 0) && ((status & 0x7f) < 0x7f)
    }

    pub {const} fn WTERMSIG(status: c_int) -> c_int {
        status & 0x7f
    }

    pub {const} fn WIFEXITED(status: c_int) -> bool {
        (status & 0xff) == 0
    }

    pub {const} fn WEXITSTATUS(status: c_int) -> c_int {
        (status >> 8) & 0xff
    }

    // RTEMS doesn't have native WIFCONTINUED.
    pub {const} fn WIFCONTINUED(_status: c_int) -> bool {
        true
    }

    // RTEMS doesn't have native WCOREDUMP.
    pub {const} fn WCOREDUMP(_status: c_int) -> bool {
        false
    }
}

extern "C" {
    pub fn futimens(fd: c_int, times: *const crate::timespec) -> c_int;
    pub fn writev(fd: c_int, iov: *const crate::iovec, iovcnt: c_int) -> ssize_t;
    pub fn readv(fd: c_int, iov: *const crate::iovec, iovcnt: c_int) -> ssize_t;

    pub fn pthread_create(
        native: *mut crate::pthread_t,
        attr: *const crate::pthread_attr_t,
        f: extern "C" fn(_: *mut c_void) -> *mut c_void,
        value: *mut c_void,
    ) -> c_int;

    pub fn pthread_condattr_setclock(
        attr: *mut crate::pthread_condattr_t,
        clock_id: crate::clockid_t,
    ) -> c_int;

    pub fn getentropy(buf: *mut c_void, buflen: size_t) -> c_int;

    pub fn arc4random_buf(buf: *mut core::ffi::c_void, nbytes: size_t);

    pub fn setgroups(ngroups: c_int, grouplist: *const crate::gid_t) -> c_int;
}
