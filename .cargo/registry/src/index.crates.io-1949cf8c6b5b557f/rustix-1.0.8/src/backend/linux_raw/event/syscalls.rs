//! linux_raw syscalls supporting `rustix::event`.
//!
//! # Safety
//!
//! See the `rustix::backend` module documentation for details.
#![allow(unsafe_code, clippy::undocumented_unsafe_blocks)]

use crate::backend::conv::{
    by_ref, c_int, c_uint, opt_mut, opt_ref, pass_usize, ret, ret_c_int, ret_error, ret_owned_fd,
    ret_usize, size_of, slice_mut, zero,
};
use crate::event::{epoll, EventfdFlags, FdSetElement, PollFd, Timespec};
use crate::fd::{BorrowedFd, OwnedFd};
use crate::io;
use core::ptr::null_mut;
use linux_raw_sys::general::{kernel_sigset_t, EPOLL_CTL_ADD, EPOLL_CTL_DEL, EPOLL_CTL_MOD};

#[inline]
pub(crate) fn poll(fds: &mut [PollFd<'_>], timeout: Option<&Timespec>) -> io::Result<usize> {
    let (fds_addr_mut, fds_len) = slice_mut(fds);

    #[cfg(target_pointer_width = "32")]
    unsafe {
        // If we don't have Linux 5.1, and the timeout fits in a
        // `__kernel_old_timespec`, use plain `ppoll`.
        //
        // We do this unconditionally, rather than trying `ppoll_time64` and
        // falling back on `Errno::NOSYS`, because seccomp configurations will
        // sometimes abort the process on syscalls they don't recognize.
        #[cfg(not(feature = "linux_5_1"))]
        {
            use linux_raw_sys::general::__kernel_old_timespec;

            // If we don't have a timeout, or if we can convert the timeout to
            // a `__kernel_old_timespec`, the use `__NR_ppoll`.
            fn convert(timeout: &Timespec) -> Option<__kernel_old_timespec> {
                Some(__kernel_old_timespec {
                    tv_sec: timeout.tv_sec.try_into().ok()?,
                    tv_nsec: timeout.tv_nsec.try_into().ok()?,
                })
            }
            let old_timeout = if let Some(timeout) = timeout {
                match convert(timeout) {
                    // Could not convert timeout.
                    None => None,
                    // Could convert timeout. Ok!
                    Some(old_timeout) => Some(Some(old_timeout)),
                }
            } else {
                // No timeout. Ok!
                Some(None)
            };
            if let Some(mut old_timeout) = old_timeout {
                // Call `ppoll`.
                //
                // Linux's `ppoll` mutates the timeout argument. Our public
                // interface does not do this, because it's not portable to other
                // platforms, so we create a temporary value to hide this behavior.
                return ret_usize(syscall!(
                    __NR_ppoll,
                    fds_addr_mut,
                    fds_len,
                    opt_mut(old_timeout.as_mut()),
                    zero(),
                    size_of::<kernel_sigset_t, _>()
                ));
            }
        }

        // We either have Linux 5.1 or the timeout didn't fit in
        // `__kernel_old_timespec` so `__NR_ppoll_time64` will either
        // succeed or fail due to our having no other options.

        // Call `ppoll_time64`.
        //
        // Linux's `ppoll_time64` mutates the timeout argument. Our public
        // interface does not do this, because it's not portable to other
        // platforms, so we create a temporary value to hide this behavior.
        ret_usize(syscall!(
            __NR_ppoll_time64,
            fds_addr_mut,
            fds_len,
            opt_mut(timeout.copied().as_mut()),
            zero(),
            size_of::<kernel_sigset_t, _>()
        ))
    }

    #[cfg(target_pointer_width = "64")]
    unsafe {
        // Call `ppoll`.
        //
        // Linux's `ppoll` mutates the timeout argument. Our public interface
        // does not do this, because it's not portable to other platforms, so
        // we create a temporary value to hide this behavior.
        ret_usize(syscall!(
            __NR_ppoll,
            fds_addr_mut,
            fds_len,
            opt_mut(timeout.copied().as_mut()),
            zero(),
            size_of::<kernel_sigset_t, _>()
        ))
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

    #[cfg(target_pointer_width = "32")]
    {
        // If we don't have Linux 5.1, and the timeout fits in a
        // `__kernel_old_timespec`, use plain `pselect6`.
        //
        // We do this unconditionally, rather than trying `pselect6_time64` and
        // falling back on `Errno::NOSYS`, because seccomp configurations will
        // sometimes abort the process on syscalls they don't recognize.
        #[cfg(not(feature = "linux_5_1"))]
        {
            use linux_raw_sys::general::__kernel_old_timespec;

            // If we don't have a timeout, or if we can convert the timeout to
            // a `__kernel_old_timespec`, the use `__NR_pselect6`.
            fn convert(timeout: &Timespec) -> Option<__kernel_old_timespec> {
                Some(__kernel_old_timespec {
                    tv_sec: timeout.tv_sec.try_into().ok()?,
                    tv_nsec: timeout.tv_nsec.try_into().ok()?,
                })
            }
            let old_timeout = if let Some(timeout) = timeout {
                match convert(timeout) {
                    // Could not convert timeout.
                    None => None,
                    // Could convert timeout. Ok!
                    Some(old_timeout) => Some(Some(old_timeout)),
                }
            } else {
                // No timeout. Ok!
                Some(None)
            };
            if let Some(mut old_timeout) = old_timeout {
                // Call `pselect6`.
                //
                // Linux's `pselect6` mutates the timeout argument. Our public
                // interface does not do this, because it's not portable to other
                // platforms, so we create a temporary value to hide this behavior.
                return ret_c_int(syscall!(
                    __NR_pselect6,
                    c_int(nfds),
                    readfds,
                    writefds,
                    exceptfds,
                    opt_mut(old_timeout.as_mut()),
                    zero()
                ));
            }
        }

        // We either have Linux 5.1 or the timeout didn't fit in
        // `__kernel_old_timespec` so `__NR_pselect6_time64` will either
        // succeed or fail due to our having no other options.

        // Call `pselect6_time64`.
        //
        // Linux's `pselect6_time64` mutates the timeout argument. Our public
        // interface does not do this, because it's not portable to other
        // platforms, so we create a temporary value to hide this behavior.
        ret_c_int(syscall!(
            __NR_pselect6_time64,
            c_int(nfds),
            readfds,
            writefds,
            exceptfds,
            opt_mut(timeout.copied().as_mut()),
            zero()
        ))
    }

    #[cfg(target_pointer_width = "64")]
    {
        // Call `pselect6`.
        //
        // Linux's `pselect6` mutates the timeout argument. Our public interface
        // does not do this, because it's not portable to other platforms, so we
        // create a temporary value to hide this behavior.
        ret_c_int(syscall!(
            __NR_pselect6,
            c_int(nfds),
            readfds,
            writefds,
            exceptfds,
            opt_mut(timeout.copied().as_mut()),
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

#[inline]
pub(crate) unsafe fn epoll_wait(
    epfd: BorrowedFd<'_>,
    events: (*mut crate::event::epoll::Event, usize),
    timeout: Option<&Timespec>,
) -> io::Result<usize> {
    // If we don't have Linux 5.1, and the timeout fits in an `i32`, use plain
    // `epoll_pwait`.
    //
    // We do this unconditionally, rather than trying `epoll_pwait2` and
    // falling back on `Errno::NOSYS`, because seccomp configurations will
    // sometimes abort the process on syscalls they don't recognize.
    #[cfg(not(feature = "linux_5_11"))]
    {
        // If we don't have a timeout, or if we can convert the timeout to an
        // `i32`, the use `__NR_epoll_pwait`.
        let old_timeout = if let Some(timeout) = timeout {
            // Try to convert the timeout; if this is `Some`, we're ok!
            timeout.as_c_int_millis()
        } else {
            // No timeout. Ok!
            Some(-1)
        };
        if let Some(old_timeout) = old_timeout {
            // Call `epoll_pwait`.
            return ret_usize(syscall!(
                __NR_epoll_pwait,
                epfd,
                events.0,
                pass_usize(events.1),
                c_int(old_timeout),
                zero()
            ));
        }
    }

    // Call `epoll_pwait2`.
    //
    // We either have Linux 5.1 or the timeout didn't fit in an `i32`, so
    // `__NR_epoll_pwait2` will either succeed or fail due to our having no
    // other options.
    ret_usize(syscall!(
        __NR_epoll_pwait2,
        epfd,
        events.0,
        pass_usize(events.1),
        opt_ref(timeout),
        zero()
    ))
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
