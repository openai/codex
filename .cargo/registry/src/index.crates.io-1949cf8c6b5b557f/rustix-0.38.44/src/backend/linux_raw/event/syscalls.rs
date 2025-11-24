//! linux_raw syscalls supporting `rustix::event`.
//!
//! # Safety
//!
//! See the `rustix::backend` module documentation for details.
#![allow(unsafe_code, clippy::undocumented_unsafe_blocks)]

use crate::backend::c;
use crate::backend::conv::{
    by_ref, c_int, c_uint, ret, ret_c_int, ret_error, ret_owned_fd, ret_usize, slice_mut, zero,
};
use crate::event::{epoll, EventfdFlags, FdSetElement, PollFd};
use crate::fd::{BorrowedFd, OwnedFd};
use crate::io;
use crate::utils::as_mut_ptr;
#[cfg(feature = "alloc")]
use core::mem::MaybeUninit;
use core::ptr::null_mut;
use linux_raw_sys::general::{EPOLL_CTL_ADD, EPOLL_CTL_DEL, EPOLL_CTL_MOD};
#[cfg(any(target_arch = "aarch64", target_arch = "riscv64"))]
use {
    crate::backend::conv::{opt_ref, size_of},
    linux_raw_sys::general::{__kernel_timespec, kernel_sigset_t},
};

#[inline]
pub(crate) fn poll(fds: &mut [PollFd<'_>], timeout: c::c_int) -> io::Result<usize> {
    let (fds_addr_mut, fds_len) = slice_mut(fds);

    #[cfg(any(target_arch = "aarch64", target_arch = "riscv64"))]
    unsafe {
        let timeout = if timeout >= 0 {
            Some(__kernel_timespec {
                tv_sec: (timeout as i64) / 1000,
                tv_nsec: (timeout as i64) % 1000 * 1_000_000,
            })
        } else {
            None
        };
        ret_usize(syscall!(
            __NR_ppoll,
            fds_addr_mut,
            fds_len,
            opt_ref(timeout.as_ref()),
            zero(),
            size_of::<kernel_sigset_t, _>()
        ))
    }
    #[cfg(not(any(target_arch = "aarch64", target_arch = "riscv64")))]
    unsafe {
        ret_usize(syscall!(__NR_poll, fds_addr_mut, fds_len, c_int(timeout)))
    }
}

pub(crate) unsafe fn select(
    nfds: i32,
    readfds: Option<&mut [FdSetElement]>,
    writefds: Option<&mut [FdSetElement]>,
    exceptfds: Option<&mut [FdSetElement]>,
    timeout: Option<&crate::timespec::Timespec>,
) -> io::Result<i32> {
    let len = crate::event::fd_set_num_elements_for_bitvector(nfds);

    let readfds = match readfds {
        Some(readfds) => {
            assert!(readfds.len() >= len);
            readfds.as_mut_ptr()
        }
        None => null_mut(),
    };
    let writefds = match writefds {
        Some(writefds) => {
            assert!(writefds.len() >= len);
            writefds.as_mut_ptr()
        }
        None => null_mut(),
    };
    let exceptfds = match exceptfds {
        Some(exceptfds) => {
            assert!(exceptfds.len() >= len);
            exceptfds.as_mut_ptr()
        }
        None => null_mut(),
    };

    // Linux's `pselect6` mutates the timeout argument. Our public interface
    // does not do this, because it's not portable to other platforms, so we
    // create a temporary value to hide this behavior.
    let mut timeout_data;
    let timeout_ptr = match timeout {
        Some(timeout) => {
            timeout_data = *timeout;
            as_mut_ptr(&mut timeout_data)
        }
        None => null_mut(),
    };

    #[cfg(any(
        target_arch = "arm",
        target_arch = "powerpc",
        target_arch = "sparc",
        target_arch = "csky",
        target_arch = "x86",
        target_arch = "mips32r6",
        target_arch = "riscv32",
        target_arch = "mips"
    ))]
    {
        ret_c_int(syscall!(
            __NR_pselect6_time64,
            c_int(nfds),
            readfds,
            writefds,
            exceptfds,
            timeout_ptr,
            zero()
        ))
    }

    #[cfg(target_pointer_width = "64")]
    {
        ret_c_int(syscall!(
            __NR_pselect6,
            c_int(nfds),
            readfds,
            writefds,
            exceptfds,
            timeout_ptr,
            zero()
        ))
    }
}

#[inline]
pub(crate) fn epoll_create(flags: epoll::CreateFlags) -> io::Result<OwnedFd> {
    // SAFETY: `__NR_epoll_create1` doesn't access any user memory.
    unsafe { ret_owned_fd(syscall_readonly!(__NR_epoll_create1, flags)) }
}

#[inline]
pub(crate) fn epoll_add(
    epfd: BorrowedFd<'_>,
    fd: BorrowedFd<'_>,
    event: &epoll::Event,
) -> io::Result<()> {
    // SAFETY: `__NR_epoll_ctl` with `EPOLL_CTL_ADD` doesn't modify any user
    // memory, and it only reads from `event`.
    unsafe {
        ret(syscall_readonly!(
            __NR_epoll_ctl,
            epfd,
            c_uint(EPOLL_CTL_ADD),
            fd,
            by_ref(event)
        ))
    }
}

#[inline]
pub(crate) fn epoll_mod(
    epfd: BorrowedFd<'_>,
    fd: BorrowedFd<'_>,
    event: &epoll::Event,
) -> io::Result<()> {
    // SAFETY: `__NR_epoll_ctl` with `EPOLL_CTL_MOD` doesn't modify any user
    // memory, and it only reads from `event`.
    unsafe {
        ret(syscall_readonly!(
            __NR_epoll_ctl,
            epfd,
            c_uint(EPOLL_CTL_MOD),
            fd,
            by_ref(event)
        ))
    }
}

#[inline]
pub(crate) fn epoll_del(epfd: BorrowedFd<'_>, fd: BorrowedFd<'_>) -> io::Result<()> {
    // SAFETY: `__NR_epoll_ctl` with `EPOLL_CTL_DEL` doesn't access any user
    // memory.
    unsafe {
        ret(syscall_readonly!(
            __NR_epoll_ctl,
            epfd,
            c_uint(EPOLL_CTL_DEL),
            fd,
            zero()
        ))
    }
}

#[cfg(feature = "alloc")]
#[inline]
pub(crate) fn epoll_wait(
    epfd: BorrowedFd<'_>,
    events: &mut [MaybeUninit<crate::event::epoll::Event>],
    timeout: c::c_int,
) -> io::Result<usize> {
    let (buf_addr_mut, buf_len) = slice_mut(events);
    // SAFETY: `__NR_epoll_wait` doesn't access any user memory outside of
    // the `events` array.
    #[cfg(not(any(target_arch = "aarch64", target_arch = "riscv64")))]
    unsafe {
        ret_usize(syscall!(
            __NR_epoll_wait,
            epfd,
            buf_addr_mut,
            buf_len,
            c_int(timeout)
        ))
    }
    // SAFETY: `__NR_epoll_pwait` doesn't access any user memory outside of
    // the `events` array, as we don't pass it a `sigmask`.
    #[cfg(any(target_arch = "aarch64", target_arch = "riscv64"))]
    unsafe {
        ret_usize(syscall!(
            __NR_epoll_pwait,
            epfd,
            buf_addr_mut,
            buf_len,
            c_int(timeout),
            zero()
        ))
    }
}

#[inline]
pub(crate) fn eventfd(initval: u32, flags: EventfdFlags) -> io::Result<OwnedFd> {
    unsafe { ret_owned_fd(syscall_readonly!(__NR_eventfd2, c_uint(initval), flags)) }
}

#[inline]
pub(crate) fn pause() {
    unsafe {
        #[cfg(any(target_arch = "aarch64", target_arch = "riscv64"))]
        let error = ret_error(syscall_readonly!(
            __NR_ppoll,
            zero(),
            zero(),
            zero(),
            zero()
        ));

        #[cfg(not(any(target_arch = "aarch64", target_arch = "riscv64")))]
        let error = ret_error(syscall_readonly!(__NR_pause));

        debug_assert_eq!(error, io::Errno::INTR);
    }
}
