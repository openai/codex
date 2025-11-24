use crate::backend::c;
use bitflags::bitflags;

bitflags! {
    /// `O_*` constants for use with [`shm::open`].
    ///
    /// [`shm::open`]: crate:shm::open
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct ShmOFlags: u32 {
        /// `O_CREAT`
        #[doc(alias = "CREAT")]
        const CREATE = bitcast!(c::O_CREAT);

        /// `O_EXCL`
        const EXCL = bitcast!(c::O_EXCL);

        /// `O_RDONLY`
        const RDONLY = bitcast!(c::O_RDONLY);

        /// `O_RDWR`
        const RDWR = bitcast!(c::O_RDWR);

        /// `O_TRUNC`
        const TRUNC = bitcast!(c::O_TRUNC);

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}
