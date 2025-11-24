use crate::prelude::*;
pub type size_t = usize;
pub type ssize_t = isize;

pub type off_t = i64;

pub type c_uint8_t = u8;
pub type c_uint16_t = u16;
pub type c_uint32_t = u32;
pub type c_uint64_t = u64;

pub type c_int8_t = i8;
pub type c_int16_t = i16;
pub type c_int32_t = i32;
pub type c_int64_t = i64;

pub type intptr_t = isize;
pub type uintptr_t = usize;

pub type time_t = c_long;

pub type clockid_t = c_int;

s! {
    pub struct iovec {
        pub iov_base: *mut c_void,
        pub iov_len: size_t,
    }

    pub struct timespec {
        pub tv_sec: time_t,
        pub tv_nsec: c_long,
    }
}

pub const PROT_READ: i32 = 1;
pub const PROT_WRITE: i32 = 2;

// Trusty only supports `CLOCK_BOOTTIME`.
pub const CLOCK_BOOTTIME: clockid_t = 7;

pub const STDOUT_FILENO: c_int = 1;
pub const STDERR_FILENO: c_int = 2;

pub const AT_PAGESZ: c_ulong = 6;

pub const MAP_FAILED: *mut c_void = !0 as *mut c_void;

extern "C" {
    pub fn calloc(nobj: size_t, size: size_t) -> *mut c_void;
    pub fn malloc(size: size_t) -> *mut c_void;
    pub fn realloc(p: *mut c_void, size: size_t) -> *mut c_void;
    pub fn free(p: *mut c_void);
    pub fn memalign(align: size_t, size: size_t) -> *mut c_void;
    pub fn posix_memalign(memptr: *mut *mut c_void, align: size_t, size: size_t) -> c_int;
    pub fn write(fd: c_int, buf: *const c_void, count: size_t) -> ssize_t;
    pub fn writev(fd: c_int, iov: *const crate::iovec, iovcnt: c_int) -> ssize_t;
    pub fn close(fd: c_int) -> c_int;
    pub fn strlen(cs: *const c_char) -> size_t;
    pub fn getauxval(type_: c_ulong) -> c_ulong;
    pub fn mmap(
        addr: *mut c_void,
        len: size_t,
        prot: c_int,
        flags: c_int,
        fd: c_int,
        offset: off_t,
    ) -> *mut c_void;
    pub fn munmap(addr: *mut c_void, len: size_t) -> c_int;
    pub fn clock_gettime(clk_id: crate::clockid_t, tp: *mut crate::timespec) -> c_int;
    pub fn nanosleep(rqtp: *const crate::timespec, rmtp: *mut crate::timespec) -> c_int;
}
