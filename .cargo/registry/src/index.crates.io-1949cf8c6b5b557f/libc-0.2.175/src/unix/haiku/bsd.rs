//! This file contains the BSD APIs available in Haiku. It corresponds to the
//! header files in `headers/compatibility/bsd`.
//!
//! Note that Haiku's BSD compatibility is a combination of system APIs and
//! utility libraries. There should only be system APIs in `libc`. When you are
//! trying to determine whether something should be included in this file, the
//! best indicator is whether it also exists in the BSD-specific definitions in
//! this libc crate.

use crate::prelude::*;

// stringlist.h (utility library)
// Note: this is kept because it was previously introduced
pub type StringList = _stringlist;

s! {
    // stringlist.h (utility library)
    // Note: this is kept because it was previously introduced
    pub struct _stringlist {
        pub sl_str: *mut *mut c_char,
        pub sl_max: size_t,
        pub sl_cur: size_t,
    }

    // sys/event.h
    pub struct kevent {
        pub ident: crate::uintptr_t,
        pub filter: c_short,
        pub flags: c_ushort,
        pub fflags: c_uint,
        pub data: i64,
        pub udata: *mut c_void,
        pub ext: [u64; 4],
    }

    // sys/link_elf.h
    pub struct dl_phdr_info {
        pub dlpi_addr: crate::Elf_Addr,
        pub dlpi_name: *const c_char,
        pub dlpi_phdr: *const crate::Elf_Phdr,
        pub dlpi_phnum: crate::Elf_Half,
    }
}

// sys/event.h
pub const EVFILT_READ: i16 = -1;
pub const EVFILT_WRITE: i16 = -2;
pub const EVFILT_PROC: i16 = -5;
pub const EV_ADD: u16 = 0x0001;
pub const EV_DELETE: u16 = 0x0002;
pub const EV_ONESHOT: u16 = 0x0010;
pub const EV_CLEAR: u16 = 0x0020;
pub const EV_EOF: u16 = 0x8000;
pub const EV_ERROR: u16 = 0x4000;
pub const NOTE_EXIT: u32 = 0x80000000;

// sys/ioccom.h
pub const IOC_VOID: c_ulong = 0x20000000;
pub const IOC_OUT: c_ulong = 0x40000000;
pub const IOC_IN: c_ulong = 0x80000000;
pub const IOC_INOUT: c_ulong = IOC_IN | IOC_OUT;
pub const IOC_DIRMASK: c_ulong = 0xe0000000;

#[link(name = "bsd")]
extern "C" {
    // stdlib.h
    pub fn daemon(nochdir: c_int, noclose: c_int) -> c_int;
    pub fn getprogname() -> *const c_char;
    pub fn setprogname(progname: *const c_char);
    pub fn arc4random() -> u32;
    pub fn arc4random_uniform(upper_bound: u32) -> u32;
    pub fn arc4random_buf(buf: *mut c_void, n: size_t);
    pub fn mkstemps(template: *mut c_char, suffixlen: c_int) -> c_int;
    pub fn strtonum(
        nptr: *const c_char,
        minval: c_longlong,
        maxval: c_longlong,
        errstr: *mut *const c_char,
    ) -> c_longlong;

    // pty.h
    pub fn openpty(
        amaster: *mut c_int,
        aslave: *mut c_int,
        name: *mut c_char,
        termp: *mut crate::termios,
        winp: *mut crate::winsize,
    ) -> c_int;
    pub fn login_tty(_fd: c_int) -> c_int;
    pub fn forkpty(
        amaster: *mut c_int,
        name: *mut c_char,
        termp: *mut crate::termios,
        winp: *mut crate::winsize,
    ) -> crate::pid_t;

    // string.h
    pub fn strsep(string: *mut *mut c_char, delimiters: *const c_char) -> *mut c_char;
    pub fn explicit_bzero(buf: *mut c_void, len: size_t);

    // stringlist.h (utility library)
    // Note: this is kept because it was previously introduced
    pub fn sl_init() -> *mut StringList;
    pub fn sl_add(sl: *mut StringList, n: *mut c_char) -> c_int;
    pub fn sl_free(sl: *mut StringList, i: c_int);
    pub fn sl_find(sl: *mut StringList, n: *mut c_char) -> *mut c_char;

    // sys/event.h
    pub fn kqueue() -> c_int;
    pub fn kevent(
        kq: c_int,
        changelist: *const kevent,
        nchanges: c_int,
        eventlist: *mut kevent,
        nevents: c_int,
        timeout: *const crate::timespec,
    ) -> c_int;

    // sys/link_elf.h
    pub fn dl_iterate_phdr(
        callback: Option<
            unsafe extern "C" fn(info: *mut dl_phdr_info, size: usize, data: *mut c_void) -> c_int,
        >,
        data: *mut c_void,
    ) -> c_int;

    // sys/time.h
    pub fn lutimes(file: *const c_char, times: *const crate::timeval) -> c_int;

    // sys/uov.h
    pub fn preadv(
        fd: c_int,
        iov: *const crate::iovec,
        iovcnt: c_int,
        offset: crate::off_t,
    ) -> ssize_t;
    pub fn pwritev(
        fd: c_int,
        iov: *const crate::iovec,
        iovcnt: c_int,
        offset: crate::off_t,
    ) -> ssize_t;

    // sys/wait.h
    pub fn wait4(
        pid: crate::pid_t,
        status: *mut c_int,
        options: c_int,
        rusage: *mut crate::rusage,
    ) -> crate::pid_t;
}
