use crate::off64_t;
use crate::prelude::*;

#[inline]
pub unsafe extern "C" fn creat64(path: *const c_char, mode: crate::mode_t) -> c_int {
    crate::creat(path, mode)
}

#[inline]
pub unsafe extern "C" fn fallocate64(
    fd: c_int,
    mode: c_int,
    offset: off64_t,
    len: off64_t,
) -> c_int {
    crate::fallocate(fd, mode, offset, len)
}

#[inline]
pub unsafe extern "C" fn fgetpos64(stream: *mut crate::FILE, pos: *mut crate::fpos64_t) -> c_int {
    crate::fgetpos(stream, pos as *mut _)
}

#[inline]
pub unsafe extern "C" fn fopen64(pathname: *const c_char, mode: *const c_char) -> *mut crate::FILE {
    crate::fopen(pathname, mode)
}

#[inline]
pub unsafe extern "C" fn freopen64(
    pathname: *const c_char,
    mode: *const c_char,
    stream: *mut crate::FILE,
) -> *mut crate::FILE {
    crate::freopen(pathname, mode, stream)
}

#[inline]
pub unsafe extern "C" fn fseeko64(
    stream: *mut crate::FILE,
    offset: off64_t,
    whence: c_int,
) -> c_int {
    crate::fseeko(stream, offset, whence)
}

#[inline]
pub unsafe extern "C" fn fsetpos64(stream: *mut crate::FILE, pos: *const crate::fpos64_t) -> c_int {
    crate::fsetpos(stream, pos as *mut _)
}

#[inline]
pub unsafe extern "C" fn fstat64(fildes: c_int, buf: *mut crate::stat64) -> c_int {
    crate::fstat(fildes, buf as *mut _)
}

#[inline]
pub unsafe extern "C" fn fstatat64(
    fd: c_int,
    path: *const c_char,
    buf: *mut crate::stat64,
    flag: c_int,
) -> c_int {
    crate::fstatat(fd, path, buf as *mut _, flag)
}

#[inline]
pub unsafe extern "C" fn fstatfs64(fd: c_int, buf: *mut crate::statfs64) -> c_int {
    crate::fstatfs(fd, buf as *mut _)
}

#[inline]
pub unsafe extern "C" fn fstatvfs64(fd: c_int, buf: *mut crate::statvfs64) -> c_int {
    crate::fstatvfs(fd, buf as *mut _)
}

#[inline]
pub unsafe extern "C" fn ftello64(stream: *mut crate::FILE) -> off64_t {
    crate::ftello(stream)
}

#[inline]
pub unsafe extern "C" fn ftruncate64(fd: c_int, length: off64_t) -> c_int {
    crate::ftruncate(fd, length)
}

#[inline]
pub unsafe extern "C" fn getrlimit64(resource: c_int, rlim: *mut crate::rlimit64) -> c_int {
    crate::getrlimit(resource, rlim as *mut _)
}

#[inline]
pub unsafe extern "C" fn lseek64(fd: c_int, offset: off64_t, whence: c_int) -> off64_t {
    crate::lseek(fd, offset, whence)
}

#[inline]
pub unsafe extern "C" fn lstat64(path: *const c_char, buf: *mut crate::stat64) -> c_int {
    crate::lstat(path, buf as *mut _)
}

#[inline]
pub unsafe extern "C" fn mmap64(
    addr: *mut c_void,
    length: size_t,
    prot: c_int,
    flags: c_int,
    fd: c_int,
    offset: off64_t,
) -> *mut c_void {
    crate::mmap(addr, length, prot, flags, fd, offset)
}

// These functions are variadic in the C ABI since the `mode` argument is "optional".  Variadic
// `extern "C"` functions are unstable in Rust so we cannot write a shim function for these
// entrypoints.  See https://github.com/rust-lang/rust/issues/44930.
//
// These aliases are mostly fine though, neither function takes a LFS64-namespaced type as an
// argument, nor do their names clash with any declared types.
pub use crate::{open as open64, openat as openat64};

#[inline]
pub unsafe extern "C" fn posix_fadvise64(
    fd: c_int,
    offset: off64_t,
    len: off64_t,
    advice: c_int,
) -> c_int {
    crate::posix_fadvise(fd, offset, len, advice)
}

#[inline]
pub unsafe extern "C" fn posix_fallocate64(fd: c_int, offset: off64_t, len: off64_t) -> c_int {
    crate::posix_fallocate(fd, offset, len)
}

#[inline]
pub unsafe extern "C" fn pread64(
    fd: c_int,
    buf: *mut c_void,
    count: size_t,
    offset: off64_t,
) -> ssize_t {
    crate::pread(fd, buf, count, offset)
}

#[inline]
pub unsafe extern "C" fn preadv64(
    fd: c_int,
    iov: *const crate::iovec,
    iovcnt: c_int,
    offset: off64_t,
) -> ssize_t {
    crate::preadv(fd, iov, iovcnt, offset)
}

#[inline]
pub unsafe extern "C" fn prlimit64(
    pid: crate::pid_t,
    resource: c_int,
    new_limit: *const crate::rlimit64,
    old_limit: *mut crate::rlimit64,
) -> c_int {
    crate::prlimit(pid, resource, new_limit as *mut _, old_limit as *mut _)
}

#[inline]
pub unsafe extern "C" fn pwrite64(
    fd: c_int,
    buf: *const c_void,
    count: size_t,
    offset: off64_t,
) -> ssize_t {
    crate::pwrite(fd, buf, count, offset)
}

#[inline]
pub unsafe extern "C" fn pwritev64(
    fd: c_int,
    iov: *const crate::iovec,
    iovcnt: c_int,
    offset: off64_t,
) -> ssize_t {
    crate::pwritev(fd, iov, iovcnt, offset)
}

#[inline]
pub unsafe extern "C" fn readdir64(dirp: *mut crate::DIR) -> *mut crate::dirent64 {
    crate::readdir(dirp) as *mut _
}

#[inline]
pub unsafe extern "C" fn readdir64_r(
    dirp: *mut crate::DIR,
    entry: *mut crate::dirent64,
    result: *mut *mut crate::dirent64,
) -> c_int {
    crate::readdir_r(dirp, entry as *mut _, result as *mut _)
}

#[inline]
pub unsafe extern "C" fn sendfile64(
    out_fd: c_int,
    in_fd: c_int,
    offset: *mut off64_t,
    count: size_t,
) -> ssize_t {
    crate::sendfile(out_fd, in_fd, offset, count)
}

#[inline]
pub unsafe extern "C" fn setrlimit64(resource: c_int, rlim: *const crate::rlimit64) -> c_int {
    crate::setrlimit(resource, rlim as *mut _)
}

#[inline]
pub unsafe extern "C" fn stat64(pathname: *const c_char, statbuf: *mut crate::stat64) -> c_int {
    crate::stat(pathname, statbuf as *mut _)
}

#[inline]
pub unsafe extern "C" fn statfs64(pathname: *const c_char, buf: *mut crate::statfs64) -> c_int {
    crate::statfs(pathname, buf as *mut _)
}

#[inline]
pub unsafe extern "C" fn statvfs64(path: *const c_char, buf: *mut crate::statvfs64) -> c_int {
    crate::statvfs(path, buf as *mut _)
}

#[inline]
pub unsafe extern "C" fn tmpfile64() -> *mut crate::FILE {
    crate::tmpfile()
}

#[inline]
pub unsafe extern "C" fn truncate64(path: *const c_char, length: off64_t) -> c_int {
    crate::truncate(path, length)
}
