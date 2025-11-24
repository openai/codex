use bitflags::bitflags;

bitflags! {
    /// `GRND_*` flags for use with [`getrandom`].
    ///
    /// [`getrandom`]: crate::rand::getrandom
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct GetRandomFlags: u32 {
        /// `GRND_RANDOM`
        const RANDOM = linux_raw_sys::general::GRND_RANDOM;
        /// `GRND_NONBLOCK`
        const NONBLOCK = linux_raw_sys::general::GRND_NONBLOCK;
        /// `GRND_INSECURE`
        const INSECURE = linux_raw_sys::general::GRND_INSECURE;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}
