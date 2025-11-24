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
        const CONFIRM = c::MSG_CONFIRM;
        /// `MSG_DONTROUTE`
        const DONTROUTE = c::MSG_DONTROUTE;
        /// `MSG_DONTWAIT`
        const DONTWAIT = c::MSG_DONTWAIT;
        /// Deprecated alias for [`EOR`].
        ///
        /// [`EOR`]: Self::EOR
        #[deprecated(note = "`rustix::net::SendFlags::EOT` is renamed to `rustix::net::SendFlags::EOR`.")]
        const EOT = c::MSG_EOR;
        /// `MSG_EOR`
        const EOR = c::MSG_EOR;
        /// `MSG_MORE`
        const MORE = c::MSG_MORE;
        /// `MSG_NOSIGNAL`
        const NOSIGNAL = c::MSG_NOSIGNAL;
        /// `MSG_OOB`
        const OOB = c::MSG_OOB;

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
        const CMSG_CLOEXEC = c::MSG_CMSG_CLOEXEC;
        /// `MSG_DONTWAIT`
        const DONTWAIT = c::MSG_DONTWAIT;
        /// `MSG_ERRQUEUE`
        const ERRQUEUE = c::MSG_ERRQUEUE;
        /// `MSG_OOB`
        const OOB = c::MSG_OOB;
        /// `MSG_PEEK`
        const PEEK = c::MSG_PEEK;
        /// `MSG_TRUNC`
        const TRUNC = c::MSG_TRUNC;
        /// `MSG_WAITALL`
        const WAITALL = c::MSG_WAITALL;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}
