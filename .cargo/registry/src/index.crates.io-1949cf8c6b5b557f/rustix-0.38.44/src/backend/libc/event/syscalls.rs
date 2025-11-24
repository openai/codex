//! libc syscalls supporting `rustix::event`.

use crate::backend::c;
#[cfg(any(linux_kernel, solarish, target_os = "redox"))]
use crate::backend::conv::ret;
use crate::backend::conv::ret_c_int;
#[cfg(feature = "alloc")]
#[cfg(any(linux_kernel, target_os = "illumos", target_os = "redox"))]
use crate::backend::conv::ret_u32;
#[cfg(solarish)]
use crate::event::port::Event;
#[cfg(any(
    linux_kernel,
    target_os = "freebsd",
    target_os = "illumos",
    target_os = "espidf"
))]
use crate::event::EventfdFlags;
#[cfg(any(bsd, linux_kernel, target_os = "wasi"))]
use crate::event::FdSetElement;
use crate::event::PollFd;
use crate::io;
#[cfg(solarish)]
use crate::utils::as_mut_ptr;
#[cfg(any(linux_kernel, target_os = "illumos", target_os = "redox"))]
use crate::utils::as_ptr;
#[cfg(any(
    all(feature = "alloc", bsd),
    solarish,
    all(feature = "alloc", any(linux_kernel, target_os = "redox")),
))]
use core::mem::MaybeUninit;
#[cfg(any(bsd, linux_kernel, target_os = "wasi"))]
use core::ptr::null;
#[cfg(any(bsd, linux_kernel, solarish, target_os = "redox", target_os = "wasi"))]
use core::ptr::null_mut;
#[cfg(any(
    linux_kernel,
    solarish,
    target_os = "redox",
    all(feature = "alloc", bsd)
))]
use {crate::backend::conv::borrowed_fd, crate::fd::BorrowedFd};
#[cfg(any(
    linux_kernel,
    solarish,
    target_os = "freebsd",
    target_os = "illumos",
    target_os = "espidf",
    target_os = "redox",
    all(feature = "alloc", bsd)
))]
use {crate::backend::conv::ret_owned_fd, crate::fd::OwnedFd};
#[cfg(all(feature = "alloc", bsd))]
use {crate::event::kqueue::Event, crate::utils::as_ptr};

#[cfg(any(
    linux_kernel,
    target_os = "freebsd",
    target_os = "illumos",
    target_os = "espidf"
))]
pub(crate) fn eventfd(initval: u32, flags: EventfdFlags) -> io::Result<OwnedFd> {
    #[cfg(linux_kernel)]
    unsafe {
        syscall! {
            fn eventfd2(
                initval: c::c_uint,
                flags: c::c_int
            ) via SYS_eventfd2 -> c::c_int
        }
        ret_owned_fd(eventfd2(initval, bitflags_bits!(flags)))
    }

    // `eventfd` was added in FreeBSD 13, so it isn't available on FreeBSD 12.
    #[cfg(target_os = "freebsd")]
    unsafe {
        weakcall! {
            fn eventfd(
                initval: c::c_uint,
                flags: c::c_int
            ) -> c::c_int
        }
        ret_owned_fd(eventfd(initval, bitflags_bits!(flags)))
    }

    #[cfg(any(target_os = "illumos", target_os = "espidf"))]
    unsafe {
        ret_owned_fd(c::eventfd(initval, bitflags_bits!(flags)))
    }
}

#[cfg(all(feature = "alloc", bsd))]
pub(crate) fn kqueue() -> io::Result<OwnedFd> {
    unsafe { ret_owned_fd(c::kqueue()) }
}

#[cfg(all(feature = "alloc", bsd))]
pub(crate) unsafe fn kevent(
    kq: BorrowedFd<'_>,
    changelist: &[Event],
    eventlist: &mut [MaybeUninit<Event>],
    timeout: Option<&c::timespec>,
) -> io::Result<c::c_int> {
    ret_c_int(c::kevent(
        borrowed_fd(kq),
        changelist.as_ptr().cast(),
        changelist
            .len()
            .try_into()
            .map_err(|_| io::Errno::OVERFLOW)?,
        eventlist.as_mut_ptr().cast(),
        eventlist
            .len()
            .try_into()
            .map_err(|_| io::Errno::OVERFLOW)?,
        timeout.map_or(null(), as_ptr),
    ))
}

#[inline]
pub(crate) fn poll(fds: &mut [PollFd<'_>], timeout: c::c_int) -> io::Result<usize> {
    let nfds = fds
        .len()
        .try_into()
        .map_err(|_convert_err| io::Errno::INVAL)?;

    ret_c_int(unsafe { c::poll(fds.as_mut_ptr().cast(), nfds, timeout) })
        .map(|nready| nready as usize)
}

#[cfg(any(bsd, linux_kernel))]
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

    let timeout_data;
    let timeout_ptr = match timeout {
        Some(timeout) => {
            // Convert from `Timespec` to `c::timeval`.
            timeout_data = c::timeval {
                tv_sec: timeout.tv_sec.try_into().map_err(|_| io::Errno::OVERFLOW)?,
                tv_usec: ((timeout.tv_nsec + 999) / 1000) as _,
            };
            &timeout_data
        }
        None => null(),
    };

    // On Apple platforms, use the specially mangled `select` which doesn't
    // have an `FD_SETSIZE` limitation.
    #[cfg(apple)]
    {
        extern "C" {
            #[link_name = "select$DARWIN_EXTSN$NOCANCEL"]
            fn select(
                nfds: c::c_int,
                readfds: *mut FdSetElement,
                writefds: *mut FdSetElement,
                errorfds: *mut FdSetElement,
                timeout: *const c::timeval,
            ) -> c::c_int;
        }

        ret_c_int(select(nfds, readfds, writefds, exceptfds, timeout_ptr))
    }

    // Otherwise just use the normal `select`.
    #[cfg(not(apple))]
    {
        ret_c_int(c::select(
            nfds,
            readfds.cast(),
            writefds.cast(),
            exceptfds.cast(),
            timeout_ptr as *mut c::timeval,
        ))
    }
}

// WASI uses a count + array instead of a bitvector.
#[cfg(target_os = "wasi")]
pub(crate) unsafe fn select(
    nfds: i32,
    readfds: Option<&mut [FdSetElement]>,
    writefds: Option<&mut [FdSetElement]>,
    exceptfds: Option<&mut [FdSetElement]>,
    timeout: Option<&crate::timespec::Timespec>,
) -> io::Result<i32> {
    let len = crate::event::fd_set_num_elements_for_fd_array(nfds as usize);

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

    let timeout_data;
    let timeout_ptr = match timeout {
        Some(timeout) => {
            // Convert from `Timespec` to `c::timeval`.
            timeout_data = c::timeval {
                tv_sec: timeout.tv_sec.try_into().map_err(|_| io::Errno::OVERFLOW)?,
                tv_usec: ((timeout.tv_nsec + 999) / 1000) as _,
            };
            &timeout_data
        }
        None => null(),
    };

    ret_c_int(c::select(
        nfds,
        readfds.cast(),
        writefds.cast(),
        exceptfds.cast(),
        timeout_ptr as *mut c::timeval,
    ))
}

#[cfg(solarish)]
pub(crate) fn port_create() -> io::Result<OwnedFd> {
    unsafe { ret_owned_fd(c::port_create()) }
}

#[cfg(solarish)]
pub(crate) unsafe fn port_associate(
    port: BorrowedFd<'_>,
    source: c::c_int,
    object: c::uintptr_t,
    events: c::c_int,
    user: *mut c::c_void,
) -> io::Result<()> {
    ret(c::port_associate(
        borrowed_fd(port),
        source,
        object,
        events,
        user,
    ))
}

#[cfg(solarish)]
pub(crate) unsafe fn port_dissociate(
    port: BorrowedFd<'_>,
    source: c::c_int,
    object: c::uintptr_t,
) -> io::Result<()> {
    ret(c::port_dissociate(borrowed_fd(port), source, object))
}

#[cfg(solarish)]
pub(crate) fn port_get(
    port: BorrowedFd<'_>,
    timeout: Option<&mut c::timespec>,
) -> io::Result<Event> {
    let mut event = MaybeUninit::<c::port_event>::uninit();
    let timeout = timeout.map_or(null_mut(), as_mut_ptr);

    unsafe {
        ret(c::port_get(borrowed_fd(port), event.as_mut_ptr(), timeout))?;
    }

    // If we're done, initialize the event and return it.
    Ok(Event(unsafe { event.assume_init() }))
}

#[cfg(all(feature = "alloc", solarish))]
pub(crate) fn port_getn(
    port: BorrowedFd<'_>,
    timeout: Option<&mut c::timespec>,
    events: &mut Vec<Event>,
    mut nget: u32,
) -> io::Result<()> {
    // `port_getn` special-cases a max value of 0 to be a query that returns
    // the number of events. We don't want to do the `set_len` in that case, so
    // so bail out early if needed.
    if events.capacity() == 0 {
        return Ok(());
    }

    let timeout = timeout.map_or(null_mut(), as_mut_ptr);
    unsafe {
        ret(c::port_getn(
            borrowed_fd(port),
            events.as_mut_ptr().cast(),
            events.capacity().try_into().unwrap(),
            &mut nget,
            timeout,
        ))?;
    }

    // Update the vector length.
    unsafe {
        events.set_len(nget.try_into().unwrap());
    }

    Ok(())
}

#[cfg(solarish)]
pub(crate) fn port_getn_query(port: BorrowedFd<'_>) -> io::Result<u32> {
    let mut nget: u32 = 0;

    // Pass a `max` of 0 to query the number of available events.
    unsafe {
        ret(c::port_getn(
            borrowed_fd(port),
            null_mut(),
            0,
            &mut nget,
            null_mut(),
        ))?;
    }

    Ok(nget)
}

#[cfg(solarish)]
pub(crate) fn port_send(
    port: BorrowedFd<'_>,
    events: c::c_int,
    userdata: *mut c::c_void,
) -> io::Result<()> {
    unsafe { ret(c::port_send(borrowed_fd(port), events, userdata)) }
}

#[cfg(not(any(target_os = "redox", target_os = "wasi")))]
pub(crate) fn pause() {
    let r = unsafe { c::pause() };
    let errno = libc_errno::errno().0;
    debug_assert_eq!(r, -1);
    debug_assert_eq!(errno, c::EINTR);
}

#[inline]
#[cfg(any(linux_kernel, target_os = "illumos", target_os = "redox"))]
pub(crate) fn epoll_create(flags: super::epoll::CreateFlags) -> io::Result<OwnedFd> {
    unsafe { ret_owned_fd(c::epoll_create1(bitflags_bits!(flags))) }
}

#[inline]
#[cfg(any(linux_kernel, target_os = "illumos", target_os = "redox"))]
pub(crate) fn epoll_add(
    epoll: BorrowedFd<'_>,
    source: BorrowedFd<'_>,
    event: &crate::event::epoll::Event,
) -> io::Result<()> {
    // We use our own `Event` struct instead of libc's because
    // ours preserves pointer provenance instead of just using a `u64`,
    // and we have tests elsewhere for layout equivalence.
    unsafe {
        ret(c::epoll_ctl(
            borrowed_fd(epoll),
            c::EPOLL_CTL_ADD,
            borrowed_fd(source),
            // The event is read-only even though libc has a non-const pointer.
            as_ptr(event) as *mut c::epoll_event,
        ))
    }
}

#[inline]
#[cfg(any(linux_kernel, target_os = "illumos", target_os = "redox"))]
pub(crate) fn epoll_mod(
    epoll: BorrowedFd<'_>,
    source: BorrowedFd<'_>,
    event: &crate::event::epoll::Event,
) -> io::Result<()> {
    unsafe {
        ret(c::epoll_ctl(
            borrowed_fd(epoll),
            c::EPOLL_CTL_MOD,
            borrowed_fd(source),
            // The event is read-only even though libc has a non-const pointer.
            as_ptr(event) as *mut c::epoll_event,
        ))
    }
}

#[inline]
#[cfg(any(linux_kernel, target_os = "illumos", target_os = "redox"))]
pub(crate) fn epoll_del(epoll: BorrowedFd<'_>, source: BorrowedFd<'_>) -> io::Result<()> {
    unsafe {
        ret(c::epoll_ctl(
            borrowed_fd(epoll),
            c::EPOLL_CTL_DEL,
            borrowed_fd(source),
            null_mut(),
        ))
    }
}

#[inline]
#[cfg(feature = "alloc")]
#[cfg(any(linux_kernel, target_os = "illumos", target_os = "redox"))]
pub(crate) fn epoll_wait(
    epoll: BorrowedFd<'_>,
    events: &mut [MaybeUninit<crate::event::epoll::Event>],
    timeout: c::c_int,
) -> io::Result<usize> {
    unsafe {
        ret_u32(c::epoll_wait(
            borrowed_fd(epoll),
            events.as_mut_ptr().cast::<c::epoll_event>(),
            events.len().try_into().unwrap_or(i32::MAX),
            timeout,
        ))
        .map(|i| i.try_into().unwrap_or(usize::MAX))
    }
}
