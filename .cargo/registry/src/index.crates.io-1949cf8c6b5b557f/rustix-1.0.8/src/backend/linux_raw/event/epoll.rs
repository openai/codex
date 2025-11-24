use crate::ffi;
use bitflags::bitflags;

bitflags! {
    /// `EPOLL_*` for use with [`epoll::create`].
    ///
    /// [`epoll::create`]: crate::event::epoll::create
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct CreateFlags: ffi::c_uint {
        /// `EPOLL_CLOEXEC`
        const CLOEXEC = linux_raw_sys::general::EPOLL_CLOEXEC;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

bitflags! {
    /// `EPOLL*` for use with [`epoll::add`].
    ///
    /// [`epoll::add`]: crate::event::epoll::add
    #[repr(transparent)]
    #[derive(Default, Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct EventFlags: u32 {
        /// `EPOLLIN`
        const IN = linux_raw_sys::general::EPOLLIN as u32;

        /// `EPOLLOUT`
        const OUT = linux_raw_sys::general::EPOLLOUT as u32;

        /// `EPOLLPRI`
        const PRI = linux_raw_sys::general::EPOLLPRI as u32;

        /// `EPOLLERR`
        const ERR = linux_raw_sys::general::EPOLLERR as u32;

        /// `EPOLLHUP`
        const HUP = linux_raw_sys::general::EPOLLHUP as u32;

        /// `EPOLLRDNORM`
        const RDNORM = linux_raw_sys::general::EPOLLRDNORM as u32;

        /// `EPOLLRDBAND`
        const RDBAND = linux_raw_sys::general::EPOLLRDBAND as u32;

        /// `EPOLLWRNORM`
        const WRNORM = linux_raw_sys::general::EPOLLWRNORM as u32;

        /// `EPOLLWRBAND`
        const WRBAND = linux_raw_sys::general::EPOLLWRBAND as u32;

        /// `EPOLLMSG`
        const MSG = linux_raw_sys::general::EPOLLMSG as u32;

        /// `EPOLLRDHUP`
        const RDHUP = linux_raw_sys::general::EPOLLRDHUP as u32;

        /// `EPOLLET`
        const ET = linux_raw_sys::general::EPOLLET as u32;

        /// `EPOLLONESHOT`
        const ONESHOT = linux_raw_sys::general::EPOLLONESHOT as u32;

        /// `EPOLLWAKEUP`
        const WAKEUP = linux_raw_sys::general::EPOLLWAKEUP as u32;

        /// `EPOLLEXCLUSIVE`
        const EXCLUSIVE = linux_raw_sys::general::EPOLLEXCLUSIVE as u32;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}
