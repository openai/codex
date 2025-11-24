//! Vectored I/O

use crate::errno::Errno;
use crate::Result;
use libc::{self, c_int, off_t, size_t};
use std::io::{IoSlice, IoSliceMut};
use std::os::unix::io::{AsFd, AsRawFd};

/// Low-level vectored write to a raw file descriptor
///
/// See also [writev(2)](https://pubs.opengroup.org/onlinepubs/9699919799/functions/writev.html)
pub fn writev<Fd: AsFd>(fd: Fd, iov: &[IoSlice<'_>]) -> Result<usize> {
    // SAFETY: to quote the documentation for `IoSlice`:
    //
    // [IoSlice] is semantically a wrapper around a &[u8], but is
    // guaranteed to be ABI compatible with the iovec type on Unix
    // platforms.
    //
    // Because it is ABI compatible, a pointer cast here is valid
    let res = unsafe {
        libc::writev(
            fd.as_fd().as_raw_fd(),
            iov.as_ptr().cast(),
            iov.len() as c_int,
        )
    };

    Errno::result(res).map(|r| r as usize)
}

/// Low-level vectored read from a raw file descriptor
///
/// See also [readv(2)](https://pubs.opengroup.org/onlinepubs/9699919799/functions/readv.html)
// Clippy doesn't know that we need to pass iov mutably only because the
// mutation happens after converting iov to a pointer
#[allow(clippy::needless_pass_by_ref_mut)]
pub fn readv<Fd: AsFd>(fd: Fd, iov: &mut [IoSliceMut<'_>]) -> Result<usize> {
    // SAFETY: same as in writev(), IoSliceMut is ABI-compatible with iovec
    let res = unsafe {
        libc::readv(
            fd.as_fd().as_raw_fd(),
            iov.as_ptr().cast(),
            iov.len() as c_int,
        )
    };

    Errno::result(res).map(|r| r as usize)
}

/// Write to `fd` at `offset` from buffers in `iov`.
///
/// Buffers in `iov` will be written in order until all buffers have been written
/// or an error occurs. The file offset is not changed.
///
/// See also: [`writev`](fn.writev.html) and [`pwrite`](fn.pwrite.html)
#[cfg(not(any(target_os = "redox", target_os = "haiku", target_os = "solaris")))]
pub fn pwritev<Fd: AsFd>(
    fd: Fd,
    iov: &[IoSlice<'_>],
    offset: off_t,
) -> Result<usize> {
    #[cfg(target_env = "uclibc")]
    let offset = offset as libc::off64_t; // uclibc doesn't use off_t

    // SAFETY: same as in writev()
    let res = unsafe {
        libc::pwritev(
            fd.as_fd().as_raw_fd(),
            iov.as_ptr().cast(),
            iov.len() as c_int,
            offset,
        )
    };

    Errno::result(res).map(|r| r as usize)
}

/// Read from `fd` at `offset` filling buffers in `iov`.
///
/// Buffers in `iov` will be filled in order until all buffers have been filled,
/// no more bytes are available, or an error occurs. The file offset is not
/// changed.
///
/// See also: [`readv`](fn.readv.html) and [`pread`](fn.pread.html)
#[cfg(not(any(target_os = "redox", target_os = "haiku", target_os = "solaris")))]
// Clippy doesn't know that we need to pass iov mutably only because the
// mutation happens after converting iov to a pointer
#[allow(clippy::needless_pass_by_ref_mut)]
pub fn preadv<Fd: AsFd>(
    fd: Fd,
    iov: &mut [IoSliceMut<'_>],
    offset: off_t,
) -> Result<usize> {
    #[cfg(target_env = "uclibc")]
    let offset = offset as libc::off64_t; // uclibc doesn't use off_t

    // SAFETY: same as in readv()
    let res = unsafe {
        libc::preadv(
            fd.as_fd().as_raw_fd(),
            iov.as_ptr().cast(),
            iov.len() as c_int,
            offset,
        )
    };

    Errno::result(res).map(|r| r as usize)
}

/// Low-level write to a file, with specified offset.
///
/// See also [pwrite(2)](https://pubs.opengroup.org/onlinepubs/9699919799/functions/pwrite.html)
// TODO: move to unistd
pub fn pwrite<Fd: AsFd>(fd: Fd, buf: &[u8], offset: off_t) -> Result<usize> {
    let res = unsafe {
        libc::pwrite(
            fd.as_fd().as_raw_fd(),
            buf.as_ptr().cast(),
            buf.len() as size_t,
            offset,
        )
    };

    Errno::result(res).map(|r| r as usize)
}

/// Low-level read from a file, with specified offset.
///
/// See also [pread(2)](https://pubs.opengroup.org/onlinepubs/9699919799/functions/pread.html)
// TODO: move to unistd
pub fn pread<Fd: AsFd>(fd: Fd, buf: &mut [u8], offset: off_t) -> Result<usize> {
    let res = unsafe {
        libc::pread(
            fd.as_fd().as_raw_fd(),
            buf.as_mut_ptr().cast(),
            buf.len() as size_t,
            offset,
        )
    };

    Errno::result(res).map(|r| r as usize)
}

/// A slice of memory in a remote process, starting at address `base`
/// and consisting of `len` bytes.
///
/// This is the same underlying C structure as `IoSlice`,
/// except that it refers to memory in some other process, and is
/// therefore not represented in Rust by an actual slice as `IoSlice` is. It
/// is used with [`process_vm_readv`](fn.process_vm_readv.html)
/// and [`process_vm_writev`](fn.process_vm_writev.html).
#[cfg(linux_android)]
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct RemoteIoVec {
    /// The starting address of this slice (`iov_base`).
    pub base: usize,
    /// The number of bytes in this slice (`iov_len`).
    pub len: usize,
}

feature! {
#![feature = "process"]

/// Write data directly to another process's virtual memory
/// (see [`process_vm_writev`(2)]).
///
/// `local_iov` is a list of [`IoSlice`]s containing the data to be written,
/// and `remote_iov` is a list of [`RemoteIoVec`]s identifying where the
/// data should be written in the target process. On success, returns the
/// number of bytes written, which will always be a whole
/// number of `remote_iov` chunks.
///
/// This requires the same permissions as debugging the process using
/// [ptrace]: you must either be a privileged process (with
/// `CAP_SYS_PTRACE`), or you must be running as the same user as the
/// target process and the OS must have unprivileged debugging enabled.
///
/// This function is only available on Linux and Android(SDK23+).
///
/// [`process_vm_writev`(2)]: https://man7.org/linux/man-pages/man2/process_vm_writev.2.html
/// [ptrace]: ../ptrace/index.html
/// [`IoSlice`]: https://doc.rust-lang.org/std/io/struct.IoSlice.html
/// [`RemoteIoVec`]: struct.RemoteIoVec.html
#[cfg(all(linux_android, not(target_env = "uclibc")))]
pub fn process_vm_writev(
    pid: crate::unistd::Pid,
    local_iov: &[IoSlice<'_>],
    remote_iov: &[RemoteIoVec]) -> Result<usize>
{
    let res = unsafe {
        libc::process_vm_writev(pid.into(),
                                local_iov.as_ptr().cast(), local_iov.len() as libc::c_ulong,
                                remote_iov.as_ptr().cast(), remote_iov.len() as libc::c_ulong, 0)
    };

    Errno::result(res).map(|r| r as usize)
}

/// Read data directly from another process's virtual memory
/// (see [`process_vm_readv`(2)]).
///
/// `local_iov` is a list of [`IoSliceMut`]s containing the buffer to copy
/// data into, and `remote_iov` is a list of [`RemoteIoVec`]s identifying
/// where the source data is in the target process. On success,
/// returns the number of bytes written, which will always be a whole
/// number of `remote_iov` chunks.
///
/// This requires the same permissions as debugging the process using
/// [`ptrace`]: you must either be a privileged process (with
/// `CAP_SYS_PTRACE`), or you must be running as the same user as the
/// target process and the OS must have unprivileged debugging enabled.
///
/// This function is only available on Linux and Android(SDK23+).
///
/// [`process_vm_readv`(2)]: https://man7.org/linux/man-pages/man2/process_vm_readv.2.html
/// [`ptrace`]: ../ptrace/index.html
/// [`IoSliceMut`]: https://doc.rust-lang.org/std/io/struct.IoSliceMut.html
/// [`RemoteIoVec`]: struct.RemoteIoVec.html
#[cfg(all(linux_android, not(target_env = "uclibc")))]
pub fn process_vm_readv(
    pid: crate::unistd::Pid,
    local_iov: &mut [IoSliceMut<'_>],
    remote_iov: &[RemoteIoVec]) -> Result<usize>
{
    let res = unsafe {
        libc::process_vm_readv(pid.into(),
                               local_iov.as_ptr().cast(), local_iov.len() as libc::c_ulong,
                               remote_iov.as_ptr().cast(), remote_iov.len() as libc::c_ulong, 0)
    };

    Errno::result(res).map(|r| r as usize)
}
}
