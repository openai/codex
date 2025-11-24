use crate::backend::c;
use bitflags::bitflags;

bitflags! {
    /// `MSG_*` flags for use with [`send`], [`sendto`], and related
    /// functions.
    ///
    /// [`send`]: crate::net::send
    /// [`sendto`]: crate::net::sendto
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct SendFlags: u32 {
        /// `MSG_CONFIRM`
        #[cfg(not(any(
            bsd,
            solarish,
            windows,
            target_os = "aix",
            target_os = "cygwin",
            target_os = "espidf",
            target_os = "nto",
            target_os = "haiku",
            target_os = "horizon",
            target_os = "hurd",
            target_os = "redox",
            target_os = "vita",
        )))]
        const CONFIRM = bitcast!(c::MSG_CONFIRM);
        /// `MSG_DONTROUTE`
        const DONTROUTE = bitcast!(c::MSG_DONTROUTE);
        /// `MSG_DONTWAIT`
        #[cfg(not(windows))]
        const DONTWAIT = bitcast!(c::MSG_DONTWAIT);
        /// `MSG_EOR`
        #[cfg(not(any(windows, target_os = "horizon")))]
        const EOR = bitcast!(c::MSG_EOR);
        /// `MSG_MORE`
        #[cfg(not(any(
            bsd,
            solarish,
            windows,
            target_os = "aix",
            target_os = "cygwin",
            target_os = "haiku",
            target_os = "hurd",
            target_os = "nto",
            target_os = "redox",
            target_os = "vita",
        )))]
        const MORE = bitcast!(c::MSG_MORE);
        #[cfg(not(any(apple, windows, target_os = "redox", target_os = "vita")))]
        /// `MSG_NOSIGNAL`
        const NOSIGNAL = bitcast!(c::MSG_NOSIGNAL);
        /// `MSG_OOB`
        const OOB = bitcast!(c::MSG_OOB);

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

bitflags! {
    /// `MSG_*` flags for use with [`recv`], [`recvfrom`], and related
    /// functions.
    ///
    /// [`recv`]: crate::net::recv
    /// [`recvfrom`]: crate::net::recvfrom
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct RecvFlags: u32 {
        /// `MSG_CMSG_CLOEXEC`
        #[cfg(not(any(
            apple,
            solarish,
            windows,
            target_os = "aix",
            target_os = "espidf",
            target_os = "haiku",
            target_os = "horizon",
            target_os = "nto",
            target_os = "redox",
            target_os = "vita",
        )))]
        const CMSG_CLOEXEC = bitcast!(c::MSG_CMSG_CLOEXEC);
        /// `MSG_DONTWAIT`
        #[cfg(not(windows))]
        const DONTWAIT = bitcast!(c::MSG_DONTWAIT);
        /// `MSG_ERRQUEUE`
        #[cfg(not(any(
            bsd,
            solarish,
            windows,
            target_os = "aix",
            target_os = "cygwin",
            target_os = "espidf",
            target_os = "haiku",
            target_os = "horizon",
            target_os = "hurd",
            target_os = "nto",
            target_os = "redox",
            target_os = "vita",
        )))]
        const ERRQUEUE = bitcast!(c::MSG_ERRQUEUE);
        /// `MSG_OOB`
        const OOB = bitcast!(c::MSG_OOB);
        /// `MSG_PEEK`
        const PEEK = bitcast!(c::MSG_PEEK);
        /// `MSG_TRUNC`
        // Apple, illumos, and NetBSD have `MSG_TRUNC` but it's not documented
        // for use with `recv` and friends, and in practice appears to be
        // ignored.
        #[cfg(not(any(apple, solarish, target_os = "horizon", target_os = "netbsd")))]
        const TRUNC = bitcast!(c::MSG_TRUNC);
        /// `MSG_WAITALL`
        const WAITALL = bitcast!(c::MSG_WAITALL);

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

bitflags! {
    /// `MSG_*` flags returned from [`recvmsg`], in the `flags` field of
    /// [`RecvMsg`]
    ///
    /// [`recvmsg`]: crate::net::recvmsg
    /// [`RecvMsg`]: crate::net::RecvMsg
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct ReturnFlags: u32 {
        /// `MSG_OOB`
        const OOB = bitcast!(c::MSG_OOB);
        /// `MSG_EOR`
        #[cfg(not(any(windows, target_os = "horizon")))]
        const EOR = bitcast!(c::MSG_EOR);
        /// `MSG_TRUNC`
        #[cfg(not(target_os = "horizon"))]
        const TRUNC = bitcast!(c::MSG_TRUNC);
        /// `MSG_CTRUNC`
        #[cfg(not(target_os = "horizon"))]
        const CTRUNC = bitcast!(c::MSG_CTRUNC);

        /// `MSG_CMSG_CLOEXEC`
        #[cfg(linux_kernel)]
        const CMSG_CLOEXEC = bitcast!(c::MSG_CMSG_CLOEXEC);
        /// `MSG_ERRQUEUE`
        #[cfg(linux_kernel)]
        const ERRQUEUE = bitcast!(c::MSG_ERRQUEUE);

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}
