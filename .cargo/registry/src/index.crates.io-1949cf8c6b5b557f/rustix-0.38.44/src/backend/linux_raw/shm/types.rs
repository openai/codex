use crate::backend::c;
use bitflags::bitflags;

bitflags! {
    /// `O_*` constants for use with [`shm::open`].
    ///
    /// [`shm::open`]: crate:shm::open
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct ShmOFlags: c::c_uint {
        /// `O_CREAT`
        #[doc(alias = "CREAT")]
        const CREATE = linux_raw_sys::general::O_CREAT;

        /// `O_EXCL`
        const EXCL = linux_raw_sys::general::O_EXCL;

        /// `O_RDONLY`
        const RDONLY = linux_raw_sys::general::O_RDONLY;

        /// `O_RDWR`
        const RDWR = linux_raw_sys::general::O_RDWR;

        /// `O_TRUNC`
        const TRUNC = linux_raw_sys::general::O_TRUNC;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}
