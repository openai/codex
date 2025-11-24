#[cfg(linux_kernel)]
use crate::backend::c;
#[cfg(linux_kernel)]
use bitflags::bitflags;

#[cfg(linux_kernel)]
bitflags! {
    /// `GRND_*` flags for use with [`getrandom`].
    ///
    /// [`getrandom`]: crate::rand::getrandom
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct GetRandomFlags: u32 {
        /// `GRND_RANDOM`
        const RANDOM = c::GRND_RANDOM;
        /// `GRND_NONBLOCK`
        const NONBLOCK = c::GRND_NONBLOCK;
        /// `GRND_INSECURE`
        const INSECURE = c::GRND_INSECURE;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}
