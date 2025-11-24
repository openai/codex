use crate::backend::c;
use bitflags::bitflags;

bitflags! {
    /// `EPOLL_*` for use with [`epoll::create`].
    ///
    /// [`epoll::create`]: crate::event::epoll::create
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct CreateFlags: u32 {
        /// `EPOLL_CLOEXEC`
        const CLOEXEC = bitcast!(c::EPOLL_CLOEXEC);

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
        const IN = bitcast!(c::EPOLLIN);

        /// `EPOLLOUT`
        const OUT = bitcast!(c::EPOLLOUT);

        /// `EPOLLPRI`
        const PRI = bitcast!(c::EPOLLPRI);

        /// `EPOLLERR`
        const ERR = bitcast!(c::EPOLLERR);

        /// `EPOLLHUP`
        const HUP = bitcast!(c::EPOLLHUP);

        /// `EPOLLRDNORM`
        const RDNORM = bitcast!(c::EPOLLRDNORM);

        /// `EPOLLRDBAND`
        const RDBAND = bitcast!(c::EPOLLRDBAND);

        /// `EPOLLWRNORM`
        const WRNORM = bitcast!(c::EPOLLWRNORM);

        /// `EPOLLWRBAND`
        const WRBAND = bitcast!(c::EPOLLWRBAND);

        /// `EPOLLMSG`
        const MSG = bitcast!(c::EPOLLMSG);

        /// `EPOLLRDHUP`
        const RDHUP = bitcast!(c::EPOLLRDHUP);

        /// `EPOLLET`
        const ET = bitcast!(c::EPOLLET);

        /// `EPOLLONESHOT`
        const ONESHOT = bitcast!(c::EPOLLONESHOT);

        /// `EPOLLWAKEUP`
        const WAKEUP = bitcast!(c::EPOLLWAKEUP);

        /// `EPOLLEXCLUSIVE`
        #[cfg(not(target_os = "android"))]
        const EXCLUSIVE = bitcast!(c::EPOLLEXCLUSIVE);

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}
